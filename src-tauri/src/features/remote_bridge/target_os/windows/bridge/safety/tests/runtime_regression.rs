use super::{
    policy_lifecycle::source,
    process,
    runtime_integration::{shutdown_deadline, start_child, Layout},
    *,
};
use crate::features::remote_bridge::target_os::windows_bridge::safety::{
    local_files::{delete_by_handle, PrivateDirectory},
    owned_process::{raw, OwnedProcess, PROCESS_WAIT_MS},
    policy::lifecycle::{
        Enrollment, StatusRequest, StatusResponse, StatusSource, StatusUnavailable,
    },
    runtime::{
        local::{Completion, End, LocalRuntime, Shutdown},
        Task,
    },
};
use std::{
    fs,
    os::windows::{fs::OpenOptionsExt, io::OwnedHandle},
};
use windows::Win32::{
    Foundation::{ERROR_INVALID_NAME, GENERIC_READ, WAIT_OBJECT_0},
    Storage::FileSystem::{
        DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
        FILE_SHARE_MODE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    },
    System::Threading::WaitForSingleObject,
};

const EXIT_BUDGET: Duration = Duration::from_millis(200);

fn prepare<'p, 'review>(
    fixture: &Fixture,
    plan: &'p WindowsBridgePlan<'review>,
    layout: &Layout,
) -> (LocalRuntime<'p, 'review>, Task) {
    let (mut runtime, task) =
        LocalRuntime::begin(plan, layout.history(), Enrollment::IndependentlyApproved).unwrap();
    runtime
        .prepare(
            &task,
            fixture.input(),
            fixture.clock(0),
            &layout.frozen,
            fixture.trust(),
        )
        .unwrap();
    (runtime, task)
}

fn authorize(runtime: &mut LocalRuntime<'_, '_>, task: &Task, fixture: &Fixture) {
    let lease = fixture.sign(LEASE_DOMAIN, fixture.lease());
    assert!(runtime.refresh(
        task,
        fixture.input(),
        lease.record(),
        &mut source(fixture, Fixture::status()),
        || fixture.clock(0)
    ));
}

fn activate(runtime: &mut LocalRuntime<'_, '_>, task: &Task, fixture: &Fixture) -> OwnedHandle {
    authorize(runtime, task, fixture);
    let mut witness = None;
    assert!(runtime.activate(
        task,
        fixture.input(),
        || fixture.clock(1),
        || {
            let child = start_child()?;
            witness = Some(child.witness()?.0);
            Ok(child)
        }
    ));
    assert!(runtime.available(task, fixture.clock(1)));
    witness.unwrap()
}

fn confirm_exit(witness: &OwnedHandle) {
    // SAFETY: this duplicate only observes the fixture child created by this test.
    assert_eq!(
        unsafe { WaitForSingleObject(raw(witness), PROCESS_WAIT_MS) },
        WAIT_OBJECT_0
    );
}

#[test]
fn runtime_boundaries_cancel_preparation_and_reject_stale_actions() {
    let fixture = Fixture::new();
    let old_layout = Layout::new();
    let plan = fixture.plan();
    let (mut old, old_task) = LocalRuntime::begin(
        &plan,
        old_layout.history(),
        Enrollment::IndependentlyApproved,
    )
    .unwrap();
    let initial = old.stop(&old_task, End::Cancelled, Instant::now()).unwrap();
    assert_eq!(initial.completion, Completion::Complete);
    assert!(initial.process.is_none());
    assert!(old
        .prepare(
            &old_task,
            fixture.input(),
            fixture.clock(0),
            &old_layout.frozen,
            fixture.trust()
        )
        .is_err());
    assert!(!old_layout.frozen.exists());

    let layout = Layout::new();
    let (mut current, task) = prepare(&fixture, &plan, &layout);
    assert!(current
        .stop(&old_task, End::ApplicationExit, Instant::now())
        .is_none());
    assert!(!current.activate(
        &old_task,
        fixture.input(),
        || fixture.clock(1),
        || panic!("foreign start")
    ));
    authorize(&mut current, &task, &fixture);
    let report = current
        .stop(&task, End::ApplicationExit, Instant::now() + EXIT_BUDGET)
        .unwrap();
    assert_eq!(report.end, End::ApplicationExit);
    assert_eq!(report.completion, Completion::Complete);
    assert!(!layout.frozen.exists());
}

pub(super) fn check_startup_failure() {
    for foreign_file in [false, true] {
        let fixture = Fixture::new();
        let layout = Layout::new();
        let plan = fixture.plan();
        let (mut runtime, task) = prepare(&fixture, &plan, &layout);
        authorize(&mut runtime, &task, &fixture);
        let marker = layout.frozen.join("fixture-foreign.txt");
        assert!(!runtime.activate(
            &task,
            fixture.input(),
            || fixture.clock(1),
            || {
                assert!(layout.frozen.join("ssh_config").is_file());
                if foreign_file {
                    fs::write(&marker, b"synthetic foreign content").unwrap();
                }
                OwnedProcess::invalid_fixture()
            }
        ));
        let report = runtime
            .stop(&task, End::Cancelled, shutdown_deadline())
            .unwrap();
        assert_eq!(report.end, End::SetupFailed);
        assert!(report.process.is_none());
        assert_eq!(
            report.setup_error.as_ref().unwrap().raw_os_error(),
            Some(ERROR_INVALID_NAME.0 as i32)
        );
        for name in ["ssh_config", "known_hosts", "revoked_hosts"] {
            assert!(!layout.frozen.join(name).exists());
        }
        if foreign_file {
            assert_eq!(report.completion, Completion::Partial);
            assert_eq!(report.pending.len(), 1);
            assert_eq!(report.pending[0].name, ".");
            assert_eq!(fs::read(&marker).unwrap(), b"synthetic foreign content");
            fs::remove_file(&marker).unwrap();
            reclaim_fixture(&layout, report);
        } else {
            assert_eq!(report.completion, Completion::Complete);
            assert!(report.pending.is_empty());
        }
        assert!(!layout.frozen.exists());
    }
}

fn reclaim_fixture(layout: &Layout, report: &Shutdown) {
    let directory =
        PrivateDirectory::new(&layout.frozen, false, GENERIC_READ.0 | DELETE.0).unwrap();
    for entry in &report.pending {
        if entry.name == "." {
            assert_eq!(entry.id, Some(directory.id));
            continue;
        }
        assert!(matches!(
            entry.name,
            "ssh_config" | "known_hosts" | "revoked_hosts"
        ));
        let file = directory
            .file(
                entry.name,
                GENERIC_READ.0 | DELETE.0,
                FILE_SHARE_MODE(0),
                false,
            )
            .unwrap();
        assert_eq!(entry.id, Some(directory.validate(&file, false).unwrap()));
        delete_by_handle(&file).unwrap();
    }
    delete_by_handle(&directory.directory).unwrap();
}

struct LateStatus<'a, S> {
    clock: &'a Cell<ClockSample>,
    arrives: ClockSample,
    source: S,
}

impl<S: StatusSource> StatusSource for LateStatus<'_, S> {
    fn query(&mut self, request: &StatusRequest<'_>) -> Result<StatusResponse, StatusUnavailable> {
        let signed = self.source.query(request)?;
        self.clock.set(self.arrives);
        Ok(signed)
    }
}

pub(super) fn check_policy_failures() {
    for expected in [End::HistoryUnavailable, End::Expired] {
        let fixture = Fixture::new();
        let layout = Layout::new();
        let plan = fixture.plan();
        let (mut runtime, task) = prepare(&fixture, &plan, &layout);
        let witness = activate(&mut runtime, &task, &fixture);
        if expected == End::HistoryUnavailable {
            fs::write(
                layout.history.join("history.bin"),
                b"corrupted active snapshot",
            )
            .unwrap();
            assert!(!runtime.poll(&task, fixture.input(), || fixture.clock(2)));
        } else {
            let clock = Cell::new(fixture.clock(2));
            let mut authority = LateStatus {
                clock: &clock,
                arrives: fixture.clock(30),
                source: source(
                    &fixture,
                    Status {
                        sequence: 10,
                        expires: 188,
                        ..Fixture::status()
                    },
                ),
            };
            let lease = fixture.sign(LEASE_DOMAIN, fixture.lease());
            assert!(!runtime.refresh(
                &task,
                fixture.input(),
                lease.record(),
                &mut authority,
                || clock.get()
            ));
        }
        let report = runtime
            .stop(&task, End::Cancelled, shutdown_deadline())
            .unwrap();
        assert_eq!(report.end, expected);
        assert_eq!(report.completion, Completion::Complete);
        assert!(!layout.frozen.exists());
        confirm_exit(&witness);
    }
}

pub(super) fn check_application_exit() {
    for budget in [Duration::ZERO, EXIT_BUDGET] {
        let fixture = Fixture::new();
        let layout = Layout::new();
        let plan = fixture.plan();
        let (mut runtime, task) = prepare(&fixture, &plan, &layout);
        let witness = activate(&mut runtime, &task, &fixture);
        let report = runtime
            .stop(&task, End::ApplicationExit, Instant::now() + budget)
            .unwrap();
        assert_eq!(report.end, End::ApplicationExit);
        confirm_exit(&witness);
        if budget.is_zero() {
            assert_eq!(report.completion, Completion::TimedOut);
            assert!(report.process.unwrap().timed_out);
            assert_eq!(report.pending.len(), 3);
        }
        match report.completion {
            Completion::Complete => assert!(report.pending.is_empty()),
            Completion::TimedOut if !report.pending.is_empty() => reclaim_fixture(&layout, report),
            Completion::TimedOut => {}
            ref other => panic!("unexpected exit cleanup: {other:?}"),
        }
        assert!(!layout.frozen.exists());
    }
}

pub(super) fn check_drop() {
    let fixture = Fixture::new();
    let layout = Layout::new();
    let plan = fixture.plan();
    let (mut runtime, task) = prepare(&fixture, &plan, &layout);
    let witness = activate(&mut runtime, &task, &fixture);
    let observer = fs::OpenOptions::new()
        .read(true)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
        .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
        .open(&layout.frozen)
        .unwrap();
    drop(runtime);
    confirm_exit(&witness);
    assert!(!fs::read_dir(layout.frozen.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .any(|name| name == layout.frozen.file_name().unwrap()));
    assert!(!layout.frozen.exists());
    drop(observer);
}

#[test]
fn runtime_boundaries_roll_back_partial_setup_and_keep_unowned_files() {
    process::run_isolated("runtime-startup-failure");
}

#[test]
fn runtime_boundaries_stop_active_job_for_corrupt_history_or_late_authority() {
    process::run_isolated("runtime-policy-failures");
}

#[test]
fn runtime_boundaries_report_application_exit_and_exhausted_budget() {
    process::run_isolated("runtime-exit-budget");
}

#[test]
fn runtime_boundaries_drop_reaps_job_and_removes_frozen_files() {
    process::run_isolated("runtime-drop");
}
