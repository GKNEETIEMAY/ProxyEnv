use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemoteTargetSource {
    Openssh,
    Vscode,
    Mobaxterm,
    Manual,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemoteTargetCompatibility {
    Compatible,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SshAuthMode {
    #[default]
    NonInteractive,
    Interactive,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SshAuthMethod {
    IdentityFile,
    Agent,
    Password,
    KeyboardInteractive,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SshAuthState {
    pub mode: SshAuthMode,
    pub method: SshAuthMethod,
    pub authenticated: bool,
    pub password_stored: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTarget {
    pub id: String,
    pub display_name: String,
    pub source: RemoteTargetSource,
    pub source_label: String,
    pub config_path: String,
    pub ssh_alias: Option<String>,
    pub host: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub authentication_method: SshAuthMethod,
    pub available: bool,
    pub compatibility: RemoteTargetCompatibility,
    pub unavailable_reason: Option<String>,
    pub can_open_vscode: bool,
    pub can_open_mobaxterm: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub target_id: String,
    pub proxy_port: Option<u16>,
    pub cc_port: Option<u16>,
    pub cc_local_port: u16,
    pub expected_revision: u64,
}
