use crate::{
    error::{ProxyEnvError, Result},
    features::{
        application_assistant::{
            self, launcher, LaunchApplicationResult, LaunchEnvironmentMode, ManagedApplication,
            RunningApplication,
        },
        proxy::{self, ProxyEndpoint},
    },
};

#[tauri::command]
pub async fn list_running_applications() -> Result<Vec<RunningApplication>> {
    tauri::async_runtime::spawn_blocking(application_assistant::list_running_applications)
        .await
        .map_err(|error| ProxyEnvError::Detection(error.to_string()))
}

#[tauri::command]
pub async fn launch_application_with_current_proxy(
    application: ManagedApplication,
) -> Result<LaunchApplicationResult> {
    let candidates = proxy::detect()?;
    let candidate = candidates
        .iter()
        .find(|candidate| candidate.listening)
        .ok_or(ProxyEnvError::ActiveProxyMissing)?;
    let endpoint = ProxyEndpoint {
        host: candidate.host.clone(),
        port: candidate.port,
        protocol: candidate.protocol,
    };
    tauri::async_runtime::spawn_blocking(move || {
        launcher::launch(
            &application,
            LaunchEnvironmentMode::UseCurrentProxy,
            Some(&endpoint),
        )
    })
    .await
    .map_err(|error| ProxyEnvError::ApplicationLaunch(error.to_string()))?
}

#[tauri::command]
pub async fn launch_application_without_proxy(
    application: ManagedApplication,
) -> Result<LaunchApplicationResult> {
    tauri::async_runtime::spawn_blocking(move || {
        launcher::launch(
            &application,
            LaunchEnvironmentMode::ClearProxyVariables,
            None,
        )
    })
    .await
    .map_err(|error| ProxyEnvError::ApplicationLaunch(error.to_string()))?
}
