use std::{
    ffi::OsStr,
    fs::File,
    io,
    mem::{size_of, size_of_val},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Component, Path, PathBuf, Prefix},
};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{LocalFree, HANDLE, HLOCAL},
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
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

const SECURITY_INFO: windows::Win32::Security::OBJECT_SECURITY_INFORMATION =
    windows::Win32::Security::OBJECT_SECURITY_INFORMATION(
        OWNER_SECURITY_INFORMATION.0 | DACL_SECURITY_INFORMATION.0,
    );
const DIRECTORY_SHARE: FILE_SHARE_MODE = FILE_SHARE_MODE(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0);

pub(super) fn win_error(error: windows::core::Error) -> io::Error {
    let code = error.code().0 as u32;
    if code >> 16 == 0x8007 {
        io::Error::from_raw_os_error((code & 0xffff) as i32)
    } else {
        io::Error::other(error)
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain([0]).collect()
}

pub(super) struct LocalBuffer(pub HLOCAL);

impl Drop for LocalBuffer {
    fn drop(&mut self) {
        // SAFETY: the allocation was returned by a Win32 API transferring LocalFree ownership.
        unsafe { LocalFree(Some(self.0)) };
    }
}

fn take_string(value: PWSTR) -> io::Result<String> {
    let _owned = LocalBuffer(HLOCAL(value.0.cast()));
    // SAFETY: the conversion API returned a terminated string in the owned allocation.
    unsafe { value.to_string() }.map_err(io::Error::other)
}

fn current_sid() -> io::Result<String> {
    // SAFETY: the token and the aligned token buffer outlive SID conversion.
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).map_err(win_error)?;
        let token = OwnedHandle::from_raw_handle(token.0);
        let handle = HANDLE(token.as_raw_handle());
        let mut bytes = 0;
        let _ = GetTokenInformation(handle, TokenUser, None, 0, &mut bytes);
        if (bytes as usize) < size_of::<TOKEN_USER>() {
            return Err(io::Error::other("token user size"));
        }
        let mut buffer = vec![0_usize; (bytes as usize).div_ceil(size_of::<usize>())];
        GetTokenInformation(
            handle,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            bytes,
            &mut bytes,
        )
        .map_err(win_error)?;
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut sid).map_err(win_error)?;
        take_string(sid)
    }
}

struct Descriptor(LocalBuffer);

impl Descriptor {
    fn user() -> io::Result<Self> {
        let sid = current_sid()?;
        let sddl = wide(OsStr::new(&format!("O:{sid}D:P(A;;FA;;;{sid})")));
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: sddl is terminated and the resulting descriptor is freed by LocalBuffer.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
            .map_err(win_error)?;
        }
        Ok(Self(LocalBuffer(HLOCAL(descriptor.0))))
    }

    fn of(file: &File) -> io::Result<Self> {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: the file handle and output slot are live for the call.
        unsafe {
            GetSecurityInfo(
                handle(file),
                SE_FILE_OBJECT,
                SECURITY_INFO,
                None,
                None,
                None,
                None,
                Some(&mut descriptor),
            )
            .ok()
            .map_err(win_error)?;
        }
        Ok(Self(LocalBuffer(HLOCAL(descriptor.0))))
    }

    fn text(&self) -> io::Result<String> {
        let mut result = PWSTR::null();
        // SAFETY: the descriptor is live and the API returns a newly allocated string.
        unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                PSECURITY_DESCRIPTOR(self.0 .0 .0),
                SDDL_REVISION_1,
                SECURITY_INFO,
                &mut result,
                None,
            )
            .map_err(win_error)?;
        }
        take_string(result)
    }

    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0 .0 .0,
            ..Default::default()
        }
    }
}

pub(super) fn handle(file: &File) -> HANDLE {
    HANDLE(file.as_raw_handle())
}

fn open(
    path: &Path,
    access: u32,
    share: FILE_SHARE_MODE,
    directory: bool,
    descriptor: Option<&Descriptor>,
) -> io::Result<File> {
    let path = wide(path.as_os_str());
    let attributes = descriptor.map(Descriptor::attributes);
    let attributes = attributes.as_ref().map(std::ptr::from_ref);
    let mode = if descriptor.is_some() {
        CREATE_NEW
    } else {
        OPEN_EXISTING
    };
    let mut flags = FILE_FLAG_OPEN_REPARSE_POINT;
    if directory {
        flags |= FILE_FLAG_BACKUP_SEMANTICS;
    }
    // SAFETY: path and optional descriptor are live and the returned handle is uniquely owned.
    unsafe {
        let raw = CreateFileW(
            PCWSTR(path.as_ptr()),
            access,
            share,
            attributes,
            mode,
            flags,
            None,
        )
        .map_err(win_error)?;
        Ok(File::from_raw_handle(raw.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FileId {
    pub volume: u64,
    pub object: [u8; 16],
}

fn object_info(file: &File, directory: bool) -> io::Result<FileId> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: file remains live and info is a writable output.
    unsafe { GetFileInformationByHandle(handle(file), &mut info) }.map_err(win_error)?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
        || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0) != directory
        || (!directory && info.nNumberOfLinks != 1)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected NTFS object",
        ));
    }
    let mut id = FILE_ID_INFO::default();
    // SAFETY: id is a writable output with the required size and layout.
    unsafe {
        GetFileInformationByHandleEx(
            handle(file),
            FileIdInfo,
            (&mut id as *mut FILE_ID_INFO).cast(),
            size_of_val(&id) as u32,
        )
    }
    .map_err(win_error)?;
    Ok(FileId {
        volume: id.VolumeSerialNumber,
        object: id.FileId.Identifier,
    })
}

fn leaf(name: &OsStr) -> io::Result<()> {
    let value = name.as_encoded_bytes();
    if !matches!(
        Path::new(name).components().collect::<Vec<_>>().as_slice(),
        [Component::Normal(_)]
    ) || value
        .iter()
        .any(|byte| *byte < 32 || matches!(*byte, b':' | b'/' | b'\\'))
        || matches!(value.last(), Some(b'.' | b' '))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsafe NTFS name",
        ));
    }
    Ok(())
}

fn pin_ancestors(parent: &Path) -> io::Result<Vec<File>> {
    let mut components = parent.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "absolute drive path required",
        ));
    };
    if !matches!(prefix.kind(), Prefix::Disk(_)) || components.next() != Some(Component::RootDir) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "absolute drive path required",
        ));
    }
    let mut path = PathBuf::from(prefix.as_os_str());
    path.push(Path::new("\\"));
    let mut handles = Vec::new();
    for component in std::iter::once(None).chain(components.map(Some)) {
        if let Some(component) = component {
            let Component::Normal(name) = component else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unsafe directory path",
                ));
            };
            leaf(name)?;
            path.push(name);
        }
        let directory = open(&path, FILE_READ_ATTRIBUTES.0, DIRECTORY_SHARE, true, None)?;
        object_info(&directory, true)?;
        handles.push(directory);
    }
    Ok(handles)
}

pub(super) struct PrivateDirectory {
    // These handles prevent ancestor rename and deletion while this instance uses paths.
    _ancestors: Vec<File>,
    pub(super) directory: File,
    pub(super) path: PathBuf,
    descriptor: Descriptor,
    pub(super) id: FileId,
}

impl PrivateDirectory {
    pub(super) fn new(path: &Path, create: bool, access: u32) -> io::Result<Self> {
        Self::open_tracked(path, create, access, &mut false)
    }

    pub(super) fn open_tracked(
        path: &Path,
        create: bool,
        access: u32,
        created: &mut bool,
    ) -> io::Result<Self> {
        let parent = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
        leaf(path.file_name().ok_or(io::ErrorKind::InvalidInput)?)?;
        let ancestors = pin_ancestors(parent)?;
        let descriptor = Descriptor::user()?;
        if create {
            let path = wide(path.as_os_str());
            let attributes = descriptor.attributes();
            // SAFETY: both the terminated path and its descriptor are live during creation.
            unsafe { CreateDirectoryW(PCWSTR(path.as_ptr()), Some(&attributes)) }
                .map_err(win_error)?;
            *created = true;
        }
        let directory = open(path, access, DIRECTORY_SHARE, true, None)?;
        let mut filesystem = [0_u16; 32];
        // SAFETY: directory remains live and filesystem is a writable buffer.
        unsafe {
            GetVolumeInformationByHandleW(
                handle(&directory),
                None,
                None,
                None,
                None,
                Some(&mut filesystem),
            )
        }
        .map_err(win_error)?;
        if filesystem[..5] != [78, 84, 70, 83, 0] {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "NTFS required"));
        }
        let id = Self::inspect(&directory, &descriptor, true)?;
        Ok(Self {
            _ancestors: ancestors,
            directory,
            path: path.into(),
            descriptor,
            id,
        })
    }

    fn inspect(file: &File, descriptor: &Descriptor, directory: bool) -> io::Result<FileId> {
        let id = object_info(file, directory)?;
        if Descriptor::of(file)?.text()? != descriptor.text()? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unexpected NTFS owner or DACL",
            ));
        }
        Ok(id)
    }

    pub(super) fn validate(&self, file: &File, directory: bool) -> io::Result<FileId> {
        Self::inspect(file, &self.descriptor, directory)
    }

    pub(super) fn file(
        &self,
        name: &str,
        access: u32,
        share: FILE_SHARE_MODE,
        create: bool,
    ) -> io::Result<File> {
        leaf(OsStr::new(name))?;
        let file = open(
            &self.path.join(name),
            access,
            share,
            false,
            create.then_some(&self.descriptor),
        )?;
        self.validate(&file, false)?;
        Ok(file)
    }
}

pub(super) fn delete_by_handle(file: &File) -> io::Result<()> {
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: file remains live and disposition matches the requested Win32 class.
    unsafe {
        SetFileInformationByHandle(
            handle(file),
            FileDispositionInfo,
            std::ptr::from_ref(&disposition).cast(),
            size_of_val(&disposition) as u32,
        )
    }
    .map_err(win_error)
}
