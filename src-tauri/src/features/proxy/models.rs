use serde::{Deserialize, Serialize};

use crate::environment::EnvironmentEntry;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum ProxyProtocol {
    Http,
    Socks5,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyEndpoint {
    pub host: String,
    pub port: u16,
    pub protocol: ProxyProtocol,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProxyVariable {
    Http,
    Https,
    All,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum DetectionSource {
    WindowsSystemProxy,
    ProcessListener,
    ProtocolProbe,
    ClientConfig,
    Composite,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum Confidence {
    VeryHigh,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyCandidate {
    pub id: String,
    pub client_name: Option<String>,
    pub icon_key: Option<String>,
    pub process_name: Option<String>,
    pub pid: Option<u32>,
    pub host: String,
    pub port: u16,
    pub protocol: ProxyProtocol,
    pub source: Vec<DetectionSource>,
    pub confidence: Confidence,
    pub listening: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyEnvironmentStatus {
    pub state: ProxyEnvironmentState,
    pub entries: Vec<EnvironmentEntry>,
    pub selected_variables: Vec<ProxyVariable>,
    pub active_candidate: Option<ProxyCandidate>,
    pub candidates: Vec<ProxyCandidate>,
    pub matches_active_proxy: bool,
    pub snapshot_available: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProxyEnvironmentState {
    Disabled,
    Partial,
    Enabled,
    Mismatch,
}

impl ProxyEnvironmentState {
    pub fn is_configured(self) -> bool {
        matches!(self, Self::Partial | Self::Enabled | Self::Mismatch)
    }
}
