//! Change one lane without replacing the other lane's SSH transport or AI session.
use super::ssh::ManagedSsh;
use super::*;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    Proxy,
    Cc,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityChange {
    pub target_id: String,
    pub capability: Capability,
    pub enabled: bool,
    pub expected_revision: u64,
    pub cc_local_port: u16,
}

// Paused forwards keep their ports reserved, but their relay gate rejects all
// traffic. Recovery must retain that reservation without reopening the gate.
pub(super) fn transport_summary(state: &Store) -> Summary {
    let mut summary = state.summary.clone();
    summary.proxy = summary.proxy.or_else(|| state.paused_proxy.clone());
    summary.cc = summary.cc.or_else(|| state.paused_cc.clone());
    summary
}

fn enabled(state: &Store, capability: Capability) -> bool {
    match capability {
        Capability::Proxy => state.summary.proxy.is_some(),
        Capability::Cc => state.summary.cc.is_some(),
    }
}

pub fn set_capability(change: CapabilityChange, confirmed: bool) -> BridgeResult<Summary> {
    if !confirmed {
        return Err("confirmationRequired".into());
    }
    // Discovery owns the same Store mutex; only run it before acquiring the
    // operation lock, and only when adding an AI lane for the first time.
    let needs_cc = {
        let state = lock()?;
        change.enabled && change.capability == Capability::Cc && state.ai_relay.is_none()
    };
    let cc_port = if needs_cc {
        let detected = discover_cc(change.cc_local_port)?;
        if detected.state != CcDetectionState::Confirmed {
            return Err("ccUnavailable".into());
        }
        detected.local_port
    } else {
        change.cc_local_port
    };
    let mut state = lock()?;
    refresh(&mut state);
    if state.child.is_none()
        || state
            .summary
            .target
            .as_ref()
            .map(|target| target.id.as_str())
            != Some(change.target_id.as_str())
    {
        return Err("bridgeUnavailable".into());
    }
    if state.summary.post_connect_status == PostConnectStatus::Preparing
        || state.profile_syncing
        || state.pending.is_some()
        || state.extension_pending.is_some()
    {
        return Err("bridgeBusy".into());
    }
    let fingerprint = state.target_fingerprint.clone().ok_or("sshConfigChanged")?;
    if ssh::fingerprint(&change.target_id)? != fingerprint {
        return Err("sshConfigChanged".into());
    }
    if enabled(&state, change.capability) == change.enabled {
        return Ok(exposed_summary(&state.summary));
    }
    if change.enabled {
        enable(&mut state, &change, cc_port, &fingerprint)?;
    } else {
        disable(
            &mut state,
            change.capability,
            &change.target_id,
            &fingerprint,
        )?;
    }
    let proxy_port = state
        .summary
        .proxy
        .as_ref()
        .map(|endpoint| endpoint.remote_port);
    let cc_port = state
        .summary
        .cc
        .as_ref()
        .map(|endpoint| endpoint.remote_port);
    let revision = state.summary.active_proxy_revision;
    if let Some(request) = state.last_request.as_mut() {
        request.proxy_port = proxy_port;
        request.cc_port = cc_port;
        if let Some(revision) = revision {
            request.expected_revision = revision;
        }
    }
    state.pending = None;
    state.extension_pending = None;
    sync_tool_states(&mut state.summary);
    refresh(&mut state);
    Ok(exposed_summary(&state.summary))
}

fn network_setup(
    state: &mut Store,
    proxy: Option<Endpoint>,
    ai_enabled: bool,
    fingerprint: &str,
) -> BridgeResult<()> {
    let target = state.summary.target.as_ref().ok_or("invalidTarget")?;
    let managed = state.capability_vscode_managed || vscode_network::is_managed(&target.id)?;
    if proxy.is_none() && !ai_enabled {
        vscode_network::restore(&target.id)?;
        // Restoration removes the persistent ownership record. Remember
        // ownership for this live bridge so resuming can reapply its setup.
        state.capability_vscode_managed = managed;
        return Ok(());
    }
    if !managed {
        return Ok(());
    }
    let session = state
        .ai_relay
        .as_ref()
        .or(state.proxy_relay.as_ref())
        .ok_or("relayUnavailable")?
        .session_id();
    vscode_network::apply(&target.id, session, fingerprint, proxy.as_ref())?;
    state.capability_vscode_managed = true;
    Ok(())
}

fn remove_environment(state: &Store, target_id: &str) -> BridgeResult<()> {
    if let Some(relay) = state.proxy_relay.as_ref() {
        ssh::remote(
            target_id,
            json!({"operation":"session-env-remove", "sessionId":relay.session_id()}),
        )?;
    }
    Ok(())
}

fn disable(
    state: &mut Store,
    capability: Capability,
    target_id: &str,
    fingerprint: &str,
) -> BridgeResult<()> {
    match capability {
        Capability::Proxy => {
            let ai_enabled = state.summary.cc.is_some();
            network_setup(state, None, ai_enabled, fingerprint)?;
            if let Err(error) = remove_environment(state, target_id) {
                state.summary.session_environment_state = RuntimeState::Warning;
                let original = state.summary.proxy.clone();
                if network_setup(state, original, ai_enabled, fingerprint).is_err() {
                    state.summary.vscode_state = RuntimeState::Warning;
                    return Err("capabilityPartialFailure".into());
                }
                return Err(error);
            }
            state
                .proxy_relay
                .as_ref()
                .ok_or("relayUnavailable")?
                .set_enabled(false);
            state.paused_proxy = state.summary.proxy.take();
            state.summary.proxy_status = None;
            state.proxy_status = Status::Disconnected;
            state.summary.session_environment_state = RuntimeState::Ready;
        }
        Capability::Cc => {
            // Preflight every owned tool before restoring the first file.
            // Each file keeps its existing hash-checked restore transaction.
            let mut plans = Vec::new();
            for adapter in tool_adapter::adapters() {
                if adapter.configured(&state.summary) {
                    let plan = restore_plan(target_id, adapter.id())?;
                    plans.push((adapter.id(), plan));
                }
            }
            let proxy = state.summary.proxy.clone();
            network_setup(state, proxy, false, fingerprint)?;
            for (tool, plan) in plans {
                if let Err(error) = ssh::remote(target_id, plan) {
                    sync_tool_states(&mut state.summary);
                    let proxy = state.summary.proxy.clone();
                    let recovered = network_setup(state, proxy, true, fingerprint).is_ok();
                    return Err(if state.paused_tools.is_empty() && recovered {
                        error
                    } else {
                        "capabilityPartialFailure".into()
                    });
                }
                if !state.paused_tools.contains(&tool) {
                    state.paused_tools.push(tool);
                }
                tool_adapter::by_id(tool).restore(&mut state.summary);
                clear_cached_profile(state, tool);
            }
            state
                .ai_relay
                .as_ref()
                .ok_or("relayUnavailable")?
                .set_enabled(false);
            state.paused_cc = state.summary.cc.take();
            state.summary.cc_status = None;
            state.cc_status = Status::Disconnected;
            state.profile_sync_state = settings::ProfileSyncState::NotStarted;
            state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
            state.summary.codex_state = RuntimeState::Ready;
            state.summary.claude_state = RuntimeState::Ready;
        }
    }
    state.summary.vscode_state = RuntimeState::Ready;
    Ok(())
}

fn restore_plan(
    target_id: &str,
    tool: tool_adapter::RemoteToolId,
) -> BridgeResult<serde_json::Value> {
    let preview = ssh::remote(
        target_id,
        json!({"operation":"restore-preview", "tool":tool.as_str()}),
    )?;
    Ok(json!({"operation":"restore", "tool":tool.as_str(),
        "expectedHash":preview["expectedHash"].as_str().ok_or("remoteFailed")?,
        "backupHash":preview["backupHash"].as_str().ok_or("remoteFailed")?,
        "repairPermissions":preview["permissionHardening"].as_bool().ok_or("remoteFailed")?}))
}

fn enable(
    state: &mut Store,
    change: &CapabilityChange,
    cc_port: u16,
    fingerprint: &str,
) -> BridgeResult<()> {
    let retained = match change.capability {
        Capability::Proxy => state.paused_proxy.clone(),
        Capability::Cc => state.paused_cc.clone(),
    };
    if retained.is_none() {
        add_lane(state, change, cc_port, fingerprint)?;
    }
    let endpoint = match change.capability {
        Capability::Proxy => state.paused_proxy.clone(),
        Capability::Cc => state.paused_cc.clone(),
    }
    .ok_or("relayUnavailable")?;
    if !listening(&endpoint.local) {
        return Err(match change.capability {
            Capability::Proxy => "proxyUnavailable",
            Capability::Cc => "ccUnavailable",
        }
        .into());
    }
    if change.capability == Capability::Proxy {
        let context = active::snapshot().map_err(|_| "activeChanged")?;
        if !context.available
            || context.revision != change.expected_revision
            || Some(context.revision) != state.summary.active_proxy_revision
        {
            return Err("activeChanged".into());
        }
        let mut next = state.summary.clone();
        next.proxy = Some(endpoint.clone());
        let session_id = state
            .proxy_relay
            .as_ref()
            .ok_or("relayUnavailable")?
            .session_id();
        apply_session_environment(&change.target_id, &next, Some(session_id))?;
        let ai_enabled = state.summary.cc.is_some();
        if let Err(error) = network_setup(state, Some(endpoint.clone()), ai_enabled, fingerprint) {
            if remove_environment(state, &change.target_id).is_err() {
                state.summary.session_environment_state = RuntimeState::Warning;
                return Err("capabilityPartialFailure".into());
            }
            return Err(error);
        }
        state
            .proxy_relay
            .as_ref()
            .ok_or("relayUnavailable")?
            .set_enabled(true);
        state.summary.proxy = state.paused_proxy.take();
        state.summary.proxy_status = Some(Status::Connected);
        state.summary.session_environment_state = RuntimeState::Ready;
    } else {
        // Inspect every local profile before writing any remote tool file.
        let profiles = state
            .paused_tools
            .iter()
            .copied()
            .map(|tool| {
                LocalToolProfile::inspect(tool)
                    .map(|profile| (tool, profile))
                    .map_err(|_| {
                        String::from(match tool {
                            tool_adapter::RemoteToolId::Codex => "localCodexProfileInvalid",
                            tool_adapter::RemoteToolId::Claude => "localClaudeProfileInvalid",
                        })
                    })
            })
            .collect::<BridgeResult<Vec<_>>>()?;
        let proxy = state.summary.proxy.clone();
        network_setup(state, proxy, true, fingerprint)?;
        let token = state.ai_relay.as_ref().ok_or("relayUnavailable")?.token();
        // Previously enabled tools are reapplied through their original
        // preview/apply/verify flow, with fresh local profiles and no IPC token.
        let mut applied = Vec::new();
        for (tool, profile) in profiles {
            let mut attempted_write = false;
            match apply_tool(
                &change.target_id,
                tool,
                endpoint.remote_port,
                token.as_str(),
                &profile,
                &mut attempted_write,
            ) {
                Ok(()) => applied.push((tool, profile)),
                Err(error) => {
                    // Keep the route gate closed. Restore any completed tool
                    // transaction; never claim success after partial writes.
                    let mut cleanup_ok = true;
                    for attempted in applied
                        .iter()
                        .map(|(tool, _)| *tool)
                        .chain(attempted_write.then_some(tool))
                    {
                        cleanup_ok &= restore_plan(&change.target_id, attempted)
                            .and_then(|plan| ssh::remote(&change.target_id, plan))
                            .is_ok();
                    }
                    let proxy = state.summary.proxy.clone();
                    cleanup_ok &= network_setup(state, proxy, false, fingerprint).is_ok();
                    if !cleanup_ok {
                        state.summary.vscode_state = RuntimeState::Warning;
                    }
                    return Err(if cleanup_ok {
                        error
                    } else {
                        "capabilityPartialFailure".into()
                    });
                }
            }
        }
        for (tool, profile) in applied {
            tool_adapter::by_id(tool).apply(&mut state.summary);
            set_cached_profile(state, Some(profile));
            if tool == tool_adapter::RemoteToolId::Codex {
                state.profile_sync_state = settings::ProfileSyncState::RestartRequired;
            } else {
                state.summary.claude_profile_state = settings::ProfileSyncState::RestartRequired;
            }
        }
        state
            .ai_relay
            .as_ref()
            .ok_or("relayUnavailable")?
            .set_enabled(true);
        state.summary.cc = state.paused_cc.take();
        state.summary.cc_status = Some(Status::Connected);
        state.paused_tools.clear();
    }
    state.summary.vscode_state = RuntimeState::Ready;
    Ok(())
}

fn apply_tool(
    target: &str,
    tool: tool_adapter::RemoteToolId,
    port: u16,
    token: &str,
    profile: &LocalToolProfile,
    attempted_write: &mut bool,
) -> BridgeResult<()> {
    let adapter = tool_adapter::by_id(tool);
    let preview = ssh::remote(
        target,
        remote_request("preview", adapter, port, Some(profile), Some(token)),
    )?;
    let mut request = remote_request("apply", adapter, port, Some(profile), Some(token));
    request["expectedHash"] = json!(preview["expectedHash"].as_str().ok_or("remoteFailed")?);
    request["repairPermissions"] = json!(preview["permissionHardening"]
        .as_bool()
        .ok_or("remoteFailed")?);
    *attempted_write = true;
    ssh::remote(target, request)?;
    let verified = ssh::remote(
        target,
        remote_request("preview", adapter, port, Some(profile), Some(token)),
    )?;
    if verified["profileHash"].as_str() != Some(profile.hash())
        || verified["previousPort"].as_u64() != Some(u64::from(port))
    {
        return Err("profileVerifyFailed".into());
    }
    Ok(())
}

fn add_lane(
    state: &mut Store,
    change: &CapabilityChange,
    cc_port: u16,
    fingerprint: &str,
) -> BridgeResult<()> {
    let context = active::snapshot().map_err(|_| "activeChanged")?;
    let local = match change.capability {
        Capability::Proxy => {
            if context.revision != change.expected_revision {
                return Err("activeChanged".into());
            }
            plan::validate_and_normalize_endpoint(&active::endpoint(
                context.available_candidate().ok_or("proxyUnavailable")?,
            ))
            .map_err(|_| "proxyUnavailable")?
        }
        Capability::Cc => ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: cc_port,
            protocol: ProxyProtocol::Http,
        },
    };
    if !listening(&local) {
        return Err("relayUnavailable".into());
    }
    let reserved = transport_summary(state);
    let preferred = match change.capability {
        Capability::Proxy => runtime_expected_port(
            state.summary.target.as_ref().ok_or("invalidTarget")?,
            Some(local.protocol),
        )
        .unwrap_or(local.port),
        Capability::Cc => DEFAULT_CC_REMOTE_PORT,
    };
    let lane = if change.capability == Capability::Proxy {
        0
    } else {
        1
    };
    let remote_port = first_available_port(
        std::iter::once(preferred)
            .chain((0..24).map(|round| derived_port(fingerprint, round, lane))),
        None,
        |port| {
            if reserved
                .proxy
                .as_ref()
                .is_some_and(|endpoint| endpoint.remote_port == port)
                || reserved
                    .cc
                    .as_ref()
                    .is_some_and(|endpoint| endpoint.remote_port == port)
            {
                return Ok(false);
            }
            match ssh::remote(
                &change.target_id,
                json!({"operation":"check", "ports":[port]}),
            ) {
                Ok(_) => Ok(true),
                Err(code) if code == "portInUse" => Ok(false),
                Err(code) => Err(code),
            }
        },
    )?;
    let endpoint = Endpoint { local, remote_port };
    let mode = if change.capability == Capability::Proxy {
        authenticated_relay::RelayMode::General(endpoint.local.protocol)
    } else {
        authenticated_relay::RelayMode::AiHttp
    };
    let ai_route = state
        .ai_relay
        .as_ref()
        .zip(reserved.cc.as_ref())
        .map(|(relay, endpoint)| relay.ai_route(endpoint.remote_port));
    let relay = authenticated_relay::AuthenticatedRelay::start_with_ai_route(
        endpoint.local.clone(),
        mode,
        ai_route,
    )?;
    relay.set_enabled(false);
    let mut request = state.last_request.clone().ok_or("bridgeUnavailable")?;
    request.proxy_port = (change.capability == Capability::Proxy).then_some(remote_port);
    request.cc_port = (change.capability == Capability::Cc).then_some(remote_port);
    let mut child = ssh::tunnel(&request, &[(remote_port, "127.0.0.1".into(), relay.port())])?;
    let mut verified = false;
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(300));
        if !matches!(child.is_running(), Ok(true)) {
            return Err("forwardDenied".into());
        }
        match ssh::remote(
            &change.target_id,
            json!({"operation":"verify", "ports":[remote_port]}),
        ) {
            Ok(_) => {
                verified = true;
                break;
            }
            Err(code) if code == "unsafeBinding" => return Err(code),
            Err(_) => {}
        }
    }
    if !verified {
        return Err("forwardDenied".into());
    }
    if ssh::fingerprint(&change.target_id)? != fingerprint {
        return Err("sshConfigChanged".into());
    }
    match change.capability {
        Capability::Proxy => {
            state.paused_proxy = Some(endpoint);
            state.proxy_relay = Some(relay);
            state.summary.active_proxy_revision = Some(context.revision);
        }
        Capability::Cc => {
            if let Some(proxy) = state.proxy_relay.as_ref() {
                proxy.set_ai_route(Some(relay.ai_route(remote_port)));
            }
            state.paused_cc = Some(endpoint);
            state.ai_relay = Some(relay);
        }
    }
    state.additional_tunnels.push(Box::new(child));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capability_changes_require_explicit_confirmation_before_discovery_or_io() {
        let change = CapabilityChange {
            target_id: "invalid".into(),
            capability: Capability::Cc,
            enabled: true,
            expected_revision: 0,
            cc_local_port: 0,
        };
        assert_eq!(
            set_capability(change, false).unwrap_err(),
            "confirmationRequired"
        );
    }
    #[test]
    fn recovery_reserves_paused_ports_without_changing_effective_capabilities() {
        let endpoint = |port| Endpoint {
            local: ProxyEndpoint {
                host: "127.0.0.1".into(),
                port,
                protocol: ProxyProtocol::Http,
            },
            remote_port: port,
        };
        let state = Store {
            paused_proxy: Some(endpoint(7897)),
            paused_cc: Some(endpoint(15721)),
            ..Store::default()
        };
        let transport = transport_summary(&state);
        assert_eq!(transport.proxy.unwrap().remote_port, 7897);
        assert_eq!(transport.cc.unwrap().remote_port, 15721);
        assert!(state.summary.proxy.is_none() && state.summary.cc.is_none());
    }
    #[test]
    fn ipc_rejects_unknown_capabilities_and_extra_fields() {
        assert!(serde_json::from_value::<CapabilityChange>(json!({"targetId":"target", "capability":"other", "enabled":true,"expectedRevision":0,"ccLocalPort":15721})).is_err());
        assert!(serde_json::from_value::<CapabilityChange>(json!({"targetId":"target", "capability":"proxy", "enabled":true,"expectedRevision":0,"ccLocalPort":15721,"sessionToken":"never accepted"})).is_err());
    }
}
