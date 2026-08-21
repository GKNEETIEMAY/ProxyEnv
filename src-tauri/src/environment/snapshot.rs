use std::{collections::HashMap, fs, io::Write, path::PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::error::{ProxyEnvError, Result};

use super::{EnvironmentEntry, EnvironmentScope, EnvironmentSnapshot, SnapshotReason};

fn data_dir() -> Result<PathBuf> {
    dirs::data_local_dir()
        .map(|path| path.join("ProxyEnv"))
        .ok_or_else(|| ProxyEnvError::SnapshotWrite("local data directory is unavailable".into()))
}

fn snapshots_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("snapshots"))
}

fn latest_path() -> Result<PathBuf> {
    Ok(snapshots_dir()?.join("latest.json"))
}

fn legacy_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("env-snapshot.json"))
}

#[cfg(windows)]
fn replace_file(source: &std::path::Path, destination: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::PCWSTR,
        Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        },
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(std::io::Error::other)
}

#[cfg(not(windows))]
fn replace_file(source: &std::path::Path, destination: &std::path::Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

pub fn save_atomic(snapshot: &EnvironmentSnapshot) -> Result<()> {
    let directory = snapshots_dir()?;
    fs::create_dir_all(&directory)
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
    let destination = latest_path()?;
    let temporary = directory.join("latest.json.tmp");
    let bytes = serde_json::to_vec_pretty(snapshot)
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
    let mut file = fs::File::create(&temporary)
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
    replace_file(&temporary, &destination)
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))
}

pub fn load_latest() -> Result<Option<EnvironmentSnapshot>> {
    let path = latest_path().map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    if path.exists() {
        return read_snapshot(&path).map(Some);
    }
    load_legacy()
}

fn read_snapshot(path: &std::path::Path) -> Result<EnvironmentSnapshot> {
    let bytes = fs::read(path).map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySnapshot {
    created_at: DateTime<Utc>,
    values: HashMap<String, Option<String>>,
}

fn load_legacy() -> Result<Option<EnvironmentSnapshot>> {
    let path = legacy_path().map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    let legacy: LegacySnapshot = serde_json::from_slice(&bytes)
        .map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    let entries = legacy
        .values
        .into_iter()
        .map(|(name, value)| EnvironmentEntry {
            name,
            exists: value.is_some(),
            value,
            scope: EnvironmentScope::User,
        })
        .collect();
    Ok(Some(EnvironmentSnapshot {
        schema_version: 1,
        id: legacy.created_at.format("%Y%m%dT%H%M%S%.3fZ").to_string(),
        created_at: legacy.created_at,
        scope: EnvironmentScope::User,
        reason: SnapshotReason::FeatureChange,
        entries,
    }))
}
