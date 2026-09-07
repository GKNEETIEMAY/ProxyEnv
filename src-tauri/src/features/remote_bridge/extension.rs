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
    pub runtime_version: String,
    pub configuration: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inspection {
    pub user: String,
    pub context_hash: String,
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
fn checked_inspection(value: serde_json::Value) -> BridgeResult<Inspection> {
    let result: Inspection = serde_json::from_value(value).map_err(|_| "remoteFailed")?;
    if !valid_hash(&result.context_hash)
        || !ssh::safe_name(&result.user)
        || result.user.len() > 64
        || result.extensions.len() != 2
    {
        return Err("remoteFailed".into());
    }
    for (entry, tool) in result.extensions.iter().zip(["codex", "claude"]) {
        if !["notConfigured", "configured", "conflict"].contains(&entry.configuration.as_str())
            || entry.tool != tool
            || !valid_version(&entry.version)
            || !valid_version(&entry.runtime_version)
        {
            return Err("remoteFailed".into());
        }
    }
    Ok(result)
}
pub fn inspect(alias: String) -> BridgeResult<Inspection> {
    let fingerprint = ssh::fingerprint(&alias)?;
    let inspection = checked_inspection(ssh::extension_remote(
        &alias,
        &json!({"operation":"inspect"}),
    )?)?;
    if ssh::fingerprint(&alias)? != fingerprint {
        return Err("sshConfigChanged".into());
    }
    let mut state = lock()?;
    if state.summary.alias.as_ref() == Some(&alias) {
        state.summary.codex_extension = Some(inspection.extensions[0].configuration.clone());
        state.summary.claude_extension = Some(inspection.extensions[1].configuration.clone());
    }
    Ok(inspection)
}
pub fn preview(selection: Selection) -> BridgeResult<Preview> {
    if !selection.remote_confirmed {
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
        if state.child.is_none() || state.summary.alias.as_ref() != Some(&selection.alias) {
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
    if !selection.restore && !capability.supported {
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
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| "stateUnavailable")?;
    let preview = Preview {
        id: hex::encode(nonce),
        alias: selection.alias,
        tool: selection.tool.clone(),
        path: if selection.tool == "codex" {
            "~/.codex/config.toml"
        } else {
            "~/.vscode-server/data/Machine/settings.json"
        }
        .into(),
        version: capability.version.clone(),
        runtime_version: capability.runtime_version.clone(),
        port,
        previous_port,
        original_exists: value["originalExists"].as_bool().ok_or("remoteFailed")?,
        restore: selection.restore,
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
            || state.summary.alias.as_ref() != Some(&pending.preview.alias)
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
    if state.summary.alias.as_ref() == Some(&pending.preview.alias) {
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
