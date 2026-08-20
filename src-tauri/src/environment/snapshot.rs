use std::{fs, io::Write, path::PathBuf};

use crate::error::{ProxyEnvError, Result};

use super::ProxyEnvironmentSnapshot;

fn data_dir() -> Result<PathBuf> {
    dirs::data_local_dir()
        .map(|path| path.join("ProxyEnv"))
        .ok_or_else(|| ProxyEnvError::SnapshotWrite("LOCALAPPDATA is unavailable".into()))
}

fn snapshot_path() -> Result<PathBuf> {
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

pub fn save_atomic(snapshot: &ProxyEnvironmentSnapshot) -> Result<()> {
    let directory = data_dir()?;
    fs::create_dir_all(&directory)
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
    let destination = snapshot_path()?;
    let temporary = directory.join("env-snapshot.json.tmp");
    let bytes = serde_json::to_vec_pretty(snapshot)
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
    let mut file = fs::File::create(&temporary)
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
    replace_file(&temporary, &destination)
        .map_err(|error| ProxyEnvError::SnapshotWrite(error.to_string()))?;
    Ok(())
}

pub fn load() -> Result<Option<ProxyEnvironmentSnapshot>> {
    let path = snapshot_path().map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| ProxyEnvError::SnapshotRead(error.to_string()))
}
