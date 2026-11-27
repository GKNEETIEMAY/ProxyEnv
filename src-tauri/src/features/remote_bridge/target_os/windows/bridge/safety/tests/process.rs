use super::*;
use crate::features::remote_bridge::target_os::windows_bridge::safety::owned_process::{
    check_failed_fixture_launch, raw, CloseReport, OwnedProcess, ParentPipes, PROCESS_WAIT_MS,
};
use std::{
    io::{self, PipeReader, PipeWriter, Read, Write},
    os::windows::{
        io::{AsRawHandle, OwnedHandle},
        process::CommandExt,
    },
    process::{Command, Stdio},
    thread,
};
use windows::{
    core::HRESULT,
    Win32::{
        Foundation::{ERROR_BROKEN_PIPE, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{
            Pipes::PeekNamedPipe,
            Threading::{WaitForSingleObject, CREATE_NO_WINDOW},
        },
    },
};

const STDOUT_MARKER: &[u8] = b"routespan fixture stdout\n";
const STDERR_MARKER: &[u8] = b"routespan fixture stderr\n";
const WORKER_MODE: &str = "ROUTESPAN_LOCAL_PIPE_FIXTURE";

pub(super) fn test_name(name: &str) -> String {
    format!("{}::{name}", module_path!().split_once("::").unwrap().1)
}

struct LocalProcess {
    owner: OwnedProcess,
    witness: OwnedHandle,
    pipes: ParentPipes,
}

fn local_job(resume: bool) -> LocalProcess {
    let mut owner = OwnedProcess::start_fixture(&test_name("job_pipe_child")).unwrap();
    let (witness, pipes) = owner.witness().unwrap();
    assert_eq!(available(&pipes.output), Some(0));
    assert_eq!(available(&pipes.error), Some(0));
    if resume {
        owner.resume().unwrap();
        // SAFETY: the duplicate observes only this local fixture child.
        assert_eq!(
            unsafe { WaitForSingleObject(raw(&witness), 0) },
            WAIT_TIMEOUT
        );
    } else {
        assert_eq!(owner.close(), forced_reclaim());
    }
    LocalProcess {
        owner,
        witness,
        pipes,
    }
}

fn forced_reclaim() -> CloseReport {
    CloseReport {
        process_exited: true,
        job_empty: true,
        forced: true,
        termination_error: None,
        timed_out: false,
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

fn assert_reaped(witness: &OwnedHandle, input: &mut PipeWriter) {
    // SAFETY: the witness owns a synchronization handle for this child.
    assert_eq!(
        unsafe { WaitForSingleObject(raw(witness), PROCESS_WAIT_MS) },
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
    if signal == *b"e" {
        return;
    }
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
            owner,
            witness,
            pipes:
                ParentPipes {
                    mut input,
                    mut output,
                    mut error,
                },
        } = local_job(true);
        input.write_all(b"r").unwrap();
        read_pipe(&mut output, Some(STDOUT_MARKER));
        read_pipe(&mut error, Some(STDERR_MARKER));
        let mut gate = f.verification(owner);
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
    assert_reaped(&process.witness, &mut process.pipes.input);
    assert!(read_pipe(&mut process.pipes.output, None).is_empty());
    assert!(read_pipe(&mut process.pipes.error, None).is_empty());
}

fn check_policy_shutdown() {
    for revoked in [false, true] {
        let LocalProcess {
            mut owner,
            witness,
            pipes:
                ParentPipes {
                    mut input,
                    mut output,
                    mut error,
                },
        } = local_job(true);
        input.write_all(b"r").unwrap();
        read_pipe(&mut output, Some(STDOUT_MARKER));
        read_pipe(&mut error, Some(STDERR_MARKER));
        super::policy_lifecycle::drive_local_policy_stop(revoked, || {
            let outcome = owner.close();
            assert_eq!(outcome, forced_reclaim());
            assert_eq!(owner.close(), outcome);
        });
        assert_reaped(&witness, &mut input);
        read_pipe(&mut output, None);
        read_pipe(&mut error, None);
    }
}

#[test]
#[ignore = "run as a dedicated handle-inheritance fixture process"]
fn isolated_job_worker() {
    match std::env::var(WORKER_MODE).as_deref() {
        Ok("shutdown") => check_shutdown_paths(),
        Ok("startup-abort") => check_startup_abort(),
        Ok("policy-stop") => check_policy_shutdown(),
        Ok("startup-error") => check_failed_fixture_launch(),
        Ok("runtime-integration") => super::runtime_integration::check_runtime_components(),
        Ok("runtime-startup-failure") => super::runtime_regression::check_startup_failure(),
        Ok("runtime-policy-failures") => super::runtime_regression::check_policy_failures(),
        Ok("runtime-exit-budget") => super::runtime_regression::check_application_exit(),
        Ok("runtime-drop") => super::runtime_regression::check_drop(),
        _ => panic!("missing fixture case"),
    }
}

pub(super) fn run_isolated(mode: &str) {
    // The child ends must briefly be inheritable for CreateProcessW. Keeping
    // them in this worker's handle table prevents unrelated test launches
    // in the parent test runner from inheriting them.
    let mut worker = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &test_name("isolated_job_worker"), "--ignored"])
        .env(WORKER_MODE, mode)
        .creation_flags(CREATE_NO_WINDOW.0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(25);
    while worker.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = worker.kill();
            let _ = worker.wait();
            panic!("worker {mode} did not exit within its deadline");
        }
        thread::sleep(Duration::from_millis(5));
    }
    let output = worker.wait_with_output().unwrap();
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

#[test]
fn owned_process_releases_pipes_after_policy_expiry_and_revocation() {
    run_isolated("policy-stop");
}

#[test]
fn owned_process_reports_startup_error_without_an_owned_child() {
    run_isolated("startup-error");
}
