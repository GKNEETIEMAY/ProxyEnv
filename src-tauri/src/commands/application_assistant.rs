use crate::{
    error::{ProxyEnvError, Result},
    features::application_assistant::{self, RunningApplication},
};

#[tauri::command]
pub async fn list_running_applications() -> Result<Vec<RunningApplication>> {
    tauri::async_runtime::spawn_blocking(application_assistant::list_running_applications)
        .await
        .map_err(|error| ProxyEnvError::Detection(error.to_string()))
}
