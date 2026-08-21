use std::collections::HashSet;

use crate::error::{ProxyEnvError, Result};

use super::{
    broadcast,
    models::{
        EnvironmentApplyResult, EnvironmentEntry, EnvironmentMutation, EnvironmentScope,
        EnvironmentSnapshot, SnapshotReason,
    },
    registry, snapshot,
};

pub struct EnvironmentManager;

impl EnvironmentManager {
    pub fn read(names: &[String], scope: EnvironmentScope) -> Result<Vec<EnvironmentEntry>> {
        registry::read_entries(names, scope)
    }

    pub fn apply(
        mutations: &[EnvironmentMutation],
        scope: EnvironmentScope,
    ) -> Result<EnvironmentApplyResult> {
        ensure_unique_names(mutations)?;
        let names = mutations
            .iter()
            .map(|mutation| mutation.name().to_owned())
            .collect::<Vec<_>>();
        let before = registry::read_entries(&names, scope)?;
        registry::apply_mutations(mutations, scope)?;
        broadcast::environment_changed()?;
        let after = registry::read_entries(&names, scope)?;
        let verified = mutations.iter().all(|mutation| {
            after.iter().any(|entry| {
                entry.name == mutation.name() && entry.value.as_deref() == mutation.expected_value()
            })
        });
        if !verified {
            return Err(ProxyEnvError::VerificationFailed(
                "one or more environment mutations did not persist".into(),
            ));
        }
        let changed = names
            .into_iter()
            .filter(|name| {
                let previous = before.iter().find(|entry| entry.name == *name);
                let current = after.iter().find(|entry| entry.name == *name);
                previous.map(|entry| &entry.value) != current.map(|entry| &entry.value)
            })
            .collect();
        Ok(EnvironmentApplyResult {
            changed,
            verified,
            broadcast_sent: true,
        })
    }

    pub fn snapshot(
        names: &[String],
        scope: EnvironmentScope,
        reason: SnapshotReason,
    ) -> Result<EnvironmentSnapshot> {
        let entries = Self::read(names, scope)?;
        let snapshot = EnvironmentSnapshot::new(entries, scope, reason);
        snapshot::save_atomic(&snapshot)?;
        Ok(snapshot)
    }

    pub fn latest_snapshot() -> Result<Option<EnvironmentSnapshot>> {
        snapshot::load_latest()
    }

    pub fn restore(snapshot: &EnvironmentSnapshot) -> Result<EnvironmentApplyResult> {
        let mutations = snapshot
            .entries
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
            .collect::<Vec<_>>();
        Self::apply(&mutations, snapshot.scope)
    }
}

fn ensure_unique_names(mutations: &[EnvironmentMutation]) -> Result<()> {
    let mut names = HashSet::new();
    if mutations
        .iter()
        .all(|mutation| names.insert(mutation.name()))
    {
        Ok(())
    } else {
        Err(ProxyEnvError::VerificationFailed(
            "an environment apply request contains duplicate names".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_mutation_names() {
        let mutations = [
            EnvironmentMutation::Delete {
                name: "EXAMPLE".into(),
            },
            EnvironmentMutation::Set {
                name: "EXAMPLE".into(),
                value: "value".into(),
            },
        ];
        assert!(ensure_unique_names(&mutations).is_err());
    }
}
