mod detector;
mod listeners;
mod models;
mod probe;
mod processes;
mod system_proxy;

pub use models::ProxyCandidate;

use crate::error::Result;

pub fn detect() -> Result<Vec<ProxyCandidate>> {
    detector::detect()
}
