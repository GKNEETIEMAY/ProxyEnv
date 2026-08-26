use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub(crate) const CURRENT_SNAPSHOT_SCHEMA_VERSION: u32 = 2;
pub(crate) const MAX_SNAPSHOT_ENTRIES: usize = 3;
pub(crate) const MAX_ENVIRONMENT_VALUE_BYTES: usize = 4 * 1024;

#[cfg(windows)]
const MANAGED_ENVIRONMENT_VARIABLES: &[&str] = &["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"];

#[cfg(any(target_os = "linux", target_os = "macos"))]
const MANAGED_ENVIRONMENT_VARIABLES: &[&str] = &["http_proxy", "https_proxy", "all_proxy"];

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
const MANAGED_ENVIRONMENT_VARIABLES: &[&str] = &["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"];

pub(crate) fn is_managed_environment_variable(name: &str) -> bool {
    MANAGED_ENVIRONMENT_VARIABLES.contains(&name)
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentScope {
    User,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentEntry {
    pub name: String,
    pub value: Option<String>,
    pub exists: bool,
    pub scope: EnvironmentScope,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum EnvironmentMutation {
    Set { name: String, value: String },
    Delete { name: String },
}

impl EnvironmentMutation {
    pub fn name(&self) -> &str {
        match self {
            Self::Set { name, .. } | Self::Delete { name } => name,
        }
    }

    pub fn expected_value(&self) -> Option<&str> {
        match self {
            Self::Set { value, .. } => Some(value),
            Self::Delete { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentApplyResult {
    pub changed: Vec<String>,
    pub verified: bool,
    pub broadcast_sent: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SnapshotReason {
    FeatureChange,
    Manual,
    BeforeApply,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentSnapshot {
    pub schema_version: u32,
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub scope: EnvironmentScope,
    pub reason: SnapshotReason,
    pub before: Vec<EnvironmentEntry>,
    pub applied: Vec<EnvironmentEntry>,
}

impl EnvironmentSnapshot {
    pub fn new(
        before: Vec<EnvironmentEntry>,
        applied: Vec<EnvironmentEntry>,
        scope: EnvironmentScope,
        reason: SnapshotReason,
    ) -> Self {
        let created_at = Utc::now();
        Self {
            schema_version: CURRENT_SNAPSHOT_SCHEMA_VERSION,
            id: created_at.format("%Y%m%dT%H%M%S%.3fZ").to_string(),
            created_at,
            scope,
            reason,
            before,
            applied,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_preserves_missing_and_present_values() {
        let before = vec![
            EnvironmentEntry {
                name: MANAGED_ENVIRONMENT_VARIABLES[0].into(),
                value: Some("value".into()),
                exists: true,
                scope: EnvironmentScope::User,
            },
            EnvironmentEntry {
                name: MANAGED_ENVIRONMENT_VARIABLES[1].into(),
                value: None,
                exists: false,
                scope: EnvironmentScope::User,
            },
        ];
        let applied = vec![
            EnvironmentEntry {
                name: MANAGED_ENVIRONMENT_VARIABLES[0].into(),
                value: Some("next".into()),
                exists: true,
                scope: EnvironmentScope::User,
            },
            EnvironmentEntry {
                name: MANAGED_ENVIRONMENT_VARIABLES[1].into(),
                value: None,
                exists: false,
                scope: EnvironmentScope::User,
            },
        ];
        let snapshot = EnvironmentSnapshot::new(
            before,
            applied,
            EnvironmentScope::User,
            SnapshotReason::BeforeApply,
        );
        assert_eq!(snapshot.before[0].value.as_deref(), Some("value"));
        assert_eq!(snapshot.applied[0].value.as_deref(), Some("next"));
        assert_eq!(snapshot.schema_version, CURRENT_SNAPSHOT_SCHEMA_VERSION);
    }
}
