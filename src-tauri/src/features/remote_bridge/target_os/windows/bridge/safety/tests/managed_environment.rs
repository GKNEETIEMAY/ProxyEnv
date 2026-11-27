use super::{
    process,
    runtime_integration::{shutdown_deadline, Layout},
    runtime_regression::{authorize, confirm_exit, prepare},
    *,
};
use crate::features::remote_bridge::target_os::windows_bridge::safety::{
    owned_process::{shell_fixture::{CWD_WITNESS, OUTPUT_DONE}, OwnedProcess, ParentPipes},
    runtime::{
        local::{managed::ManagedStatus, Completion, End, LocalRuntime},
        Task,
    },
};
use crate::features::proxy::ProxyProtocol;
pub(super) use crate::features::remote_bridge::target_os::windows_bridge::safety::owned_process::shell_fixture::ShellProgram;
use std::{env, fs, io::{self, Write}, os::windows::io::OwnedHandle, path::{Path, PathBuf}};

const PARENT: [(&str, &str); 5] = [
    ("HTTP_PROXY", "http://127.0.0.1:29991"),
    ("HTTPS_PROXY", "http://127.0.0.1:29992"),
    ("ALL_PROXY", "socks5h://127.0.0.1:29993"),
    ("NO_PROXY", "old.example.test"),
    ("ROUTESPAN_M33_UNRELATED", "fixture-value"),
];

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

struct WorkingFixture(PathBuf);

impl Drop for WorkingFixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join(CWD_WITNESS));
        let _ = fs::remove_dir(&self.0);
    }
}

pub(super) fn check_parent_environment() {
    for (name, expected) in PARENT {
        assert!(env::var_os(name).is_some_and(|value| value == expected));
    }
}

fn shell_environment(
    directory: &Path,
    program: ShellProgram,
) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    let mut environment: Vec<_> = PARENT
        .iter()
        .map(|(name, _)| ((*name).into(), env::var_os(name).unwrap()))
        .collect();
    environment.extend([
        ("hTtP_pRoXy".into(), "http://127.0.0.1:29994".into()),
        ("no_proxy".into(), "OLD.EXAMPLE.TEST,internal.test".into()),
        ("SystemRoot".into(), env::var_os("SystemRoot").unwrap()),
        ("TEMP".into(), directory.as_os_str().to_owned()),
        ("TMP".into(), directory.as_os_str().to_owned()),
    ]);
    if matches!(program, ShellProgram::Cmd) {
        environment.push((process::WORKER_MODE.into(), "managed-cmd-payload".into()));
    }
    environment
}

pub(super) fn check_cmd_payload() {
    let mut receipt = String::new();
    io::stdin().read_line(&mut receipt).unwrap();
    let receipt = receipt.trim_end_matches(['\r', '\n']);
    assert!(receipt.starts_with("ROUTESPAN-MANAGED/1 "));
    assert!(Path::new(CWD_WITNESS).is_file());
    println!("{receipt}");
    println!("ROUTESPAN-SHELL=cmd");
    for (name, _) in PARENT {
        if let Some(value) = env::var_os(name) {
            println!("{name}={}", value.to_str().unwrap());
        }
    }
    println!("ROUTESPAN-CWD=OK");
    println!("{OUTPUT_DONE}");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut String::new()).unwrap();
}

pub(super) fn check_shell(program: ShellProgram) {
    check_parent_environment();
    let mut verified_version = None;
    for protocol in [
        ProxyProtocol::Http,
        ProxyProtocol::Socks5,
        ProxyProtocol::Mixed,
    ] {
        let mut fixture = Fixture::new();
        fixture.proxy.candidate.as_mut().unwrap().protocol = protocol;
        fixture.platform = Some(super::super::super::WindowsTargetProbe::Confirmed {
            shell: program.kind(),
        });
        let layout = Layout::new();
        let directory = layout.history.with_extension("Case 进程环境");
        fs::create_dir(&directory).unwrap();
        let directory = WorkingFixture(directory);
        fs::write(directory.0.join(CWD_WITNESS), []).unwrap();
        let case_variant = layout.history.with_extension("cASE 进程环境");
        let plan = fixture.plan();
        let (mut runtime, task) = prepare(&fixture, &plan, &layout);
        authorize(&mut runtime, &task, &fixture);
        let mut witness = None;
        let mut challenge = None;
        assert!(runtime.start_managed(
            &task,
            fixture.input(),
            shell_environment(&directory.0, program),
            &case_variant,
            || fixture.clock(1),
            |launch| {
                let process = OwnedProcess::start_shell_fixture(
                    program,
                    &launch,
                    &process::test_name("isolated_job_worker"),
                )?;
                witness = Some(process.witness()?);
                if matches!(program, ShellProgram::Cmd) {
                    challenge = Some(launch.receipt.to_owned());
                }
                Ok(process)
            },
        ));
        let (witness, mut pipes) = witness.unwrap();
        if let Some(challenge) = challenge {
            writeln!(pipes.input, "{challenge}").unwrap();
        }
        let bytes = process::read_pipe_until(&mut pipes.output, Some(OUTPUT_DONE.as_bytes()));
        let output = String::from_utf8(bytes).unwrap();
        if !output.contains(OUTPUT_DONE) {
            confirm_exit(&witness);
            let error = process::read_pipe(&mut pipes.error, None);
            let fixture_path = directory.0.to_string_lossy();
            let worker_path = env::current_exe().unwrap();
            let redact = |text: &str| {
                text.replace(fixture_path.as_ref(), "<working directory>")
                    .replace(worker_path.to_string_lossy().as_ref(), "<test executable>")
            };
            panic!(
                "managed shell exited without a receipt\n{}\n{}",
                redact(&output),
                redact(&String::from_utf8_lossy(&error))
            );
        }
        let mut lines = output.split(['\r', '\n']).filter(|line| !line.is_empty());
        let receipt = lines
            .find(|line| line.starts_with("ROUTESPAN-MANAGED/1 "))
            .expect("managed child did not report its receipt");
        let version = lines
            .next()
            .expect("managed child did not identify its shell")
            .strip_prefix("ROUTESPAN-SHELL=")
            .expect("managed child did not report its shell version after the receipt");
        match program {
            ShellProgram::Cmd => assert_eq!(version, "cmd"),
            ShellProgram::WindowsPowerShell => assert!(version.starts_with("5.1.")),
            ShellProgram::PowerShell7 => assert!(version.starts_with("7.")),
        }
        if let Some(previous) = &verified_version {
            assert_eq!(version, previous);
        }
        verified_version = Some(version.to_owned());
        let mut entries: Vec<_> = lines.collect();
        assert_eq!(entries.pop(), Some(OUTPUT_DONE));
        let directory_evidence = entries.pop().unwrap();
        if matches!(program, ShellProgram::Cmd) {
            assert_eq!(directory_evidence, "ROUTESPAN-CWD=OK");
        } else {
            assert!(Path::new(directory_evidence)
                .canonicalize()
                .ok()
                .zip(directory.0.canonicalize().ok())
                .is_some_and(|(reported, expected)| reported == expected));
        }
        entries.sort_unstable();
        let mut expected = vec![
            "NO_PROXY=old.example.test,internal.test,localhost,127.0.0.1,::1",
            "ROUTESPAN_M33_UNRELATED=fixture-value",
        ];
        if matches!(protocol, ProxyProtocol::Http | ProxyProtocol::Mixed) {
            expected.extend([
                "HTTP_PROXY=http://127.0.0.1:20500",
                "HTTPS_PROXY=http://127.0.0.1:20500",
            ]);
        }
        if matches!(protocol, ProxyProtocol::Socks5 | ProxyProtocol::Mixed) {
            expected.push("ALL_PROXY=socks5h://127.0.0.1:20500");
        }
        expected.sort_unstable();
        assert_eq!(entries, expected);
        check_parent_environment();
        if protocol == ProxyProtocol::Mixed {
            process::run_isolated("managed-observer");
            let changed = WindowsBridgeInput {
                generation: 24,
                verified_host: Some(ssh::VerifiedHostIdentity::fixture(
                    &fixture.target,
                    24,
                    fixture.review[0].host_key_sha256,
                )),
                ..fixture.input()
            };
            assert!(!runtime.confirm_managed(&task, changed, receipt, || fixture.clock(2)));
            let Some(ManagedStatus::Closed(report)) = runtime.managed_status(&task) else {
                panic!("stale managed environment attempt was left open");
            };
            assert_eq!(report.end, End::AuthorizationUnavailable);
        } else {
            assert!(runtime.confirm_managed(&task, fixture.input(), receipt, || fixture.clock(2)));
            assert!(matches!(
                runtime.managed_status(&task),
                Some(ManagedStatus::Prepared)
            ));
        }
        let report = runtime
            .stop(&task, End::Cancelled, shutdown_deadline())
            .unwrap();
        assert_eq!(report.completion, Completion::Complete);
        assert!(report.pending.is_empty());
        assert!(report
            .process
            .is_some_and(|process| process.process_exited && process.job_empty));
        confirm_exit(&witness);
        drop(pipes);
        drop(witness);
        assert!(!layout.frozen.exists());
        check_parent_environment();
        fs::remove_file(directory.0.join(CWD_WITNESS)).expect("remove current fixture witness");
        fs::remove_dir(&directory.0).expect("remove current fixture working directory");
    }
    println!("M33 SHELL {} HTTP/SOCKS5/Mixed", verified_version.unwrap());
}

fn verify_shell(program: ShellProgram, mode: &str) {
    let stdout = process::run_isolated_with_environment(mode, &PARENT);
    let evidence = stdout
        .lines()
        .find(|line| line.starts_with("M33 SHELL "))
        .unwrap();
    match program {
        ShellProgram::Cmd => assert!(evidence.starts_with("M33 SHELL cmd ")),
        ShellProgram::WindowsPowerShell => assert!(evidence.starts_with("M33 SHELL 5.1.")),
        ShellProgram::PowerShell7 => assert!(evidence.starts_with("M33 SHELL 7.")),
    }
    println!("{evidence}");
}

#[test]
fn managed_child_cmd_inherits_and_isolates_each_protocol() {
    verify_shell(ShellProgram::Cmd, "managed-cmd");
}

#[test]
fn managed_child_windows_powershell_51_inherits_and_isolates_each_protocol() {
    verify_shell(ShellProgram::WindowsPowerShell, "managed-powershell51");
}

#[test]
fn managed_child_powershell_7_inherits_and_isolates_each_protocol_when_installed() {
    if ShellProgram::PowerShell7.executable().is_none() {
        eprintln!("M33 SHELL PowerShell 7 PENDING (optional executable absent)");
        return;
    }
    verify_shell(ShellProgram::PowerShell7, "managed-powershell7");
}
