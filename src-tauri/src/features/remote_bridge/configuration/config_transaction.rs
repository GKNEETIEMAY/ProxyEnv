use super::{
    base64, base64_bytes, cancel_tool_profile_sync, capability_ready, elapsed_ms, listening, lock,
    refresh, remote_request, set_cached_profile, settings, ssh, sync_tool_states, tool_adapter,
    BridgeResult, ConfigPreview, DiagnosticState, ErrorCategory, EventAction, EventComponent,
    EventOutcome, LocalToolProfile, Pending, Store, ToolVerificationResult,
};
use serde_json::json;
use std::time::Instant;

pub(super) fn config_preview_locked(
    state: &mut Store,
    adapter: &'static dyn tool_adapter::RemoteToolAdapter,
) -> BridgeResult<ConfigPreview> {
    if state.profile_syncing.is_some() {
        return Err("configConflict".into());
    }
    if state.child.is_none() || state.summary.cc.is_none() {
        return Err("bridgeUnavailable".into());
    }
    if let Some(observation) = state
        .summary
        .tool_installations
        .iter()
        .find(|item| item.tool == adapter.id())
    {
        match observation.state {
            _ if observation.extension_state == tool_adapter::RemoteToolInstallState::Installed => {
            }
            tool_adapter::RemoteToolInstallState::Missing => return Err("cliMissing".into()),
            tool_adapter::RemoteToolInstallState::Unsupported => {
                return Err("cliUnsupported".into())
            }
            tool_adapter::RemoteToolInstallState::Broken => return Err("cliUnsupported".into()),
            tool_adapter::RemoteToolInstallState::Checking => return Err("stateUnavailable".into()),
            _ => {}
        }
    }
    let local_profile = Some(LocalToolProfile::inspect(adapter.id()).map_err(
        |_| match adapter.id() {
            tool_adapter::RemoteToolId::Codex => "localCodexProfileInvalid",
            tool_adapter::RemoteToolId::Claude => "localClaudeProfileInvalid",
        },
    )?);
    let local_port = state.summary.cc.as_ref().ok_or("ccUnavailable")?.local.port;
    if !local_profile
        .as_ref()
        .is_some_and(|profile| profile.routes_to(local_port))
    {
        return Err("localRouteInactive".into());
    }
    let target_id = state
        .summary
        .target
        .as_ref()
        .map(|target| target.id.clone())
        .ok_or("invalidTarget")?;
    let target_fingerprint = ssh::fingerprint(&target_id)?;
    if state.target_fingerprint.as_ref() != Some(&target_fingerprint) {
        return Err("sshConfigChanged".into());
    }
    if state
        .summary
        .cc
        .as_ref()
        .is_none_or(|e| !listening(&e.local))
    {
        return Err("ccUnavailable".into());
    }
    let port = state
        .summary
        .cc
        .as_ref()
        .ok_or("ccUnavailable")?
        .remote_port;
    let session_token = state
        .ai_relay
        .as_ref()
        .map(|relay| relay.token())
        .ok_or("ccUnavailable")?;
    let value = ssh::remote(
        &target_id,
        remote_request(
            "preview",
            adapter,
            port,
            local_profile.as_ref(),
            Some(session_token.as_str()),
        ),
    )?;
    if adapter.id() == tool_adapter::RemoteToolId::Codex
        && value["remoteChanged"].as_bool() == Some(true)
    {
        state.summary.codex_profile_state = settings::ProfileSyncState::RemoteChanged;
    } else if adapter.id() == tool_adapter::RemoteToolId::Claude
        && value["remoteChanged"].as_bool() == Some(true)
    {
        state.summary.claude_profile_state = settings::ProfileSyncState::RemoteChanged;
    }
    let hash = value["expectedHash"]
        .as_str()
        .ok_or("remoteFailed")?
        .to_string();
    let existing_config = value["configExists"].as_bool().ok_or("remoteFailed")?;
    let permission_hardening = value["permissionHardening"]
        .as_bool()
        .ok_or("remoteFailed")?;
    let previous_port = value["previousPort"].as_u64();
    let before = match value["previousPort"].as_u64() {
        Some(p) if (1024..=65535).contains(&p) => {
            adapter
                .preview(p as u16, value["remoteModel"].as_str())
                .content
        }
        None if value["previousPort"].is_null() => String::new(),
        _ => return Err("remoteFailed".into()),
    };
    let version = value["version"]
        .as_str()
        .filter(|s| s.len() < 32 && s.bytes().all(|b| b.is_ascii_digit() || b == b'.'))
        .ok_or("remoteFailed")?
        .to_owned();
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| "stateUnavailable")?;
    if !adapter.detect(&version) {
        return Err("cliUnsupported".into());
    }
    let plan = adapter.preview(
        port,
        local_profile.as_ref().and_then(LocalToolProfile::model),
    );
    let preview = ConfigPreview {
        id: hex::encode(nonce),
        tool: adapter.id(),
        path: plan.path.into(),
        before,
        after: plan.content,
        version,
        launch: plan.launch.into(),
        alias: target_id.clone(),
        restore: false,
        existing_config,
        route_update: previous_port.is_some_and(|previous| previous != u64::from(port)),
        permission_hardening,
    };
    state.pending = Some(Pending {
        preview: preview.clone(),
        hash,
        target_id,
        port,
        backup_hash: None,
        target_fingerprint,
        local_profile,
    });
    Ok(preview)
}

pub(super) fn config_apply_locked(state: &mut Store, id: String) -> BridgeResult<()> {
    if state.child.is_none() {
        return Err("bridgeUnavailable".into());
    }
    let pending = state
        .pending
        .take()
        .filter(|p| p.preview.id == id && !p.preview.restore)
        .ok_or("configConflict")?;
    if ssh::fingerprint(&pending.target_id)? != pending.target_fingerprint {
        return Err("sshConfigChanged".into());
    }
    if state
        .summary
        .cc
        .as_ref()
        .is_none_or(|e| !listening(&e.local))
    {
        return Err("ccUnavailable".into());
    }
    let adapter = tool_adapter::by_id(pending.preview.tool);
    let session_token = state
        .ai_relay
        .as_ref()
        .map(|relay| relay.token())
        .ok_or("ccUnavailable")?;
    let applied = ssh::remote(
        &pending.target_id,
        json!({
            "operation":"apply",
            "tool":pending.preview.tool.as_str(),
            "port":pending.port,
            "expectedHash":pending.hash,
            "repairPermissions":pending.preview.permission_hardening,
            "profileModelBase64":pending.local_profile.as_ref().and_then(|profile| match profile { LocalToolProfile::Codex(profile) => Some(base64(&profile.model)), _ => None }),
            "profileCatalogBase64":pending.local_profile.as_ref().and_then(|profile| match profile { LocalToolProfile::Codex(profile) => Some(base64_bytes(&profile.catalog_bytes)), _ => None }),
            "profileSettingsBase64":pending.local_profile.as_ref().and_then(|profile| match profile { LocalToolProfile::Claude(profile) => Some(base64_bytes(&profile.settings_bytes)), _ => None }),
            "profileHash":pending.local_profile.as_ref().map(LocalToolProfile::hash),
            "sessionToken":session_token.as_str()
        }),
    )?;
    let applied_hash = applied["appliedHash"].as_str().ok_or("remoteFailed")?;
    let backup_hash = applied["backupHash"].as_str().ok_or("remoteFailed")?;
    let rollback = || {
        ssh::remote(
            &pending.target_id,
            json!({
                "operation":"restore",
                "tool":pending.preview.tool.as_str(),
                "expectedHash":applied_hash,
                "backupHash":backup_hash,
                "repairPermissions":pending.preview.permission_hardening
            }),
        )
    };
    let verified = match ssh::remote(
        &pending.target_id,
        remote_request(
            "preview",
            adapter,
            pending.port,
            pending.local_profile.as_ref(),
            Some(session_token.as_str()),
        ),
    ) {
        Ok(value) => value,
        Err(_) => {
            return Err(if rollback().is_ok() {
                "writeRolledBack"
            } else {
                "rollbackFailed"
            }
            .into());
        }
    };
    let profile_verified = pending
        .local_profile
        .as_ref()
        .is_none_or(|profile| verified["profileHash"].as_str() == Some(profile.hash()));
    if verified["configured"].as_bool() != Some(true)
        || verified["previousPort"].as_u64() != Some(u64::from(pending.port))
        || !profile_verified
    {
        return Err(if rollback().is_ok() {
            "writeRolledBack"
        } else {
            "rollbackFailed"
        }
        .into());
    }
    tool_adapter::by_id(pending.preview.tool).apply(&mut state.summary);
    if let Some(profile) = pending.local_profile {
        let tool = pending.preview.tool;
        set_cached_profile(state, Some(profile));
        if tool == tool_adapter::RemoteToolId::Codex {
            state.summary.codex_profile_state = settings::ProfileSyncState::Synced;
        } else {
            state.summary.claude_profile_state = settings::ProfileSyncState::Synced;
        }
    }
    state
        .disabled_tools
        .retain(|tool| *tool != pending.preview.tool);
    sync_tool_states(&mut state.summary);
    Ok(())
}

pub fn config_restore_preview(target_id: String, tool: String) -> BridgeResult<ConfigPreview> {
    let adapter = tool_adapter::by_name(&tool)?;
    let target_fingerprint = ssh::fingerprint(&target_id)?;
    let mut state = lock()?;
    if state.profile_syncing.is_some() {
        return Err("configConflict".into());
    }
    let value = ssh::remote(
        &target_id,
        json!({"operation":"restore-preview","tool":adapter.id().as_str()}),
    )?;
    let content = |key: &str| -> BridgeResult<String> {
        match value[key].as_u64() {
            Some(p) if (1024..=65535).contains(&p) => Ok(adapter.preview(p as u16, None).content),
            None if value[key].is_null() => Ok(String::new()),
            _ => Err("remoteFailed".into()),
        }
    };
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| "stateUnavailable")?;
    let preview = ConfigPreview {
        id: hex::encode(nonce),
        alias: target_id.clone(),
        tool: adapter.id(),
        restore: true,
        existing_config: value["originalExists"].as_bool().ok_or("remoteFailed")?,
        route_update: false,
        permission_hardening: value["permissionHardening"]
            .as_bool()
            .ok_or("remoteFailed")?,
        path: adapter.config_path().into(),
        before: content("previousPort")?,
        after: content("originalPort")?,
        version: String::new(),
        launch: String::new(),
    };
    state.pending = Some(Pending {
        preview: preview.clone(),
        target_id,
        port: 25721,
        hash: value["expectedHash"].as_str().ok_or("remoteFailed")?.into(),
        backup_hash: Some(value["backupHash"].as_str().ok_or("remoteFailed")?.into()),
        target_fingerprint,
        local_profile: None,
    });
    Ok(preview)
}

pub fn config_restore(id: String, confirmed: bool) -> BridgeResult<()> {
    if !confirmed {
        return Err("confirmationRequired".into());
    }
    let mut state = lock()?;
    let pending = state
        .pending
        .take()
        .filter(|p| p.preview.id == id && p.preview.restore)
        .ok_or("configConflict")?;
    if ssh::fingerprint(&pending.target_id)? != pending.target_fingerprint {
        return Err("sshConfigChanged".into());
    }
    ssh::remote(
        &pending.target_id,
        json!({
            "operation":"restore",
            "tool":pending.preview.tool.as_str(),
            "expectedHash":pending.hash,
            "backupHash":pending.backup_hash,
            "repairPermissions":pending.preview.permission_hardening
        }),
    )?;
    if state.summary.target.as_ref().map(|target| &target.id) == Some(&pending.target_id) {
        cancel_tool_profile_sync(&mut state, pending.preview.tool);
        if !state.disabled_tools.contains(&pending.preview.tool) {
            state.disabled_tools.push(pending.preview.tool);
        }
        tool_adapter::by_id(pending.preview.tool).restore(&mut state.summary);
        if pending.preview.tool == tool_adapter::RemoteToolId::Codex {
            state.local_codex_profile = None;
            state.summary.codex_profile_state = settings::ProfileSyncState::NotStarted;
        } else {
            state.local_claude_profile = None;
            state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
        }
        sync_tool_states(&mut state.summary);
    }
    state.pending = None;
    Ok(())
}

pub(super) fn verify_tool_for_generation(
    tool: String,
    expected_generation: Option<u64>,
) -> BridgeResult<ToolVerificationResult> {
    let started = Instant::now();
    let adapter = tool_adapter::by_name(&tool)?;
    if !adapter.verification_supported() {
        return Err("toolVerificationUnsupported".into());
    }
    let (target_id, target_fingerprint, route_port, session_token, generation, ticket) = {
        let mut state = lock()?;
        if expected_generation.is_some_and(|expected| state.connection_generation != expected) {
            return Err("bridgeUnavailable".into());
        }
        refresh(&mut state);
        if !capability_ready(state.child.is_some(), state.summary.cc_status) {
            return Err("bridgeUnavailable".into());
        }
        if !adapter.configured(&state.summary) {
            return Err("toolNotConfigured".into());
        }
        if !adapter.inspect(&state.summary).verification_supported {
            return Err("toolVerificationUnsupported".into());
        }
        if state.summary.diagnostics.ai_route_verification.state == DiagnosticState::Testing {
            return Err("bridgeBusy".into());
        }
        let target_id = state
            .summary
            .target
            .as_ref()
            .map(|target| target.id.clone())
            .ok_or("invalidTarget")?;
        let fingerprint = state.target_fingerprint.clone().ok_or("sshConfigChanged")?;
        let route_port = state
            .summary
            .cc
            .as_ref()
            .map(|endpoint| endpoint.remote_port)
            .ok_or("bridgeUnavailable")?;
        let session_token = state
            .ai_relay
            .as_ref()
            .map(|relay| relay.token())
            .ok_or("bridgeUnavailable")?;
        if ssh::fingerprint(&target_id)? != fingerprint {
            return Err("sshConfigChanged".into());
        }
        adapter.verify(
            &mut state.summary,
            tool_adapter::RemoteToolVerification::VerifyPending,
        );
        sync_tool_states(&mut state.summary);
        let ticket = state.summary.diagnostics.ai_route_verification.begin();
        state.events.push(
            EventComponent::AiRoute,
            EventAction::ToolVerify,
            EventOutcome::Started,
            None,
            None,
        );
        (
            target_id,
            fingerprint,
            route_port,
            session_token,
            state.connection_generation,
            ticket,
        )
    };

    let result: BridgeResult<tool_adapter::RemoteToolVerification> = (|| {
        let value = ssh::remote(
            &target_id,
            json!({
                "operation":"tool-verify",
                "tool":adapter.id().as_str(),
                "port":route_port,
                "sessionToken":session_token.as_str()
            }),
        )?;
        let verification: tool_adapter::RemoteToolVerification =
            serde_json::from_value(value.get("verification").cloned().ok_or("remoteFailed")?)
                .map_err(|_| "remoteFailed")?;
        if matches!(
            verification,
            tool_adapter::RemoteToolVerification::NotConfigured
                | tool_adapter::RemoteToolVerification::VerifyPending
        ) {
            return Err("remoteFailed".into());
        }
        if ssh::fingerprint(&target_id)? != target_fingerprint {
            return Err("sshConfigChanged".into());
        }
        Ok(verification)
    })();

    let mut state = lock()?;
    refresh(&mut state);
    if !capability_ready(state.child.is_some(), state.summary.cc_status)
        || state.connection_generation != generation
        || state.summary.target.as_ref().map(|target| &target.id) != Some(&target_id)
        || state.target_fingerprint.as_ref() != Some(&target_fingerprint)
        || state
            .summary
            .cc
            .as_ref()
            .map(|endpoint| endpoint.remote_port)
            != Some(route_port)
        || state
            .ai_relay
            .as_ref()
            .is_none_or(|relay| *relay.token() != *session_token)
        || !state
            .summary
            .diagnostics
            .ai_route_verification
            .current(ticket)
        || !adapter.configured(&state.summary)
    {
        return Err("bridgeUnavailable".into());
    }
    use tool_adapter::RemoteToolVerification as V;
    let error = match &result {
        Ok(V::Verified) => None,
        Ok(V::AuthenticationRequired) => Some(ErrorCategory::Authentication),
        Ok(V::RouteUnavailable) => Some(ErrorCategory::Unavailable),
        Ok(V::TimedOut) => Some(ErrorCategory::Network),
        Ok(_) => Some(ErrorCategory::Unknown),
        Err(code) => Some(ErrorCategory::from_code(code)),
    };
    state
        .summary
        .diagnostics
        .ai_route_verification
        .finish(ticket, error, elapsed_ms(started));
    state.events.push(
        EventComponent::AiRoute,
        EventAction::ToolVerify,
        if error.is_none() {
            EventOutcome::Success
        } else {
            EventOutcome::Failed
        },
        error,
        Some(elapsed_ms(started)),
    );
    adapter.verify(
        &mut state.summary,
        result.as_ref().copied().unwrap_or(V::Failed),
    );
    sync_tool_states(&mut state.summary);
    let verification = result?;
    Ok(ToolVerificationResult {
        tool: adapter.id(),
        verification,
    })
}
