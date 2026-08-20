mod broadcast;
mod models;
mod registry;
mod snapshot;

pub use models::{EnvironmentEntry, EnvironmentStatus, ProxyEnvironmentSnapshot};

use crate::error::{ProxyEnvError, Result};
use crate::proxy::ProxyProtocol;
use std::collections::HashMap;

#[cfg(windows)]
pub const MANAGED_PROXY_VARS: &[&str] = &["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"];
#[cfg(windows)]
pub const DISPLAY_PROXY_VARS: &[&str] = &["NO_PROXY"];

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub const MANAGED_PROXY_VARS: &[&str] = &["http_proxy", "https_proxy", "all_proxy"];
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub const DISPLAY_PROXY_VARS: &[&str] = &["no_proxy"];

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub const MANAGED_PROXY_VARS: &[&str] = &["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"];
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub const DISPLAY_PROXY_VARS: &[&str] = &["NO_PROXY"];

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

    pub fn enable_for_proxy(
        host: &str,
        port: u16,
        protocol: ProxyProtocol,
    ) -> Result<EnvironmentStatus> {
        let expected = proxy_values(host, port, protocol);
        let actual = registry::read_entries(MANAGED_PROXY_VARS)?;
        if entries_match(&expected, &actual) {
            return Self::status();
        }

        registry::restore_entries(&expected)?;
        broadcast::environment_changed()?;
        let actual = registry::read_entries(MANAGED_PROXY_VARS)?;
        if !entries_match(&expected, &actual) {
            return Err(ProxyEnvError::VerificationFailed(
                "proxy variables differ from the detected endpoint".into(),
            ));
        }
        Self::status()
    }
}

fn proxy_values(host: &str, port: u16, protocol: ProxyProtocol) -> HashMap<String, Option<String>> {
    let scheme = if matches!(protocol, ProxyProtocol::Socks5) {
        "socks5"
    } else {
        "http"
    };
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    let value = format!("{scheme}://{host}:{port}");
    MANAGED_PROXY_VARS
        .iter()
        .map(|name| ((*name).to_owned(), Some(value.clone())))
        .collect()
}

fn entries_match(expected: &HashMap<String, Option<String>>, actual: &[EnvironmentEntry]) -> bool {
    actual
        .iter()
        .all(|entry| expected.get(&entry.name) == Some(&entry.value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_http_proxy_values_for_v2rayn_http_port() {
        let values = proxy_values("127.0.0.1", 10809, ProxyProtocol::Http);
        assert_eq!(values.len(), MANAGED_PROXY_VARS.len());
        assert_eq!(
            values[MANAGED_PROXY_VARS[0]].as_deref(),
            Some("http://127.0.0.1:10809")
        );
        assert_eq!(
            values[MANAGED_PROXY_VARS[2]].as_deref(),
            Some("http://127.0.0.1:10809")
        );
    }

    #[test]
    fn brackets_ipv6_proxy_hosts() {
        let values = proxy_values("::1", 10808, ProxyProtocol::Socks5);
        assert_eq!(
            values[MANAGED_PROXY_VARS[1]].as_deref(),
            Some("socks5://[::1]:10808")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_uses_one_uppercase_variable_set() {
        assert_eq!(
            MANAGED_PROXY_VARS,
            &["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"]
        );
        assert_eq!(DISPLAY_PROXY_VARS, &["NO_PROXY"]);
    }
}
