use tauri::{AppHandle, Emitter};

use crate::{
    desktop::tray,
    error::{ProxyEnvError, Result},
    services::settings::{self, AppSettings},
};

#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
pub async fn get_app_settings(app: AppHandle) -> Result<AppSettings> {
    let mut settings = settings::load()?;
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        settings.launch_at_startup = app
            .autolaunch()
            .is_enabled()
            .map_err(|error| ProxyEnvError::Autostart(error.to_string()))?;
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
            .map_err(|error| ProxyEnvError::Autostart(error.to_string()))?;
        if settings.launch_at_startup != previous {
            let update = if settings.launch_at_startup {
                autostart.enable()
            } else {
                autostart.disable()
            };
            update.map_err(|error| ProxyEnvError::Autostart(error.to_string()))?;
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
