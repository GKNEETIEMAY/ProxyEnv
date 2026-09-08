use crate::features::remote_bridge::{
    self as bridge, BridgeResult, CcDetection, ConfigPreview, PortAllocation,
    RemoteNetworkObservation, RemoteTarget, Request, Summary,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeCommandError {
    code: String,
    phase: &'static str,
    target: Option<&'static str>,
    retryable: bool,
}

type CommandResult<T> = Result<T, BridgeCommandError>;

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
            | "portInUse"
            | "portAllocationFailed"
            | "ccUnavailable"
            | "proxyUnavailable"
            | "bridgeUnavailable"
            | "networkFailed"
            | "processFailed"
            | "remoteFailed"
            | "stateUnavailable"
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
pub async fn remote_bridge_summary() -> CommandResult<Summary> {
    run("stateRead", None, bridge::summary).await
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
pub async fn remote_bridge_allocate_ports(target_id: String) -> CommandResult<PortAllocation> {
    run("portAllocation", Some("remoteLoopback"), move || {
        bridge::allocate_ports(target_id)
    })
    .await
}
#[tauri::command]
pub async fn remote_bridge_detect_cc(local_port: u16) -> CommandResult<CcDetection> {
    run("localDetection", Some("ccSwitch"), move || {
        bridge::detect_cc(local_port)
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
pub async fn remote_bridge_test() -> CommandResult<()> {
    run("connectivityTest", Some("proxy"), bridge::test).await
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
pub async fn remote_bridge_open_vscode(target_id: String) -> CommandResult<()> {
    run("openTarget", Some("vscode"), move || {
        bridge::vscode::open(target_id)
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
