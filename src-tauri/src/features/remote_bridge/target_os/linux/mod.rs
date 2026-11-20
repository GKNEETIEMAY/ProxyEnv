//! Linux wire operations. The bundled scripts reject other platforms before writing.
use super::super::{tool_adapter, tool_discovery, BridgeResult};
use zeroize::Zeroizing;

pub(in super::super) const SCRIPT_COMMAND: &str = "sh -s";
pub(in super::super) const SKILL_SCRIPT: &str = include_str!("../../skill-remote.sh");

pub(in super::super) fn managed_terminal_remote_command(session_id: &str) -> BridgeResult<String> {
    if session_id.len() != 32 || !session_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalidRequest".into());
    }
    Ok(format!(
        "proxyenv_session=\"$HOME/.proxyenv/sessions/{session_id}/env.sh\"; [ -f \"$proxyenv_session\" ] || exit 1; . \"$proxyenv_session\"; unset proxyenv_session; exec \"${{SHELL:-/bin/sh}}\" -i"
    ))
}

pub(in super::super) fn session_environment_source_command(session_id: &str) -> String {
    format!(". \"$HOME/.proxyenv/sessions/{session_id}/env.sh\"")
}

pub(in super::super) fn tunnel_remote_command(ports: &[u16], marker: &str) -> String {
    let checks = ports
        .iter()
        .map(|port| {
            format!(
                "lines=\"$(ss -H -ltn 2>/dev/null)\"; printf \"%s\\n\" \"$lines\" | grep -Eq \"127\\.0\\.0\\.1:{port}([[:space:]]|$)\" || ready=0; printf \"%s\\n\" \"$lines\" | grep -Eq \"(^|[[:space:]])(\\*|0\\.0\\.0\\.0|\\[::\\]):{port}([[:space:]]|$)\" && exit 74 || :;"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "sh -c 'set -u; i=0; while [ \"$i\" -lt 12 ]; do ready=1; {checks} if [ \"$ready\" -eq 1 ]; then printf \"\\n{}\\n\"; exec sh -c \"while :; do sleep 3600; done\"; fi; i=$((i+1)); sleep 1; done; exit 73'",
        marker
    )
}

pub(in super::super) fn interactive_check_remote_command(ports: &[u16], marker: &str) -> String {
    let ports = ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "sh -c 'set -eu; printf \"\\n{marker}\\n\"; fail() {{ printf \"{{\\\"error\\\":\\\"%s\\\"}}\\n\" \"$1\"; exit 0; }}; [ \"$(uname -s)\" = Linux ] || fail remoteUnsupported; [ \"$(id -u)\" != 0 ] || fail rootForbidden; for utility in ss awk sha256sum mktemp flock sync stat grep cut cp mv cat unlink; do command -v \"$utility\" >/dev/null 2>&1 || fail dependencyMissing; done; for number in {ports}; do entries=\"$(ss -H -ltn \"sport = :$number\")\" || fail remoteUnsupported; [ -z \"$entries\" ] || fail portInUse; done; printf \"{{\\\"verified\\\":true}}\\n\"'"
    )
}

pub(in super::super) fn remote_payload(
    request: &serde_json::Value,
) -> BridgeResult<(Zeroizing<String>, &'static str)> {
    let operation = request["operation"].as_str().ok_or("invalidRequest")?;
    if ![
        "check",
        "verify",
        "internet",
        "test",
        "preview",
        "status",
        "apply",
        "restore",
        "restore-preview",
        "tool-verify",
        "tool-discover",
        "session-env-apply",
        "session-env-remove",
    ]
    .contains(&operation)
    {
        return Err("invalidRequest".into());
    }
    let tool = request["tool"].as_str().unwrap_or("codex");
    if !["codex", "claude"].contains(&tool) {
        return Err("invalidRequest".into());
    }
    let port = request["port"].as_u64().unwrap_or(25721);
    if !(1024..=65535).contains(&port) {
        return Err("invalidPort".into());
    }
    let mut ports = Vec::new();
    if let Some(values) = request["ports"].as_array() {
        for value in values {
            let value = value.as_u64().ok_or("invalidPort")?;
            if !(1024..=65535).contains(&value) {
                return Err("invalidPort".into());
            }
            ports.push(value.to_string());
        }
    }
    if operation == "test" {
        ports = vec![port.to_string()];
    }
    let expected = request["expectedHash"].as_str().unwrap_or("absent");
    let expected_backup = request["backupHash"].as_str().unwrap_or("absent");
    let repair_permissions = request["repairPermissions"].as_bool().unwrap_or(false);
    for hash in [expected, expected_backup] {
        if hash != "absent" && !(hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err("invalidRequest".into());
        }
    }
    let protocol = request["protocol"].as_str().unwrap_or("http");
    if !["http", "socks5", "mixed"].contains(&protocol) {
        return Err("invalidRequest".into());
    }
    let scheme = if protocol == "socks5" {
        "socks5h"
    } else {
        "http"
    };
    let profile_model = request["profileModelBase64"].as_str().unwrap_or("");
    let profile_catalog = request["profileCatalogBase64"].as_str().unwrap_or("");
    let profile_settings = request["profileSettingsBase64"].as_str().unwrap_or("");
    let profile_hash = request["profileHash"].as_str().unwrap_or("");
    let session_token = request["sessionToken"].as_str().unwrap_or("");
    let session_id = request["sessionId"].as_str().unwrap_or("");
    let valid_base64 = |value: &str, limit: usize| {
        value.len() <= limit
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
    };
    if !valid_base64(profile_model, 512)
        || !valid_base64(profile_catalog, 11 * 1024 * 1024)
        || !valid_base64(profile_settings, 256 * 1024)
        || (!profile_hash.is_empty()
            && !(profile_hash.len() == 64
                && profile_hash.bytes().all(|byte| byte.is_ascii_hexdigit())))
        || (!session_token.is_empty()
            && !(session_token.len() == 64
                && session_token.bytes().all(|byte| byte.is_ascii_hexdigit())))
        || (!session_id.is_empty()
            && !(session_id.len() == 32 && session_id.bytes().all(|byte| byte.is_ascii_hexdigit())))
        || (matches!(operation, "preview" | "status" | "apply" | "tool-verify")
            && session_token.is_empty())
        || (matches!(operation, "session-env-apply" | "session-env-remove")
            && session_id.is_empty())
        || (tool == "codex"
            && matches!(operation, "preview" | "apply")
            && (profile_model.is_empty() || profile_catalog.is_empty() || profile_hash.is_empty()))
        || (tool == "claude"
            && matches!(operation, "preview" | "apply")
            && (profile_settings.is_empty() || profile_hash.is_empty()))
    {
        return Err("invalidRequest".into());
    }
    let extension_version = request["extensionVersion"].as_str().unwrap_or("");
    if !extension_version.is_empty()
        && (!tool_adapter::valid_version(extension_version)
            || tool_adapter::by_name(tool)?.compatibility(extension_version)
                != tool_adapter::RemoteToolCompatibility::Compatible
            || !matches!(operation, "preview" | "apply"))
    {
        return Err("invalidRequest".into());
    }
    let bundled = include_str!("../../remote.sh");
    // Read-only transport probes return before any configuration code runs.
    // Do not keep writing the entire configuration helper after the remote
    // shell has already emitted its result and closed its stdin.
    let script = if matches!(operation, "check" | "verify" | "internet" | "test") {
        bundled
            .split_once("\numask 077\n")
            .ok_or("stateUnavailable")?
            .0
    } else if operation == "tool-discover" {
        bundled
            .split_once("\n# PROXYENV_CONFIG_DEPENDENCIES\n")
            .ok_or("stateUnavailable")?
            .0
    } else {
        bundled
    };
    let script = format!("extension_version='{extension_version}'\n{script}");
    let source = Zeroizing::new(format!("operation='{operation}'\ntool='{tool}'\nport={port}\nports='{}'\nexpected='{expected}'\nexpected_backup='{expected_backup}'\nrepair_permissions={repair_permissions}\nprotocol='{protocol}'\nscheme='{scheme}'\nprofile_model_b64='{profile_model}'\nprofile_catalog_b64='{profile_catalog}'\nprofile_settings_b64='{profile_settings}'\nprofile_hash='{profile_hash}'\nsession_id='{session_id}'\nsession_token='{session_token}'\nPROXYENV_SESSION_TOKEN=\"$session_token\"\nexport PROXYENV_SESSION_TOKEN\n{script}", ports.join(" ")));
    let operation = match operation {
        "check" => "check",
        "verify" => "verify",
        "internet" => "internet",
        "test" => "test",
        "preview" => "preview",
        "status" => "status",
        "apply" => "apply",
        "restore" => "restore",
        "restore-preview" => "restore-preview",
        "tool-verify" => "tool-verify",
        "tool-discover" => "tool-discover",
        "session-env-apply" => "session-env-apply",
        "session-env-remove" => "session-env-remove",
        _ => return Err("invalidRequest".into()),
    };
    Ok((source, operation))
}

pub(in super::super) fn parse_remote_output(
    operation: &str,
    text: &str,
) -> BridgeResult<serde_json::Value> {
    if operation == "tool-discover" && text.len() > 4096 {
        return Err("remoteUnsupported".into());
    }
    let value: serde_json::Value =
        serde_json::from_str(text.trim()).map_err(|_| "remoteUnsupported")?;
    if let Some(code) = value.get("error").and_then(|v| v.as_str()) {
        // This allowlist also prevents a malicious SSH server leaking arbitrary text.
        const CODES: &[&str] = &[
            "invalidPort",
            "remoteUnsupported",
            "unsafeBinding",
            "portInUse",
            "unsafePath",
            "configConflict",
            "legacyModelSelectionRequired",
            "localCodexProfileInvalid",
            "localClaudeProfileInvalid",
            "localCatalogMissing",
            "localCatalogUnsafe",
            "remoteProfileConflict",
            "remoteProfileOutOfSync",
            "remoteCatalogWriteFailed",
            "remoteConfigWriteFailed",
            "profileVerifyFailed",
            "codexRestartRequired",
            "routeOutdated",
            "cliMissing",
            "cliUnsupported",
            "customHome",
            "invalidRequest",
            "noBackup",
            "verifyFailed",
            "rollbackConflict",
            "rollbackFailed",
            "writeRolledBack",
            "networkFailed",
            "dependencyMissing",
            "jsonEditorMissing",
            "remoteFailed",
            "rootForbidden",
        ];
        return Err(if CODES.contains(&code) {
            code
        } else {
            "remoteFailed"
        }
        .into());
    }
    let valid = match operation {
        "check" | "verify" => value["verified"] == true,
        "test" => value["tested"] == true,
        "apply" => value["configured"] == true,
        "status" => {
            value["configured"].is_boolean()
                && (value["previousPort"].is_null()
                    || value["previousPort"]
                        .as_u64()
                        .is_some_and(|port| (1024..=65535).contains(&port)))
                && (value["remoteModel"].is_null()
                    || value["remoteModel"].as_str().is_some_and(|model| {
                        !model.is_empty()
                            && model.len() <= 256
                            && !model.chars().any(char::is_control)
                    }))
                && value["profileHash"].as_str().is_some_and(|hash| {
                    hash.is_empty()
                        || (hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
                })
        }
        "restore" => value["configured"] == false,
        "session-env-apply" => value["sessionEnvironment"] == "applied",
        "session-env-remove" => value["sessionEnvironment"] == "removed",
        "tool-verify" => matches!(
            value["verification"].as_str(),
            Some(
                "verified" | "authenticationRequired" | "routeUnavailable" | "timedOut" | "failed"
            )
        ),
        _ => value.is_object(),
    };
    if !valid {
        return Err("remoteFailed".into());
    }
    if operation == "tool-verify" {
        return Ok(serde_json::json!({
            "verification": value["verification"].as_str().ok_or("remoteFailed")?
        }));
    }
    if operation == "tool-discover" {
        tool_discovery::parse(value.clone())?;
    }
    Ok(value)
}

fn skill_value<'a>(request: &'a serde_json::Value, name: &str) -> BridgeResult<&'a str> {
    request[name]
        .as_str()
        .ok_or_else(|| "invalidRequest".into())
}

fn skill_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value != "."
        && value != ".."
        && !value.starts_with(".proxyenv-")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub(in super::super) fn skill_payload(
    request: &serde_json::Value,
) -> BridgeResult<Zeroizing<String>> {
    let operation = skill_value(request, "operation")?;
    let tool = skill_value(request, "tool")?;
    let name = skill_value(request, "skillName")?;
    let hash = skill_value(request, "skillHash")?;
    let manifest = request["manifestBase64"].as_str().unwrap_or("");
    let directories = request["directoriesBase64"].as_str().unwrap_or("");
    let file_count = request["fileCount"].as_u64().unwrap_or(0);
    let total_size = request["totalSize"].as_u64().unwrap_or(0);
    let valid_base64 = |value: &str, limit: usize| {
        value.len() <= limit
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
    };
    if !["status", "prepare", "apply", "remove", "cleanup"].contains(&operation)
        || !["codex", "claude"].contains(&tool)
        || !skill_component(name)
        || hash.len() != 64
        || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        || file_count > 256
        || total_size > 8 * 1024 * 1024
        || !valid_base64(manifest, 512 * 1024)
        || !valid_base64(directories, 256 * 1024)
        || (operation == "prepare" && (manifest.is_empty() || file_count == 0))
    {
        return Err("invalidRequest".into());
    }
    Ok(Zeroizing::new(format!(
        "operation='{operation}'\ntool='{tool}'\nskill_name='{name}'\nskill_hash='{hash}'\nfile_count={file_count}\ntotal_size={total_size}\nmanifest_b64='{manifest}'\ndirectories_b64='{directories}'\n{}",
        // Also protect builds from existing Windows checkouts or editor CRLF saves.
        SKILL_SCRIPT.replace("\r\n", "\n")
    )))
}

// The request is JSON data inside a quoted heredoc, never a remote command.
pub(in super::super) fn extension_payload(request: &serde_json::Value) -> Zeroizing<String> {
    let source = format!("{}\n\"$bridge_node\" - <<'PROXYENV_EXTENSION_JS'\nglobalThis.bridgeExtensionRequest = {};\n{}\nPROXYENV_EXTENSION_JS\n",
        include_str!("../../extension-launch.sh"), request, include_str!("../../extension-helper.cjs"));
    Zeroizing::new(source)
}

pub(in super::super) fn parse_extension_output(text: &str) -> BridgeResult<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).map_err(|_| "remoteFailed")?;
    if let Some(error) = value["error"].as_str() {
        return Err(match error {
            "configConflict"
            | "unsafePath"
            | "noBackup"
            | "verifyFailed"
            | "rollbackConflict"
            | "rollbackFailed"
            | "writeRolledBack"
            | "extensionMissing"
            | "extensionUnsupported"
            | "remoteNodeMissing"
            | "remoteNodeUnsafe"
            | "remoteNodeUnsupported"
            | "vscodeServerMissing"
            | "vscodeServerAmbiguous"
            | "vscodeNetworkBusy"
            | "vscodeNetworkConflict"
            | "vscodeNetworkUnsafe"
            | "extensionContextChanged"
            | "customHome"
            | "remoteUnsupported" => error,
            _ => "remoteFailed",
        }
        .into());
    }
    Ok(value)
}

pub(in super::super) fn parse_skill_output(text: &str) -> BridgeResult<serde_json::Value> {
    let value: serde_json::Value =
        serde_json::from_str(text.trim()).map_err(|_| "remoteUnsupported")?;
    if let Some(error) = value["error"].as_str() {
        return Err(match error {
            "invalidRequest" | "remoteUnsupported" | "rootForbidden" | "dependencyMissing"
            | "unsafePath" | "skillConflict" | "skillBusy" | "skillVerifyFailed"
            | "remoteFailed" | "rollbackFailed" | "writeRolledBack" => error,
            _ => "remoteFailed",
        }
        .into());
    }
    Ok(value)
}

pub(in super::super) fn skill_upload_path(
    tool: &str,
    name: &str,
    hash: &str,
    relative_path: &str,
) -> BridgeResult<String> {
    if !["codex", "claude"].contains(&tool)
        || !skill_component(name)
        || hash.len() != 64
        || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        || relative_path.split('/').any(|part| !skill_component(part))
    {
        return Err("invalidRequest".into());
    }
    Ok(format!(
        "~/.proxyenv/staging/skills/{tool}/{name}/{hash}/{relative_path}"
    ))
}

#[cfg(test)]
#[path = "linux_contract.rs"]
mod contract;
