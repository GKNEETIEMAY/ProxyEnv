use super::{
    frozen::{FrozenFiles, FrozenProfile},
    local_files::{delete_by_handle, FileId, PrivateDirectory},
};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};
use windows::Win32::{
    Foundation::{GENERIC_READ, GENERIC_WRITE},
    Storage::FileSystem::{DELETE, FILE_SHARE_MODE, FILE_SHARE_READ, FILE_SHARE_WRITE},
};

const FILES: [&str; 3] = ["ssh_config", "known_hosts", "revoked_hosts"];

struct OwnedFile {
    name: &'static str,
    id: FileId,
    reader: Option<File>,
}

#[derive(Debug)]
pub(super) struct PendingCleanup {
    pub(super) name: &'static str,
    pub(super) id: Option<FileId>,
    pub(super) error: io::Error,
}

#[derive(Debug)]
pub(super) struct CreationError {
    pub(super) error: io::Error,
    pub(super) pending: Vec<PendingCleanup>,
}

pub(super) struct FrozenResources {
    directory: Option<PrivateDirectory>,
    profile: FrozenProfile,
    files: Vec<OwnedFile>,
}

impl FrozenResources {
    pub(super) fn create(path: &Path, profile: FrozenProfile) -> Result<Self, CreationError> {
        let mut resources = Self::new(path, profile)?;
        if let Err(error) = resources.populate().and_then(|()| resources.validate()) {
            let pending = resources.close();
            return Err(CreationError { error, pending });
        }
        Ok(resources)
    }

    fn new(path: &Path, profile: FrozenProfile) -> Result<Self, CreationError> {
        if !path.to_str().is_some_and(|path| {
            profile.config_path == format!("{}/{}", path.replace('\\', "/"), FILES[0])
        }) {
            return Err(CreationError {
                error: io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "frozen profile directory mismatch",
                ),
                pending: Vec::new(),
            });
        }
        let mut created = false;
        let directory =
            PrivateDirectory::open_tracked(path, true, GENERIC_READ.0 | DELETE.0, &mut created)
                .map_err(|error| {
                    let pending = created
                        .then(|| PendingCleanup {
                            name: ".",
                            id: None,
                            error: io::Error::from(error.kind()),
                        })
                        .into_iter()
                        .collect();
                    CreationError { error, pending }
                })?;
        Ok(Self {
            directory: Some(directory),
            profile,
            files: Vec::new(),
        })
    }

    fn populate(&mut self) -> io::Result<()> {
        let directory = self.directory.as_ref().ok_or(io::ErrorKind::NotFound)?;
        for (name, bytes) in FILES.into_iter().zip([
            self.profile.config.as_bytes(),
            self.profile.known_hosts.as_bytes(),
            self.profile.revoked_hosts.as_bytes(),
        ]) {
            let mut writer = directory.file(
                name,
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_READ,
                true,
            )?;
            let id = directory.validate(&writer, false)?;
            self.files.push(OwnedFile {
                name,
                id,
                reader: None,
            });
            writer.write_all(bytes)?;
            writer.sync_all()?;

            // The anchor forbids replacement while the write handle is exchanged for a read handle.
            let anchor = directory.file(
                name,
                GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                false,
            )?;
            if directory.validate(&anchor, false)? != id {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "frozen anchor replaced",
                ));
            }
            drop(writer);
            let reader = directory.file(name, GENERIC_READ.0, FILE_SHARE_READ, false)?;
            if directory.validate(&reader, false)? != id {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "frozen reader replaced",
                ));
            }
            self.files.last_mut().ok_or(io::ErrorKind::NotFound)?.reader = Some(reader);
        }
        Ok(())
    }

    pub(super) fn validate(&mut self) -> io::Result<()> {
        let directory = self.directory.as_ref().ok_or(io::ErrorKind::NotFound)?;
        if directory.validate(&directory.directory, true)? != directory.id
            || self.files.len() != FILES.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frozen directory incomplete",
            ));
        }
        let mut bytes = [Vec::new(), Vec::new(), Vec::new()];
        for ((file, output), limit) in self.files.iter_mut().zip(&mut bytes).zip([
            self.profile.config.len(),
            self.profile.known_hosts.len(),
            self.profile.revoked_hosts.len(),
        ]) {
            let reader = file.reader.as_mut().ok_or(io::ErrorKind::NotFound)?;
            if directory.validate(reader, false)? != file.id
                || file.id.volume != directory.id.volume
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "frozen file identity changed",
                ));
            }
            reader.seek(SeekFrom::Start(0))?;
            reader.take((limit + 1) as u64).read_to_end(output)?;
        }
        self.profile
            .check_files(FrozenFiles {
                config: &bytes[0],
                known_hosts: &bytes[1],
                revoked_hosts: &bytes[2],
            })
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "frozen profile bytes changed"))
    }

    #[must_use]
    pub(super) fn close(&mut self) -> Vec<PendingCleanup> {
        let Some(directory) = self.directory.as_ref() else {
            return Vec::new();
        };
        let mut pending = Vec::new();
        self.files.retain_mut(|entry| {
            entry.reader.take();
            let removed = directory
                .file(
                    entry.name,
                    GENERIC_READ.0 | DELETE.0,
                    FILE_SHARE_MODE(0),
                    false,
                )
                .and_then(|file| {
                    if directory.validate(&file, false)? != entry.id {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "cleanup file identity changed",
                        ));
                    }
                    delete_by_handle(&file)
                });
            if let Err(error) = removed {
                pending.push(PendingCleanup {
                    name: entry.name,
                    id: Some(entry.id),
                    error,
                });
                return true;
            }
            false
        });
        if self.files.is_empty() {
            let removed = directory
                .validate(&directory.directory, true)
                .and_then(|id| {
                    if id != directory.id {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "cleanup directory identity changed",
                        ));
                    }
                    delete_by_handle(&directory.directory)
                });
            match removed {
                Ok(()) => self.directory = None,
                Err(error) => pending.push(PendingCleanup {
                    name: ".",
                    id: Some(directory.id),
                    error,
                }),
            }
        }
        pending
    }
}

impl Drop for FrozenResources {
    fn drop(&mut self) {
        for pending in self.close() {
            tracing::warn!(resource = pending.name, inode = ?pending.id, kind = ?pending.error.kind(),
                "frozen resource cleanup pending");
        }
    }
}

#[cfg(test)]
#[path = "tests/frozen_resources.rs"]
mod tests;
