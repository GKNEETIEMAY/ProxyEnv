use tauri::AppHandle;

use crate::{
    desktop::tray,
    error::Result,
    features::proxy::{
        self, ProxyEndpoint, ProxyEnvironmentService, ProxyEnvironmentStatus, ProxyProtocol,
    },
    services::settings,
};

#[tauri::command]
pub async fn get_environment_status() -> Result<ProxyEnvironmentStatus> {
    let settings = settings::load()?;
    let active = proxy::detect()?
        .into_iter()
        .find(|candidate| candidate.listening);
    ProxyEnvironmentService::status(&settings.proxy_variables, active)
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
        (Some(host), Some(port), Some(protocol)) => {
            ProxyEnvironmentService::sync(&host, port, protocol, &settings.proxy_variables)
        }
        _ => Err(crate::error::ProxyEnvError::ActiveProxyMissing),
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
    let active = proxy::detect()?
        .into_iter()
        .find(|candidate| candidate.listening);
    let status = ProxyEnvironmentService::status(&settings.proxy_variables, active)?;
    if !status.state.is_configured() {
        return Ok(status);
    }
    let status = ProxyEnvironmentService::sync(&host, port, protocol, &settings.proxy_variables)?;
    tray::update_proxy_state(&app, status.state.is_configured());
    Ok(status)
}

#[tauri::command]
pub async fn sync_manual_proxy_environment(
    app: AppHandle,
    endpoint: ProxyEndpoint,
) -> Result<ProxyEnvironmentStatus> {
    let settings = settings::load()?;
    let status = ProxyEnvironmentService::sync_manual(&endpoint, &settings.proxy_variables)?;
    tray::update_proxy_state(&app, status.state.is_configured());
    Ok(status)
}

#[tauri::command]
pub async fn restore_proxy_environment(app: AppHandle) -> Result<ProxyEnvironmentStatus> {
    let settings = settings::load()?;
    let status = ProxyEnvironmentService::restore(&settings.proxy_variables)?;
    tray::update_proxy_state(&app, status.state.is_configured());
    Ok(status)
}

#[tauri::command]
pub async fn disable_proxy_environment(app: AppHandle) -> Result<ProxyEnvironmentStatus> {
    let status = ProxyEnvironmentService::disable()?;
    tray::update_proxy_state(&app, status.state.is_configured());
    Ok(status)
}
