use tauri::AppHandle;

use crate::{
    desktop::tray,
    error::Result,
    features::proxy::{ProxyEnvironmentService, ProxyEnvironmentStatus, ProxyProtocol},
    services::settings,
};

#[tauri::command]
pub async fn get_environment_status() -> Result<ProxyEnvironmentStatus> {
    let settings = settings::load()?;
    ProxyEnvironmentService::status(&settings.proxy_variables)
}

#[tauri::command]
pub async fn enable_proxy_environment(
    app: AppHandle,
    host: Option<String>,
    port: Option<u16>,
    protocol: Option<ProxyProtocol>,
) -> Result<ProxyEnvironmentStatus> {
    let settings = settings::load()?;
    let status = match (host, port, protocol) {
        (Some(host), Some(port), Some(protocol)) => ProxyEnvironmentService::enable_for_proxy(
            &host,
            port,
            protocol,
            &settings.proxy_variables,
        ),
        _ => ProxyEnvironmentService::enable(&settings.proxy_variables),
    }?;
    tray::update_proxy_state(&app, status.state.is_configured());
    Ok(status)
}

#[tauri::command]
pub async fn sync_proxy_environment(
    app: AppHandle,
    host: String,
    port: u16,
    protocol: ProxyProtocol,
) -> Result<ProxyEnvironmentStatus> {
    let settings = settings::load()?;
    let status = ProxyEnvironmentService::status(&settings.proxy_variables)?;
    if !status.state.is_configured() {
        return Ok(status);
    }
    let status = ProxyEnvironmentService::enable_for_proxy(
        &host,
        port,
        protocol,
        &settings.proxy_variables,
    )?;
    tray::update_proxy_state(&app, status.state.is_configured());
    Ok(status)
}

#[tauri::command]
pub async fn disable_proxy_environment(app: AppHandle) -> Result<ProxyEnvironmentStatus> {
    let status = ProxyEnvironmentService::disable()?;
    tray::update_proxy_state(&app, status.state.is_configured());
    Ok(status)
}
