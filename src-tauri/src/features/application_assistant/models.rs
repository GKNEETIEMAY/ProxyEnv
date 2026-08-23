use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningApplication {
    pub pid: u32,
    pub process_name: String,
    pub display_name: String,
    pub executable_path: Option<PathBuf>,
    pub icon_available: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum ApplicationActionKind {
    LaunchWithProxy,
    LaunchWithoutProxy,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ManagedApplication {
    pub id: String,
    pub display_name: String,
    pub executable_path: PathBuf,
    pub icon_key: Option<String>,
    pub rule_id: Option<String>,
    pub last_action: Option<ApplicationActionKind>,
}
