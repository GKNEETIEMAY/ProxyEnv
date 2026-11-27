use super::{
    local_files::{delete_by_handle, handle, win_error, LocalBuffer, PrivateDirectory},
    policy::lifecycle::{Decision, HistoryError, HistoryKey, PolicyHistory},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read, Write},
    mem::{offset_of, size_of},
    os::windows::ffi::OsStrExt,
    path::Path,
};
use windows::Win32::{
    Foundation::{ERROR_SHARING_VIOLATION, GENERIC_READ, GENERIC_WRITE, HLOCAL},
    Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    },
    Storage::FileSystem::{
        FileRenameInfo, FileRenameInfoEx, SetFileInformationByHandle, DELETE, FILE_RENAME_INFO,
        FILE_SHARE_DELETE, FILE_SHARE_MODE, FILE_SHARE_READ,
    },
};

const LOCK: &str = "history.lock";
const DATA: &str = "history.bin";
const MAX_BYTES: usize = 1024 * 1024;
// FILE_RENAME_INFORMATION flags for FileRenameInfoEx.
const FILE_RENAME_REPLACE_IF_EXISTS: u32 = 0x0000_0001;
const FILE_RENAME_POSIX_SEMANTICS: u32 = 0x0000_0002;
type HistoryResult<T> = Result<T, HistoryError>;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    key: HistoryKey,
    decision: Decision,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    version: u32,
    entries: Vec<Entry>,
}

pub(super) struct PolicyStorage {
    directory: PrivateDirectory,
}

impl PolicyStorage {
    /// Initializes a newly created directory. Reopening an existing store never initializes it.
    pub(super) fn initialize(path: &Path) -> HistoryResult<Self> {
        let storage = Self::directory(path, true)?;
        let _lock = storage.lock(true)?;
        storage.save(
            &Snapshot {
                version: 1,
                entries: Vec::new(),
            },
            false,
        )?;
        Ok(storage)
    }

    pub(super) fn open(path: &Path) -> HistoryResult<Self> {
        Self::directory(path, false)
    }

    fn directory(path: &Path, create: bool) -> HistoryResult<Self> {
        let directory = PrivateDirectory::new(path, create, GENERIC_READ.0)
            .map_err(|error| history_io("open directory", error))?;
        Ok(Self { directory })
    }

    fn lock(&self, create: bool) -> HistoryResult<File> {
        // An exclusive file open also serializes independently started processes.
        self.directory
            .file(LOCK, GENERIC_READ.0, FILE_SHARE_MODE(0), create)
            .map_err(|error| {
                if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION.0 as i32) {
                    #[cfg(test)]
                    eprintln!("policy history lock contention (Win32 sharing violation)");
                    HistoryError::Conflict
                } else {
                    history_io("open lock", error)
                }
            })
    }

    fn entropy(&self) -> Vec<u8> {
        [
            b"RouteSpan/windows-policy-history/v1\0".as_slice(),
            &self.directory.id.volume.to_le_bytes(),
            &self.directory.id.object,
        ]
        .concat()
    }

    fn load(&self) -> HistoryResult<Snapshot> {
        let file = self
            .directory
            .file(DATA, GENERIC_READ.0, FILE_SHARE_READ, false)
            .map_err(|error| history_io("open snapshot", error))?;
        let mut blob = Vec::new();
        file.take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut blob)
            .map_err(|error| history_io("read snapshot", error))?;
        let plain = crypt(&blob, &self.entropy(), false)?;
        let snapshot: Snapshot =
            serde_json::from_slice(&plain).map_err(|_| HistoryError::Corrupt)?;
        let mut keys = HashSet::new();
        if snapshot.version != 1
            || snapshot.entries.iter().any(|entry| {
                entry.decision.sequence == 0
                    || entry.key.review.review.is_empty()
                    || entry.key.review.revision == 0
                    || entry.key.review.deployment == 0
                    || !keys.insert(&entry.key)
            })
        {
            return Err(HistoryError::Corrupt);
        }
        Ok(snapshot)
    }

    fn stage(&self, snapshot: &Snapshot) -> HistoryResult<File> {
        let plain = serde_json::to_vec(snapshot).map_err(|_| HistoryError::Unavailable)?;
        let blob = crypt(&plain, &self.entropy(), true)?;
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| HistoryError::Unavailable)?;
        let name = format!("history-{}.stage", hex::encode(nonce));
        let mut file = self
            .directory
            .file(
                &name,
                GENERIC_READ.0 | GENERIC_WRITE.0 | DELETE.0,
                FILE_SHARE_MODE(0),
                true,
            )
            .map_err(|error| history_io("create stage", error))?;
        if let Err(error) = file.write_all(&blob).and_then(|()| file.sync_all()) {
            let _ = delete_by_handle(&file);
            return Err(history_io("flush stage", error));
        }
        Ok(file)
    }

    fn save(&self, snapshot: &Snapshot, replace: bool) -> HistoryResult<()> {
        // Keep the validated old object pinned until publication. Observers must have opted
        // into delete sharing. A reader that denies deletion still blocks this transaction.
        let _destination = if replace {
            Some(
                self.directory
                    .file(
                        DATA,
                        GENERIC_READ.0 | DELETE.0,
                        FILE_SHARE_READ | FILE_SHARE_DELETE,
                        false,
                    )
                    .map_err(|error| history_io("open snapshot for replacement", error))?,
            )
        } else {
            None
        };
        let file = self.stage(snapshot)?;
        if let Err(error) = rename(&file, &self.directory.path.join(DATA), replace) {
            let _ = delete_by_handle(&file);
            return Err(history_io("publish snapshot", error));
        }
        file.sync_all()
            .map_err(|error| history_io("flush snapshot", error))
    }
}

impl PolicyHistory for PolicyStorage {
    fn read(&self, key: &HistoryKey) -> HistoryResult<Option<Decision>> {
        let _lock = self.lock(false)?;
        Ok(self
            .load()?
            .entries
            .into_iter()
            .find(|entry| &entry.key == key)
            .map(|entry| entry.decision))
    }

    fn commit(
        &self,
        key: &HistoryKey,
        expected: Option<Decision>,
        next: Decision,
    ) -> HistoryResult<()> {
        let _lock = self.lock(false)?;
        let mut snapshot = self.load()?;
        let current = snapshot.entries.iter_mut().find(|entry| &entry.key == key);
        if current.as_ref().map(|entry| entry.decision) != expected {
            return Err(HistoryError::Conflict);
        }
        if next.sequence == 0
            || key.review.review.is_empty()
            || key.review.revision == 0
            || key.review.deployment == 0
        {
            return Err(HistoryError::Corrupt);
        }
        if let Some(entry) = current {
            let old = entry.decision;
            if next.sequence < old.sequence
                || (next.sequence == old.sequence && next != old)
                || (old.revoked && !next.revoked)
            {
                return Err(HistoryError::Conflict);
            }
            if next == old {
                return Ok(());
            }
            entry.decision = next;
        } else {
            snapshot.entries.push(Entry {
                key: key.clone(),
                decision: next,
            });
        }
        self.save(&snapshot, true)
    }
}

fn history_io(_operation: &str, error: io::Error) -> HistoryError {
    #[cfg(test)]
    eprintln!(
        "policy history {_operation}: {:?} ({:?})",
        error.kind(),
        error.raw_os_error()
    );
    match error.kind() {
        io::ErrorKind::NotFound => HistoryError::Missing,
        io::ErrorKind::InvalidData => HistoryError::Corrupt,
        _ => HistoryError::Unavailable,
    }
}

fn crypt(data: &[u8], entropy: &[u8], protect: bool) -> HistoryResult<Vec<u8>> {
    if data.is_empty() || data.len() > MAX_BYTES {
        return Err(HistoryError::Corrupt);
    }
    let blob = |bytes: &[u8]| CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let input = blob(data);
    let entropy = blob(entropy);
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: input buffers stay live, and DPAPI owns the allocation until it is transferred back.
    unsafe {
        if protect {
            CryptProtectData(
                &input,
                None,
                Some(&entropy),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                None,
                Some(&entropy),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    }
    .map_err(|_| {
        if protect {
            HistoryError::Unavailable
        } else {
            HistoryError::Corrupt
        }
    })?;
    let _owned = LocalBuffer(HLOCAL(output.pbData.cast()));
    if output.pbData.is_null() || output.cbData == 0 || output.cbData as usize > MAX_BYTES {
        return Err(HistoryError::Corrupt);
    }
    // SAFETY: DPAPI returned cbData readable bytes in the still-owned allocation.
    Ok(unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec())
}

fn rename(file: &File, destination: &Path, replace: bool) -> io::Result<()> {
    let name: Vec<u16> = destination.as_os_str().encode_wide().chain([0]).collect();
    let bytes = name.len() * size_of::<u16>();
    let length =
        (offset_of!(FILE_RENAME_INFO, FileName) + bytes).max(size_of::<FILE_RENAME_INFO>());
    let mut buffer = vec![0_usize; length.div_ceil(size_of::<usize>())];
    // SAFETY: this zeroed, aligned buffer has space for the header and complete UTF-16 name.
    unsafe {
        let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        let mode = if replace {
            (*info).Anonymous.Flags = FILE_RENAME_REPLACE_IF_EXISTS | FILE_RENAME_POSIX_SEMANTICS;
            FileRenameInfoEx
        } else {
            // The zeroed ReplaceIfExists member refuses to overwrite on initialization.
            FileRenameInfo
        };
        (*info).FileNameLength = (bytes - size_of::<u16>()) as u32;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            std::ptr::addr_of_mut!((*info).FileName).cast(),
            name.len(),
        );
        SetFileInformationByHandle(handle(file), mode, info.cast(), length as u32)
    }
    .map_err(win_error)
}

#[cfg(test)]
#[path = "tests/policy_storage.rs"]
mod tests;
