use super::{
    policy_lifecycle::source,
    process,
    runtime_integration::{shutdown_deadline, start_child, Layout},
    *,
};
use crate::features::remote_bridge::target_os::windows_bridge::safety::{
    local_files::{delete_by_handle, PrivateDirectory},
    owned_process::{raw, CloseReport, OwnedProcess, ParentPipes, PROCESS_WAIT_MS},
    policy::lifecycle::{
        Enrollment, StatusRequest, StatusResponse, StatusSource, StatusUnavailable,
    },
    runtime::{
        local::{
            Completion, End, LocalRuntime, LocalVerification, Shutdown, COORDINATED_EXIT_BUDGET,
            EXIT_FALLBACK_BUDGET,
        },
        Task,
    },
};
use std::{
    fs,
    io::{self, Write},
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
    activate_with_pipes(runtime, task, fixture).0
}

fn activate_with_pipes(
    runtime: &mut LocalRuntime<'_, '_>,
    task: &Task,
    fixture: &Fixture,
) -> (OwnedHandle, ParentPipes) {
    authorize(runtime, task, fixture);
    let mut witness = None;
    assert!(runtime.activate(
        task,
        fixture.input(),
        || fixture.clock(1),
        || {
            let child = start_child()?;
            witness = Some(child.witness()?);
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

fn verification_args(
    fixture: &Fixture,
    plan: &WindowsBridgePlan<'_>,
    layout: &Layout,
) -> [String; 7] {
    let directory = layout.frozen.to_str().unwrap().replace('\\', "/");
    let profile = FrozenProfile::build(
        plan.input.identity,
        plan.input.host_key_sha256,
        fixture.trust(),
        &directory,
    )
    .unwrap();
    profile.forward_args(
        &plan
            .forward_args(fixture.input(), fixture.clock(1).unix_seconds)
            .unwrap(),
    )
}

fn receive_proof(attempt: &mut LocalVerification, fixture: &Fixture) {
    let listener = report(attempt.fixture_proof());
    let receipt = attempt.fixture_proof().expected_receipt();
    attempt
        .listener_report(&listener, fixture.clock(2))
        .unwrap();
    attempt.receipt(&receipt, true, fixture.clock(2)).unwrap();
}

#[test]
fn desktop_verification_requires_authorized_owned_resources_and_preserves_the_owner() {
    let fixture = Fixture::new();
    let layout = Layout::new();
    let plan = fixture.plan();
    let args = verification_args(&fixture, &plan, &layout);
    let (mut runtime, task) = prepare(&fixture, &plan, &layout);
    rejected(
        runtime.begin_verification(&task, fixture.input(), &args, || fixture.clock(0)),
        "windowsForwardPolicyDenied",
    );
    assert!(!layout.frozen.exists());

    let witness = activate(&mut runtime, &task, &fixture);
    let mut attempt = runtime
        .begin_verification(&task, fixture.input(), &args, || fixture.clock(1))
        .unwrap();
    receive_proof(&mut attempt, &fixture);
    runtime
        .finish_verification(attempt, fixture.input(), &args, || fixture.clock(2))
        .unwrap();
    assert!(runtime.available(&task, fixture.clock(2)));
    assert!(layout.frozen.join("ssh_config").is_file());

    let closed = runtime
        .stop(&task, End::Cancelled, shutdown_deadline())
        .unwrap();
    assert_eq!(closed.completion, Completion::Complete);
    assert!(closed.pending.is_empty());
    confirm_exit(&witness);
    assert!(!layout.frozen.exists());
}

#[test]
fn desktop_verification_rejects_other_tasks_and_host_key_changes() {
    let fixture = Fixture::new();
    let old_layout = Layout::new();
    let plan = fixture.plan();
    let old_args = verification_args(&fixture, &plan, &old_layout);
    let (mut old, old_task) = prepare(&fixture, &plan, &old_layout);
    let old_witness = activate(&mut old, &old_task, &fixture);
    let mut stale = old
        .begin_verification(&old_task, fixture.input(), &old_args, || fixture.clock(1))
        .unwrap();
    receive_proof(&mut stale, &fixture);
    old.stop(&old_task, End::Cancelled, shutdown_deadline())
        .unwrap();
    confirm_exit(&old_witness);

    let layout = Layout::new();
    let args = verification_args(&fixture, &plan, &layout);
    let (mut runtime, task) = prepare(&fixture, &plan, &layout);
    let witness = activate(&mut runtime, &task, &fixture);
    rejected(
        runtime.finish_verification(stale, fixture.input(), &args, || fixture.clock(2)),
        "windowsBridgeContextChanged",
    );
    assert!(runtime.available(&task, fixture.clock(2)));
    assert!(layout.frozen.join("ssh_config").is_file());

    let mut pending = runtime
        .begin_verification(&task, fixture.input(), &args, || fixture.clock(2))
        .unwrap();
    receive_proof(&mut pending, &fixture);
    let changed = WindowsBridgeInput {
        verified_host: Some(ssh::VerifiedHostIdentity::fixture(
            &fixture.target,
            23,
            [0x2a; 32],
        )),
        ..fixture.input()
    };
    rejected(
        runtime.finish_verification(pending, changed, &args, || fixture.clock(3)),
        "hostKeyChanged",
    );
    let closed = runtime
        .stop(&task, End::Cancelled, shutdown_deadline())
        .unwrap();
    assert_eq!(closed.end, End::AuthorizationUnavailable);
    assert_eq!(closed.completion, Completion::Complete);
    confirm_exit(&witness);
    assert!(!layout.frozen.exists());
}

#[test]
fn desktop_verification_rechecks_history_deadlines_and_arguments_before_completion() {
    enum Fault {
        ExpiredGrant,
        ExpiredProof,
        UnavailableHistory,
        ChangedArguments,
        MissingReceipt,
    }
    for fault in [
        Fault::ExpiredGrant,
        Fault::ExpiredProof,
        Fault::UnavailableHistory,
        Fault::ChangedArguments,
        Fault::MissingReceipt,
    ] {
        let fixture = Fixture::new();
        let layout = Layout::new();
        let plan = fixture.plan();
        let mut args = verification_args(&fixture, &plan, &layout);
        let (mut runtime, task) = prepare(&fixture, &plan, &layout);
        let witness = activate(&mut runtime, &task, &fixture);
        let mut attempt = runtime
            .begin_verification(&task, fixture.input(), &args, || fixture.clock(1))
            .unwrap();
        if !matches!(fault, Fault::MissingReceipt) {
            receive_proof(&mut attempt, &fixture);
        }
        let (seconds, code, end) = match fault {
            Fault::ExpiredGrant => (30, "windowsForwardPolicyDenied", End::Expired),
            Fault::ExpiredProof => (16, "windowsForwardVerificationClosed", End::SetupFailed),
            Fault::UnavailableHistory => {
                fs::write(
                    layout.history.join("history.bin"),
                    b"invalid protected history",
                )
                .unwrap();
                (3, "windowsForwardPolicyDenied", End::HistoryUnavailable)
            }
            Fault::ChangedArguments => {
                args[2] = "-v".into();
                (3, "sshConfigUnsafe", End::SetupFailed)
            }
            Fault::MissingReceipt => (3, "windowsForwardReceiptInvalid", End::SetupFailed),
        };
        rejected(
            runtime.finish_verification(attempt, fixture.input(), &args, || fixture.clock(seconds)),
            code,
        );
        let closed = runtime
            .stop(&task, End::Cancelled, shutdown_deadline())
            .unwrap();
        assert_eq!(closed.end, end);
        assert_eq!(closed.completion, Completion::Complete);
        assert!(closed.pending.is_empty());
        confirm_exit(&witness);
        assert!(!layout.frozen.exists());
    }
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
        .stop(
            &task,
            End::ApplicationExit,
            Instant::now() + EXIT_FALLBACK_BUDGET,
        )
        .unwrap();
    assert_eq!(report.end, End::ApplicationExit);
    assert_eq!(report.completion, Completion::Complete);
    assert!(!layout.frozen.exists());
}

pub(super) fn check_startup_failure() {
    #[derive(Clone, Copy)]
    enum RollbackCase {
        Complete,
        PartialForeignFile,
        FailedLockedFiles,
    }
    for case in [
        RollbackCase::Complete,
        RollbackCase::PartialForeignFile,
        RollbackCase::FailedLockedFiles,
    ] {
        let fixture = Fixture::new();
        let layout = Layout::new();
        let plan = fixture.plan();
        let (mut runtime, task) = prepare(&fixture, &plan, &layout);
        authorize(&mut runtime, &task, &fixture);
        let marker = layout.frozen.join("fixture-foreign.txt");
        let mut locked = Vec::new();
        assert!(!runtime.activate(
            &task,
            fixture.input(),
            || fixture.clock(1),
            || {
                assert!(layout.frozen.join("ssh_config").is_file());
                match case {
                    RollbackCase::Complete => {}
                    RollbackCase::PartialForeignFile => {
                        fs::write(&marker, b"synthetic foreign content").unwrap();
                    }
                    RollbackCase::FailedLockedFiles => {
                        for name in ["ssh_config", "known_hosts", "revoked_hosts"] {
                            locked.push(
                                fs::OpenOptions::new()
                                    .read(true)
                                    .share_mode(FILE_SHARE_READ.0)
                                    .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
                                    .open(layout.frozen.join(name))
                                    .unwrap(),
                            );
                        }
                    }
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
        match case {
            RollbackCase::Complete => {
                assert_eq!(report.completion, Completion::Complete);
                assert!(report.pending.is_empty());
            }
            RollbackCase::PartialForeignFile => {
                for name in ["ssh_config", "known_hosts", "revoked_hosts"] {
                    assert!(!layout.frozen.join(name).exists());
                }
                assert_eq!(report.completion, Completion::Partial);
                assert_eq!(report.pending.len(), 1);
                assert_eq!(report.pending[0].name, ".");
                assert_eq!(fs::read(&marker).unwrap(), b"synthetic foreign content");
                fs::remove_file(&marker).unwrap();
                reclaim_fixture(&layout, report);
            }
            RollbackCase::FailedLockedFiles => {
                assert_eq!(report.completion, Completion::Failed);
                assert_eq!(
                    report
                        .pending
                        .iter()
                        .map(|entry| entry.name)
                        .collect::<Vec<_>>(),
                    vec!["ssh_config", "known_hosts", "revoked_hosts", "."]
                );
                assert!(report.pending.iter().all(|entry| entry.id.is_some()));
                drop(locked);
                reclaim_fixture(&layout, report);
            }
        }
        assert!(!layout.frozen.exists());
    }
}

fn reclaim_fixture(layout: &Layout, report: &Shutdown) {
    let directory =
        PrivateDirectory::new(&layout.frozen, false, GENERIC_READ.0 | DELETE.0).unwrap();
    let mut directory_id = None;
    for entry in &report.pending {
        if entry.name == "." {
            directory_id = entry.id;
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
    assert_eq!(directory_id, Some(directory.id));
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
    // 1. Coordinated 3s exit after child has already exited on its own (normal exit, not forced).
    let fixture = Fixture::new();
    let layout = Layout::new();
    let plan = fixture.plan();
    let (mut runtime, task) = prepare(&fixture, &plan, &layout);
    let (witness, mut pipes) = activate_with_pipes(&mut runtime, &task, &fixture);
    pipes.input.write_all(b"e").unwrap();
    drop(pipes);
    confirm_exit(&witness);
    let report = runtime
        .stop(
            &task,
            End::ApplicationExit,
            Instant::now() + COORDINATED_EXIT_BUDGET,
        )
        .unwrap();
    assert_eq!(report.end, End::ApplicationExit);
    assert_eq!(report.completion, Completion::Complete);
    assert_eq!(
        report.process,
        Some(CloseReport {
            process_exited: true,
            job_empty: true,
            forced: false,
            termination_error: None,
            timed_out: false,
        })
    );
    assert!(report.pending.is_empty());
    assert!(!layout.frozen.exists());

    // 2. Coordinated 3s exit and 200ms fallback exit on an active owned Job (forced reclamation),
    // plus exhausted 0ms shared budget proving no nested wait re-acquires a full 3s/5s budget.
    for budget in [
        COORDINATED_EXIT_BUDGET,
        EXIT_FALLBACK_BUDGET,
        Duration::ZERO,
    ] {
        let fixture = Fixture::new();
        let layout = Layout::new();
        let plan = fixture.plan();
        let (mut runtime, task) = prepare(&fixture, &plan, &layout);
        let (witness, mut pipes) = activate_with_pipes(&mut runtime, &task, &fixture);
        pipes.input.write_all(b"r").unwrap();
        drop(pipes);
        let started = Instant::now();
        let report = runtime
            .stop(&task, End::ApplicationExit, started + budget)
            .unwrap();
        assert_eq!(report.end, End::ApplicationExit);
        let process_report = report.process.unwrap();
        assert!(process_report.forced);
        if budget == COORDINATED_EXIT_BUDGET {
            assert_eq!(report.completion, Completion::Complete);
            assert_eq!(
                process_report,
                CloseReport {
                    process_exited: true,
                    job_empty: true,
                    forced: true,
                    termination_error: None,
                    timed_out: false,
                }
            );
            assert!(report.pending.is_empty());
            assert!(started.elapsed() < COORDINATED_EXIT_BUDGET);
            let fallback_started = Instant::now();
            let repeated = runtime
                .stop(
                    &task,
                    End::ApplicationExit,
                    fallback_started + EXIT_FALLBACK_BUDGET,
                )
                .unwrap();
            assert_eq!(repeated.completion, Completion::Complete);
            assert_eq!(repeated.process, Some(process_report));
            confirm_exit(&witness);
            drop(runtime);
            assert!(!layout.frozen.exists());
            continue;
        } else if budget.is_zero() {
            assert_eq!(report.completion, Completion::TimedOut);
            assert!(process_report.timed_out);
            assert_eq!(
                report
                    .pending
                    .iter()
                    .map(|entry| entry.name)
                    .collect::<Vec<_>>(),
                vec!["ssh_config", "known_hosts", "revoked_hosts", "."]
            );
            assert!(report
                .pending
                .iter()
                .all(|entry| entry.id.is_some() && entry.error.kind() == io::ErrorKind::TimedOut));
            let pending_ids: Vec<_> = report
                .pending
                .iter()
                .map(|entry| (entry.name, entry.id.unwrap()))
                .collect();
            reclaim_pending_after_drop(runtime, &witness, &layout, &pending_ids, started);
            assert!(!layout.frozen.exists());
            continue;
        }
        confirm_exit(&witness);
        match report.completion {
            Completion::Complete => assert!(report.pending.is_empty()),
            Completion::TimedOut if !report.pending.is_empty() => reclaim_fixture(&layout, report),
            Completion::TimedOut => {}
            ref other => panic!("unexpected exit cleanup: {other:?}"),
        }
        drop(runtime);
        assert!(!layout.frozen.exists());
    }
}

fn reclaim_pending_after_drop(
    runtime: LocalRuntime<'_, '_>,
    witness: &OwnedHandle,
    layout: &Layout,
    pending_ids: &[(
        &'static str,
        crate::features::remote_bridge::target_os::windows_bridge::safety::local_files::FileId,
    )],
    started: Instant,
) {
    drop(runtime);
    // Dropping a timed-out LocalRuntime must not re-acquire a 3s or 5s wait budget.
    assert!(started.elapsed() < Duration::from_secs(1));
    confirm_exit(witness);
    let directory =
        PrivateDirectory::new(&layout.frozen, false, GENERIC_READ.0 | DELETE.0).unwrap();
    let mut directory_id = None;
    for &(name, expected_id) in pending_ids {
        if name == "." {
            directory_id = Some(expected_id);
            continue;
        }
        let file = directory
            .file(name, GENERIC_READ.0 | DELETE.0, FILE_SHARE_MODE(0), false)
            .unwrap();
        assert_eq!(directory.validate(&file, false).unwrap(), expected_id);
        delete_by_handle(&file).unwrap();
    }
    assert_eq!(directory_id, Some(directory.id));
    delete_by_handle(&directory.directory).unwrap();
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
