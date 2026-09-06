use crate::features::remote_bridge::{
    self as bridge, BridgeResult, ConfigPreview, Request, Summary,
};
async fn run<T: Send + 'static>(
    f: impl FnOnce() -> BridgeResult<T> + Send + 'static,
) -> BridgeResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| "stateUnavailable".to_string())?
}
#[tauri::command]
pub async fn remote_bridge_targets() -> BridgeResult<Vec<String>> {
    run(bridge::targets).await
}
#[tauri::command]
pub async fn remote_bridge_summary() -> BridgeResult<Summary> {
    run(bridge::summary).await
}
#[tauri::command]
pub async fn remote_bridge_check(alias: String) -> BridgeResult<()> {
    run(move || bridge::check(alias)).await
}
#[tauri::command]
pub async fn remote_bridge_detect_cc(local_port: u16) -> BridgeResult<bool> {
    run(move || bridge::detect_cc(local_port)).await
}
#[tauri::command]
pub async fn remote_bridge_preview(request: Request) -> BridgeResult<Summary> {
    run(move || bridge::preview(&request)).await
}
#[tauri::command]
pub async fn remote_bridge_connect(request: Request, confirmed: bool) -> BridgeResult<Summary> {
    run(move || bridge::connect(request, confirmed)).await
}
#[tauri::command]
pub async fn remote_bridge_disconnect(confirmed: bool) -> BridgeResult<Summary> {
    run(move || bridge::disconnect(confirmed)).await
}
#[tauri::command]
pub async fn remote_bridge_test() -> BridgeResult<()> {
    run(bridge::test).await
}
#[tauri::command]
pub async fn remote_bridge_config_preview(tool: String) -> BridgeResult<ConfigPreview> {
    run(move || bridge::config_preview(tool)).await
}
#[tauri::command]
pub async fn remote_bridge_config_apply(id: String, confirmed: bool) -> BridgeResult<()> {
    run(move || bridge::config_apply(id, confirmed)).await
}
#[tauri::command]
pub async fn remote_bridge_config_restore(id: String, confirmed: bool) -> BridgeResult<()> {
    run(move || bridge::config_restore(id, confirmed)).await
}
#[tauri::command]
pub async fn remote_bridge_config_restore_preview(
    alias: String,
    tool: String,
) -> BridgeResult<ConfigPreview> {
    run(move || bridge::config_restore_preview(alias, tool)).await
}
#[tauri::command]
pub async fn remote_bridge_open_vscode(alias: String) -> BridgeResult<()> {
    run(move || bridge::vscode::open(alias)).await
}
