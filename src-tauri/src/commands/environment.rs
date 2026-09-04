use tauri::AppHandle;

use crate::{
    desktop::tray,
    error::Result,
    features::proxy::{active, ProxyEndpoint, ProxyEnvironmentService, ProxyEnvironmentStatus},
    services::settings,
};

#[tauri::command]
pub async fn get_environment_status() -> Result<ProxyEnvironmentStatus> {
    let settings = settings::load()?;
    active::status(&settings.proxy_variables)
}

#[tauri::command]
pub async fn enable_proxy_environment(
    app: AppHandle,
    expected_revision: u64,
) -> Result<ProxyEnvironmentStatus> {
    sync_proxy_environment(app, expected_revision).await
}

#[tauri::command]
pub async fn sync_proxy_environment(
    app: AppHandle,
    expected_revision: u64,
) -> Result<ProxyEnvironmentStatus> {
    let settings = settings::load()?;
    active::with_current(Some(expected_revision), |candidate| {
        ProxyEnvironmentService::sync(
            &candidate.host,
            candidate.port,
            candidate.protocol,
            &settings.proxy_variables,
        )
    })?;
    let status = active::status(&settings.proxy_variables)?;
    tray::update_proxy_state(&app, status.state.is_configured());
    Ok(status)
}

#[tauri::command]
pub async fn sync_manual_proxy_environment(
    app: AppHandle,
    endpoint: ProxyEndpoint,
) -> Result<ProxyEnvironmentStatus> {
    let settings = settings::load()?;
    let status = active::sync_manual(&endpoint, &settings.proxy_variables)?;
    tray::update_proxy_state(&app, status.state.is_configured());
    Ok(status)
}

#[tauri::command]
pub async fn restore_proxy_environment(app: AppHandle) -> Result<ProxyEnvironmentStatus> {
    let settings = settings::load()?;
    ProxyEnvironmentService::restore(&settings.proxy_variables)?;
    let status = active::status(&settings.proxy_variables)?;
    tray::update_proxy_state(&app, status.state.is_configured());
    Ok(status)
}

#[tauri::command]
pub async fn disable_proxy_environment(app: AppHandle) -> Result<ProxyEnvironmentStatus> {
    ProxyEnvironmentService::disable()?;
    let settings = settings::load()?;
    let status = active::status(&settings.proxy_variables)?;
    tray::update_proxy_state(&app, status.state.is_configured());
    Ok(status)
}
