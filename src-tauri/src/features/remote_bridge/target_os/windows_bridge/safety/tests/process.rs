use super::*;
use std::{mem::size_of, os::windows::ffi::OsStrExt};
use windows::{
    core::{BOOL, PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{JobObjects::*, Threading::*},
    },
};

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
                WaitForSingleObject(self.handle.0, 5000);
            }
        }
    }
}

// This is a local process fixture. The only executable is the Rust test binary
// running its existing sleeping child test. It accepts no command or target.
fn local_job() -> (Handle, Handle) {
    let exe = std::env::current_exe().unwrap();
    let mut program: Vec<u16> = exe.as_os_str().encode_wide().collect();
    program.push(0);
    let mut command = vec![u16::from(b'"')];
    command.extend(exe.as_os_str().encode_wide());
    command.extend(
        "\" --exact features::remote_bridge::ssh::tests::owned_sleep_child --ignored\0"
            .encode_utf16(),
    );
    let mut info = PROCESS_INFORMATION::default();
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
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
            false,
            CREATE_SUSPENDED | CREATE_NO_WINDOW,
            None,
            PCWSTR::null(),
            &startup,
            &mut info,
        )
        .unwrap();
        let mut child = Suspended {
            handle: Handle(info.hProcess),
            abort: true,
        };
        let thread = Handle(info.hThread);
        AssignProcessToJobObject(job.0, child.handle.0).unwrap();
        let mut member = BOOL::default();
        IsProcessInJob(child.handle.0, Some(job.0), &mut member).unwrap();
        assert!(
            member.as_bool(),
            "must be in the Job before any child code runs"
        );
        let witness = Handle(OpenProcess(PROCESS_SYNCHRONIZE, false, info.dwProcessId).unwrap());
        assert_eq!(ResumeThread(thread.0), 1, "primary thread was suspended");
        child.abort = false;
        assert_eq!(WaitForSingleObject(witness.0, 0), WAIT_TIMEOUT);
        (job, witness)
    }
}

#[test]
fn safety_local_process_is_contained_before_running_and_reaped_on_cancel_or_error() {
    let f = Fixture::new();
    for valid_report in [true, false] {
        let (job, child) = local_job();
        let mut gate = f.verification(job);
        if valid_report {
            gate.listener_report(&report(&gate), f.clock(0)).unwrap();
            gate.cancel();
        } else {
            rejected(gate.listener_report(b"{}", f.clock(0)), "unsafeBinding");
        }
        // SAFETY: the witness holds a live synchronization handle until return.
        assert_eq!(unsafe { WaitForSingleObject(child.0, 5000) }, WAIT_OBJECT_0);
    }
}
