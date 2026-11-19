use super::*;
use std::{
    io::{self, PipeReader, PipeWriter, Read, Write},
    mem::{size_of, size_of_val},
    os::windows::{ffi::OsStrExt, io::AsRawHandle, process::CommandExt},
    process::Command,
    thread,
};
use windows::{
    core::{BOOL, HRESULT, PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            CloseHandle, SetHandleInformation, ERROR_BROKEN_PIPE, HANDLE, HANDLE_FLAG_INHERIT,
            WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        System::{JobObjects::*, Pipes::PeekNamedPipe, Threading::*},
    },
};

const PROCESS_WAIT_MS: u32 = 5000;
const STDOUT_MARKER: &[u8] = b"routespan fixture stdout\n";
const STDERR_MARKER: &[u8] = b"routespan fixture stderr\n";
const WORKER_MODE: &str = "ROUTESPAN_LOCAL_PIPE_FIXTURE";

fn test_name(name: &str) -> String {
    format!("{}::{name}", module_path!().split_once("::").unwrap().1)
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns the handle returned by Win32.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

struct Suspended {
    handle: Handle,
    abort: bool,
}

impl Drop for Suspended {
    fn drop(&mut self) {
        if self.abort {
            // SAFETY: the suspended child is ours and has not been released.
            unsafe {
                let _ = TerminateProcess(self.handle.0, 1);
                WaitForSingleObject(self.handle.0, PROCESS_WAIT_MS);
            }
        }
    }
}

struct ChildHandles<'a> {
    list: LPPROC_THREAD_ATTRIBUTE_LIST,
    _storage: Vec<usize>,
    _handles: &'a [HANDLE; 3],
}

impl<'a> ChildHandles<'a> {
    fn new(handles: &'a [HANDLE; 3]) -> Self {
        // SAFETY: these are the three owned child pipe ends. The aligned
        // storage and borrowed handle array outlive the attribute list.
        unsafe {
            for handle in handles {
                SetHandleInformation(*handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT).unwrap();
            }
            let mut bytes = 0;
            let _ = InitializeProcThreadAttributeList(None, 1, None, &mut bytes);
            assert_ne!(bytes, 0);
            let mut storage = vec![0_usize; bytes.div_ceil(size_of::<usize>())];
            let list = LPPROC_THREAD_ATTRIBUTE_LIST(storage.as_mut_ptr().cast());
            InitializeProcThreadAttributeList(Some(list), 1, None, &mut bytes).unwrap();
            let attributes = Self {
                list,
                _storage: storage,
                _handles: handles,
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
            .unwrap();
            attributes
        }
    }
}

impl Drop for ChildHandles<'_> {
    fn drop(&mut self) {
        // SAFETY: the list was initialized once and still has its storage.
        unsafe { DeleteProcThreadAttributeList(self.list) };
    }
}

struct LocalProcess {
    job: Handle,
    witness: Handle,
    input: PipeWriter,
    output: PipeReader,
    error: PipeReader,
}

// Launches only this test binary with a fixed local, pipe-only child entry.
fn local_job(resume: bool) -> LocalProcess {
    let exe = std::env::current_exe().unwrap();
    let mut program: Vec<u16> = exe.as_os_str().encode_wide().collect();
    program.push(0);
    let mut command = vec![u16::from(b'"')];
    command.extend(exe.as_os_str().encode_wide());
    command.extend("\" --exact ".encode_utf16());
    command.extend(test_name("job_pipe_child").encode_utf16());
    command.extend(" --ignored --nocapture\0".encode_utf16());
    let (child_input, input) = io::pipe().unwrap();
    let (output, child_output) = io::pipe().unwrap();
    let (error, child_error) = io::pipe().unwrap();
    let child_handles = [
        HANDLE(child_input.as_raw_handle()),
        HANDLE(child_output.as_raw_handle()),
        HANDLE(child_error.as_raw_handle()),
    ];
    let attributes = ChildHandles::new(&child_handles);
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
    // SAFETY: strings are terminated, the command line buffer is writable, and
    // handles from successful calls are uniquely owned by the wrappers below.
    unsafe {
        let job = Handle(CreateJobObjectW(None, None).unwrap());
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .unwrap();
        CreateProcessW(
            PCWSTR(program.as_ptr()),
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
        .unwrap();
        let mut child = Suspended {
            handle: Handle(info.hProcess),
            abort: true,
        };
        let thread = Handle(info.hThread);
        drop(attributes);
        drop((child_input, child_output, child_error));
        AssignProcessToJobObject(job.0, child.handle.0).unwrap();
        let mut member = BOOL::default();
        IsProcessInJob(child.handle.0, Some(job.0), &mut member).unwrap();
        assert!(
            member.as_bool(),
            "must be in the Job before any child code runs"
        );
        let witness = Handle(OpenProcess(PROCESS_SYNCHRONIZE, false, info.dwProcessId).unwrap());
        assert_eq!(available(&output), Some(0));
        assert_eq!(available(&error), Some(0));
        if resume {
            assert_eq!(ResumeThread(thread.0), 1, "primary thread was suspended");
            child.abort = false;
            assert_eq!(WaitForSingleObject(witness.0, 0), WAIT_TIMEOUT);
        }
        LocalProcess {
            job,
            witness,
            input,
            output,
            error,
        }
    }
}

fn available(pipe: &PipeReader) -> Option<u32> {
    let mut bytes = 0;
    // SAFETY: the reader and its output count remain live during the call.
    let result = unsafe {
        PeekNamedPipe(
            HANDLE(pipe.as_raw_handle()),
            None,
            0,
            None,
            Some(&mut bytes),
            None,
        )
    };
    match result {
        Ok(()) => Some(bytes),
        Err(err) if err.code() == HRESULT::from_win32(ERROR_BROKEN_PIPE.0) => None,
        Err(err) => panic!("could not inspect fixture pipe: {err}"),
    }
}

// Peek first so a leaked writer fails by deadline instead of blocking the test.
fn read_pipe(pipe: &mut PipeReader, marker: Option<&[u8]>) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_millis(u64::from(PROCESS_WAIT_MS));
    let mut bytes = Vec::new();
    loop {
        assert!(
            Instant::now() < deadline,
            "fixture pipe did not reach {marker:?}"
        );
        match available(pipe) {
            Some(0) => thread::sleep(Duration::from_millis(5)),
            Some(count) => {
                let end = bytes.len() + count as usize;
                assert!(end <= 4096, "unexpected fixture output");
                let start = bytes.len();
                bytes.resize(end, 0);
                pipe.read_exact(&mut bytes[start..]).unwrap();
                if marker.is_some_and(|marker| bytes.windows(marker.len()).any(|w| w == marker)) {
                    return bytes;
                }
            }
            None => {
                assert!(
                    marker.is_none(),
                    "child closed pipe before writing its marker"
                );
                return bytes;
            }
        }
    }
}

fn assert_reaped(witness: &Handle, input: &mut PipeWriter) {
    // SAFETY: the witness owns a synchronization handle for this child.
    assert_eq!(
        unsafe { WaitForSingleObject(witness.0, PROCESS_WAIT_MS) },
        WAIT_OBJECT_0
    );
    assert_eq!(
        input.write_all(b"x").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[test]
#[ignore = "executed only as the suspended local fixture child"]
fn job_pipe_child() {
    let mut signal = [0];
    io::stdin().read_exact(&mut signal).unwrap();
    assert_eq!(signal, *b"r");
    io::stdout().write_all(STDOUT_MARKER).unwrap();
    io::stdout().flush().unwrap();
    io::stderr().write_all(STDERR_MARKER).unwrap();
    io::stderr().flush().unwrap();
    io::stdin().read_exact(&mut signal).unwrap();
}

fn check_shutdown_paths() {
    let f = Fixture::new();
    enum Shutdown {
        Cancel,
        Error,
        Timeout,
    }
    for reason in [Shutdown::Cancel, Shutdown::Error, Shutdown::Timeout] {
        let LocalProcess {
            job,
            witness,
            mut input,
            mut output,
            mut error,
        } = local_job(true);
        input.write_all(b"r").unwrap();
        read_pipe(&mut output, Some(STDOUT_MARKER));
        read_pipe(&mut error, Some(STDERR_MARKER));
        let mut gate = f.verification(job);
        match reason {
            Shutdown::Cancel => gate.cancel(),
            Shutdown::Error => {
                rejected(gate.listener_report(b"{}", f.clock(0)), "unsafeBinding");
            }
            Shutdown::Timeout => {
                gate.listener_report(&report(&gate), f.clock(0)).unwrap();
                rejected(
                    gate.receipt(&gate.expected_receipt(), true, f.clock(16)),
                    "windowsForwardVerificationClosed",
                );
            }
        }
        assert!(gate.resources.is_none());
        assert_reaped(&witness, &mut input);
        read_pipe(&mut output, None);
        read_pipe(&mut error, None);
    }
}

fn check_startup_abort() {
    let mut process = local_job(false);
    assert_reaped(&process.witness, &mut process.input);
    assert!(read_pipe(&mut process.output, None).is_empty());
    assert!(read_pipe(&mut process.error, None).is_empty());
}

#[test]
#[ignore = "run as a dedicated handle-inheritance fixture process"]
fn isolated_job_worker() {
    match std::env::var(WORKER_MODE).as_deref() {
        Ok("shutdown") => check_shutdown_paths(),
        Ok("startup-abort") => check_startup_abort(),
        _ => panic!("missing fixture case"),
    }
}

fn run_isolated(mode: &str) {
    // The child ends must briefly be inheritable for CreateProcessW. Keeping
    // them in this worker's handle table prevents unrelated test launches
    // in the parent test runner from inheriting them.
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &test_name("isolated_job_worker"), "--ignored"])
        .env(WORKER_MODE, mode)
        .creation_flags(CREATE_NO_WINDOW.0)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("test result: ok. 1 passed;"),
        "worker {mode} exited {}\n{stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn safety_local_process_and_pipes_are_reaped_on_cancel_error_or_timeout() {
    run_isolated("shutdown");
}

#[test]
fn safety_local_startup_abort_reaps_suspended_child_without_running_it() {
    run_isolated("startup-abort");
}
