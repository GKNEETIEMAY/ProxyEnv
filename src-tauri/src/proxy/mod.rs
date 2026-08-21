mod detector;
mod environment;
mod listeners;
mod models;
mod probe;
mod processes;
mod system_proxy;

pub use environment::ProxyEnvironmentService;
pub use models::{ProxyCandidate, ProxyEnvironmentStatus, ProxyProtocol};

use crate::error::Result;

pub fn detect() -> Result<Vec<ProxyCandidate>> {
    detector::detect()
}
