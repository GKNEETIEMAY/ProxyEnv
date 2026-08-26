use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn safe_read(path: &Path, max_bytes: u64) -> std::io::Result<Option<Vec<u8>>> {
    let Some(initial_metadata) = safe_file_metadata(path)? else {
        return Ok(None);
    };
    if initial_metadata.len() > max_bytes {
        return Err(size_limit_error());
    }

    let (mut file, opened_metadata) = open_regular_file(path)?;
    if opened_metadata.len() > max_bytes {
        return Err(size_limit_error());
    }
    let mut bytes = Vec::with_capacity(opened_metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(size_limit_error());
    }
    Ok(Some(bytes))
}

pub(crate) fn atomic_write(
    destination: &Path,
    bytes: &[u8],
    operation_id: &str,
) -> std::io::Result<()> {
    let directory = destination.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination has no parent directory",
        )
    })?;
    ensure_safe_directory(directory)?;
    let destination_metadata = safe_file_metadata(destination)?;
    let file_name = destination.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination has no filename",
        )
    })?;
    let temporary = temporary_path(directory, file_name, operation_id);

    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        if let Some(metadata) = destination_metadata {
            file.set_permissions(metadata.permissions())?;
            file.sync_all()?;
        }
        drop(file);
        replace_file(&temporary, destination)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn temporary_path(directory: &Path, file_name: &std::ffi::OsStr, operation_id: &str) -> PathBuf {
    let sequence = NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    directory.join(format!(
        ".{}.proxyenv-{operation_id}-{}-{timestamp}-{sequence}.tmp",
        file_name.to_string_lossy(),
        std::process::id()
    ))
}

pub(crate) fn ensure_safe_directory(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_safe_directory(&metadata) => Ok(()),
        Ok(_) => Err(unsafe_file_error("directory is not a safe local directory")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path)?;
            ensure_safe_directory(path)
        }
        Err(error) => Err(error),
    }
}

fn safe_file_metadata(path: &Path) -> std::io::Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_safe_regular_file(&metadata) => Ok(Some(metadata)),
        Ok(_) => Err(unsafe_file_error("path is not a safe regular file")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn open_regular_file(path: &Path) -> std::io::Result<(File, fs::Metadata)> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)?;
    let metadata = file.metadata()?;
    if !is_safe_regular_file(&metadata) {
        return Err(unsafe_file_error("opened path is not a safe regular file"));
    }
    Ok((file, metadata))
}

#[cfg(not(windows))]
fn open_regular_file(path: &Path) -> std::io::Result<(File, fs::Metadata)> {
    let file = OpenOptions::new().read(true).open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Err(unsafe_file_error("opened path is not a safe regular file"));
    }
    Ok((file, metadata))
}

fn is_safe_regular_file(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_file()
        && !metadata.file_type().is_symlink()
        && !is_windows_reparse_point(metadata)
}

fn is_safe_directory(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_dir()
        && !metadata.file_type().is_symlink()
        && !is_windows_reparse_point(metadata)
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

fn size_limit_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "local data file exceeds the size limit",
    )
}

fn unsafe_file_error(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
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
