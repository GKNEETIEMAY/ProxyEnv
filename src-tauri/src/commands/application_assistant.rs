use crate::{
    error::{ProxyEnvError, Result},
    features::{
        application_assistant::{
            self, launcher, ApplicationDiagnosis, LaunchApplicationResult, LaunchEnvironmentMode,
            ManagedApplication, RuleApplyResult, RuleChangePlan, RuleChangePreview,
            RuleRestoreResult, RunningApplication,
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
pub async fn preview_application_rule_fix(
    application: ManagedApplication,
) -> Result<RuleChangePreview> {
    tauri::async_runtime::spawn_blocking(move || {
        application_assistant::preview_application_rule_fix(&application)
    })
    .await
    .map_err(|error| ProxyEnvError::Detection(error.to_string()))?
}

#[tauri::command]
pub async fn apply_application_rule_fix(
    application: ManagedApplication,
    expected_plan: RuleChangePlan,
    confirmed: bool,
) -> Result<RuleApplyResult> {
    tauri::async_runtime::spawn_blocking(move || {
        application_assistant::apply_application_rule_fix(&application, &expected_plan, confirmed)
    })
    .await
    .map_err(|error| ProxyEnvError::Detection(error.to_string()))?
}

#[tauri::command]
pub async fn restore_application_rule_change(
    backup_id: String,
    confirmed: bool,
) -> Result<RuleRestoreResult> {
    tauri::async_runtime::spawn_blocking(move || {
        application_assistant::restore_application_rule_change(&backup_id, confirmed)
    })
    .await
    .map_err(|error| ProxyEnvError::Detection(error.to_string()))
}

#[tauri::command]
pub async fn diagnose_application(application: ManagedApplication) -> Result<ApplicationDiagnosis> {
    tauri::async_runtime::spawn_blocking(move || {
        application_assistant::diagnose_application(application)
    })
    .await
    .map_err(|error| ProxyEnvError::Detection(error.to_string()))?
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
