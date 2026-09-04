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
    expected_revision: u64,
) -> crate::error::Result<RuleChangePreview> {
    crate::features::proxy::active::with_current(Some(expected_revision), |candidate| {
        let endpoint = crate::features::proxy::active::endpoint(candidate);
        Ok(rules::preview_application(
            &application.executable_path,
            Some(&endpoint),
        ))
    })
}

pub fn apply_application_rule_fix(
    application: &ManagedApplication,
    expected_plan: &RuleChangePlan,
    confirmed: bool,
    expected_revision: u64,
) -> crate::error::Result<RuleApplyResult> {
    crate::features::proxy::active::with_current(Some(expected_revision), |candidate| {
        let endpoint = crate::features::proxy::active::endpoint(candidate);
        Ok(rules::apply_application(
            &application.executable_path,
            Some(&endpoint),
            expected_plan,
            confirmed,
        ))
    })
}

pub fn restore_application_rule_change(backup_id: &str, confirmed: bool) -> RuleRestoreResult {
    rules::restore_rule_change(backup_id, confirmed)
}
