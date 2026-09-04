mod authorization;
mod diagnosis;
pub(crate) mod launcher;
pub(crate) mod models;
mod processes;
pub(crate) mod relauncher;
mod rules;

pub use models::{
    ApplicationDiagnosis, ApplicationNetworkState, LaunchApplicationResult, LaunchEnvironmentMode,
    ManagedApplication, RecommendedAction, RunningApplication,
};
pub use rules::{RuleApplyResult, RuleChangePlan, RuleChangePreview, RuleRestoreResult};

pub(crate) use authorization::{
    authorize_path, authorize_running_applications, resolve_application,
};

pub fn list_running_applications() -> Vec<RunningApplication> {
    processes::enumerate()
}

pub fn diagnose_application(
    application: ManagedApplication,
) -> crate::error::Result<ApplicationDiagnosis> {
    diagnosis::diagnose(application)
}

pub fn preview_application_rule_fix(
    application: &ManagedApplication,
) -> crate::error::Result<RuleChangePreview> {
    let candidates = crate::features::proxy::detect()?;
    let endpoint = candidates
        .iter()
        .find(|candidate| candidate.listening)
        .map(|candidate| crate::features::proxy::ProxyEndpoint {
            host: candidate.host.clone(),
            port: candidate.port,
            protocol: candidate.protocol,
        });
    Ok(rules::preview_application(
        &application.executable_path,
        endpoint.as_ref(),
    ))
}

pub fn apply_application_rule_fix(
    application: &ManagedApplication,
    expected_plan: &RuleChangePlan,
    confirmed: bool,
) -> crate::error::Result<RuleApplyResult> {
    let candidates = crate::features::proxy::detect()?;
    let endpoint = candidates
        .iter()
        .find(|candidate| candidate.listening)
        .map(|candidate| crate::features::proxy::ProxyEndpoint {
            host: candidate.host.clone(),
            port: candidate.port,
            protocol: candidate.protocol,
        });
    Ok(rules::apply_application(
        &application.executable_path,
        endpoint.as_ref(),
        expected_plan,
        confirmed,
    ))
}

pub fn restore_application_rule_change(backup_id: &str, confirmed: bool) -> RuleRestoreResult {
    rules::restore_rule_change(backup_id, confirmed)
}
