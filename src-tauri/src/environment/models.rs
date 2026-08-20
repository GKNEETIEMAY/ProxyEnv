use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentEntry {
    pub name: String,
    pub value: Option<String>,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentStatus {
    pub enabled: bool,
    pub entries: Vec<EnvironmentEntry>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyEnvironmentSnapshot {
    pub created_at: DateTime<Utc>,
    pub values: HashMap<String, Option<String>>,
}

impl ProxyEnvironmentSnapshot {
    pub fn new(entries: Vec<EnvironmentEntry>) -> Self {
        Self {
            created_at: Utc::now(),
            values: entries
                .into_iter()
                .map(|entry| (entry.name, entry.value))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_preserves_missing_and_present_values() {
        let snapshot = ProxyEnvironmentSnapshot::new(vec![
            EnvironmentEntry {
                name: "HTTP_PROXY".into(),
                value: Some("http://127.0.0.1:7890".into()),
                exists: true,
            },
            EnvironmentEntry {
                name: "ALL_PROXY".into(),
                value: None,
                exists: false,
            },
        ]);
        assert_eq!(
            snapshot.values["HTTP_PROXY"].as_deref(),
            Some("http://127.0.0.1:7890")
        );
        assert_eq!(snapshot.values["ALL_PROXY"], None);
    }
}
