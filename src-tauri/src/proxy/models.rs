use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum ProxyProtocol {
    Http,
    Socks5,
    Mixed,
    Unknown,
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
