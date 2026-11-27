use super::{policy_lifecycle::source, process, *};
use crate::features::remote_bridge::target_os::windows_bridge::safety::{
    owned_process::{raw, OwnedProcess, PROCESS_WAIT_MS},
    policy::lifecycle::{
        Enrollment, StatusRequest, StatusResponse, StatusSource, StatusUnavailable,
    },
    policy_storage::PolicyStorage,
    runtime::local::{Completion, End, LocalRuntime},
};
use std::{fs, io, path::PathBuf};
use windows::Win32::{Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject};

pub(super) struct Layout {
    pub(super) history: PathBuf,
    pub(super) frozen: PathBuf,
}

impl Layout {
    pub(super) fn new() -> Self {
        let mut nonce = [0; 12];
        getrandom::fill(&mut nonce).unwrap();
        let namespace = hex::encode(nonce);
        // Keep short-lived protected files beside the test binary, outside source-tree scans.
        let executable = std::env::current_exe().unwrap();
        let parent = executable.parent().unwrap();
        Self {
            history: parent.join(format!("routespan-runtime-history-{namespace}")),
            frozen: parent.join(format!("routespan-runtime-frozen-{namespace}")),
        }
    }

    pub(super) fn history(&self) -> PolicyStorage {
        PolicyStorage::initialize(&self.history).unwrap()
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        for name in ["history.bin", "history.lock"] {
            let _ = fs::remove_file(self.history.join(name));
        }
        let _ = fs::remove_dir(&self.history);
    }
}

pub(super) fn start_child() -> io::Result<OwnedProcess> {
    OwnedProcess::start_fixture(&process::test_name("job_pipe_child"))
}

pub(super) fn shutdown_deadline() -> Instant {
    Instant::now() + Duration::from_millis(u64::from(PROCESS_WAIT_MS))
}

pub(super) fn check_runtime_components() {
    for end in [End::Cancelled, End::Expired, End::Revoked] {
        let fixture = Fixture::new();
        let layout = Layout::new();
        let plan = fixture.plan();
        let (mut runtime, task) =
            LocalRuntime::begin(&plan, layout.history(), Enrollment::IndependentlyApproved)
                .unwrap();
        runtime
            .prepare(
                &task,
                fixture.input(),
                fixture.clock(0),
                &layout.frozen,
                fixture.trust(),
            )
            .unwrap();
        assert!(!runtime.available(&task, fixture.clock(0)));
        assert!(!runtime.activate(
            &task,
            fixture.input(),
            || fixture.clock(0),
            || panic!("started before authorization")
        ));
        assert!(!layout.frozen.exists());
        let lease = fixture.sign(LEASE_DOMAIN, fixture.lease());
        if !runtime.refresh(
            &task,
            fixture.input(),
            lease.record(),
            &mut source(&fixture, Fixture::status()),
            || fixture.clock(0),
        ) {
            let report = runtime
                .stop(&task, End::Cancelled, shutdown_deadline())
                .unwrap();
            panic!("initial grant for {end:?} rejected: {report:?}");
        }
        let mut witness = None;
        assert!(runtime.activate(
            &task,
            fixture.input(),
            || fixture.clock(1),
            || {
                let owner = start_child()?;
                witness = Some(owner.witness()?.0);
                Ok(owner)
            }
        ));
        assert!(runtime.available(&task, fixture.clock(1)));
        assert!(layout.frozen.join("ssh_config").is_file());
        match end {
            End::Expired => {
                assert!(!runtime.available(&task, fixture.clock(30)));
                assert!(!runtime.poll(&task, fixture.input(), || fixture.clock(30)));
            }
            End::Revoked => {
                let revoked = Status {
                    sequence: 10,
                    revoked: true,
                    ..Fixture::status()
                };
                assert!(!runtime.refresh(
                    &task,
                    fixture.input(),
                    lease.record(),
                    &mut source(&fixture, revoked),
                    || fixture.clock(2)
                ));
            }
            End::Cancelled => {}
            _ => unreachable!(),
        }
        let report = runtime
            .stop(&task, End::Cancelled, shutdown_deadline())
            .unwrap();
        assert_eq!(report.end, end);
        assert_eq!(report.completion, Completion::Complete);
        assert!(report.pending.is_empty());
        assert!(report.setup_error.is_none());
        let exit = report.process.unwrap();
        assert!(exit.process_exited && exit.job_empty);
        assert!(!runtime.available(&task, fixture.clock(30)));
        assert!(!layout.frozen.exists());
        let witness = witness.unwrap();
        // SAFETY: the duplicate is only an observation handle for this fixture child.
        assert_eq!(
            unsafe { WaitForSingleObject(raw(&witness), 0) },
            WAIT_OBJECT_0
        );
        let repeated = runtime
            .stop(&task, End::ApplicationExit, Instant::now())
            .unwrap();
        assert_eq!(repeated.end, end);
        assert_eq!(repeated.process, Some(exit));
    }
}

#[test]
fn runtime_components_close_real_local_resources_on_policy_events() {
    process::run_isolated("runtime-integration");
}

struct Unavailable;

impl StatusSource for Unavailable {
    fn query(&mut self, _: &StatusRequest<'_>) -> Result<StatusResponse, StatusUnavailable> {
        Err(StatusUnavailable)
    }
}

#[test]
fn runtime_components_fail_closed_when_authority_history_or_preparation_fails() {
    for expected in [
        End::AuthorizationUnavailable,
        End::HistoryUnavailable,
        End::SetupFailed,
    ] {
        let fixture = Fixture::new();
        let layout = Layout::new();
        let plan = fixture.plan();
        let (mut runtime, task) =
            LocalRuntime::begin(&plan, layout.history(), Enrollment::IndependentlyApproved)
                .unwrap();
        runtime
            .prepare(
                &task,
                fixture.input(),
                fixture.clock(0),
                &layout.frozen,
                fixture.trust(),
            )
            .unwrap();
        let lease = fixture.sign(LEASE_DOMAIN, fixture.lease());
        if expected == End::AuthorizationUnavailable {
            assert!(!runtime.refresh(
                &task,
                fixture.input(),
                lease.record(),
                &mut Unavailable,
                || fixture.clock(0)
            ));
        } else {
            if expected == End::HistoryUnavailable {
                fs::write(
                    layout.history.join("history.bin"),
                    b"invalid synthetic DPAPI data",
                )
                .unwrap();
            }
            let grant = runtime.refresh(
                &task,
                fixture.input(),
                lease.record(),
                &mut source(&fixture, Fixture::status()),
                || fixture.clock(0),
            );
            assert_eq!(grant, expected == End::SetupFailed);
        }
        if expected == End::SetupFailed {
            fs::create_dir(&layout.frozen).unwrap();
        }
        assert!(!runtime.activate(
            &task,
            fixture.input(),
            || fixture.clock(1),
            || panic!("launcher reached")
        ));
        let report = runtime
            .stop(&task, End::Cancelled, shutdown_deadline())
            .unwrap();
        assert_eq!(report.end, expected);
        assert_eq!(report.completion, Completion::Complete);
        assert!(report.process.is_none() && report.pending.is_empty());
        assert_eq!(report.setup_error.is_some(), expected == End::SetupFailed);
        assert!(!runtime.available(&task, fixture.clock(1)));
        if expected == End::SetupFailed {
            assert!(layout.frozen.is_dir());
            fs::remove_dir(&layout.frozen).unwrap();
        } else {
            assert!(!layout.frozen.exists());
        }
    }
}
