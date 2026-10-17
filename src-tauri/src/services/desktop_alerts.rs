//! Explicit desktop consent is separate from the legacy, downgrade-safe settings schema.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{ProxyEnvError, Result};

use super::local_file;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Consent {
    enabled: bool,
}

fn path() -> Result<PathBuf> {
    dirs::data_local_dir()
        .map(|directory| directory.join("ProxyEnv/routespan-desktop-alerts.v1.json"))
        .ok_or_else(|| ProxyEnvError::SettingsRead("local data directory is unavailable".into()))
}

fn read(path: &Path) -> Result<bool> {
    let Some(bytes) = local_file::safe_read(path, 1024)
        .map_err(|error| ProxyEnvError::SettingsRead(error.to_string()))?
    else {
        return Ok(false);
    };
    serde_json::from_slice::<Consent>(&bytes)
        .map(|consent| consent.enabled)
        .map_err(|error| ProxyEnvError::SettingsRead(error.to_string()))
}

pub fn load() -> Result<bool> {
    read(&path()?)
}

pub fn save(enabled: bool) -> Result<()> {
    write(&path()?, enabled)
}

fn write(path: &Path, enabled: bool) -> Result<()> {
    let bytes = serde_json::to_vec(&Consent { enabled })
        .map_err(|error| ProxyEnvError::SettingsWrite(error.to_string()))?;
    local_file::atomic_write(path, &bytes, "desktop-alerts")
        .map_err(|error| ProxyEnvError::SettingsWrite(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory() -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "routespan-alerts-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        directory
    }

    #[test]
    fn consent_is_opt_in_and_round_trips_independently() {
        let directory = directory();
        let path = directory.join("consent.json");
        assert!(!read(&path).unwrap());
        write(&path, true).unwrap();
        assert!(read(&path).unwrap());
        write(&path, false).unwrap();
        assert!(!read(&path).unwrap());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn malformed_oversized_or_unsupported_consent_never_authorizes_desktop_alerts() {
        let directory = directory();
        let path = directory.join("consent.json");
        for bytes in [
            b"".as_slice(),
            b"true",
            b"{}",
            br#"{"enabled":"true"}"#,
            br#"{"enabled":true,"unexpected":true}"#,
            &[b' '; 1025],
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(!read(&path).unwrap_or(false));
        }
        assert!(read(&directory).is_err());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
