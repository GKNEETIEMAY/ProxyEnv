use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::Deserialize;

use crate::error::{ProxyEnvError, Result};

use super::models::{
    is_managed_environment_variable, EnvironmentEntry, EnvironmentSnapshot,
    CURRENT_SNAPSHOT_SCHEMA_VERSION, MAX_ENVIRONMENT_VALUE_BYTES, MAX_SNAPSHOT_ENTRIES,
};

const MAX_SNAPSHOT_BYTES: u64 = 1024 * 1024;

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

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
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
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

pub fn save_atomic(snapshot: &EnvironmentSnapshot) -> Result<()> {
    validate(snapshot)?;
    let directory = snapshots_dir()?;
    ensure_safe_directory(&directory)?;
    let destination = latest_path()?;
    ensure_safe_destination(&destination)?;

    let nonce = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let temporary = directory.join(format!("latest.{}.{}.tmp", std::process::id(), nonce));
    let bytes = serde_json::to_vec_pretty(snapshot)
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
    if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
        return Err(ProxyEnvError::SnapshotWrite(
            "the environment snapshot exceeds the size limit".into(),
        ));
    }

    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
        ensure_safe_destination(&destination)?;
        replace_file(&temporary, &destination)
            .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

pub fn load_latest() -> Result<Option<EnvironmentSnapshot>> {
    let path = latest_path().map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    if safe_file_metadata(&path)?.is_none() {
        return Ok(None);
    }
    read_snapshot(&path)
}

pub(crate) fn validate(snapshot: &EnvironmentSnapshot) -> Result<()> {
    if snapshot.schema_version != CURRENT_SNAPSHOT_SCHEMA_VERSION {
        return Err(ProxyEnvError::SnapshotRead(format!(
            "unsupported environment snapshot schema version {}",
            snapshot.schema_version
        )));
    }
    if snapshot.id.is_empty()
        || snapshot.id.len() > 96
        || !snapshot
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
    {
        return Err(ProxyEnvError::SnapshotRead(
            "the environment snapshot id is invalid".into(),
        ));
    }
    validate_entries("before", &snapshot.before, snapshot.scope)?;
    validate_entries("applied", &snapshot.applied, snapshot.scope)?;

    let before_names = snapshot
        .before
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<HashSet<_>>();
    let applied_names = snapshot
        .applied
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<HashSet<_>>();
    if before_names != applied_names {
        return Err(ProxyEnvError::SnapshotRead(
            "the before and applied snapshot entries do not contain the same variables".into(),
        ));
    }
    Ok(())
}

fn validate_entries(
    section: &str,
    entries: &[EnvironmentEntry],
    scope: super::EnvironmentScope,
) -> Result<()> {
    if entries.is_empty() || entries.len() > MAX_SNAPSHOT_ENTRIES {
        return Err(ProxyEnvError::SnapshotRead(format!(
            "the {section} snapshot entry count is invalid"
        )));
    }
    let mut names = HashSet::new();
    for entry in entries {
        if !names.insert(entry.name.as_str()) {
            return Err(ProxyEnvError::SnapshotRead(format!(
                "the {section} snapshot contains duplicate variables"
            )));
        }
        if !is_managed_environment_variable(&entry.name) {
            return Err(ProxyEnvError::SnapshotRead(format!(
                "environment variable {} is outside the ProxyEnv allowlist",
                entry.name
            )));
        }
        if entry.scope != scope {
            return Err(ProxyEnvError::SnapshotRead(format!(
                "environment variable {} has an inconsistent scope",
                entry.name
            )));
        }
        if entry.exists != entry.value.is_some() {
            return Err(ProxyEnvError::SnapshotRead(format!(
                "environment variable {} has inconsistent existence metadata",
                entry.name
            )));
        }
        if entry
            .value
            .as_deref()
            .is_some_and(|value| value.len() > MAX_ENVIRONMENT_VALUE_BYTES || value.contains('\0'))
        {
            return Err(ProxyEnvError::SnapshotRead(format!(
                "environment variable {} contains an invalid value",
                entry.name
            )));
        }
    }
    Ok(())
}

fn read_snapshot(path: &Path) -> Result<Option<EnvironmentSnapshot>> {
    safe_file_metadata(path)?.ok_or_else(|| {
        ProxyEnvError::SnapshotRead("the environment snapshot disappeared while reading".into())
    })?;
    let (mut file, metadata) = open_snapshot_for_read(path)?;
    if metadata.len() > MAX_SNAPSHOT_BYTES {
        return Err(ProxyEnvError::SnapshotRead(
            "the environment snapshot exceeds the size limit".into(),
        ));
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_SNAPSHOT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    decode_snapshot(&bytes)
}

#[cfg(windows)]
fn open_snapshot_for_read(path: &Path) -> Result<(fs::File, fs::Metadata)> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
        .map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    if !metadata.file_type().is_file() || is_windows_reparse_point(&metadata) {
        return Err(ProxyEnvError::SnapshotRead(
            "the environment snapshot is not a safe regular file".into(),
        ));
    }
    Ok((file, metadata))
}

#[cfg(not(windows))]
fn open_snapshot_for_read(path: &Path) -> Result<(fs::File, fs::Metadata)> {
    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    if !metadata.file_type().is_file() {
        return Err(ProxyEnvError::SnapshotRead(
            "the environment snapshot is not a safe regular file".into(),
        ));
    }
    Ok((file, metadata))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotSchemaProbe {
    schema_version: u32,
}

fn decode_snapshot(bytes: &[u8]) -> Result<Option<EnvironmentSnapshot>> {
    if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
        return Err(ProxyEnvError::SnapshotRead(
            "the environment snapshot exceeds the size limit".into(),
        ));
    }
    let probe: SnapshotSchemaProbe = serde_json::from_slice(bytes)
        .map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    if probe.schema_version == 1 {
        return Ok(None);
    }
    if probe.schema_version != CURRENT_SNAPSHOT_SCHEMA_VERSION {
        return Err(ProxyEnvError::SnapshotRead(format!(
            "unsupported environment snapshot schema version {}",
            probe.schema_version
        )));
    }
    let snapshot: EnvironmentSnapshot = serde_json::from_slice(bytes)
        .map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    validate(&snapshot)?;
    Ok(Some(snapshot))
}

fn ensure_safe_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir()
                || metadata.file_type().is_symlink()
                || is_windows_reparse_point(&metadata)
            {
                return Err(ProxyEnvError::SnapshotWrite(
                    "the snapshot directory is not a safe local directory".into(),
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path)
                .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
            return ensure_safe_directory(path);
        }
        Err(error) => return Err(ProxyEnvError::SnapshotWrite(error.to_string())),
    }
    Ok(())
}

fn ensure_safe_destination(path: &Path) -> Result<()> {
    safe_file_metadata(path)
        .map(|_| ())
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))
}

fn safe_file_metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file()
                || metadata.file_type().is_symlink()
                || is_windows_reparse_point(&metadata)
            {
                return Err(ProxyEnvError::SnapshotRead(
                    "the environment snapshot is not a safe regular file".into(),
                ));
            }
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ProxyEnvError::SnapshotRead(error.to_string())),
    }
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{EnvironmentScope, SnapshotReason};

    #[cfg(windows)]
    fn names() -> [&'static str; 2] {
        ["HTTP_PROXY", "HTTPS_PROXY"]
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn names() -> [&'static str; 2] {
        ["http_proxy", "https_proxy"]
    }

    fn valid_snapshot() -> EnvironmentSnapshot {
        let names = names();
        EnvironmentSnapshot::new(
            vec![
                EnvironmentEntry {
                    name: names[0].into(),
                    value: Some("old".into()),
                    exists: true,
                    scope: EnvironmentScope::User,
                },
                EnvironmentEntry {
                    name: names[1].into(),
                    value: None,
                    exists: false,
                    scope: EnvironmentScope::User,
                },
            ],
            vec![
                EnvironmentEntry {
                    name: names[0].into(),
                    value: Some("next".into()),
                    exists: true,
                    scope: EnvironmentScope::User,
                },
                EnvironmentEntry {
                    name: names[1].into(),
                    value: Some("next".into()),
                    exists: true,
                    scope: EnvironmentScope::User,
                },
            ],
            EnvironmentScope::User,
            SnapshotReason::BeforeApply,
        )
    }

    #[test]
    fn accepts_only_current_schema_and_matching_allowlisted_entries() {
        let snapshot = valid_snapshot();
        assert!(validate(&snapshot).is_ok());

        let mut unknown_schema = snapshot.clone();
        unknown_schema.schema_version = 99;
        assert!(validate(&unknown_schema).is_err());

        let mut foreign_variable = snapshot.clone();
        foreign_variable.before[0].name = "PATH".into();
        foreign_variable.applied[0].name = "PATH".into();
        assert!(validate(&foreign_variable).is_err());

        let mut duplicate = snapshot.clone();
        duplicate.before[1].name = duplicate.before[0].name.clone();
        assert!(validate(&duplicate).is_err());

        let mut inconsistent = snapshot;
        inconsistent.applied[0].exists = false;
        assert!(validate(&inconsistent).is_err());
    }

    #[test]
    fn legacy_v1_snapshots_are_retired_without_becoming_restorable() {
        let legacy = br#"{
            "schemaVersion": 1,
            "id": "legacy",
            "createdAt": "2026-01-01T00:00:00Z",
            "scope": "user",
            "reason": "beforeApply",
            "entries": []
        }"#;
        assert!(decode_snapshot(legacy).unwrap().is_none());
    }

    #[test]
    fn rejects_unknown_schema_and_oversized_documents() {
        assert!(decode_snapshot(br#"{"schemaVersion":99}"#).is_err());
        assert!(decode_snapshot(&vec![b' '; MAX_SNAPSHOT_BYTES as usize + 1]).is_err());
    }

    #[test]
    fn rejects_unknown_fields_and_invalid_scope_values() {
        let mut document = serde_json::to_value(valid_snapshot()).unwrap();
        document
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), true.into());
        assert!(decode_snapshot(&serde_json::to_vec(&document).unwrap()).is_err());

        let mut invalid_scope = serde_json::to_value(valid_snapshot()).unwrap();
        invalid_scope["scope"] = "system".into();
        assert!(decode_snapshot(&serde_json::to_vec(&invalid_scope).unwrap()).is_err());
    }
}
