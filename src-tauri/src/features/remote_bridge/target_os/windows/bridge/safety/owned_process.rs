use super::local_files::win_error;
use std::{
    ffi::OsStr,
    io::{self, PipeReader, PipeWriter},
    mem::{size_of, size_of_val},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    thread,
    time::{Duration, Instant},
};
use windows::{
    core::{BOOL, PCWSTR, PWSTR},
    Win32::{
        Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, WAIT_OBJECT_0},
        System::{JobObjects::*, Threading::*},
    },
};

pub(super) const PROCESS_WAIT_MS: u32 = 5000;

pub(super) fn raw(handle: &OwnedHandle) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}

struct Unassigned(Option<OwnedHandle>);

impl Drop for Unassigned {
    fn drop(&mut self) {
        if let Some(child) = &self.0 {
            // SAFETY: this handle is the exact child we created suspended. It never ran.
            let (termination, wait) = unsafe {
                (
                    TerminateProcess(raw(child), 1),
                    WaitForSingleObject(raw(child), PROCESS_WAIT_MS),
                )
            };
            if termination.is_err() || wait != WAIT_OBJECT_0 {
                tracing::warn!(?termination, ?wait, "suspended child cleanup incomplete");
            }
        }
    }
}

struct InheritedPipes<'a> {
    list: LPPROC_THREAD_ATTRIBUTE_LIST,
    _storage: Vec<usize>,
    _borrowed: &'a [HANDLE; 3],
}

impl<'a> InheritedPipes<'a> {
    fn new(handles: &'a [HANDLE; 3]) -> io::Result<Self> {
        // SAFETY: the three child pipe ends and the aligned list storage outlive the attribute list.
        unsafe {
            for handle in handles {
                SetHandleInformation(*handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT)
                    .map_err(win_error)?;
            }
            let mut bytes = 0;
            let _ = InitializeProcThreadAttributeList(None, 1, None, &mut bytes);
            if bytes == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut storage = vec![0_usize; bytes.div_ceil(size_of::<usize>())];
            let list = LPPROC_THREAD_ATTRIBUTE_LIST(storage.as_mut_ptr().cast());
            InitializeProcThreadAttributeList(Some(list), 1, None, &mut bytes)
                .map_err(win_error)?;
            let attributes = Self {
                list,
                _storage: storage,
                _borrowed: handles,
            };
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(handles.as_ptr().cast()),
                size_of_val(handles),
                None,
                None,
            )
            .map_err(win_error)?;
            Ok(attributes)
        }
    }
}

impl Drop for InheritedPipes<'_> {
    fn drop(&mut self) {
        // SAFETY: this initialized list is uniquely owned and its storage is still live.
        unsafe { DeleteProcThreadAttributeList(self.list) };
    }
}

pub(super) struct ParentPipes {
    pub(super) input: PipeWriter,
    pub(super) output: PipeReader,
    pub(super) error: PipeReader,
}

struct Child {
    job: OwnedHandle,
    process: OwnedHandle,
    primary: Option<OwnedHandle>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CloseReport {
    pub(super) process_exited: bool,
    pub(super) job_empty: bool,
    pub(super) termination_error: Option<i32>,
}

pub(super) struct OwnedProcess {
    child: Option<Child>,
    pipes: Option<ParentPipes>,
    closed: Option<CloseReport>,
}

impl OwnedProcess {
    fn spawn_suspended(exe: &OsStr, command: &mut [u16]) -> io::Result<Self> {
        let application: Vec<_> = exe.encode_wide().chain([0]).collect();
        let (child_input, input) = io::pipe()?;
        let (output, child_output) = io::pipe()?;
        let (error, child_error) = io::pipe()?;
        let child_handles = [
            HANDLE(child_input.as_raw_handle()),
            HANDLE(child_output.as_raw_handle()),
            HANDLE(child_error.as_raw_handle()),
        ];
        let attributes = InheritedPipes::new(&child_handles)?;
        let mut info = PROCESS_INFORMATION::default();
        let startup = STARTUPINFOEXW {
            StartupInfo: STARTUPINFOW {
                cb: size_of::<STARTUPINFOEXW>() as u32,
                dwFlags: STARTF_USESTDHANDLES,
                hStdInput: child_handles[0],
                hStdOutput: child_handles[1],
                hStdError: child_handles[2],
                ..Default::default()
            },
            lpAttributeList: attributes.list,
        };
        // SAFETY: the writable command and application strings are terminated, the inherited
        // ends are live, and each handle from a successful call is transferred to a unique owner.
        let child = unsafe {
            let job = CreateJobObjectW(None, None).map_err(win_error)?;
            let job = OwnedHandle::from_raw_handle(job.0);
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                raw(&job),
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                size_of_val(&limits) as u32,
            )
            .map_err(win_error)?;
            CreateProcessW(
                PCWSTR(application.as_ptr()),
                Some(PWSTR(command.as_mut_ptr())),
                None,
                None,
                true,
                CREATE_SUSPENDED | CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT,
                None,
                PCWSTR::null(),
                &startup.StartupInfo,
                &mut info,
            )
            .map_err(win_error)?;
            let mut guard = Unassigned(Some(OwnedHandle::from_raw_handle(info.hProcess.0)));
            let primary = OwnedHandle::from_raw_handle(info.hThread.0);
            AssignProcessToJobObject(raw(&job), info.hProcess).map_err(win_error)?;
            let mut member = BOOL::default();
            IsProcessInJob(info.hProcess, Some(raw(&job)), &mut member).map_err(win_error)?;
            if !member.as_bool() {
                return Err(io::Error::other("suspended child is outside its Job"));
            }
            Child {
                job,
                process: guard
                    .0
                    .take()
                    .ok_or_else(|| io::Error::other("lost child handle"))?,
                primary: Some(primary),
            }
        };
        Ok(Self {
            child: Some(child),
            pipes: Some(ParentPipes {
                input,
                output,
                error,
            }),
            closed: None,
        })
    }

    #[cfg(test)]
    pub(super) fn start_fixture(test: &str) -> io::Result<Self> {
        let exe = std::env::current_exe()?;
        let mut command = vec![u16::from(b'"')];
        command.extend(exe.as_os_str().encode_wide());
        command.extend("\" --exact ".encode_utf16());
        command.extend(test.encode_utf16());
        command.extend(" --ignored --nocapture\0".encode_utf16());
        Self::spawn_suspended(exe.as_os_str(), &mut command)
    }

    pub(super) fn resume(&mut self) -> io::Result<()> {
        let primary = self
            .child
            .as_mut()
            .and_then(|child| child.primary.take())
            .ok_or_else(|| io::Error::other("suspended child unavailable"))?;
        // SAFETY: the private thread handle belongs to the suspended child assigned to the Job.
        if unsafe { ResumeThread(raw(&primary)) } != 1 {
            self.close();
            return Err(io::Error::other("child did not resume from one suspension"));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn witness(&self) -> io::Result<(OwnedHandle, ParentPipes)> {
        let child = self.child.as_ref().ok_or(io::ErrorKind::NotFound)?;
        let pipes = self.pipes.as_ref().ok_or(io::ErrorKind::NotFound)?;
        Ok((
            child.process.try_clone()?,
            ParentPipes {
                input: pipes.input.try_clone()?,
                output: pipes.output.try_clone()?,
                error: pipes.error.try_clone()?,
            },
        ))
    }

    pub(super) fn close(&mut self) -> CloseReport {
        if let Some(report) = self.closed {
            return report;
        }
        self.pipes.take();
        let mut report = CloseReport {
            process_exited: false,
            job_empty: false,
            termination_error: None,
        };
        if let Some(child) = self.child.take() {
            let deadline = Instant::now() + Duration::from_millis(u64::from(PROCESS_WAIT_MS));
            // SAFETY: this Job and process are exclusively the handles returned by spawn_suspended.
            unsafe {
                report.termination_error = TerminateJobObject(raw(&child.job), 1)
                    .err()
                    .map(|error| error.code().0);
                report.process_exited =
                    WaitForSingleObject(raw(&child.process), PROCESS_WAIT_MS) == WAIT_OBJECT_0;
            }
            report.job_empty = wait_empty(&child.job, deadline);
            // Dropping the private Job handle also requests kill-on-close as a fallback.
        }
        self.closed = Some(report);
        report
    }
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        let report = self.close();
        if !report.process_exited || !report.job_empty || report.termination_error.is_some() {
            tracing::warn!(?report, "local Job cleanup incomplete");
        }
    }
}

fn wait_empty(job: &OwnedHandle, deadline: Instant) -> bool {
    loop {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: the buffer is writable and matches the requested class for this live Job.
        let result = unsafe {
            QueryInformationJobObject(
                Some(raw(job)),
                JobObjectBasicAccountingInformation,
                std::ptr::from_mut(&mut info).cast(),
                size_of_val(&info) as u32,
                None,
            )
        };
        if result.is_err() {
            return false;
        }
        if info.ActiveProcesses == 0 {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
pub(super) fn check_failed_fixture_launch() {
    let path = std::env::current_dir()
        .unwrap()
        .join("<invalid-fixture-image>.exe");
    let Err(error) = OwnedProcess::spawn_suspended(path.as_os_str(), &mut [0]) else {
        panic!("unexpected fixture image");
    };
    assert_eq!(
        error.raw_os_error(),
        Some(windows::Win32::Foundation::ERROR_INVALID_NAME.0 as i32)
    );
}
