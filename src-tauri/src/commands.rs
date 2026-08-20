use crate::{
    environment::{EnvironmentManager, EnvironmentStatus},
    error::Result,
    proxy::{self, ProxyCandidate, ProxyProtocol},
    settings::{self, AppSettings},
    tray,
};
use tauri::{AppHandle, Emitter};

#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
use tauri_plugin_autostart::ManagerExt;

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

#[tauri::command]
pub async fn detect_proxies() -> Result<Vec<ProxyCandidate>> {
    proxy::detect()
}

#[tauri::command]
pub async fn get_app_settings(app: AppHandle) -> Result<AppSettings> {
    let mut settings = settings::load()?;
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        settings.launch_at_startup = app
            .autolaunch()
            .is_enabled()
            .map_err(|error| crate::error::ProxyEnvError::Autostart(error.to_string()))?;
    }
    Ok(settings)
}

#[tauri::command]
pub async fn save_app_settings(app: AppHandle, mut settings: AppSettings) -> Result<AppSettings> {
    if settings.proxy_variables.is_empty() {
        settings.proxy_variables = AppSettings::default().proxy_variables;
    }
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        let autostart = app.autolaunch();
        let previous = autostart
            .is_enabled()
            .map_err(|error| crate::error::ProxyEnvError::Autostart(error.to_string()))?;
        if settings.launch_at_startup != previous {
            let update = if settings.launch_at_startup {
                autostart.enable()
            } else {
                autostart.disable()
            };
            update.map_err(|error| crate::error::ProxyEnvError::Autostart(error.to_string()))?;
        }
        if let Err(error) = settings::save(&settings) {
            let _ = if previous {
                autostart.enable()
            } else {
                autostart.disable()
            };
            return Err(error);
        }
    }
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    settings::save(&settings)?;

    tray::update_language(&app, settings.clone().resolved_language());
    let _ = app.emit("settings-changed", settings.clone());
    Ok(settings)
}
