use super::{blob, FrozenProfile, HostTrustEvidence};
use crate::features::remote_bridge::base64_bytes;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Seek, SeekFrom, Write},
    mem::{size_of, size_of_val},
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Output},
};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            LocalFree, ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION, GENERIC_READ, GENERIC_WRITE,
            HANDLE, HLOCAL,
        },
        Security::{
            Authorization::{
                ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
                ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
                SDDL_REVISION_1, SE_FILE_OBJECT,
            },
            GetTokenInformation, TokenUser, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
            PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        },
        Storage::FileSystem::*,
        System::Threading::{GetCurrentProcess, OpenProcessToken, CREATE_NO_WINDOW},
    },
};

const SHARED_READ_WRITE: FILE_SHARE_MODE = FILE_SHARE_MODE(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0);
const SECURITY_INFO: windows::Win32::Security::OBJECT_SECURITY_INFORMATION =
    windows::Win32::Security::OBJECT_SECURITY_INFORMATION(
        OWNER_SECURITY_INFORMATION.0 | DACL_SECURITY_INFORMATION.0,
    );

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain([0]).collect()
}

struct LocalBuffer(HLOCAL);

impl Drop for LocalBuffer {
    fn drop(&mut self) {
        // SAFETY: each buffer is returned by a Win32 API that requires LocalFree.
        unsafe { LocalFree(Some(self.0)) };
    }
}

fn take_string(text: PWSTR) -> String {
    let _buffer = LocalBuffer(HLOCAL(text.0.cast()));
    // SAFETY: the Win32 conversion returned a terminated string in this buffer.
    unsafe { text.to_string().unwrap() }
}

fn current_sid() -> String {
    // SAFETY: the token and the aligned TOKEN_USER buffer live until SID conversion.
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).unwrap();
        let owned_token = OwnedHandle::from_raw_handle(token.0);
        let mut bytes = 0;
        let token = HANDLE(owned_token.as_raw_handle());
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut bytes);
        assert!(bytes as usize >= size_of::<TOKEN_USER>());
        let mut buffer = vec![0_usize; (bytes as usize).div_ceil(size_of::<usize>())];
        GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            bytes,
            &mut bytes,
        )
        .unwrap();
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut sid).unwrap();
        take_string(sid)
    }
}

struct Descriptor {
    memory: LocalBuffer,
}

impl Descriptor {
    fn new(sddl: &str) -> Self {
        let input = wide(OsStr::new(sddl));
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: input is terminated and the returned allocation is owned here.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(input.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
            .unwrap();
        }
        Self {
            memory: LocalBuffer(HLOCAL(descriptor.0)),
        }
    }

    fn from_file(file: &File) -> Self {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: the read handle is live and the returned allocation is owned here.
        unsafe {
            GetSecurityInfo(
                HANDLE(file.as_raw_handle()),
                SE_FILE_OBJECT,
                SECURITY_INFO,
                None,
                None,
                None,
                None,
                Some(&mut descriptor),
            )
            .ok()
            .unwrap();
        }
        Self {
            memory: LocalBuffer(HLOCAL(descriptor.0)),
        }
    }

    fn text(&self) -> String {
        let mut text = PWSTR::null();
        // SAFETY: the source descriptor remains live and the output is newly allocated.
        unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                PSECURITY_DESCRIPTOR(self.memory.0 .0),
                SDDL_REVISION_1,
                SECURITY_INFO,
                &mut text,
                None,
            )
            .unwrap();
        }
        take_string(text)
    }

    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.memory.0 .0,
            ..Default::default()
        }
    }
}

struct Scratch {
    path: PathBuf,
    descriptor: Descriptor,
    sid: String,
    files: Vec<PathBuf>,
    directories: Vec<PathBuf>,
}

impl Scratch {
    fn new() -> Self {
        let sid = current_sid();
        let descriptor = Descriptor::new(&format!("O:{sid}D:P(A;;FA;;;{sid})"));
        let mut nonce = [0_u8; 12];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::current_dir()
            .unwrap()
            .join(format!("RouteSpan frozen {}", hex::encode(nonce)));
        create_dir(&path, &descriptor);
        let result = Self {
            path,
            descriptor,
            sid,
            files: Vec::new(),
            directories: Vec::new(),
        };
        let root = open(&result.path, SHARED_READ_WRITE, true).unwrap();
        let mut filesystem = [0_u16; 32];
        // SAFETY: root owns a live directory handle and filesystem is writable.
        unsafe {
            GetVolumeInformationByHandleW(
                HANDLE(root.as_raw_handle()),
                None,
                None,
                None,
                None,
                Some(&mut filesystem),
            )
            .unwrap();
        }
        assert_eq!(
            &filesystem[..5],
            &[b'N' as u16, b'T' as u16, b'F' as u16, b'S' as u16, 0]
        );
        assert!(inspect(&root, &result.descriptor, true).is_ok());
        result
    }

    fn directory(&mut self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        create_dir(&path, &self.descriptor);
        self.directories.push(path.clone());
        path
    }

    fn file(&mut self, name: &str, bytes: &[u8]) -> File {
        let path = self.path.join(name);
        self.files.push(path.clone());
        create_file(&path, bytes, &self.descriptor)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let mut cleanup_error = None;
        let mut record = |result: std::io::Result<()>| {
            if let Some(err) = result.err().filter(|err| err.kind() != ErrorKind::NotFound) {
                cleanup_error.get_or_insert(err);
            }
        };
        for path in self.files.iter().rev() {
            record(fs::remove_file(path));
        }
        for path in self.directories.iter().rev() {
            // RemoveDirectory removes the junction itself. It never traverses it.
            record(fs::remove_dir(path));
        }
        record(fs::remove_dir(&self.path));
        if !std::thread::panicking() {
            assert!(
                cleanup_error.is_none(),
                "fixture cleanup: {cleanup_error:?}"
            );
        }
    }
}

fn create_dir(path: &Path, descriptor: &Descriptor) {
    let path = wide(path.as_os_str());
    let attributes = descriptor.attributes();
    // SAFETY: the terminated path and its security descriptor are live during creation.
    unsafe { CreateDirectoryW(PCWSTR(path.as_ptr()), Some(&attributes)).unwrap() };
}

fn create_file(path: &Path, bytes: &[u8], descriptor: &Descriptor) -> File {
    let path = wide(path.as_os_str());
    let attributes = descriptor.attributes();
    // SAFETY: the path and descriptor are live, and the returned handle is uniquely owned.
    let mut file = unsafe {
        let handle = CreateFileW(
            PCWSTR(path.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ,
            Some(&attributes),
            CREATE_NEW,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
        .unwrap();
        File::from_raw_handle(handle.0)
    };
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    file
}

fn open(path: &Path, share: FILE_SHARE_MODE, directory: bool) -> std::io::Result<File> {
    let flags = if directory {
        FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS
    } else {
        FILE_FLAG_OPEN_REPARSE_POINT
    };
    OpenOptions::new()
        .read(true)
        .share_mode(share.0)
        .custom_flags(flags.0)
        .open(path)
}

#[derive(Debug, Eq, PartialEq)]
struct FileId(u64, [u8; 16]);

fn inspect(file: &File, expected: &Descriptor, directory: bool) -> Result<FileId, &'static str> {
    let handle = HANDLE(file.as_raw_handle());
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: info is writable and the handle remains live.
    unsafe { GetFileInformationByHandle(handle, &mut info).unwrap() };
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err("reparse point");
    }
    if (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0) != directory {
        return Err("object type");
    }
    if !directory && info.nNumberOfLinks != 1 {
        return Err("link count");
    }
    if Descriptor::from_file(file).text() != expected.text() {
        return Err("ACL");
    }
    let mut id = FILE_ID_INFO::default();
    // SAFETY: the call writes at most the size of id through a live handle.
    unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            (&mut id as *mut FILE_ID_INFO).cast(),
            size_of_val(&id) as u32,
        )
        .unwrap();
    }
    Ok(FileId(id.VolumeSerialNumber, id.FileId.Identifier))
}

fn check_bytes(
    mut reader: &File,
    descriptor: &Descriptor,
    id: &FileId,
    expected: &[u8],
) -> Result<(), &'static str> {
    if inspect(reader, descriptor, false)? != *id {
        return Err("file ID");
    }
    // Read one extra byte so the fixture rejects extensions as well as prefixes.
    let mut current = Vec::new();
    reader.seek(SeekFrom::Start(0)).unwrap();
    (&mut reader)
        .take((expected.len() + 1) as u64)
        .read_to_end(&mut current)
        .unwrap();
    if current != expected {
        return Err("file bytes");
    }
    Ok(())
}

fn denied(error: std::io::Error) {
    assert!(
        matches!(error.raw_os_error(), Some(code) if code == ERROR_ACCESS_DENIED.0 as i32 || code == ERROR_SHARING_VIOLATION.0 as i32),
        "{error}"
    );
}

fn system(relative: &str) -> Command {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let mut command = Command::new(PathBuf::from(root).join("System32").join(relative));
    command.creation_flags(CREATE_NO_WINDOW.0);
    command
}

fn require_success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn ntfs_fixture_rejects_broader_acl_hardlink_and_junction() {
    let mut scratch = Scratch::new();
    let path = scratch.path.join("broad");
    let sid = &scratch.sid;
    let broad = Descriptor::new(&format!("O:{sid}D:P(A;;FA;;;{sid})(A;;FR;;;AU)"));
    let reader = create_file(&path, b"synthetic", &broad);
    scratch.files.push(path);
    assert_eq!(inspect(&reader, &scratch.descriptor, false), Err("ACL"));
    drop(reader);

    drop(scratch.file("original", b"synthetic"));
    let alias = scratch.path.join("hardlink");
    fs::hard_link(scratch.path.join("original"), &alias).unwrap();
    scratch.files.push(alias.clone());
    let linked = open(&alias, FILE_SHARE_READ, false).unwrap();
    assert_eq!(
        inspect(&linked, &scratch.descriptor, false),
        Err("link count")
    );

    let target = scratch.directory("real parent");
    let junction = scratch.path.join("junction");
    scratch.directories.push(junction.clone());
    require_success(
        system("WindowsPowerShell/v1.0/powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:ROUTESPAN_FIXTURE_JUNCTION -Target $env:ROUTESPAN_FIXTURE_TARGET | Out-Null",
            ])
            .env("ROUTESPAN_FIXTURE_JUNCTION", &junction)
            .env("ROUTESPAN_FIXTURE_TARGET", &target)
            .output()
            .unwrap(),
    );
    let ancestor = open(&junction, SHARED_READ_WRITE, true).unwrap();
    assert_eq!(
        inspect(&ancestor, &scratch.descriptor, true),
        Err("reparse point")
    );
}

#[test]
fn ntfs_fixture_rechecks_identity_and_bytes_after_writer_handoff() {
    let mut scratch = Scratch::new();
    let parent = scratch.directory("parent");
    let parent_guard = open(&parent, SHARED_READ_WRITE, true).unwrap();
    assert!(inspect(&parent_guard, &scratch.descriptor, true).is_ok());
    let other_parent = scratch.path.join("other parent");
    scratch.directories.push(other_parent.clone());
    denied(fs::rename(&parent, &other_parent).unwrap_err());

    let path = scratch.path.join("freeze");
    let writer = scratch.file("freeze", b"expected");
    let id = inspect(&writer, &scratch.descriptor, false).unwrap();
    let anchor = open(&path, SHARED_READ_WRITE, false).unwrap();
    assert_eq!(inspect(&anchor, &scratch.descriptor, false).unwrap(), id);
    denied(open(&path, FILE_SHARE_READ, false).unwrap_err());
    drop(writer);
    let moved = scratch.path.join("original moved");
    scratch.files.push(moved.clone());
    denied(fs::rename(&path, &moved).unwrap_err());

    let mut changed = OpenOptions::new()
        .write(true)
        .share_mode(FILE_SHARE_READ.0)
        .open(&path)
        .unwrap();
    changed.write_all(b"modified").unwrap();
    denied(open(&path, FILE_SHARE_READ, false).unwrap_err());
    drop(changed);
    let frozen = open(&path, FILE_SHARE_READ, false).unwrap();
    assert_eq!(
        check_bytes(&frozen, &scratch.descriptor, &id, b"expected"),
        Err("file bytes")
    );
    denied(OpenOptions::new().write(true).open(&path).unwrap_err());
    denied(fs::rename(&path, &moved).unwrap_err());

    drop((frozen, anchor));
    fs::rename(&path, &moved).unwrap();
    drop(scratch.file("freeze", b"expected"));
    let replacement = open(&path, FILE_SHARE_READ, false).unwrap();
    assert_eq!(
        check_bytes(&replacement, &scratch.descriptor, &id, b"expected"),
        Err("file ID")
    );
}

#[test]
fn ntfs_fixture_opens_frozen_profile_in_windows_openssh_offline() {
    let mut scratch = Scratch::new();
    let directory = scratch.path.to_str().unwrap().replace('\\', "/");
    let (host, port) = ("fixture.invalid", 2222);
    let key = blob(0x55);
    let revoke = blob(0x66);
    let label = format!("[{host}]:{port}");
    let known = format!("{label} ssh-ed25519 {}", base64_bytes(&key));
    let profile = FrozenProfile::build(
        (host, "fixture", port),
        Sha256::digest(&key).into(),
        HostTrustEvidence {
            reviewed_public_blob: &key,
            known_hosts: Some(&known),
            revoked_blobs: Some(&[&revoke]),
        },
        &directory,
    )
    .unwrap();

    let directory_guard = open(&scratch.path, SHARED_READ_WRITE, true).unwrap();
    let volume = inspect(&directory_guard, &scratch.descriptor, true)
        .unwrap()
        .0;
    let mut readers = Vec::new();
    for (name, bytes) in [
        ("ssh_config", profile.config.as_bytes()),
        ("known_hosts", profile.known_hosts.as_bytes()),
        ("revoked_hosts", profile.revoked_hosts.as_bytes()),
    ] {
        let writer = scratch.file(name, bytes);
        let path = scratch.path.join(name);
        let id = inspect(&writer, &scratch.descriptor, false).unwrap();
        assert_eq!(id.0, volume);
        let anchor = open(&path, SHARED_READ_WRITE, false).unwrap();
        assert_eq!(inspect(&anchor, &scratch.descriptor, false).unwrap(), id);
        drop(writer);
        let frozen = open(&path, FILE_SHARE_READ, false).unwrap();
        check_bytes(&frozen, &scratch.descriptor, &id, bytes).unwrap();
        readers.push(frozen);
    }

    let parse_config = || {
        system("OpenSSH/ssh.exe")
            .args(["-G", "-T", "-F", &profile.config_path, "routespan-forward"])
            .output()
            .unwrap()
    };
    let output = require_success(parse_config());
    for option in [
        "hostname fixture.invalid".to_owned(),
        "port 2222".to_owned(),
        format!("userknownhostsfile {directory}/known_hosts"),
        format!("revokedhostkeys {directory}/revoked_hosts"),
    ] {
        assert!(
            output.lines().any(|line| line.trim_matches('\r') == option),
            "{option}"
        );
    }
    assert!(!output
        .lines()
        .any(|line| line.starts_with("remoteforward ")));
    let found = require_success(
        system("OpenSSH/ssh-keygen.exe")
            .args(["-F", &label, "-f"])
            .arg(scratch.path.join("known_hosts"))
            .output()
            .unwrap(),
    );
    assert!(found.contains(&known));
    let fingerprint = require_success(
        system("OpenSSH/ssh-keygen.exe")
            .args(["-l", "-f"])
            .arg(scratch.path.join("revoked_hosts"))
            .output()
            .unwrap(),
    );
    assert!(fingerprint.contains("SHA256:"));
    drop(readers);

    let exclusive = open(&scratch.path.join("ssh_config"), FILE_SHARE_MODE(0), false).unwrap();
    assert!(!parse_config().status.success());
    drop(exclusive);
}
