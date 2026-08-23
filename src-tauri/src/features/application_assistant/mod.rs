pub(crate) mod launcher;
pub(crate) mod models;
mod processes;

pub use models::{
    LaunchApplicationResult, LaunchEnvironmentMode, ManagedApplication, RunningApplication,
};

pub fn list_running_applications() -> Vec<RunningApplication> {
    processes::enumerate()
}
