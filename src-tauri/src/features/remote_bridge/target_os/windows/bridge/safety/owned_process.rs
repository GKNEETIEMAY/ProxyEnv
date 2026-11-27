use super::local_files::win_error;
use super::runtime::local::managed::ManagedLaunch;
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
        Foundation::{
            SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        System::{JobObjects::*, Threading::*},
    },
};

#[cfg(test)]
#[path = "owned_process/shell_fixture.rs"]
pub(super) mod shell_fixture;

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
    pub(super) forced: bool,
    pub(super) termination_error: Option<i32>,
    pub(super) timed_out: bool,
}

pub(super) struct OwnedProcess {
    child: Option<Child>,
    pipes: Option<ParentPipes>,
    closed: Option<CloseReport>,
}

impl OwnedProcess {
    fn spawn_suspended(
        exe: &OsStr,
        command: &mut [u16],
        launch: Option<&ManagedLaunch<'_>>,
    ) -> io::Result<Self> {
        let application: Vec<_> = exe.encode_wide().chain([0]).collect();
        let directory = launch.map(|launch| {
            launch
                .directory
                .as_os_str()
                .encode_wide()
                .chain([0])
                .collect::<Vec<_>>()
        });
        let mut flags = CREATE_SUSPENDED | CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT;
        let environment = launch.map(|launch| {
            flags |= CREATE_UNICODE_ENVIRONMENT;
            launch.environment.block().as_ptr().cast()
        });
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
        // SAFETY: the command, application, optional Unicode block and directory remain live
        // through creation. Successful process handles and the pipe ends have unique owners.
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
                flags,
                environment,
                directory
                    .as_ref()
                    .map_or(PCWSTR::null(), |path| PCWSTR(path.as_ptr())),
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
        Self::start_fixture_with_environment(test, None)
    }

    #[cfg(test)]
    pub(super) fn start_fixture_with_environment(
        test: &str,
        launch: Option<&ManagedLaunch<'_>>,
    ) -> io::Result<Self> {
        let exe = std::env::current_exe()?;
        let mut command = vec![u16::from(b'"')];
        command.extend(exe.as_os_str().encode_wide());
        command.extend("\" --exact ".encode_utf16());
        command.extend(test.encode_utf16());
        command.extend(" --ignored --nocapture\0".encode_utf16());
        Self::spawn_suspended(exe.as_os_str(), &mut command, launch)
    }

    #[cfg(test)]
    pub(super) fn invalid_fixture() -> io::Result<Self> {
        let path = std::env::current_dir()?.join("<invalid-fixture-image>.exe");
        Self::spawn_suspended(path.as_os_str(), &mut [0], None)
    }

    pub(super) fn resume(&mut self) -> io::Result<()> {
        let primary = self
            .child
            .as_mut()
            .and_then(|child| child.primary.take())
            .ok_or_else(|| io::Error::other("suspended child unavailable"))?;
        // SAFETY: the private thread handle belongs to the suspended child assigned to the Job.
        if unsafe { ResumeThread(raw(&primary)) } != 1 {
            return Err(io::Error::other("child did not resume from one suspension"));
        }
        Ok(())
    }

    pub(super) fn is_running(&self) -> io::Result<bool> {
        let child = self.child.as_ref().ok_or(io::ErrorKind::NotFound)?;
        // SAFETY: the process handle belongs to this owner and remains live for the wait.
        match unsafe { WaitForSingleObject(raw(&child.process), 0) } {
            WAIT_TIMEOUT => Ok(true),
            WAIT_OBJECT_0 => Ok(false),
            _ => Err(io::Error::last_os_error()),
        }
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
        self.close_until(Instant::now() + Duration::from_millis(u64::from(PROCESS_WAIT_MS)))
    }

    pub(super) fn close_until(&mut self, deadline: Instant) -> CloseReport {
        if let Some(report) = self.closed {
            return report;
        }
        self.pipes.take();
        let mut report = CloseReport {
            process_exited: false,
            job_empty: false,
            forced: false,
            termination_error: None,
            timed_out: false,
        };
        if let Some(child) = self.child.take() {
            let now = Instant::now();
            let deadline = deadline.min(now + Duration::from_millis(u64::from(PROCESS_WAIT_MS)));
            report.timed_out = now >= deadline;
            // SAFETY: child.process is the private handle returned by spawn_suspended.
            let exited_before_kill =
                unsafe { WaitForSingleObject(raw(&child.process), 0) == WAIT_OBJECT_0 };
            // Windows signals the process object slightly before decrementing Job ActiveProcesses.
            let settle = deadline.min(now + Duration::from_millis(50));
            let empty_before_kill = exited_before_kill && wait_empty(&child.job, settle).is_ok();
            if empty_before_kill {
                report.process_exited = true;
                report.job_empty = true;
            } else {
                report.forced = true;
                // SAFETY: this Job is exclusively the handle returned by spawn_suspended.
                report.termination_error = unsafe { TerminateJobObject(raw(&child.job), 1) }
                    .err()
                    .map(|error| error.code().0);
                match wait_exited(&child.process, deadline) {
                    Ok(()) => report.process_exited = true,
                    Err(true) => report.timed_out = true,
                    Err(false) => {}
                }
                match wait_empty(&child.job, deadline) {
                    Ok(()) => report.job_empty = true,
                    Err(true) => report.timed_out = true,
                    Err(false) => {}
                }
            }
            // Dropping the private Job handle also requests kill-on-close as a fallback.
        }
        self.closed = Some(report);
        report
    }
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        let report = self.close();
        if !report.process_exited
            || !report.job_empty
            || report.termination_error.is_some()
            || report.timed_out
        {
            tracing::warn!(?report, "local Job cleanup incomplete");
        }
    }
}

fn active_processes(job: &OwnedHandle) -> Result<u32, ()> {
    let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    // SAFETY: the buffer is writable and matches the requested class for this live Job.
    unsafe {
        QueryInformationJobObject(
            Some(raw(job)),
            JobObjectBasicAccountingInformation,
            std::ptr::from_mut(&mut info).cast(),
            size_of_val(&info) as u32,
            None,
        )
    }
    .map(|()| info.ActiveProcesses)
    .map_err(|_| ())
}

// Err(true) is a deadline miss, Err(false) is a native wait failure.
fn wait_exited(process: &OwnedHandle, deadline: Instant) -> Result<(), bool> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait_ms = if remaining.is_zero() {
            0
        } else {
            (remaining.as_millis() as u32).max(1)
        };
        // SAFETY: process is a live synchronization handle owned by this instance.
        let status = unsafe { WaitForSingleObject(raw(process), wait_ms) };
        if status == WAIT_OBJECT_0 {
            return Ok(());
        }
        if status != WAIT_TIMEOUT {
            return Err(false);
        }
        if wait_ms == 0 || Instant::now() >= deadline {
            return Err(true);
        }
    }
}

// Err(true) is a deadline miss, Err(false) is a native query failure.
fn wait_empty(job: &OwnedHandle, deadline: Instant) -> Result<(), bool> {
    loop {
        match active_processes(job) {
            Ok(0) => return Ok(()),
            Ok(_) => {}
            Err(()) => return Err(false),
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(true);
        }
        thread::sleep(remaining.min(Duration::from_millis(5)));
    }
}

#[cfg(test)]
pub(super) fn check_failed_fixture_launch() {
    let Err(error) = OwnedProcess::invalid_fixture() else {
        panic!("unexpected fixture image");
    };
    assert_eq!(
        error.raw_os_error(),
        Some(windows::Win32::Foundation::ERROR_INVALID_NAME.0 as i32)
    );
}
