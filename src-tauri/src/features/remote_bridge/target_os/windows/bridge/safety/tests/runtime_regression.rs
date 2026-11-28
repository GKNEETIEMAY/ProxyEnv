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

pub(super) fn prepare<'p, 'review>(
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

pub(super) fn authorize(runtime: &mut LocalRuntime<'_, '_>, task: &Task, fixture: &Fixture) {
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
    activate_with_observer(runtime, task, fixture, OwnedProcess::witness)
}

fn activate_with_observer<T>(
    runtime: &mut LocalRuntime<'_, '_>,
    task: &Task,
    fixture: &Fixture,
    observe: impl FnOnce(&OwnedProcess) -> io::Result<T>,
) -> T {
    authorize(runtime, task, fixture);
    let mut witness = None;
    assert!(runtime.activate(
        task,
        fixture.input(),
        || fixture.clock(1),
        || {
            let child = start_child()?;
            witness = Some(observe(&child)?);
            Ok(child)
        }
    ));
    assert!(runtime.available(task, fixture.clock(1)));
    witness.unwrap()
}

pub(super) fn confirm_exit(witness: &OwnedHandle) {
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
fn offline_ai_follows_verified_owner_consent_and_service_revision() {
    use crate::features::remote_bridge::{
        cc_service::{CcDetection, CcDetectionState, CcServiceState},
        target_os::WindowsAiState,
    };
    use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};

    let upstream = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    upstream.set_nonblocking(true).unwrap();
    let port = upstream.local_addr().unwrap().port();
    let service = |revision| CcDetection {
        state: CcDetectionState::Confirmed,
        local_port: port,
        service: CcServiceState::Recognized,
        revision,
    };
    let fixture = Fixture::new();
    let layout = Layout::new();
    let plan = fixture.plan();
    let args = verification_args(&fixture, &plan, &layout);
    let (mut runtime, task) = prepare(&fixture, &plan, &layout);
    let witness = activate(&mut runtime, &task, &fixture);
    assert!(runtime
        .prepare_ai(&task, fixture.input(), service(1), true, || fixture
            .clock(1))
        .is_err());
    assert!(runtime.ai_relay_fixture().is_none());

    let mut proof = runtime
        .begin_verification(&task, fixture.input(), &args, || fixture.clock(1))
        .unwrap();
    receive_proof(&mut proof, &fixture);
    runtime
        .finish_verification(proof, fixture.input(), &args, || fixture.clock(2))
        .unwrap();
    assert_eq!(
        runtime.poll_ai(&task, fixture.input(), service(1), || fixture.clock(2)),
        Some(WindowsAiState::NotEnabled)
    );
    assert_eq!(
        runtime
            .prepare_ai(&task, fixture.input(), service(1), false, || fixture
                .clock(2))
            .err()
            .as_deref(),
        Some("confirmationRequired")
    );
    assert_eq!(
        runtime.poll_ai(&task, fixture.input(), service(1), || fixture.clock(2)),
        Some(WindowsAiState::AwaitingAuthorization)
    );

    let old = runtime
        .prepare_ai(&task, fixture.input(), service(1), true, || {
            fixture.clock(2)
        })
        .unwrap();
    assert_eq!(
        runtime.poll_ai(&task, fixture.input(), service(1), || fixture.clock(2)),
        Some(WindowsAiState::Preparing)
    );
    assert!(runtime
        .prepare_ai(&task, fixture.input(), service(1), true, || fixture
            .clock(2))
        .is_err());
    assert_eq!(runtime.stop_ai(&task), Some(WindowsAiState::Closed));
    assert_eq!(runtime.stop_ai(&task), Some(WindowsAiState::Closed));
    let next = runtime
        .prepare_ai(&task, fixture.input(), service(2), true, || {
            fixture.clock(2)
        })
        .unwrap();
    assert!(runtime
        .finish_ai(
            &task,
            old,
            || fixture.input(),
            || service(1),
            || fixture.clock(2)
        )
        .is_err());
    runtime
        .finish_ai(
            &task,
            next,
            || fixture.input(),
            || service(2),
            || fixture.clock(2),
        )
        .unwrap();
    let relay_port = runtime.ai_relay_fixture().unwrap().port();
    assert_eq!(
        runtime.poll_ai(&task, fixture.input(), service(2), || fixture.clock(2)),
        Some(WindowsAiState::Prepared)
    );
    assert!(
        upstream.accept().is_err(),
        "Preparation did not issue an upstream request"
    );

    assert_eq!(
        runtime.poll_ai(&task, fixture.input(), service(3), || fixture.clock(2)),
        Some(WindowsAiState::Invalidated)
    );
    assert!(TcpStream::connect_timeout(
        &SocketAddr::from((Ipv4Addr::LOCALHOST, relay_port)),
        Duration::from_millis(100)
    )
    .is_err());
    assert!(
        runtime.available(&task, fixture.clock(2)),
        "The network lane remains owned"
    );
    let next = runtime
        .prepare_ai(&task, fixture.input(), service(3), true, || {
            fixture.clock(2)
        })
        .unwrap();
    runtime
        .finish_ai(
            &task,
            next,
            || fixture.input(),
            || service(3),
            || fixture.clock(2),
        )
        .unwrap();
    assert_eq!(
        runtime.poll_ai(&task, fixture.input(), service(3), || fixture.clock(30)),
        Some(WindowsAiState::Invalidated)
    );
    let closed = runtime
        .stop(&task, End::Cancelled, shutdown_deadline())
        .unwrap();
    assert_eq!(closed.ai, WindowsAiState::Invalidated);
    assert_eq!(closed.end, End::Expired);
    assert_eq!(closed.completion, Completion::Complete);
    assert!(runtime.ai_relay_fixture().is_none());
    confirm_exit(&witness);
}

#[test]
fn offline_ai_tool_overlay_is_bound_to_preview_owner_and_preserves_original() {
    use crate::features::remote_bridge::target_os::windows_bridge::safety::runtime::local::ai::ToolConfigApply;
    use crate::features::remote_bridge::{
        cc_service::{CcDetection, CcDetectionState, CcServiceState},
        target_os::WindowsAiState,
        tool_adapter::RemoteToolId,
    };

    let upstream = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let service = CcDetection {
        state: CcDetectionState::Confirmed,
        local_port: upstream.local_addr().unwrap().port(),
        service: CcServiceState::Recognized,
        revision: 1,
    };
    let fixture = Fixture::new();
    let layout = Layout::new();
    let plan = fixture.plan();
    let args = verification_args(&fixture, &plan, &layout);
    let (mut runtime, task) = prepare(&fixture, &plan, &layout);
    let witness = activate(&mut runtime, &task, &fixture);
    let mut proof = runtime
        .begin_verification(&task, fixture.input(), &args, || fixture.clock(1))
        .unwrap();
    receive_proof(&mut proof, &fixture);
    runtime
        .finish_verification(proof, fixture.input(), &args, || fixture.clock(2))
        .unwrap();
    let enable = |runtime: &mut LocalRuntime<'_, '_>| {
        let receipt = runtime
            .prepare_ai(&task, fixture.input(), service, true, || fixture.clock(2))
            .unwrap();
        runtime
            .finish_ai(
                &task,
                receipt,
                || fixture.input(),
                || service,
                || fixture.clock(2),
            )
            .unwrap();
    };
    enable(&mut runtime);

    let codex = RemoteToolId::Codex;
    let claude = RemoteToolId::Claude;
    let source = b"model = \"fixture-local-model\"\n";
    struct FixtureFiles([std::path::PathBuf; 3]);
    impl Drop for FixtureFiles {
        fn drop(&mut self) {
            for path in &self.0 {
                let _ = fs::remove_file(path);
            }
        }
    }
    let files = FixtureFiles([
        layout.history.join("fixture-codex-original.toml"),
        layout.history.join("fixture-codex-overlay.toml"),
        layout.history.join("config.toml"),
    ]);
    let [original_path, overlay_path, private_overlay] = &files.0;
    fs::write(original_path, source).unwrap();
    assert_eq!(
        runtime
            .preview_ai_tool(&task, fixture.clock(2), codex, "0.133.0", None)
            .err()
            .as_deref(),
        Some("cliUnsupported")
    );
    assert_eq!(
        runtime
            .preview_ai_tool(
                &task,
                fixture.clock(2),
                claude,
                "2.1.227",
                Some(br#"{"env":{"ANTHROPIC_BASE_URL":"fixture-conflict"}}"#)
            )
            .err()
            .as_deref(),
        Some("configConflict")
    );
    let preview = |runtime: &LocalRuntime<'_, '_>| {
        runtime
            .preview_ai_tool(&task, fixture.clock(2), codex, "0.134.0", Some(source))
            .unwrap()
    };
    let apply = |runtime: &mut LocalRuntime<'_, '_>, change| {
        runtime.apply_ai_tool(&task, fixture.input(), service, || fixture.clock(2), change)
    };
    for (current, occupied, confirmed, expected) in [
        (source.as_slice(), false, false, "confirmationRequired"),
        (source.as_slice(), true, true, "configConflict"),
        (
            b"model = \"fixture-edited\"\n".as_slice(),
            false,
            true,
            "configConflict",
        ),
    ] {
        let pending = preview(&runtime);
        assert_eq!(
            apply(
                &mut runtime,
                ToolConfigApply {
                    preview: pending,
                    source: Some(current),
                    occupied,
                    confirmed,
                }
            )
            .err()
            .as_deref(),
            Some(expected)
        );
    }
    let pending = preview(&runtime);
    assert_eq!(pending.filename, "config.toml");
    assert_eq!(pending.token_env, "PROXYENV_SESSION_TOKEN");
    let mut applied = apply(
        &mut runtime,
        ToolConfigApply {
            preview: pending,
            source: Some(source),
            occupied: false,
            confirmed: true,
        },
    )
    .unwrap();
    let contents = applied.content().unwrap().to_vec();
    let config = std::str::from_utf8(&contents).unwrap();
    assert!(config.contains("127.0.0.1") && config.contains("wire_api = \"responses\""));
    assert!(!config.contains("fixture-local-model") && !config.contains("model_catalog_json"));
    fs::write(overlay_path, &contents).unwrap();
    fs::write(overlay_path, b"other-owner").unwrap();
    let foreign = fs::read(overlay_path).unwrap();
    assert!(applied.restore(Some(&foreign)).is_err());
    assert_eq!(fs::read(overlay_path).unwrap(), b"other-owner");
    assert_eq!(applied.content(), Some(contents.as_slice()));
    fs::write(overlay_path, &contents).unwrap();
    applied
        .restore(Some(&fs::read(overlay_path).unwrap()))
        .unwrap();
    fs::remove_file(overlay_path).unwrap();
    applied.restore(None).unwrap();
    assert!(applied.content().is_none());
    assert_eq!(fs::read(original_path).unwrap(), source);

    let create = |runtime: &mut LocalRuntime<'_, '_>| {
        let pending = preview(runtime);
        apply(
            runtime,
            ToolConfigApply {
                preview: pending,
                source: Some(source),
                occupied: false,
                confirmed: true,
            },
        )
        .unwrap()
    };
    let mut materialized = create(&mut runtime);
    assert!(materialized
        .materialize(&layout.history, Some(b"changed fixture"))
        .is_err());
    fs::write(private_overlay, b"fixture foreign file").unwrap();
    assert!(materialized
        .materialize(&layout.history, Some(source))
        .is_err());
    assert_eq!(fs::read(private_overlay).unwrap(), b"fixture foreign file");
    fs::remove_file(private_overlay).unwrap();
    assert!(materialized
        .materialize_partial_fixture(&layout.history, Some(source))
        .is_err());
    assert!(!private_overlay.exists());

    let mut materialized = create(&mut runtime);
    materialized
        .materialize(&layout.history, Some(source))
        .unwrap();
    assert_eq!(
        fs::read(private_overlay).unwrap(),
        materialized.content().unwrap()
    );
    assert!(fs::write(private_overlay, b"fixture external replacement").is_err());
    materialized.restore_materialized().unwrap();
    materialized.restore_materialized().unwrap();
    assert!(!private_overlay.exists());
    assert_eq!(fs::read(original_path).unwrap(), source);

    let stale = preview(&runtime);
    runtime.stop_ai(&task);
    enable(&mut runtime);
    assert_eq!(
        apply(
            &mut runtime,
            ToolConfigApply {
                preview: stale,
                source: Some(source),
                occupied: false,
                confirmed: true,
            }
        )
        .err()
        .as_deref(),
        Some("windowsBridgeContextChanged")
    );
    let pending = runtime
        .preview_ai_tool(&task, fixture.clock(2), claude, "2.1.227", None)
        .unwrap();
    assert_eq!(pending.filename, "settings.json");
    assert_eq!(pending.token_env, "ANTHROPIC_AUTH_TOKEN");
    let mut applied = apply(
        &mut runtime,
        ToolConfigApply {
            preview: pending,
            source: None,
            occupied: false,
            confirmed: true,
        },
    )
    .unwrap();
    let contents = applied.content().unwrap().to_vec();
    let settings: serde_json::Value = serde_json::from_slice(&contents).unwrap();
    assert!(settings["env"]["ANTHROPIC_BASE_URL"]
        .as_str()
        .unwrap()
        .starts_with("http://127.0.0.1:"));
    assert!(settings["env"].get("ANTHROPIC_AUTH_TOKEN").is_none());
    assert_eq!(runtime.stop_ai(&task), Some(WindowsAiState::Closed));
    applied.restore(Some(&contents)).unwrap();
    assert!(runtime.available(&task, fixture.clock(2)));
    runtime
        .stop(&task, End::Cancelled, shutdown_deadline())
        .unwrap();
    confirm_exit(&witness);
}

#[test]
fn offline_ai_relay_closes_on_application_exit_and_ignores_old_target_tasks() {
    use crate::features::remote_bridge::{
        cc_service::{CcDetection, CcDetectionState, CcServiceState},
        target_os::WindowsAiState,
    };
    use std::{
        io::{BufRead, BufReader},
        net::{Ipv4Addr, TcpListener, TcpStream},
    };

    let local_upstream = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    local_upstream.set_nonblocking(true).unwrap();
    let service = CcDetection {
        state: CcDetectionState::Confirmed,
        local_port: local_upstream.local_addr().unwrap().port(),
        service: CcServiceState::Recognized,
        revision: 1,
    };
    let old_fixture = Fixture::new();
    let old_layout = Layout::new();
    let old_plan = old_fixture.plan();
    let (mut old_runtime, old_task) = prepare(&old_fixture, &old_plan, &old_layout);
    old_runtime
        .stop(&old_task, End::Cancelled, shutdown_deadline())
        .unwrap();

    let mut fixture = Fixture::new();
    fixture.target = ssh::BoundTarget::fixture("alias-b", CONFIG).unwrap();
    let layout = Layout::new();
    let plan = fixture.plan();
    let args = verification_args(&fixture, &plan, &layout);
    let (mut runtime, task) = prepare(&fixture, &plan, &layout);
    let witness = activate(&mut runtime, &task, &fixture);
    let mut proof = runtime
        .begin_verification(&task, fixture.input(), &args, || fixture.clock(1))
        .unwrap();
    receive_proof(&mut proof, &fixture);
    runtime
        .finish_verification(proof, fixture.input(), &args, || fixture.clock(2))
        .unwrap();
    let unavailable = CcDetection {
        state: CcDetectionState::ListeningUnknown,
        service: CcServiceState::Unavailable,
        ..service
    };
    assert_eq!(
        runtime
            .prepare_ai(&task, fixture.input(), unavailable, true, || fixture
                .clock(2))
            .err()
            .as_deref(),
        Some("ccUnavailable")
    );
    assert!(runtime.ai_relay_fixture().is_none());
    let pending = runtime
        .prepare_ai(&task, fixture.input(), service, true, || fixture.clock(2))
        .unwrap();
    runtime
        .finish_ai(
            &task,
            pending,
            || fixture.input(),
            || service,
            || fixture.clock(2),
        )
        .unwrap();
    assert!(runtime.stop_ai(&old_task).is_none());
    assert!(runtime
        .poll_ai(&old_task, fixture.input(), unavailable, || fixture
            .clock(30))
        .is_none());
    let relay = runtime.ai_relay_fixture().unwrap();
    let relay_addr = (Ipv4Addr::LOCALHOST, relay.port());
    let token = relay.token();
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let url = format!("http://127.0.0.1:{}/v1/responses", relay.port());
    let denied = client.post(&url).body("").send().unwrap();
    assert!(denied.status().is_client_error());
    assert!(local_upstream.accept().is_err());
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut stream = loop {
            match local_upstream.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("synthetic upstream did not receive request: {error}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut lines = BufReader::new(&mut stream);
        let mut line = String::new();
        assert!(lines.read_line(&mut line).unwrap() > 0);
        assert_eq!(line.trim_end(), "POST /v1/responses HTTP/1.1");
        loop {
            line.clear();
            assert!(lines.read_line(&mut line).unwrap() > 0);
            if line == "\r\n" {
                break;
            }
            assert!(!line.to_ascii_lowercase().starts_with("x-proxyenv-session:"));
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
    });
    let result = client
        .post(url)
        .header("X-ProxyEnv-Session", token.as_str())
        .body("")
        .send();
    server.join().unwrap();
    assert_eq!(result.unwrap().text().unwrap(), "ok");
    let stopped = runtime
        .stop(&task, End::ApplicationExit, shutdown_deadline())
        .unwrap();
    assert_eq!(stopped.ai, WindowsAiState::Closed);
    assert_eq!(stopped.completion, Completion::Complete);
    assert!(TcpStream::connect_timeout(&relay_addr.into(), Duration::from_millis(100)).is_err());
    assert!(!layout.frozen.exists());
    confirm_exit(&witness);
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
    let ((witness, mut pipes), job_drained) =
        activate_with_observer(&mut runtime, &task, &fixture, |child| {
            Ok((child.witness()?, child.job_drain_witness()?))
        });
    pipes.input.write_all(b"e").unwrap();
    drop(pipes);
    confirm_exit(&witness);
    // Windows may signal a process before the Job accounts for its exit.
    assert!(job_drained(shutdown_deadline()));
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
