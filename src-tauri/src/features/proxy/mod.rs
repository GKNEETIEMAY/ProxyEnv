mod detector;
mod listeners;
mod models;
mod probe;
mod processes;
mod service;
mod system_proxy;

pub use models::{
    ProxyCandidate, ProxyEnvironmentState, ProxyEnvironmentStatus, ProxyProtocol, ProxyVariable,
};
pub use service::ProxyEnvironmentService;

use crate::error::Result;

pub fn detect() -> Result<Vec<ProxyCandidate>> {
    detector::detect()
}
