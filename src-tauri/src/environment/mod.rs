mod broadcast;
mod models;
mod registry;
mod snapshot;

pub use models::{EnvironmentEntry, EnvironmentStatus, ProxyEnvironmentSnapshot};

use crate::error::{ProxyEnvError, Result};

pub const MANAGED_PROXY_VARS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
];

pub const DISPLAY_PROXY_VARS: &[&str] = &["NO_PROXY", "no_proxy"];

pub struct EnvironmentManager;

impl EnvironmentManager {
    pub fn status() -> Result<EnvironmentStatus> {
        let mut names = MANAGED_PROXY_VARS.to_vec();
        names.extend_from_slice(DISPLAY_PROXY_VARS);
        let entries = registry::read_entries(&names)?;
        let enabled = entries
            .iter()
            .any(|entry| MANAGED_PROXY_VARS.contains(&entry.name.as_str()) && entry.exists);
        Ok(EnvironmentStatus {
            enabled,
            entries,
            warning: None,
        })
    }

    pub fn disable() -> Result<EnvironmentStatus> {
        let entries = registry::read_entries(MANAGED_PROXY_VARS)?;
        let snapshot = ProxyEnvironmentSnapshot::new(entries);
        snapshot::save_atomic(&snapshot)?;
        registry::delete_entries(MANAGED_PROXY_VARS)?;
        broadcast::environment_changed()?;
        let remaining = registry::read_entries(MANAGED_PROXY_VARS)?;
        if remaining.iter().any(|entry| entry.exists) {
            return Err(ProxyEnvError::VerificationFailed(
                "one or more managed variables still exist".into(),
            ));
        }
        Self::status()
    }

    pub fn enable() -> Result<EnvironmentStatus> {
        let snapshot = snapshot::load()?.ok_or(ProxyEnvError::SnapshotMissing)?;
        registry::restore_entries(&snapshot.values)?;
        broadcast::environment_changed()?;
        let actual = registry::read_entries(MANAGED_PROXY_VARS)?;
        if !snapshot.matches(&actual) {
            return Err(ProxyEnvError::VerificationFailed(
                "restored values differ from the saved snapshot".into(),
            ));
        }
        Self::status()
    }
}
