pub(crate) mod models;
mod processes;

pub use models::RunningApplication;

pub fn list_running_applications() -> Vec<RunningApplication> {
    processes::enumerate()
}
