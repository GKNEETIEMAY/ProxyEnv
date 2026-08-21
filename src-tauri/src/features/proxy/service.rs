use std::collections::HashMap;

use crate::{
    environment::{
        EnvironmentEntry, EnvironmentManager, EnvironmentMutation, EnvironmentScope, SnapshotReason,
    },
    error::{ProxyEnvError, Result},
};

use super::{
    ProxyCandidate, ProxyEndpoint, ProxyEnvironmentState, ProxyEnvironmentStatus, ProxyProtocol,
    ProxyVariable,
};

#[cfg(windows)]
const MANAGED_VARIABLES: &[&str] = &["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"];
#[cfg(windows)]
const DISPLAY_VARIABLES: &[&str] = &["NO_PROXY"];

#[cfg(any(target_os = "linux", target_os = "macos"))]
const MANAGED_VARIABLES: &[&str] = &["http_proxy", "https_proxy", "all_proxy"];
#[cfg(any(target_os = "linux", target_os = "macos"))]
const DISPLAY_VARIABLES: &[&str] = &["no_proxy"];

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
const MANAGED_VARIABLES: &[&str] = &["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"];
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
const DISPLAY_VARIABLES: &[&str] = &["NO_PROXY"];

pub struct ProxyEnvironmentService;

impl ProxyEnvironmentService {
    pub fn status(
        selected: &[ProxyVariable],
        active_candidate: Option<ProxyCandidate>,
    ) -> Result<ProxyEnvironmentStatus> {
        let names = all_names();
        let entries = EnvironmentManager::read(&names, EnvironmentScope::User)?;
        let expected = active_candidate.as_ref().map(|candidate| {
            proxy_values(
                &candidate.host,
                candidate.port,
                candidate.protocol,
                selected,
            )
        });
        let matches_active_proxy = expected
            .as_ref()
            .is_some_and(|expected| entries_match(expected, &entries));
        let configured = entries
            .iter()
            .any(|entry| MANAGED_VARIABLES.contains(&entry.name.as_str()) && entry.exists);
        let endpoint_mismatch = configured && expected.is_some() && !matches_active_proxy;
        let state = environment_state(&entries, selected, endpoint_mismatch);
        let snapshot_available = EnvironmentManager::latest_snapshot()?.is_some();
        Ok(ProxyEnvironmentStatus {
            state,
            entries,
            selected_variables: selected.to_vec(),
            active_candidate,
            matches_active_proxy,
            snapshot_available,
            warning: None,
        })
    }

    pub fn disable() -> Result<ProxyEnvironmentStatus> {
        let names = managed_names();
        EnvironmentManager::snapshot(
            &names,
            EnvironmentScope::User,
            SnapshotReason::FeatureChange,
        )?;
        let mutations = names
            .into_iter()
            .map(|name| EnvironmentMutation::Delete { name })
            .collect::<Vec<_>>();
        EnvironmentManager::apply(&mutations, EnvironmentScope::User)?;
        Self::status(&[], None)
    }

    pub fn restore(selected: &[ProxyVariable]) -> Result<ProxyEnvironmentStatus> {
        let snapshot =
            EnvironmentManager::latest_snapshot()?.ok_or(ProxyEnvError::SnapshotMissing)?;
        EnvironmentManager::restore(&snapshot)?;
        Self::status(selected, None)
    }

    pub fn sync(
        host: &str,
        port: u16,
        protocol: ProxyProtocol,
        selected: &[ProxyVariable],
    ) -> Result<ProxyEnvironmentStatus> {
        let values = proxy_values(host, port, protocol, selected);
        let actual = EnvironmentManager::read(&managed_names(), EnvironmentScope::User)?;
        if entries_match(&values, &actual) {
            return Self::status(selected, None);
        }
        EnvironmentManager::snapshot(
            &managed_names(),
            EnvironmentScope::User,
            SnapshotReason::BeforeApply,
        )?;
        let mutations = values
            .into_iter()
            .map(|(name, value)| mutation(name, value))
            .collect::<Vec<_>>();
        EnvironmentManager::apply(&mutations, EnvironmentScope::User)?;
        Self::status(selected, None)
    }

    pub fn sync_manual(
        endpoint: &ProxyEndpoint,
        selected: &[ProxyVariable],
    ) -> Result<ProxyEnvironmentStatus> {
        validate_endpoint(endpoint)?;
        Self::sync(
            endpoint.host.trim(),
            endpoint.port,
            endpoint.protocol,
            selected,
        )
    }
}

fn validate_endpoint(endpoint: &ProxyEndpoint) -> Result<()> {
    if endpoint.host.trim().is_empty() {
        return Err(ProxyEnvError::InvalidProxyEndpoint(
            "host cannot be empty".into(),
        ));
    }
    if endpoint.port == 0 {
        return Err(ProxyEnvError::InvalidProxyEndpoint(
            "port must be between 1 and 65535".into(),
        ));
    }
    if matches!(endpoint.protocol, ProxyProtocol::Unknown) {
        return Err(ProxyEnvError::InvalidProxyEndpoint(
            "protocol must be HTTP, SOCKS5, or Mixed".into(),
        ));
    }
    Ok(())
}

fn environment_state(
    entries: &[EnvironmentEntry],
    selected: &[ProxyVariable],
    endpoint_mismatch: bool,
) -> ProxyEnvironmentState {
    let managed = entries
        .iter()
        .filter(|entry| MANAGED_VARIABLES.contains(&entry.name.as_str()))
        .collect::<Vec<_>>();
    if managed.iter().all(|entry| !entry.exists) {
        return ProxyEnvironmentState::Disabled;
    }
    if endpoint_mismatch {
        return ProxyEnvironmentState::Mismatch;
    }
    let selected_are_valid = !selected.is_empty()
        && selected.iter().all(|variable| {
            managed.iter().any(|entry| {
                variable_matches_name(*variable, &entry.name)
                    && entry.exists
                    && entry
                        .value
                        .as_deref()
                        .is_some_and(|value| !value.is_empty())
            })
        });
    if selected_are_valid {
        ProxyEnvironmentState::Enabled
    } else {
        ProxyEnvironmentState::Partial
    }
}

fn managed_names() -> Vec<String> {
    MANAGED_VARIABLES
        .iter()
        .map(|name| (*name).to_owned())
        .collect()
}

fn all_names() -> Vec<String> {
    MANAGED_VARIABLES
        .iter()
        .chain(DISPLAY_VARIABLES)
        .map(|name| (*name).to_owned())
        .collect()
}

fn mutation(name: String, value: Option<String>) -> EnvironmentMutation {
    match value {
        Some(value) => EnvironmentMutation::Set { name, value },
        None => EnvironmentMutation::Delete { name },
    }
}

fn proxy_values(
    host: &str,
    port: u16,
    protocol: ProxyProtocol,
    selected: &[ProxyVariable],
) -> HashMap<String, Option<String>> {
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    MANAGED_VARIABLES
        .iter()
        .map(|name| {
            let variable = variable_for_name(name);
            let value = variable
                .filter(|variable| selected.contains(variable))
                .and_then(|variable| protocol_scheme(protocol, variable))
                .map(|scheme| format!("{scheme}://{host}:{port}"));
            ((*name).to_owned(), value)
        })
        .collect()
}

fn variable_matches_name(variable: ProxyVariable, name: &str) -> bool {
    match variable {
        ProxyVariable::Http => name.eq_ignore_ascii_case("HTTP_PROXY"),
        ProxyVariable::Https => name.eq_ignore_ascii_case("HTTPS_PROXY"),
        ProxyVariable::All => name.eq_ignore_ascii_case("ALL_PROXY"),
    }
}

fn variable_for_name(name: &str) -> Option<ProxyVariable> {
    [
        ProxyVariable::Http,
        ProxyVariable::Https,
        ProxyVariable::All,
    ]
    .into_iter()
    .find(|variable| variable_matches_name(*variable, name))
}

fn protocol_scheme(protocol: ProxyProtocol, variable: ProxyVariable) -> Option<&'static str> {
    match (protocol, variable) {
        (
            ProxyProtocol::Http | ProxyProtocol::Unknown,
            ProxyVariable::Http | ProxyVariable::Https,
        ) => Some("http"),
        (ProxyProtocol::Socks5, ProxyVariable::All) => Some("socks5"),
        (ProxyProtocol::Mixed, ProxyVariable::Http | ProxyVariable::Https) => Some("http"),
        (ProxyProtocol::Mixed, ProxyVariable::All) => Some("socks5"),
        _ => None,
    }
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
    fn builds_http_values_for_selected_variables() {
        let selected = [ProxyVariable::Http, ProxyVariable::Https];
        let values = proxy_values("127.0.0.1", 10809, ProxyProtocol::Http, &selected);
        assert_eq!(
            values[MANAGED_VARIABLES[0]].as_deref(),
            Some("http://127.0.0.1:10809")
        );
        assert_eq!(values[MANAGED_VARIABLES[2]], None);
    }

    #[test]
    fn brackets_ipv6_hosts() {
        let values = proxy_values("::1", 10808, ProxyProtocol::Socks5, &[ProxyVariable::All]);
        assert_eq!(
            values[MANAGED_VARIABLES[2]].as_deref(),
            Some("socks5://[::1]:10808")
        );
    }

    #[test]
    fn maps_http_socks5_and_mixed_protocols_to_compatible_variables() {
        let selected = [
            ProxyVariable::Http,
            ProxyVariable::Https,
            ProxyVariable::All,
        ];
        let http = proxy_values("127.0.0.1", 7890, ProxyProtocol::Http, &selected);
        assert_eq!(
            http[MANAGED_VARIABLES[0]].as_deref(),
            Some("http://127.0.0.1:7890")
        );
        assert_eq!(
            http[MANAGED_VARIABLES[1]].as_deref(),
            Some("http://127.0.0.1:7890")
        );
        assert_eq!(http[MANAGED_VARIABLES[2]], None);

        let socks = proxy_values("127.0.0.1", 10808, ProxyProtocol::Socks5, &selected);
        assert_eq!(socks[MANAGED_VARIABLES[0]], None);
        assert_eq!(socks[MANAGED_VARIABLES[1]], None);
        assert_eq!(
            socks[MANAGED_VARIABLES[2]].as_deref(),
            Some("socks5://127.0.0.1:10808")
        );

        let mixed = proxy_values("127.0.0.1", 7897, ProxyProtocol::Mixed, &selected);
        assert_eq!(
            mixed[MANAGED_VARIABLES[0]].as_deref(),
            Some("http://127.0.0.1:7897")
        );
        assert_eq!(
            mixed[MANAGED_VARIABLES[1]].as_deref(),
            Some("http://127.0.0.1:7897")
        );
        assert_eq!(
            mixed[MANAGED_VARIABLES[2]].as_deref(),
            Some("socks5://127.0.0.1:7897")
        );
    }

    #[test]
    fn selected_variables_still_limit_protocol_mapping() {
        let values = proxy_values(
            "127.0.0.1",
            7897,
            ProxyProtocol::Mixed,
            &[ProxyVariable::Https],
        );
        assert_eq!(values[MANAGED_VARIABLES[0]], None);
        assert_eq!(
            values[MANAGED_VARIABLES[1]].as_deref(),
            Some("http://127.0.0.1:7897")
        );
        assert_eq!(values[MANAGED_VARIABLES[2]], None);
    }

    #[test]
    fn validates_manual_proxy_endpoints() {
        let valid = ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: 7897,
            protocol: ProxyProtocol::Mixed,
        };
        assert!(validate_endpoint(&valid).is_ok());
        assert!(validate_endpoint(&ProxyEndpoint {
            host: "  ".into(),
            ..valid.clone()
        })
        .is_err());
        assert!(validate_endpoint(&ProxyEndpoint { port: 0, ..valid }).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_uses_uppercase_variable_names() {
        assert_eq!(
            MANAGED_VARIABLES,
            &["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"]
        );
        assert_eq!(DISPLAY_VARIABLES, &["NO_PROXY"]);
    }

    #[test]
    fn classifies_disabled_partial_and_enabled_states() {
        let entries = MANAGED_VARIABLES
            .iter()
            .map(|name| EnvironmentEntry {
                name: (*name).to_owned(),
                value: None,
                exists: false,
                scope: EnvironmentScope::User,
            })
            .collect::<Vec<_>>();
        let selected = [ProxyVariable::Http, ProxyVariable::Https];
        assert_eq!(
            environment_state(&entries, &selected, false),
            ProxyEnvironmentState::Disabled
        );

        let mut partial = entries.clone();
        partial[0].exists = true;
        partial[0].value = Some("http://127.0.0.1:7890".into());
        assert_eq!(
            environment_state(&partial, &selected, false),
            ProxyEnvironmentState::Partial
        );

        let mut enabled = partial;
        enabled[1].exists = true;
        enabled[1].value = Some("http://127.0.0.1:7890".into());
        assert_eq!(
            environment_state(&enabled, &selected, false),
            ProxyEnvironmentState::Enabled
        );
        assert_eq!(
            environment_state(&enabled, &selected, true),
            ProxyEnvironmentState::Mismatch
        );
    }

    #[test]
    fn detects_an_environment_endpoint_mismatch() {
        let selected = [ProxyVariable::Http, ProxyVariable::Https];
        let expected = proxy_values("127.0.0.1", 7897, ProxyProtocol::Http, &selected);
        let actual = MANAGED_VARIABLES
            .iter()
            .map(|name| EnvironmentEntry {
                name: (*name).to_owned(),
                value: if name.eq_ignore_ascii_case("ALL_PROXY") {
                    None
                } else {
                    Some("http://127.0.0.1:7890".into())
                },
                exists: !name.eq_ignore_ascii_case("ALL_PROXY"),
                scope: EnvironmentScope::User,
            })
            .collect::<Vec<_>>();
        assert!(!entries_match(&expected, &actual));
        assert_eq!(
            environment_state(&actual, &selected, true),
            ProxyEnvironmentState::Mismatch
        );
    }
}
