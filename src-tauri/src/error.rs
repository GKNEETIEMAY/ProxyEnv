use std::{error::Error, fmt};

use serde::Serialize;

use crate::services::redaction;

pub enum ProxyEnvError {
    RegistryRead(String),
    RegistryWrite(String),
    SnapshotWrite(String),
    SnapshotRead(String),
    SnapshotMissing,
    BroadcastFailed(String),
    VerificationFailed(String),
    EnvironmentApplyRolledBack(String),
    EnvironmentRollbackIncomplete(String),
    EnvironmentRestoreConflict(String),
    Detection(String),
    ActiveProxyMissing,
    InvalidProxyEndpoint(String),
    InvalidApplication(String),
    ApplicationAuthorization(String),
    ApplicationLaunch(String),
    SettingsRead(String),
    SettingsWrite(String),
    Autostart(String),
    #[cfg(not(windows))]
    UnsupportedPlatform,
}

impl ProxyEnvError {
    fn public_parts(&self) -> (&'static str, Option<&str>) {
        match self {
            Self::RegistryRead(detail) => ("failed to read the user environment", Some(detail)),
            Self::RegistryWrite(detail) => ("failed to update the user environment", Some(detail)),
            Self::SnapshotWrite(detail) => {
                ("failed to save the environment snapshot", Some(detail))
            }
            Self::SnapshotRead(detail) => ("failed to load the environment snapshot", Some(detail)),
            Self::SnapshotMissing => ("no saved proxy environment snapshot is available", None),
            Self::BroadcastFailed(detail) => ("environment change broadcast failed", Some(detail)),
            Self::VerificationFailed(detail) => {
                ("environment update could not be verified", Some(detail))
            }
            Self::EnvironmentApplyRolledBack(detail) => (
                "environment update failed but the previous values were restored",
                Some(detail),
            ),
            Self::EnvironmentRollbackIncomplete(detail) => (
                "environment update failed and rollback was incomplete",
                Some(detail),
            ),
            Self::EnvironmentRestoreConflict(detail) => (
                "environment restore stopped because the variables were modified externally",
                Some(detail),
            ),
            Self::Detection(detail) => ("proxy detection failed", Some(detail)),
            Self::ActiveProxyMissing => ("no active proxy endpoint is available", None),
            Self::InvalidProxyEndpoint(detail) => ("invalid proxy endpoint", Some(detail)),
            Self::InvalidApplication(detail) => ("invalid application", Some(detail)),
            Self::ApplicationAuthorization(detail) => {
                ("application authorization failed", Some(detail))
            }
            Self::ApplicationLaunch(detail) => ("failed to launch application", Some(detail)),
            Self::SettingsRead(detail) => ("failed to read application settings", Some(detail)),
            Self::SettingsWrite(detail) => ("failed to save application settings", Some(detail)),
            Self::Autostart(detail) => ("failed to update launch-at-startup", Some(detail)),
            #[cfg(not(windows))]
            Self::UnsupportedPlatform => ("this operation is only available on Windows", None),
        }
    }
}

impl fmt::Display for ProxyEnvError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (message, detail) = self.public_parts();
        formatter.write_str(message)?;
        if let Some(detail) = detail {
            write!(formatter, ": {}", redaction::safe_text(detail))?;
        }
        Ok(())
    }
}

impl fmt::Debug for ProxyEnvError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ProxyEnvError")
            .field(&self.to_string())
            .finish()
    }
}

impl Error for ProxyEnvError {}

impl Serialize for ProxyEnvError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ProxyEnvError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_debug_and_serialization_redact_local_details() {
        let error = ProxyEnvError::ApplicationLaunch(
            r#"could not start "C:\Users\Alice\Apps\Code.exe" through http://name:secret@127.0.0.1:7897 (PID 4420)"#.into(),
        );

        let outputs = [
            error.to_string(),
            format!("{error:?}"),
            serde_json::to_string(&error).unwrap(),
        ];
        for output in outputs {
            for sensitive in [
                "Alice",
                "Code.exe",
                "name",
                "secret",
                "127.0.0.1",
                "7897",
                "4420",
            ] {
                assert!(!output.contains(sensitive), "leaked {sensitive}: {output}");
            }
            assert!(output.contains(redaction::REDACTED_PATH));
            assert!(output.contains(redaction::REDACTED_PROXY));
        }
    }
}
