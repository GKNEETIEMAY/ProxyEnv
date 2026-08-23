mod diagnosis;
pub(crate) mod launcher;
pub(crate) mod models;
mod processes;

pub use models::{
    ApplicationDiagnosis, ApplicationNetworkState, DiagnosisSummary, LaunchApplicationResult,
    LaunchEnvironmentMode, ManagedApplication, RecommendedAction, RunningApplication,
};

pub fn list_running_applications() -> Vec<RunningApplication> {
    processes::enumerate()
}

pub fn diagnose_application(
    application: ManagedApplication,
) -> crate::error::Result<ApplicationDiagnosis> {
    diagnosis::diagnose(application)
}
