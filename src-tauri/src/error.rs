use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProxyEnvError {
    #[error("failed to read the user environment: {0}")]
    RegistryRead(String),
    #[error("failed to update the user environment: {0}")]
    RegistryWrite(String),
    #[error("failed to save the environment snapshot: {0}")]
    SnapshotWrite(String),
    #[error("failed to load the environment snapshot: {0}")]
    SnapshotRead(String),
    #[error("no saved proxy environment snapshot is available")]
    SnapshotMissing,
    #[error("environment change broadcast failed: {0}")]
    BroadcastFailed(String),
    #[error("environment update could not be verified: {0}")]
    VerificationFailed(String),
    #[error("proxy detection failed: {0}")]
    Detection(String),
    #[error("no active proxy endpoint is available")]
    ActiveProxyMissing,
    #[error("invalid proxy endpoint: {0}")]
    InvalidProxyEndpoint(String),
    #[error("failed to read application settings: {0}")]
    SettingsRead(String),
    #[error("failed to save application settings: {0}")]
    SettingsWrite(String),
    #[error("failed to update launch-at-startup: {0}")]
    Autostart(String),
    #[cfg(not(windows))]
    #[error("this operation is only available on Windows")]
    UnsupportedPlatform,
}

impl Serialize for ProxyEnvError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ProxyEnvError>;
