use crate::features::remote_bridge::{
    self as bridge, BridgeResult, CapabilityChange, CcDetection, ConfigPreview, PortAllocation,
    RemoteNetworkObservation, RemoteTarget, Request, Summary, ToolVerificationResult,
};
use serde::Serialize;
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeCommandError {
    code: String,
    phase: &'static str,
    target: Option<&'static str>,
    retryable: bool,
}

type CommandResult<T> = Result<T, BridgeCommandError>;

#[tauri::command]
pub async fn remote_bridge_run_diagnostics(
) -> CommandResult<bridge::diagnostics::RuntimeDiagnostics> {
    run("diagnostics", None, bridge::diagnostics::run_all).await
}
#[tauri::command]
pub async fn remote_bridge_diagnostics_snapshot(
) -> CommandResult<bridge::diagnostics::RuntimeDiagnostics> {
    run("diagnostics", None, bridge::diagnostics::snapshot).await
}
#[tauri::command]
pub fn remote_bridge_log_status() -> bridge::logging::LogStatus {
    bridge::logging::status()
}
#[tauri::command]
pub async fn remote_bridge_clear_logs() -> CommandResult<()> {
    run("logs", None, bridge::logging::clear).await
}
#[tauri::command]
pub async fn remote_bridge_open_log_directory(app: tauri::AppHandle) -> CommandResult<()> {
    use tauri_plugin_opener::OpenerExt;
    run("logs", None, move || {
        let path = bridge::logging::directory()?;
        app.opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(|_| "logUnavailable".into())
    })
    .await
}

fn command_error(
    code: String,
    phase: &'static str,
    target: Option<&'static str>,
) -> BridgeCommandError {
    let retryable = matches!(
        code.as_str(),
        "sshTimeout"
            | "sshFailed"
            | "forwardDenied"
            | "relayUnavailable"
            | "portInUse"
            | "portAllocationFailed"
            | "ccUnavailable"
            | "proxyUnavailable"
            | "bridgeUnavailable"
            | "networkFailed"
            | "processFailed"
            | "ptyUnavailable"
            | "sshAuthPending"
            | "remoteFailed"
            | "routeOutdated"
            | "stateUnavailable"
            | "skillBusy"
            | "logBusy"
    );
    BridgeCommandError {
        code,
        phase,
        target,
        retryable,
    }
}

async fn run<T: Send + 'static>(
    phase: &'static str,
    target: Option<&'static str>,
    f: impl FnOnce() -> BridgeResult<T> + Send + 'static,
) -> CommandResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| command_error("stateUnavailable".into(), phase, target))?
        .map_err(|code| command_error(code, phase, target))
}
#[tauri::command]
pub async fn remote_bridge_targets() -> CommandResult<Vec<RemoteTarget>> {
    run("targetDiscovery", Some("ssh"), bridge::targets).await
}
#[tauri::command]
pub async fn remote_bridge_pick_identity_file(
    app: tauri::AppHandle,
    initial_path: Option<String>,
) -> CommandResult<Option<String>> {
    run("identitySelection", Some("ssh"), move || {
        // No extension filter: standard id_ed25519/id_rsa files usually have no suffix.
        let mut dialog = app.dialog().file();
        if let Some(path) = initial_path
            .as_deref()
            .and_then(|path| bridge::connections::validate_identity_file(path).ok())
            .map(std::path::PathBuf::from)
        {
            if let Some(parent) = path.parent() {
                dialog = dialog.set_directory(parent);
            }
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                dialog = dialog.set_file_name(name);
            }
        }
        let Some(file) = dialog.blocking_pick_file() else {
            return Ok(None);
        };
        let path = file.into_path().map_err(|_| "identityFileInvalid")?;
        let path = path.to_str().ok_or("identityFileInvalid")?;
        bridge::connections::validate_identity_file(path).map(Some)
    })
    .await
}

#[tauri::command]
pub async fn remote_bridge_add_connection(
    input: bridge::connections::ManualConnectionInput,
) -> CommandResult<RemoteTarget> {
    run("targetWrite", Some("ssh"), move || {
        bridge::add_connection(input)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_update_connection(
    id: String,
    input: bridge::connections::ManualConnectionInput,
) -> CommandResult<RemoteTarget> {
    run("targetWrite", Some("ssh"), move || {
        bridge::update_connection(id, input)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_remove_connection(id: String) -> CommandResult<()> {
    run("targetWrite", Some("ssh"), move || {
        bridge::remove_connection(id)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_summary() -> CommandResult<Summary> {
    run("stateRead", None, bridge::summary).await
}

#[tauri::command]
pub async fn remote_bridge_events(
    limit: Option<usize>,
) -> CommandResult<Vec<bridge::events::BridgeEvent>> {
    run("stateRead", None, move || {
        bridge::recent_events(limit.unwrap_or(50))
    })
    .await
}

#[tauri::command]
pub async fn remote_bridge_skills() -> CommandResult<Vec<bridge::SkillView>> {
    run("skillInspection", Some("skills"), bridge::skill_views).await
}

#[tauri::command]
pub async fn remote_bridge_enable_skill(id: String) -> CommandResult<bridge::SkillView> {
    run("skillProjection", Some("skills"), move || {
        bridge::enable_skill(id)
    })
    .await
}

#[tauri::command]
pub async fn remote_bridge_disable_skill(id: String) -> CommandResult<bridge::SkillView> {
    run("skillProjection", Some("skills"), move || {
        bridge::disable_skill(id)
    })
    .await
}

#[tauri::command]
pub async fn remote_bridge_model_settings(
) -> CommandResult<bridge::settings::RemoteBridgeSettingsView> {
    run("modelSettingsRead", Some("codex"), bridge::model_settings).await
}

#[tauri::command]
pub async fn remote_bridge_save_model_settings(
    settings: bridge::settings::RemoteBridgeSettings,
) -> CommandResult<bridge::settings::RemoteBridgeSettingsView> {
    run("modelSettingsWrite", Some("codex"), move || {
        bridge::save_model_settings(settings)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_check(target_id: String) -> CommandResult<PortAllocation> {
    run("connectionCheck", Some("ssh"), move || {
        bridge::check(target_id)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_check_network(
    target_id: String,
) -> CommandResult<RemoteNetworkObservation> {
    run("networkCheck", Some("serverInternet"), move || {
        bridge::check_remote_network(target_id)
    })
    .await
}

#[tauri::command]
pub async fn ssh_auth_begin(
    operation: bridge::ssh_auth::Operation,
    target_id: String,
    request: Option<Request>,
) -> CommandResult<bridge::ssh_auth::Snapshot> {
    run("sshAuthentication", Some("ssh"), move || {
        bridge::ssh_auth::begin(operation, target_id, request)
    })
    .await
}

#[tauri::command]
pub async fn ssh_auth_state(session_id: String) -> CommandResult<bridge::ssh_auth::Snapshot> {
    run("sshAuthentication", Some("ssh"), move || {
        bridge::ssh_auth::state(&session_id)
    })
    .await
}

#[tauri::command]
pub async fn ssh_auth_submit(
    session_id: String,
    prompt_id: String,
    response: String,
) -> CommandResult<bridge::ssh_auth::Snapshot> {
    run("sshAuthentication", Some("ssh"), move || {
        bridge::ssh_auth::submit(&session_id, &prompt_id, response)
    })
    .await
}

#[tauri::command]
pub async fn ssh_auth_confirm_host(
    session_id: String,
    prompt_id: String,
) -> CommandResult<bridge::ssh_auth::Snapshot> {
    run("sshAuthentication", Some("sshHostKey"), move || {
        bridge::ssh_auth::confirm_host(&session_id, &prompt_id)
    })
    .await
}

#[tauri::command]
pub async fn ssh_auth_finish(session_id: String) -> CommandResult<bridge::ssh_auth::Outcome> {
    run("sshAuthentication", Some("ssh"), move || {
        bridge::ssh_auth::finish(&session_id)
    })
    .await
}

#[tauri::command]
pub async fn ssh_auth_cancel(session_id: String) -> CommandResult<()> {
    run("sshAuthentication", Some("ssh"), move || {
        bridge::ssh_auth::cancel(&session_id)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_allocate_ports(
    target_id: String,
    prefer_defaults: Option<bool>,
) -> CommandResult<PortAllocation> {
    run("portAllocation", Some("remoteLoopback"), move || {
        bridge::allocate_ports(target_id, prefer_defaults.unwrap_or(true))
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_detect_cc(
    local_port: u16,
    discover: Option<bool>,
) -> CommandResult<CcDetection> {
    run("localDetection", Some("ccSwitch"), move || {
        if discover.unwrap_or(false) {
            bridge::discover_cc(local_port)
        } else {
            bridge::detect_cc(local_port)
        }
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_preview(request: Request) -> CommandResult<Summary> {
    run("bridgePreview", None, move || bridge::preview(&request)).await
}
#[tauri::command]
pub async fn remote_bridge_connect(request: Request, confirmed: bool) -> CommandResult<Summary> {
    run("bridgeConnect", None, move || {
        bridge::connect(request, confirmed)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_disconnect(confirmed: bool) -> CommandResult<Summary> {
    run("bridgeDisconnect", None, move || {
        bridge::disconnect(confirmed)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_set_capability(
    change: CapabilityChange,
    confirmed: bool,
) -> CommandResult<Summary> {
    run("capabilityChange", None, move || {
        bridge::set_capability(change, confirmed)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_retry_reconnect() -> CommandResult<Summary> {
    run("bridgeReconnect", Some("ssh"), bridge::retry_reconnect).await
}
#[tauri::command]
pub async fn remote_bridge_test() -> CommandResult<()> {
    run("connectivityTest", Some("proxy"), bridge::test).await
}
#[tauri::command]
pub async fn remote_bridge_launch_proxy_terminal() -> CommandResult<()> {
    run(
        "managedTerminalLaunch",
        Some("proxy"),
        bridge::launch_proxy_terminal,
    )
    .await
}
#[tauri::command]
pub async fn remote_bridge_launch_manual_terminal() -> CommandResult<()> {
    run(
        "managedTerminalLaunch",
        Some("proxy"),
        bridge::launch_manual_terminal,
    )
    .await
}
#[tauri::command]
pub async fn remote_bridge_clear_session_credential() -> CommandResult<()> {
    run("credentialClear", Some("ssh"), || {
        bridge::clear_session_credential();
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_session_environment_command() -> CommandResult<String> {
    run(
        "sessionEnvironment",
        Some("proxy"),
        bridge::session_environment_command,
    )
    .await
}
#[tauri::command]
pub async fn remote_bridge_config_preview(tool: String) -> CommandResult<ConfigPreview> {
    run("configurationPreview", Some("cli"), move || {
        bridge::config_preview(tool)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_config_apply(id: String, confirmed: bool) -> CommandResult<()> {
    run("configurationApply", Some("cli"), move || {
        bridge::config_apply(id, confirmed)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_config_restore(id: String, confirmed: bool) -> CommandResult<()> {
    run("configurationRestore", Some("cli"), move || {
        bridge::config_restore(id, confirmed)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_config_restore_preview(
    target_id: String,
    tool: String,
) -> CommandResult<ConfigPreview> {
    run("configurationRestorePreview", Some("cli"), move || {
        bridge::config_restore_preview(target_id, tool)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_tool_verify(tool: String) -> CommandResult<ToolVerificationResult> {
    run("toolVerification", Some("cli"), move || {
        bridge::verify_tool(tool)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_open_vscode(target_id: String) -> CommandResult<Option<String>> {
    run("openTarget", Some("vscode"), move || {
        bridge::open_vscode(target_id)
    })
    .await
}

#[tauri::command]
pub async fn remote_bridge_open_vscode_settings() -> CommandResult<()> {
    run(
        "openTarget",
        Some("vscodeSettings"),
        bridge::vscode::open_settings,
    )
    .await
}

#[tauri::command]
pub async fn remote_bridge_reveal_target_config(target_id: String) -> CommandResult<()> {
    run("openTarget", Some("targetConfig"), move || {
        bridge::vscode::reveal_target_config(target_id)
    })
    .await
}

#[tauri::command]
pub async fn remote_bridge_open_target_config(target_id: String) -> CommandResult<()> {
    run("openTarget", Some("targetConfig"), move || {
        bridge::vscode::open_target_config(target_id)
    })
    .await
}

#[tauri::command]
pub async fn remote_bridge_launch_mobaxterm(target_id: String) -> CommandResult<()> {
    run("openTarget", Some("mobaxterm"), move || {
        bridge::launch_mobaxterm(target_id)
    })
    .await
}

#[tauri::command]
pub async fn remote_bridge_extension_inspect(
    target_id: String,
) -> CommandResult<bridge::extension::Inspection> {
    run("extensionInspection", Some("vscodeExtension"), move || {
        bridge::extension::inspect(target_id)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_extension_preview(
    selection: bridge::extension::Selection,
) -> CommandResult<bridge::extension::Preview> {
    run("extensionPreview", Some("vscodeExtension"), move || {
        bridge::extension::preview(selection)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_extension_apply(id: String, confirmed: bool) -> CommandResult<()> {
    run("extensionApply", Some("vscodeExtension"), move || {
        bridge::extension::apply(id, confirmed)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cc_unavailable_keeps_actionable_error_context() {
        let error = command_error("ccUnavailable".into(), "localDetection", Some("ccSwitch"));
        let value = serde_json::to_value(error).expect("serialize bridge command error");

        assert_eq!(value["code"], "ccUnavailable");
        assert_eq!(value["phase"], "localDetection");
        assert_eq!(value["target"], "ccSwitch");
        assert_eq!(value["retryable"], true);
    }

    #[test]
    fn unsupported_target_is_not_marked_retryable() {
        let error = command_error("targetUnsupported".into(), "connectionCheck", Some("ssh"));
        let value = serde_json::to_value(error).expect("serialize bridge command error");

        assert_eq!(value["code"], "targetUnsupported");
        assert_eq!(value["retryable"], false);
    }
}
