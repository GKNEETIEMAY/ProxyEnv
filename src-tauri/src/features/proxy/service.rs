use std::collections::HashMap;

use crate::{
    environment::{
        EnvironmentEntry, EnvironmentManager, EnvironmentMutation, EnvironmentScope, SnapshotReason,
    },
    error::{ProxyEnvError, Result},
};

use super::{ProxyEnvironmentState, ProxyEnvironmentStatus, ProxyProtocol, ProxyVariable};

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
    pub fn status(selected: &[ProxyVariable]) -> Result<ProxyEnvironmentStatus> {
        let names = all_names();
        let entries = EnvironmentManager::read(&names, EnvironmentScope::User)?;
        let state = environment_state(&entries, selected, false);
        Ok(ProxyEnvironmentStatus {
            state,
            entries,
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
        Self::status(&[])
    }

    pub fn enable(selected: &[ProxyVariable]) -> Result<ProxyEnvironmentStatus> {
        let snapshot =
            EnvironmentManager::latest_snapshot()?.ok_or(ProxyEnvError::SnapshotMissing)?;
        let values = snapshot
            .entries
            .into_iter()
            .map(|entry| (entry.name, entry.value))
            .collect::<HashMap<_, _>>();
        let mutations = MANAGED_VARIABLES
            .iter()
            .map(|name| {
                let value = variable_is_selected(name, selected)
                    .then(|| values.get(*name).cloned().unwrap_or(None))
                    .flatten();
                mutation((*name).to_owned(), value)
            })
            .collect::<Vec<_>>();
        EnvironmentManager::apply(&mutations, EnvironmentScope::User)?;
        Self::status(selected)
    }

    pub fn enable_for_proxy(
        host: &str,
        port: u16,
        protocol: ProxyProtocol,
        selected: &[ProxyVariable],
    ) -> Result<ProxyEnvironmentStatus> {
        let values = proxy_values(host, port, protocol, selected);
        let actual = EnvironmentManager::read(&managed_names(), EnvironmentScope::User)?;
        if entries_match(&values, &actual) {
            return Self::status(selected);
        }
        let mutations = values
            .into_iter()
            .map(|(name, value)| mutation(name, value))
            .collect::<Vec<_>>();
        EnvironmentManager::apply(&mutations, EnvironmentScope::User)?;
        Self::status(selected)
    }
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
    MANAGED_VARIABLES
        .iter()
        .map(|name| {
            let value = variable_is_selected(name, selected).then(|| value.clone());
            ((*name).to_owned(), value)
        })
        .collect()
}

fn variable_is_selected(name: &str, selected: &[ProxyVariable]) -> bool {
    selected
        .iter()
        .any(|variable| variable_matches_name(*variable, name))
}

fn variable_matches_name(variable: ProxyVariable, name: &str) -> bool {
    match variable {
        ProxyVariable::Http => name.eq_ignore_ascii_case("HTTP_PROXY"),
        ProxyVariable::Https => name.eq_ignore_ascii_case("HTTPS_PROXY"),
        ProxyVariable::All => name.eq_ignore_ascii_case("ALL_PROXY"),
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
}
