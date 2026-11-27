use super::{
    process,
    runtime_integration::{shutdown_deadline, Layout},
    runtime_regression::{authorize, confirm_exit, prepare},
    *,
};
use crate::features::remote_bridge::target_os::windows_bridge::safety::{
    owned_process::{OwnedProcess, ParentPipes},
    runtime::{
        local::{managed::ManagedStatus, Completion, End, LocalRuntime},
        Task,
    },
};
use std::{io::Write, os::windows::io::OwnedHandle, path::Path};

fn start_fixture(
    runtime: &mut LocalRuntime<'_, '_>,
    task: &Task,
    fixture: &Fixture,
    directory: &Path,
) -> (String, OwnedHandle, ParentPipes) {
    let mut launched = None;
    assert!(runtime.start_managed(
        task,
        fixture.input(),
        [],
        directory,
        || fixture.clock(1),
        |launch| {
            let child = OwnedProcess::start_fixture_with_environment(
                &process::test_name("job_pipe_child"),
                Some(&launch),
            )?;
            let (witness, pipes) = child.witness()?;
            launched = Some((launch.receipt.to_owned(), witness, pipes));
            Ok(child)
        }
    ));
    launched.unwrap()
}

#[test]
fn managed_process_rejects_early_duplicate_and_stale_operations() {
    process::run_isolated("managed-ownership");
}

pub(super) fn check_managed_ownership() {
    let fixture = Fixture::new();
    let layout = Layout::new();
    let plan = fixture.plan();
    let (mut runtime, task) = prepare(&fixture, &plan, &layout);
    assert!(matches!(
        runtime.managed_status(&task),
        Some(ManagedStatus::Unprepared)
    ));
    assert!(!runtime.start_managed(
        &task,
        fixture.input(),
        [],
        &layout.history,
        || fixture.clock(0),
        |_| panic!("launched before authorization")
    ));
    authorize(&mut runtime, &task, &fixture);
    let (receipt, witness, _pipes) = start_fixture(&mut runtime, &task, &fixture, &layout.history);
    assert!(matches!(
        runtime.managed_status(&task),
        Some(ManagedStatus::AwaitingReceipt)
    ));
    assert!(!runtime.start_managed(
        &task,
        fixture.input(),
        [],
        &layout.history,
        || fixture.clock(1),
        |_| panic!("duplicate launch")
    ));
    assert!(runtime.confirm_managed(&task, fixture.input(), &receipt, || fixture.clock(2)));
    assert!(matches!(
        runtime.managed_status(&task),
        Some(ManagedStatus::Prepared)
    ));
    assert!(!runtime.confirm_managed(&task, fixture.input(), &receipt, || fixture.clock(2)));

    let closed = runtime
        .stop(&task, End::Cancelled, shutdown_deadline())
        .unwrap();
    assert_eq!(closed.completion, Completion::Complete);
    assert!(closed.pending.is_empty());
    confirm_exit(&witness);
    assert!(!layout.frozen.exists());
    let repeated = runtime
        .stop(&task, End::ApplicationExit, Instant::now())
        .unwrap();
    assert_eq!(repeated.end, End::Cancelled);
    assert_eq!(repeated.completion, Completion::Complete);
    assert!(!runtime.confirm_managed(&task, fixture.input(), &receipt, || fixture.clock(2)));
    assert!(!runtime.start_managed(
        &task,
        fixture.input(),
        [],
        &layout.history,
        || fixture.clock(2),
        |_| panic!("launched after cancel")
    ));

    let next_layout = Layout::new();
    let (mut next, next_task) = prepare(&fixture, &plan, &next_layout);
    authorize(&mut next, &next_task, &fixture);
    let (next_receipt, next_witness, _next_pipes) =
        start_fixture(&mut next, &next_task, &fixture, &next_layout.history);
    assert_ne!(receipt, next_receipt);
    assert!(!next.confirm_managed(&task, fixture.input(), &receipt, || fixture.clock(2)));
    assert!(matches!(
        next.managed_status(&next_task),
        Some(ManagedStatus::AwaitingReceipt)
    ));
    assert!(
        next.confirm_managed(&next_task, fixture.input(), &next_receipt, || fixture
            .clock(2))
    );
    next.stop(&next_task, End::Cancelled, shutdown_deadline())
        .unwrap();
    confirm_exit(&next_witness);
}

#[test]
fn managed_process_discards_results_after_expiry_or_unexpected_exit() {
    process::run_isolated("managed-loss");
}

pub(super) fn check_managed_loss() {
    for expected in [End::Expired, End::ProcessExited] {
        let fixture = Fixture::new();
        let layout = Layout::new();
        let plan = fixture.plan();
        let (mut runtime, task) = prepare(&fixture, &plan, &layout);
        authorize(&mut runtime, &task, &fixture);
        let (receipt, witness, mut pipes) =
            start_fixture(&mut runtime, &task, &fixture, &layout.history);
        let seconds = if expected == End::ProcessExited {
            pipes.input.write_all(b"e").unwrap();
            confirm_exit(&witness);
            2
        } else {
            30
        };
        assert!(
            !runtime.confirm_managed(&task, fixture.input(), &receipt, || fixture.clock(seconds))
        );
        let Some(ManagedStatus::Closed(report)) = runtime.managed_status(&task) else {
            panic!("managed attempt was left open");
        };
        assert_eq!(report.end, expected);
        assert_eq!(report.completion, Completion::Complete);
        assert!(report.pending.is_empty());
        confirm_exit(&witness);
        assert!(!layout.frozen.exists());
    }
}

#[test]
fn managed_process_rolls_back_rejected_directory_or_startup_failure() {
    process::run_isolated("managed-startup");
}

pub(super) fn check_managed_startup() {
    for fault in ["relative", "unc", "missing", "startup"] {
        let fixture = Fixture::new();
        let layout = Layout::new();
        let plan = fixture.plan();
        let (mut runtime, task) = prepare(&fixture, &plan, &layout);
        authorize(&mut runtime, &task, &fixture);
        let missing = layout.history.join("synthetic-absent-directory");
        let directory = match fault {
            "relative" => Path::new("synthetic-relative"),
            "unc" => Path::new(r"\\example.invalid\test"),
            "missing" => &missing,
            _ => &layout.history,
        };
        assert!(!runtime.start_managed(
            &task,
            fixture.input(),
            [],
            directory,
            || fixture.clock(1),
            |_| {
                assert_eq!(fault, "startup");
                OwnedProcess::invalid_fixture()
            }
        ));
        let Some(ManagedStatus::Closed(report)) = runtime.managed_status(&task) else {
            panic!("failed managed attempt was left open");
        };
        assert_eq!(report.end, End::SetupFailed);
        assert_eq!(report.completion, Completion::Complete);
        assert!(report.setup_error.is_some());
        assert!(report.process.is_none() && report.pending.is_empty());
        assert!(!layout.frozen.exists());
    }
}
