use super::{lock, refresh, ssh, BridgeResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capability {
    pub tool: String,
    pub detected: bool,
    pub supported: bool,
    pub version: String,
    pub versions: Vec<String>,
    pub runtime_version: String,
    pub runtime_versions: Vec<String>,
    pub candidate_count: usize,
    pub location: String,
    pub configuration: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VscodeRemoteContext {
    pub status: String,
    pub edition: String,
    pub server_root: String,
    pub server_version: String,
    pub server_versions: Vec<String>,
    pub data_path: String,
    pub remote_settings_path: String,
    pub extension_root: String,
    pub evidence: String,
    pub confidence: String,
    pub candidate_count: usize,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inspection {
    pub user: String,
    pub context_hash: String,
    pub vscode: VscodeRemoteContext,
    pub extensions: Vec<Capability>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    pub alias: String,
    pub tool: String,
    pub context_hash: String,
    pub remote_confirmed: bool,
    pub restore: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub id: String,
    pub alias: String,
    pub tool: String,
    pub path: String,
    pub version: String,
    pub runtime_version: String,
    pub port: u16,
    pub previous_port: Option<u16>,
    pub original_exists: bool,
    pub restore: bool,
    pub login_prompt_change: Option<String>,
}
pub(super) struct Pending {
    preview: Preview,
    fingerprint: String,
    context_hash: String,
    expected_hash: String,
    journal_hash: String,
    created: Instant,
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn valid_version(value: &str) -> bool {
    value.len() < 48
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
}
fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn valid_display_path(value: &str) -> bool {
    value.len() <= 256
        && (value.is_empty()
            || (value.starts_with("~/")
                && !value.contains("..")
                && !value.chars().any(char::is_control)))
}
fn checked_inspection(value: serde_json::Value) -> BridgeResult<Inspection> {
    let result: Inspection = serde_json::from_value(value).map_err(|_| "remoteFailed")?;
    if !valid_hash(&result.context_hash)
        || !ssh::safe_name(&result.user)
        || result.user.len() > 64
        || result.extensions.len() != 2
    {
        return Err("remoteFailed".into());
    }
    let vscode = &result.vscode;
    if !["detected", "ambiguous", "unsupported"].contains(&vscode.status.as_str())
        || !["stable", "insiders", "legacy", "custom", "unknown"].contains(&vscode.edition.as_str())
        || ![
            "defaultStableRoot",
            "defaultInsidersRoot",
            "legacyRoot",
            "agentFolderEnvironment",
            "multipleServerRoots",
            "noServerRoot",
        ]
        .contains(&vscode.evidence.as_str())
        || !["high", "medium", "low"].contains(&vscode.confidence.as_str())
        || vscode.candidate_count > 4
        || vscode.server_versions.len() > 16
        || vscode
            .server_versions
            .iter()
            .any(|entry| !valid_token(entry))
        || !valid_display_path(&vscode.server_root)
        || !valid_display_path(&vscode.data_path)
        || !valid_display_path(&vscode.remote_settings_path)
        || !valid_display_path(&vscode.extension_root)
        || (!vscode.server_version.is_empty() && !valid_token(&vscode.server_version))
        || (vscode.status == "detected"
            && (vscode.candidate_count != 1
                || vscode.server_root.is_empty()
                || vscode.data_path.is_empty()
                || vscode.remote_settings_path.is_empty()
                || vscode.extension_root.is_empty()))
        || (vscode.status != "detected"
            && (!vscode.server_root.is_empty()
                || !vscode.data_path.is_empty()
                || !vscode.remote_settings_path.is_empty()
                || !vscode.extension_root.is_empty()))
    {
        return Err("remoteFailed".into());
    }
    for (entry, tool) in result.extensions.iter().zip(["codex", "claude"]) {
        if !["notConfigured", "configured", "conflict", "unknown"]
            .contains(&entry.configuration.as_str())
            || entry.tool != tool
            || !valid_version(&entry.version)
            || !valid_version(&entry.runtime_version)
            || entry.versions.len() > 16
            || entry.runtime_versions.len() > 16
            || entry.versions.iter().any(|value| !valid_version(value))
            || entry
                .runtime_versions
                .iter()
                .any(|value| !valid_version(value))
            || entry.candidate_count > 16
            || !["locationUnknown", "activeUnknown"].contains(&entry.location.as_str())
            || entry.detected != (entry.candidate_count > 0)
            || (entry.supported && !entry.detected)
            || (!entry.version.is_empty()
                && (entry.versions.len() != 1 || entry.versions[0] != entry.version))
            || (!entry.runtime_version.is_empty()
                && (entry.runtime_versions.len() != 1
                    || entry.runtime_versions[0] != entry.runtime_version))
        {
            return Err("remoteFailed".into());
        }
    }
    Ok(result)
}
pub fn inspect(alias: String) -> BridgeResult<Inspection> {
    let mut inspection = discover(&alias)?;
    let mut state = lock()?;
    if state.summary.target.as_ref().map(|target| &target.id) == Some(&alias) {
        if state.summary.codex_configured
            && inspection.extensions[0].detected
            && inspection.extensions[0].configuration == "notConfigured"
        {
            // Both entry points consume the same managed user profile.
            inspection.extensions[0].configuration = "configured".into();
        }
        state.summary.codex_extension = Some(inspection.extensions[0].configuration.clone());
        state.summary.claude_extension = Some(inspection.extensions[1].configuration.clone());
    }
    Ok(inspection)
}

// No store lock: discovery and configuration already guard their own generation.
pub(super) fn discover(alias: &str) -> BridgeResult<Inspection> {
    let fingerprint = ssh::fingerprint(alias)?;
    let inspection = checked_inspection(ssh::extension_remote(
        alias,
        &json!({"operation":"inspect"}),
    )?)?;
    if ssh::fingerprint(alias)? != fingerprint {
        return Err("sshConfigChanged".into());
    }
    Ok(inspection)
}

pub(super) fn profile_version(
    inspection: &Inspection,
    tool: super::tool_adapter::RemoteToolId,
) -> BridgeResult<String> {
    use super::tool_adapter::{by_id, valid_version, RemoteToolCompatibility};
    if inspection.vscode.status != "detected" {
        return Err("extensionContextChanged".into());
    }
    let capability = inspection
        .extensions
        .iter()
        .find(|entry| entry.tool == tool.as_str())
        .ok_or("extensionMissing")?;
    if !capability.detected {
        return Err("extensionMissing".into());
    }
    let versions = if tool == super::tool_adapter::RemoteToolId::Codex {
        &capability.runtime_versions
    } else {
        &capability.versions
    };
    // VS Code bundles may ship a version such as 0.151.0-alpha.7.2.
    // Keep standalone CLI discovery strict; use the plugin's measured core version.
    let versions: Vec<_> = versions
        .iter()
        .map(|version| version.split('-').next().unwrap_or_default())
        .collect();
    if !capability.supported
        || versions.is_empty()
        || versions.iter().any(|version| {
            !valid_version(version)
                || by_id(tool).compatibility(version) != RemoteToolCompatibility::Compatible
        })
    {
        return Err("extensionUnsupported".into());
    }
    // Old extension folders can coexist after an update. All must be compatible;
    // use the oldest measured core, without claiming which extension is active.
    Ok(versions
        .into_iter()
        .min_by_key(|version| {
            version
                .split('.')
                .map(|part| part.parse::<u16>().unwrap_or_default())
                .collect::<Vec<_>>()
        })
        .ok_or("extensionUnsupported")?
        .to_owned())
}
pub fn preview(selection: Selection) -> BridgeResult<Preview> {
    // Codex CLI and VS Code share the managed user profile. The old extension
    // transaction is restore-only; never create another active Codex profile.
    if selection.tool == "codex" && !selection.restore {
        return Err("extensionUnsupported".into());
    }
    if !selection.restore && !selection.remote_confirmed {
        return Err("extensionLocationRequired".into());
    }
    if !["codex", "claude"].contains(&selection.tool.as_str())
        || !valid_hash(&selection.context_hash)
    {
        return Err("invalidRequest".into());
    }
    let mut state = lock()?;
    refresh(&mut state);
    state.extension_pending = None;
    let fingerprint = ssh::fingerprint(&selection.alias)?;
    let port = if selection.restore {
        25721
    } else {
        if state.child.is_none()
            || state.summary.target.as_ref().map(|target| &target.id) != Some(&selection.alias)
        {
            return Err("bridgeUnavailable".into());
        }
        if state.target_fingerprint.as_ref() != Some(&fingerprint) {
            return Err("sshConfigChanged".into());
        }
        let endpoint = state.summary.cc.as_ref().ok_or("ccUnavailable")?;
        if !super::listening(&endpoint.local) {
            return Err("ccUnavailable".into());
        }
        endpoint.remote_port
    };
    let inspection = checked_inspection(ssh::extension_remote(
        &selection.alias,
        &json!({"operation":"inspect"}),
    )?)?;
    if inspection.context_hash != selection.context_hash {
        return Err("extensionContextChanged".into());
    }
    let capability = inspection
        .extensions
        .iter()
        .find(|e| e.tool == selection.tool)
        .ok_or("extensionMissing")?;
    if !selection.restore && !capability.detected {
        return Err("extensionMissing".into());
    }
    if !selection.restore && (inspection.vscode.status != "detected" || !capability.supported) {
        return Err("extensionUnsupported".into());
    }
    let value = ssh::extension_remote(
        &selection.alias,
        &json!({
            "operation": if selection.restore { "restore-preview" } else { "preview" },
            "tool": selection.tool, "port": port, "contextHash": inspection.context_hash,
        }),
    )?;
    let expected_hash = value["expectedHash"]
        .as_str()
        .ok_or("remoteFailed")?
        .to_owned();
    let journal_hash = value["journalHash"]
        .as_str()
        .ok_or("remoteFailed")?
        .to_owned();
    if [&expected_hash, &journal_hash]
        .iter()
        .any(|h| h.as_str() != "absent" && !valid_hash(h))
        || value["contextHash"] != inspection.context_hash
    {
        return Err("remoteFailed".into());
    }
    let previous_port = if value["previousPort"].is_null() {
        None
    } else {
        Some(
            value["previousPort"]
                .as_u64()
                .filter(|p| (1024..=65535).contains(p))
                .ok_or("remoteFailed")? as u16,
        )
    };
    let login_prompt_change = match value["loginPromptChange"].as_str() {
        Some(value) if ["add", "unchanged", "overrideFalse"].contains(&value) => {
            Some(value.to_owned())
        }
        None if selection.tool != "claude" || selection.restore => None,
        _ => return Err("remoteFailed".into()),
    };
    let path = value["path"].as_str().ok_or("remoteFailed")?;
    let expected_path = if selection.tool == "codex" {
        "~/.codex/config.toml"
    } else {
        inspection.vscode.remote_settings_path.as_str()
    };
    if path != expected_path || !valid_display_path(path) {
        return Err("remoteFailed".into());
    }
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| "stateUnavailable")?;
    let preview = Preview {
        id: hex::encode(nonce),
        alias: selection.alias,
        tool: selection.tool.clone(),
        path: path.into(),
        version: capability.version.clone(),
        runtime_version: capability.runtime_version.clone(),
        port,
        previous_port,
        original_exists: value["originalExists"].as_bool().ok_or("remoteFailed")?,
        restore: selection.restore,
        login_prompt_change,
    };
    state.extension_pending = Some(Pending {
        preview: preview.clone(),
        fingerprint,
        context_hash: inspection.context_hash,
        expected_hash,
        journal_hash,
        created: Instant::now(),
    });
    Ok(preview)
}
pub fn apply(id: String, confirmed: bool) -> BridgeResult<()> {
    if !confirmed {
        return Err("confirmationRequired".into());
    }
    let mut state = lock()?;
    refresh(&mut state);
    let pending = state
        .extension_pending
        .take()
        .filter(|p| p.preview.id == id && p.created.elapsed() < Duration::from_secs(300))
        .ok_or("configConflict")?;
    if ssh::fingerprint(&pending.preview.alias)? != pending.fingerprint {
        return Err("sshConfigChanged".into());
    }
    if !pending.preview.restore {
        if state.child.is_none()
            || state.summary.target.as_ref().map(|target| &target.id)
                != Some(&pending.preview.alias)
            || state.target_fingerprint.as_ref() != Some(&pending.fingerprint)
        {
            return Err("bridgeUnavailable".into());
        }
        let endpoint = state.summary.cc.as_ref().ok_or("ccUnavailable")?;
        if endpoint.remote_port != pending.preview.port || !super::listening(&endpoint.local) {
            return Err("ccUnavailable".into());
        }
    }
    let value = ssh::extension_remote(
        &pending.preview.alias,
        &json!({
            "operation": if pending.preview.restore { "restore" } else { "apply" },
            "tool": pending.preview.tool, "port": pending.preview.port, "contextHash": pending.context_hash,
            "expectedHash": pending.expected_hash, "journalHash": pending.journal_hash,
        }),
    )?;
    if value["configured"] != !pending.preview.restore {
        return Err("verifyFailed".into());
    }
    if state.summary.target.as_ref().map(|target| &target.id) == Some(&pending.preview.alias) {
        let status = Some(
            if pending.preview.restore {
                "notConfigured"
            } else {
                "configured"
            }
            .to_string(),
        );
        if pending.preview.tool == "codex" {
            state.summary.codex_extension = status;
        } else {
            state.summary.claude_extension = status;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_does_not_accept_unexpected_or_sensitive_metadata() {
        assert!(checked_inspection(
            json!({"user":"test", "contextHash":"a".repeat(64), "extensions":[], "token":"secret"})
        )
        .is_err());
        assert!(!valid_version("x\nsecret"));
        assert!(!valid_hash("a;id"));
    }
    #[test]
    fn inspection_accepts_explicit_server_context_and_multiple_extension_versions() {
        let inspection = checked_inspection(json!({
            "user":"test",
            "contextHash":"a".repeat(64),
            "vscode":{
                "status":"detected",
                "edition":"stable",
                "serverRoot":"~/.vscode-server",
                "serverVersion":"",
                "serverVersions":["commit-a","commit-b"],
                "dataPath":"~/.vscode-server/data",
                "remoteSettingsPath":"~/.vscode-server/data/Machine/settings.json",
                "extensionRoot":"~/.vscode-server/extensions",
                "evidence":"defaultStableRoot",
                "confidence":"medium",
                "candidateCount":1
            },
            "extensions":[
                {"tool":"codex","detected":true,"supported":true,"version":"","versions":["26.825.1","26.825.2"],"runtimeVersion":"0.151.0","runtimeVersions":["0.151.0"],"candidateCount":2,"location":"activeUnknown","configuration":"notConfigured"},
                {"tool":"claude","detected":false,"supported":false,"version":"","versions":[],"runtimeVersion":"","runtimeVersions":[],"candidateCount":0,"location":"locationUnknown","configuration":"notConfigured"}
            ]
        }))
        .expect("valid inspection");
        assert_eq!(inspection.vscode.status, "detected");
        assert_eq!(inspection.extensions[0].candidate_count, 2);

        use super::super::tool_adapter::{RemoteToolId as T, RemoteToolInstallState as S};
        assert_eq!(profile_version(&inspection, T::Codex).unwrap(), "0.151.0");
        let mut candidate = inspection.clone();
        candidate.extensions[0].runtime_version = "0.151.0-alpha.7.2".into();
        candidate.extensions[0].runtime_versions =
            vec!["0.151.0-alpha.7.2".into(), "0.152.0".into()];
        assert_eq!(profile_version(&candidate, T::Codex).unwrap(), "0.151.0");
        let mut tools = super::super::tool_adapter::unknown_installations();
        super::super::tool_discovery::project_extensions(&mut tools, &candidate);
        assert_eq!(
            tools[0].state,
            S::Unknown,
            "Plugin evidence never fabricates a CLI"
        );
        assert_eq!(tools[0].extension_state, S::Installed);
        assert_eq!(tools[1].extension_state, S::Missing);
        assert!(profile_version(&candidate, T::Claude).is_err());
        for version in ["0.133.9", "1.0.0", "", "0.151.0;echo"] {
            candidate.extensions[0].runtime_version = version.into();
            candidate.extensions[0].runtime_versions = vec![version.into()];
            assert!(profile_version(&candidate, T::Codex).is_err());
        }
        candidate.extensions[0].runtime_version = "0.151.0".into();
        candidate.extensions[0].runtime_versions = vec!["0.151.0".into()];
        candidate.extensions[0].detected = false;
        assert!(
            profile_version(&candidate, T::Codex).is_err(),
            "Removed plugins cannot authorize writes"
        );
        candidate.extensions[0].detected = true;
        candidate.vscode.status = "ambiguous".into();
        assert!(profile_version(&candidate, T::Codex).is_err());
        candidate.vscode.status = "detected".into();
        candidate.extensions[1].detected = true;
        candidate.extensions[1].supported = true;
        for (version, supported) in [
            ("2.1.226", false),
            ("2.1.227", true),
            ("2.2.0", true),
            ("3.0.0", false),
        ] {
            candidate.extensions[1].version = version.into();
            candidate.extensions[1].versions = vec![version.into()];
            assert_eq!(profile_version(&candidate, T::Claude).is_ok(), supported);
        }

        let mut value = serde_json::to_value(inspection).unwrap();
        value["vscode"]["remoteSettingsPath"] = json!("~/../private/settings.json");
        assert!(checked_inspection(value).is_err());
    }
    #[test]
    fn location_confirmation_precedes_any_remote_operation() {
        assert!(preview(Selection {
            alias: "test".into(),
            tool: "codex".into(),
            context_hash: "a".repeat(64),
            remote_confirmed: false,
            restore: false
        })
        .is_err());
        assert!(apply("anything".into(), false).is_err());
    }
}
