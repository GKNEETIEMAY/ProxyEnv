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
        let Some(file) = app.dialog().file().blocking_pick_file() else {
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
pub async fn preview_application_rule_fix(application_id: String) -> Result<RuleChangePreview> {
    let application = application_assistant::resolve_application(&application_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        application_assistant::preview_application_rule_fix(&application)
    })
    .await
    .map_err(|error| ProxyEnvError::Detection(error.to_string()))?
}

#[tauri::command]
pub async fn apply_application_rule_fix(
    application_id: String,
    expected_plan: RuleChangePlan,
    confirmed: bool,
) -> Result<RuleApplyResult> {
    let application = application_assistant::resolve_application(&application_id)?;
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
pub async fn diagnose_application(application_id: String) -> Result<ApplicationDiagnosis> {
    let application = application_assistant::resolve_application(&application_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        application_assistant::diagnose_application(application)
    })
    .await
    .map_err(|error| ProxyEnvError::Detection(error.to_string()))?
}

#[tauri::command]
pub async fn launch_application_with_current_proxy(
    application_id: String,
) -> Result<LaunchApplicationResult> {
    let application = application_assistant::resolve_application(&application_id)?;
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
    let launched = tauri::async_runtime::spawn_blocking(move || {
        launcher::launch(
            &application,
            LaunchEnvironmentMode::UseCurrentProxy,
            Some(&endpoint),
        )
    })
    .await
    .map_err(|error| ProxyEnvError::ApplicationLaunch(error.to_string()))??;
    Ok(launched)
}

#[tauri::command]
pub async fn launch_application_without_proxy(
    application_id: String,
) -> Result<LaunchApplicationResult> {
    let application = application_assistant::resolve_application(&application_id)?;
    let launched = tauri::async_runtime::spawn_blocking(move || {
        launcher::launch(
            &application,
            LaunchEnvironmentMode::ClearProxyVariables,
            None,
        )
    })
    .await
    .map_err(|error| ProxyEnvError::ApplicationLaunch(error.to_string()))??;
    Ok(launched)
}
