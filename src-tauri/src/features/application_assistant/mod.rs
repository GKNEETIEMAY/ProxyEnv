mod authorization;
mod diagnosis;
pub(crate) mod launcher;
pub(crate) mod models;
mod processes;
mod rules;

pub use models::{
    ApplicationDiagnosis, ApplicationNetworkState, LaunchApplicationResult, ManagedApplication,
    RecommendedAction, RunningApplication,
};

pub(crate) use authorization::{
    authorize_path, authorize_running_applications, resolve_application,
};

pub fn list_running_applications() -> Vec<RunningApplication> {
    processes::enumerate()
}

pub(crate) fn diagnose_report_snapshot(
    application: ManagedApplication,
    context: crate::features::proxy::active::ActiveProxyContext,
    environment: crate::features::proxy::ProxyEnvironmentState,
) -> ApplicationDiagnosis {
    diagnosis::diagnose_snapshot(application, context, environment)
}
