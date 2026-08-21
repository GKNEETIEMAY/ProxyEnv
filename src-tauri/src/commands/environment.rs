use tauri::AppHandle;

use crate::{
    desktop::tray,
    environment::{EnvironmentManager, EnvironmentStatus},
    error::Result,
    proxy::ProxyProtocol,
    services::settings,
};

#[tauri::command]
pub async fn get_environment_status() -> Result<EnvironmentStatus> {
    EnvironmentManager::status()
}

#[tauri::command]
pub async fn enable_proxy_environment(
    app: AppHandle,
    host: Option<String>,
    port: Option<u16>,
    protocol: Option<ProxyProtocol>,
) -> Result<EnvironmentStatus> {
    let settings = settings::load()?;
    let status = match (host, port, protocol) {
        (Some(host), Some(port), Some(protocol)) => {
            EnvironmentManager::enable_for_proxy(&host, port, protocol, &settings.proxy_variables)
        }
        _ => EnvironmentManager::enable(&settings.proxy_variables),
    }?;
    tray::update_proxy_state(&app, status.enabled);
    Ok(status)
}

#[tauri::command]
pub async fn sync_proxy_environment(
    app: AppHandle,
    host: String,
    port: u16,
    protocol: ProxyProtocol,
) -> Result<EnvironmentStatus> {
    let status = EnvironmentManager::status()?;
    if !status.enabled {
        return Ok(status);
    }
    let settings = settings::load()?;
    let status =
        EnvironmentManager::enable_for_proxy(&host, port, protocol, &settings.proxy_variables)?;
    tray::update_proxy_state(&app, status.enabled);
    Ok(status)
}

#[tauri::command]
pub async fn disable_proxy_environment(app: AppHandle) -> Result<EnvironmentStatus> {
    let status = EnvironmentManager::disable()?;
    tray::update_proxy_state(&app, status.enabled);
    Ok(status)
}
