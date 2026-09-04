use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::features::{
    network_observation::TunObservationState,
    proxy::{connectivity::ProxyConnectivityState, ProxyEnvironmentState},
};

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningApplication {
    pub pid: u32,
    pub application_id: Option<String>,
    pub process_name: String,
    pub display_name: String,
    pub executable_path: Option<PathBuf>,
    pub icon_available: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ApplicationActionKind {
    LaunchWithProxy,
    LaunchWithoutProxy,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedApplication {
    pub id: String,
    pub display_name: String,
    pub executable_path: PathBuf,
    pub icon_key: Option<String>,
    pub rule_id: Option<String>,
    pub last_action: Option<ApplicationActionKind>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchEnvironmentMode {
    #[allow(dead_code)]
    Inherit,
    UseCurrentProxy,
    ClearProxyVariables,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchApplicationResult {
    pub pid: u32,
    pub executable_path: PathBuf,
    pub environment_mode: LaunchEnvironmentMode,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum RecommendedAction {
    None,
    LaunchWithProxy,
    LaunchWithoutProxy,
    ApplyKnownRule { rule_id: String },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum ApplicationNetworkState {
    ConfirmedReady,
    EnvironmentConfigured,
    ProxyLaunchRecommended,
    RuleSyncRecommended,
    Conflict,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationDiagnosis {
    pub application: ManagedApplication,
    pub proxy_available: bool,
    pub system_proxy_enabled: bool,
    pub proxy_environment_state: ProxyEnvironmentState,
    pub tun_observation: TunObservationState,
    pub known_rule: Option<String>,
    pub proxy_connectivity_state: Option<ProxyConnectivityState>,
    pub application_network_state: ApplicationNetworkState,
    pub recommended_action: RecommendedAction,
}
