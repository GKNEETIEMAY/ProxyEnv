use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentScope {
    User,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSnapshot {
    pub schema_version: u32,
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub scope: EnvironmentScope,
    pub reason: SnapshotReason,
    pub entries: Vec<EnvironmentEntry>,
}

impl EnvironmentSnapshot {
    pub fn new(
        entries: Vec<EnvironmentEntry>,
        scope: EnvironmentScope,
        reason: SnapshotReason,
    ) -> Self {
        let created_at = Utc::now();
        Self {
            schema_version: 1,
            id: created_at.format("%Y%m%dT%H%M%S%.3fZ").to_string(),
            created_at,
            scope,
            reason,
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_preserves_missing_and_present_values() {
        let snapshot = EnvironmentSnapshot::new(
            vec![
                EnvironmentEntry {
                    name: "FIRST".into(),
                    value: Some("value".into()),
                    exists: true,
                    scope: EnvironmentScope::User,
                },
                EnvironmentEntry {
                    name: "SECOND".into(),
                    value: None,
                    exists: false,
                    scope: EnvironmentScope::User,
                },
            ],
            EnvironmentScope::User,
            SnapshotReason::BeforeApply,
        );
        assert_eq!(snapshot.entries[0].value.as_deref(), Some("value"));
        assert_eq!(snapshot.entries[1].value, None);
        assert_eq!(snapshot.schema_version, 1);
    }
}
