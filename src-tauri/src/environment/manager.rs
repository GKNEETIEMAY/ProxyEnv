use std::{collections::HashSet, sync::Mutex};

use crate::error::{ProxyEnvError, Result};

use super::{
    broadcast,
    models::{
        is_managed_environment_variable, EnvironmentApplyResult, EnvironmentEntry,
        EnvironmentMutation, EnvironmentScope, EnvironmentSnapshot, SnapshotReason,
        MAX_ENVIRONMENT_VALUE_BYTES,
    },
    registry, snapshot,
};

static ENVIRONMENT_TRANSACTION_LOCK: Mutex<()> = Mutex::new(());

pub struct EnvironmentManager;

trait EnvironmentBackend {
    fn read_entries(
        &mut self,
        names: &[String],
        scope: EnvironmentScope,
    ) -> Result<Vec<EnvironmentEntry>>;

    fn apply_mutations(
        &mut self,
        mutations: &[EnvironmentMutation],
        scope: EnvironmentScope,
    ) -> Result<()>;

    fn broadcast_environment_changed(&mut self) -> Result<()>;
}

struct SystemEnvironmentBackend;

impl EnvironmentBackend for SystemEnvironmentBackend {
    fn read_entries(
        &mut self,
        names: &[String],
        scope: EnvironmentScope,
    ) -> Result<Vec<EnvironmentEntry>> {
        registry::read_entries(names, scope)
    }

    fn apply_mutations(
        &mut self,
        mutations: &[EnvironmentMutation],
        scope: EnvironmentScope,
    ) -> Result<()> {
        registry::apply_mutations(mutations, scope)
    }

    fn broadcast_environment_changed(&mut self) -> Result<()> {
        broadcast::environment_changed()
    }
}

impl EnvironmentManager {
    pub fn read(names: &[String], scope: EnvironmentScope) -> Result<Vec<EnvironmentEntry>> {
        let _guard = transaction_guard()?;
        registry::read_entries(names, scope)
    }

    pub fn apply(
        mutations: &[EnvironmentMutation],
        scope: EnvironmentScope,
        reason: SnapshotReason,
    ) -> Result<EnvironmentApplyResult> {
        let _guard = transaction_guard()?;
        validate_mutations(mutations)?;

        let names = mutation_names(mutations);
        let mut backend = SystemEnvironmentBackend;
        let before = backend.read_entries(&names, scope)?;
        let applied = entries_from_mutations(mutations, scope);
        let transaction = EnvironmentSnapshot::new(before.clone(), applied, scope, reason);
        snapshot::validate(&transaction)?;
        snapshot::save_atomic(&transaction)?;

        apply_with_rollback(&mut backend, mutations, scope, &before)
    }

    pub fn latest_snapshot() -> Result<Option<EnvironmentSnapshot>> {
        snapshot::load_latest()
    }

    pub fn can_restore(snapshot: &EnvironmentSnapshot, current: &[EnvironmentEntry]) -> bool {
        conflicting_names(&snapshot.applied, current).is_empty()
    }

    pub fn restore(snapshot: &EnvironmentSnapshot) -> Result<EnvironmentApplyResult> {
        let _guard = transaction_guard()?;
        let mut backend = SystemEnvironmentBackend;
        restore_with_backend(&mut backend, snapshot)
    }
}

fn transaction_guard() -> Result<std::sync::MutexGuard<'static, ()>> {
    ENVIRONMENT_TRANSACTION_LOCK.lock().map_err(|_| {
        ProxyEnvError::VerificationFailed("the environment transaction lock is poisoned".into())
    })
}

fn validate_mutations(mutations: &[EnvironmentMutation]) -> Result<()> {
    if mutations.is_empty() {
        return Err(ProxyEnvError::VerificationFailed(
            "an environment apply request cannot be empty".into(),
        ));
    }

    let mut names = HashSet::new();
    for mutation in mutations {
        if !names.insert(mutation.name()) {
            return Err(ProxyEnvError::VerificationFailed(
                "an environment apply request contains duplicate names".into(),
            ));
        }
        if !is_managed_environment_variable(mutation.name()) {
            return Err(ProxyEnvError::VerificationFailed(format!(
                "environment variable {} is outside the ProxyEnv allowlist",
                mutation.name()
            )));
        }
        if mutation
            .expected_value()
            .is_some_and(|value| value.len() > MAX_ENVIRONMENT_VALUE_BYTES || value.contains('\0'))
        {
            return Err(ProxyEnvError::VerificationFailed(format!(
                "environment variable {} contains an invalid value",
                mutation.name()
            )));
        }
    }
    Ok(())
}

fn mutation_names(mutations: &[EnvironmentMutation]) -> Vec<String> {
    mutations
        .iter()
        .map(|mutation| mutation.name().to_owned())
        .collect()
}

fn entries_from_mutations(
    mutations: &[EnvironmentMutation],
    scope: EnvironmentScope,
) -> Vec<EnvironmentEntry> {
    mutations
        .iter()
        .map(|mutation| EnvironmentEntry {
            name: mutation.name().to_owned(),
            value: mutation.expected_value().map(str::to_owned),
            exists: mutation.expected_value().is_some(),
            scope,
        })
        .collect()
}

fn mutations_from_entries(entries: &[EnvironmentEntry]) -> Vec<EnvironmentMutation> {
    entries
        .iter()
        .map(|entry| match &entry.value {
            Some(value) => EnvironmentMutation::Set {
                name: entry.name.clone(),
                value: value.clone(),
            },
            None => EnvironmentMutation::Delete {
                name: entry.name.clone(),
            },
        })
        .collect()
}

fn apply_with_rollback<B: EnvironmentBackend>(
    backend: &mut B,
    mutations: &[EnvironmentMutation],
    scope: EnvironmentScope,
    before: &[EnvironmentEntry],
) -> Result<EnvironmentApplyResult> {
    match apply_and_verify(backend, mutations, scope) {
        Ok(after) => Ok(EnvironmentApplyResult {
            changed: changed_names(before, &after),
            verified: true,
            broadcast_sent: true,
        }),
        Err(operation_error) => {
            let rollback = mutations_from_entries(before);
            match apply_and_verify(backend, &rollback, scope) {
                Ok(restored) if entries_match(before, &restored) => Err(
                    ProxyEnvError::EnvironmentApplyRolledBack(operation_error.to_string()),
                ),
                Ok(_) => Err(ProxyEnvError::EnvironmentRollbackIncomplete(format!(
                    "{}; rollback verification did not match the previous values",
                    operation_error
                ))),
                Err(rollback_error) => Err(ProxyEnvError::EnvironmentRollbackIncomplete(format!(
                    "{}; rollback failed: {}",
                    operation_error, rollback_error
                ))),
            }
        }
    }
}

fn restore_with_backend<B: EnvironmentBackend>(
    backend: &mut B,
    snapshot: &EnvironmentSnapshot,
) -> Result<EnvironmentApplyResult> {
    snapshot::validate(snapshot)?;
    let names = snapshot
        .applied
        .iter()
        .map(|entry| entry.name.clone())
        .collect::<Vec<_>>();
    let current = backend.read_entries(&names, snapshot.scope)?;
    let conflicts = conflicting_names(&snapshot.applied, &current);
    if !conflicts.is_empty() {
        return Err(ProxyEnvError::EnvironmentRestoreConflict(
            conflicts.join(", "),
        ));
    }

    let mutations = mutations_from_entries(&snapshot.before);
    apply_with_rollback(backend, &mutations, snapshot.scope, &current)
}

fn apply_and_verify<B: EnvironmentBackend>(
    backend: &mut B,
    mutations: &[EnvironmentMutation],
    scope: EnvironmentScope,
) -> Result<Vec<EnvironmentEntry>> {
    backend.apply_mutations(mutations, scope)?;
    backend.broadcast_environment_changed()?;
    let names = mutation_names(mutations);
    let after = backend.read_entries(&names, scope)?;
    let expected = entries_from_mutations(mutations, scope);
    if !entries_match(&expected, &after) {
        return Err(ProxyEnvError::VerificationFailed(
            "one or more environment mutations did not persist".into(),
        ));
    }
    Ok(after)
}

fn entries_match(expected: &[EnvironmentEntry], actual: &[EnvironmentEntry]) -> bool {
    expected.len() == actual.len()
        && expected.iter().all(|expected_entry| {
            actual.iter().any(|actual_entry| {
                actual_entry.name == expected_entry.name
                    && actual_entry.value == expected_entry.value
                    && actual_entry.exists == expected_entry.exists
                    && actual_entry.scope == expected_entry.scope
            })
        })
}

fn conflicting_names(expected: &[EnvironmentEntry], actual: &[EnvironmentEntry]) -> Vec<String> {
    expected
        .iter()
        .filter(|expected_entry| {
            !actual.iter().any(|actual_entry| {
                actual_entry.name == expected_entry.name
                    && actual_entry.value == expected_entry.value
                    && actual_entry.exists == expected_entry.exists
                    && actual_entry.scope == expected_entry.scope
            })
        })
        .map(|entry| entry.name.clone())
        .collect()
}

fn changed_names(before: &[EnvironmentEntry], after: &[EnvironmentEntry]) -> Vec<String> {
    after
        .iter()
        .filter(
            |current| match before.iter().find(|previous| previous.name == current.name) {
                Some(previous) => {
                    previous.value != current.value || previous.exists != current.exists
                }
                None => true,
            },
        )
        .map(|entry| entry.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    struct FakeBackend {
        values: HashMap<String, Option<String>>,
        apply_calls: usize,
        fail_first_apply_after: Option<usize>,
        fail_first_broadcast: bool,
        corrupt_first_verification: bool,
        fail_rollback: bool,
    }

    impl FakeBackend {
        fn with_values(values: &[(&str, Option<&str>)]) -> Self {
            Self {
                values: values
                    .iter()
                    .map(|(name, value)| ((*name).into(), value.map(str::to_owned)))
                    .collect(),
                apply_calls: 0,
                fail_first_apply_after: None,
                fail_first_broadcast: false,
                corrupt_first_verification: false,
                fail_rollback: false,
            }
        }
    }

    impl EnvironmentBackend for FakeBackend {
        fn read_entries(
            &mut self,
            names: &[String],
            scope: EnvironmentScope,
        ) -> Result<Vec<EnvironmentEntry>> {
            let corrupt = self.corrupt_first_verification && self.apply_calls == 1;
            Ok(names
                .iter()
                .map(|name| {
                    let mut value = self.values.get(name).cloned().flatten();
                    if corrupt && name == &names[0] {
                        value = Some("externally-corrupted".into());
                    }
                    EnvironmentEntry {
                        name: name.clone(),
                        exists: value.is_some(),
                        value,
                        scope,
                    }
                })
                .collect())
        }

        fn apply_mutations(
            &mut self,
            mutations: &[EnvironmentMutation],
            _scope: EnvironmentScope,
        ) -> Result<()> {
            self.apply_calls += 1;
            if self.fail_rollback && self.apply_calls > 1 {
                return Err(ProxyEnvError::RegistryWrite("rollback denied".into()));
            }
            for (index, mutation) in mutations.iter().enumerate() {
                self.values.insert(
                    mutation.name().into(),
                    mutation.expected_value().map(str::to_owned),
                );
                if self.apply_calls == 1 && self.fail_first_apply_after == Some(index + 1) {
                    return Err(ProxyEnvError::RegistryWrite("partial write".into()));
                }
            }
            Ok(())
        }

        fn broadcast_environment_changed(&mut self) -> Result<()> {
            if self.fail_first_broadcast && self.apply_calls == 1 {
                return Err(ProxyEnvError::BroadcastFailed("simulated timeout".into()));
            }
            Ok(())
        }
    }

    #[cfg(windows)]
    fn test_names() -> [&'static str; 2] {
        ["HTTP_PROXY", "HTTPS_PROXY"]
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn test_names() -> [&'static str; 2] {
        ["http_proxy", "https_proxy"]
    }

    fn before_entries(names: &[&str; 2]) -> Vec<EnvironmentEntry> {
        vec![
            EnvironmentEntry {
                name: names[0].into(),
                value: Some("old-http".into()),
                exists: true,
                scope: EnvironmentScope::User,
            },
            EnvironmentEntry {
                name: names[1].into(),
                value: None,
                exists: false,
                scope: EnvironmentScope::User,
            },
        ]
    }

    fn next_mutations(names: &[&str; 2]) -> Vec<EnvironmentMutation> {
        vec![
            EnvironmentMutation::Set {
                name: names[0].into(),
                value: "next-http".into(),
            },
            EnvironmentMutation::Set {
                name: names[1].into(),
                value: "next-https".into(),
            },
        ]
    }

    #[test]
    fn rejects_duplicate_and_non_allowlisted_mutation_names() {
        let names = test_names();
        let duplicates = [
            EnvironmentMutation::Delete {
                name: names[0].into(),
            },
            EnvironmentMutation::Set {
                name: names[0].into(),
                value: "value".into(),
            },
        ];
        assert!(validate_mutations(&duplicates).is_err());
        assert!(validate_mutations(&[EnvironmentMutation::Delete {
            name: "PATH".into(),
        }])
        .is_err());
    }

    #[test]
    fn partial_write_failure_restores_every_previous_value() {
        let names = test_names();
        let before = before_entries(&names);
        let mut backend =
            FakeBackend::with_values(&[(names[0], Some("old-http")), (names[1], None)]);
        backend.fail_first_apply_after = Some(1);

        let error = apply_with_rollback(
            &mut backend,
            &next_mutations(&names),
            EnvironmentScope::User,
            &before,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProxyEnvError::EnvironmentApplyRolledBack(_)
        ));
        assert_eq!(
            backend.values.get(names[0]).cloned().flatten().as_deref(),
            Some("old-http")
        );
        assert_eq!(backend.values.get(names[1]).cloned().flatten(), None);
    }

    #[test]
    fn broadcast_and_verification_failures_are_rolled_back() {
        let names = test_names();
        let before = before_entries(&names);
        for failure in ["broadcast", "verify"] {
            let mut backend =
                FakeBackend::with_values(&[(names[0], Some("old-http")), (names[1], None)]);
            backend.fail_first_broadcast = failure == "broadcast";
            backend.corrupt_first_verification = failure == "verify";
            let error = apply_with_rollback(
                &mut backend,
                &next_mutations(&names),
                EnvironmentScope::User,
                &before,
            )
            .unwrap_err();
            assert!(matches!(
                error,
                ProxyEnvError::EnvironmentApplyRolledBack(_)
            ));
            assert_eq!(
                backend.values.get(names[0]).cloned().flatten().as_deref(),
                Some("old-http")
            );
            assert_eq!(backend.values.get(names[1]).cloned().flatten(), None);
        }
    }

    #[test]
    fn reports_when_rollback_cannot_complete() {
        let names = test_names();
        let before = before_entries(&names);
        let mut backend =
            FakeBackend::with_values(&[(names[0], Some("old-http")), (names[1], None)]);
        backend.fail_first_broadcast = true;
        backend.fail_rollback = true;

        let error = apply_with_rollback(
            &mut backend,
            &next_mutations(&names),
            EnvironmentScope::User,
            &before,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProxyEnvError::EnvironmentRollbackIncomplete(_)
        ));
    }

    #[test]
    fn restore_conflict_detection_is_order_independent() {
        let names = test_names();
        let expected = before_entries(&names);
        let mut actual = expected.clone();
        actual.reverse();
        assert!(conflicting_names(&expected, &actual).is_empty());
        actual[0].value = Some("external-change".into());
        actual[0].exists = true;
        assert_eq!(conflicting_names(&expected, &actual), vec![names[1]]);
    }

    #[test]
    fn restore_conflict_stops_before_any_write() {
        let names = test_names();
        let before = before_entries(&names);
        let applied = entries_from_mutations(&next_mutations(&names), EnvironmentScope::User);
        let snapshot = EnvironmentSnapshot::new(
            before,
            applied,
            EnvironmentScope::User,
            SnapshotReason::BeforeApply,
        );
        let mut backend = FakeBackend::with_values(&[
            (names[0], Some("external-change")),
            (names[1], Some("next-https")),
        ]);

        let error = restore_with_backend(&mut backend, &snapshot).unwrap_err();

        assert!(matches!(
            error,
            ProxyEnvError::EnvironmentRestoreConflict(_)
        ));
        assert_eq!(backend.apply_calls, 0);
        assert_eq!(
            backend.values.get(names[0]).cloned().flatten().as_deref(),
            Some("external-change")
        );
    }
}
