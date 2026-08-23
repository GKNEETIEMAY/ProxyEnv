use crate::{
    environment::{
        EnvironmentEntry, EnvironmentManager, EnvironmentMutation, EnvironmentScope, SnapshotReason,
    },
    error::{ProxyEnvError, Result},
};

use super::{
    plan::{
        all_proxy_variable_names, build_proxy_environment_plan, is_managed_variable,
        managed_variable_names, variable_matches_name, ProxyEnvironmentPlan,
    },
    ProxyCandidate, ProxyEndpoint, ProxyEnvironmentState, ProxyEnvironmentStatus, ProxyProtocol,
    ProxyVariable,
};

pub struct ProxyEnvironmentService;

impl ProxyEnvironmentService {
    pub fn status(
        selected: &[ProxyVariable],
        active_candidate: Option<ProxyCandidate>,
    ) -> Result<ProxyEnvironmentStatus> {
        let names = all_proxy_variable_names();
        let entries = EnvironmentManager::read(&names, EnvironmentScope::User)?;
        let expected = active_candidate
            .as_ref()
            .filter(|candidate| !matches!(candidate.protocol, ProxyProtocol::Unknown))
            .map(|candidate| {
                build_proxy_environment_plan(
                    &ProxyEndpoint {
                        host: candidate.host.clone(),
                        port: candidate.port,
                        protocol: candidate.protocol,
                    },
                    selected,
                )
            })
            .transpose()?;
        let matches_active_proxy = expected
            .as_ref()
            .is_some_and(|expected| entries_match(expected, &entries));
        let configured = entries
            .iter()
            .any(|entry| is_managed_variable(&entry.name) && entry.exists);
        let endpoint_mismatch = configured && expected.is_some() && !matches_active_proxy;
        let state = environment_state(&entries, selected, endpoint_mismatch);
        let snapshot_available = EnvironmentManager::latest_snapshot()?.is_some();
        Ok(ProxyEnvironmentStatus {
            state,
            entries,
            selected_variables: selected.to_vec(),
            active_candidate,
            candidates: Vec::new(),
            matches_active_proxy,
            snapshot_available,
            warning: None,
        })
    }

    pub fn disable() -> Result<ProxyEnvironmentStatus> {
        let names = managed_variable_names();
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
        let plan = build_proxy_environment_plan(
            &ProxyEndpoint {
                host: host.to_owned(),
                port,
                protocol,
            },
            selected,
        )?;
        let names = managed_variable_names();
        let actual = EnvironmentManager::read(&names, EnvironmentScope::User)?;
        if entries_match(&plan, &actual) {
            return Self::status(selected, None);
        }
        EnvironmentManager::snapshot(&names, EnvironmentScope::User, SnapshotReason::BeforeApply)?;
        let mutations = plan
            .variables
            .into_iter()
            .map(|entry| mutation(entry.name, entry.value))
            .collect::<Vec<_>>();
        EnvironmentManager::apply(&mutations, EnvironmentScope::User)?;
        Self::status(selected, None)
    }

    pub fn sync_manual(
        endpoint: &ProxyEndpoint,
        selected: &[ProxyVariable],
    ) -> Result<ProxyEnvironmentStatus> {
        Self::sync(
            endpoint.host.trim(),
            endpoint.port,
            endpoint.protocol,
            selected,
        )
    }
}

fn environment_state(
    entries: &[EnvironmentEntry],
    selected: &[ProxyVariable],
    endpoint_mismatch: bool,
) -> ProxyEnvironmentState {
    let managed = entries
        .iter()
        .filter(|entry| is_managed_variable(&entry.name))
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

fn mutation(name: String, value: Option<String>) -> EnvironmentMutation {
    match value {
        Some(value) => EnvironmentMutation::Set { name, value },
        None => EnvironmentMutation::Delete { name },
    }
}

fn entries_match(expected: &ProxyEnvironmentPlan, actual: &[EnvironmentEntry]) -> bool {
    expected.variables.iter().all(|expected_entry| {
        actual
            .iter()
            .any(|entry| entry.name == expected_entry.name && entry.value == expected_entry.value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_disabled_partial_and_enabled_states() {
        let entries = managed_variable_names()
            .into_iter()
            .map(|name| EnvironmentEntry {
                name,
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
        let expected = build_proxy_environment_plan(
            &ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: 7897,
                protocol: ProxyProtocol::Http,
            },
            &selected,
        )
        .unwrap();
        let actual = managed_variable_names()
            .into_iter()
            .map(|name| EnvironmentEntry {
                name: name.clone(),
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

    #[test]
    fn ignores_display_only_variables_when_matching_an_endpoint() {
        let selected = [ProxyVariable::Http, ProxyVariable::Https];
        let expected = build_proxy_environment_plan(
            &ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: 7897,
                protocol: ProxyProtocol::Http,
            },
            &selected,
        )
        .unwrap();
        let mut actual = expected
            .variables
            .iter()
            .map(|entry| EnvironmentEntry {
                name: entry.name.clone(),
                value: entry.value.clone(),
                exists: entry.value.is_some(),
                scope: EnvironmentScope::User,
            })
            .collect::<Vec<_>>();
        actual.push(EnvironmentEntry {
            name: all_proxy_variable_names()[3].clone(),
            value: Some("localhost,127.0.0.1".into()),
            exists: true,
            scope: EnvironmentScope::User,
        });
        assert!(entries_match(&expected, &actual));
    }
}
