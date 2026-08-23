mod diagnosis;
pub(crate) mod launcher;
pub(crate) mod models;
mod processes;
mod rules;

pub use models::{
    ApplicationDiagnosis, ApplicationNetworkState, DiagnosisSummary, LaunchApplicationResult,
    LaunchEnvironmentMode, ManagedApplication, RecommendedAction, RunningApplication,
};
pub use rules::RuleChangePreview;

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
