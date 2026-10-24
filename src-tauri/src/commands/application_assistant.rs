use crate::{
    error::{ProxyEnvError, Result},
    features::{
        application_assistant::{
            self, launcher, LaunchApplicationResult, ManagedApplication, RunningApplication,
        },
        proxy::active,
    },
};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn list_running_applications() -> Result<Vec<RunningApplication>> {
    tauri::async_runtime::spawn_blocking(|| {
        application_assistant::authorize_running_applications(
            application_assistant::list_running_applications(),
        )
    })
    .await
    .map_err(|error| ProxyEnvError::Detection(error.to_string()))
}

#[tauri::command]
pub async fn pick_application(app: tauri::AppHandle) -> Result<Option<ManagedApplication>> {
    tauri::async_runtime::spawn_blocking(move || {
        let dialog = app.dialog().file();
        #[cfg(windows)]
        let dialog = dialog.add_filter("Windows applications (*.exe)", &["exe"]);
        let Some(file) = dialog.blocking_pick_file() else {
            return Ok(None);
        };
        let path = file.into_path().map_err(|error| {
            ProxyEnvError::InvalidApplication(format!("the selected file is unavailable: {error}"))
        })?;
        application_assistant::authorize_path(&path).map(Some)
    })
    .await
    .map_err(|error| ProxyEnvError::Detection(error.to_string()))?
}

#[tauri::command]
pub async fn renew_application_authorization(application_id: String) -> Result<ManagedApplication> {
    application_assistant::resolve_application(&application_id)
}

#[tauri::command]
pub async fn launch_application_with_current_proxy(
    application_id: String,
    expected_revision: u64,
) -> Result<LaunchApplicationResult> {
    let application = application_assistant::resolve_application(&application_id)?;
    let launched = tauri::async_runtime::spawn_blocking(move || {
        active::with_current(Some(expected_revision), |candidate| {
            launcher::launch(&application, &active::endpoint(candidate))
        })
    })
    .await
    .map_err(|error| ProxyEnvError::ApplicationLaunch(error.to_string()))??;
    Ok(launched)
}
