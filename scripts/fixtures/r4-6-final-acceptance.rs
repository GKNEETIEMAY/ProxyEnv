//! Loaded only in a disposable Windows test archive.
use super::*;
use std::{
    fs,
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering::SeqCst},
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

const LIMIT: Duration = Duration::from_secs(5);
static POST_CONNECT_DONE: AtomicUsize = AtomicUsize::new(0);
static PAUSES: Mutex<Vec<Pause>> = Mutex::new(Vec::new());
static OPERATIONS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

pub(super) fn enabled() -> bool {
    std::env::var_os("ROUTESPAN_R46_SCRATCH").is_some()
}

fn scratch() -> PathBuf {
    PathBuf::from(std::env::var_os("ROUTESPAN_R46_SCRATCH").unwrap())
}

struct Pause {
    key: String,
    entered: Sender<()>,
    resume: Receiver<()>,
}

fn pause_once(key: &str) -> (Receiver<()>, Sender<()>) {
    let (entered, arrival) = mpsc::channel();
    let (resume, release) = mpsc::channel();
    PAUSES.lock().unwrap().push(Pause {
        key: key.into(),
        entered,
        resume: release,
    });
    (arrival, resume)
}

fn pause_at(key: &str) {
    let pending = {
        let mut pauses = PAUSES.lock().unwrap();
        pauses
            .iter()
            .position(|entry| entry.key == key)
            .map(|index| pauses.remove(index))
    };
    if let Some(pending) = pending {
        pending.entered.send(()).unwrap();
        pending.resume.recv_timeout(LIMIT).unwrap();
    }
}

pub(super) fn auth_taken(id: &str) {
    if enabled() {
        pause_at(id);
    }
}

pub(super) struct PostConnectWorker;
impl Drop for PostConnectWorker {
    fn drop(&mut self) {
        POST_CONNECT_DONE.fetch_add(1, SeqCst);
    }
}

pub(super) fn shutdown_finished() {
    if let Some(path) = std::env::var_os("ROUTESPAN_R46_EXIT_DIR") {
        fs::write(PathBuf::from(path).join("shutdown.done"), b"done").unwrap();
    }
}

pub(super) fn fingerprint(id: &str) -> String {
    assert!(matches!(id, "fixture-a" | "fixture-b"), "unexpected target");
    format!("{id}-fingerprint")
}

pub(super) fn target(id: &str) -> RemoteTarget {
    fingerprint(id);
    serde_json::from_value(json!({
        "id":id,"displayName":id,"source":"manual","sourceLabel":"Fixture",
        "configPath":"","sshAlias":null,"host":"fixture.invalid","user":null,
        "port":22,"identityFile":null,"authenticationMethod":"agent","available":true,
        "compatibility":"compatible","unavailableReason":null,
        "canOpenVscode":false,"canOpenMobaxterm":false
    }))
    .unwrap()
}

fn environment_file(alias: &str, session: &str) -> PathBuf {
    fingerprint(alias);
    assert_eq!(session.len(), 32);
    assert!(session.bytes().all(|byte| byte.is_ascii_hexdigit()));
    scratch().join(format!("{alias}-{session}.environment"))
}

pub(super) fn remote(alias: &str, request: &serde_json::Value) -> BridgeResult<serde_json::Value> {
    fingerprint(alias);
    let operation = request["operation"].as_str().unwrap();
    pause_at(&format!("{operation}:{alias}"));
    OPERATIONS
        .lock()
        .unwrap()
        .push((alias.into(), operation.into()));
    match operation {
        "tool-discover" => Ok(json!({
            "tools": [
                {"tool":"codex","found":false,"runnable":false,"version":null},
                {"tool":"claude","found":false,"runnable":false,"version":null}
            ],
            "capabilities":{"curl":false,"npm":false,"brew":false,"nodeVersion":null}
        })),
        "session-env-apply" => {
            let path = environment_file(alias, request["sessionId"].as_str().unwrap());
            fs::write(path, request["port"].to_string()).unwrap();
            Ok(json!({}))
        }
        "session-env-remove" => {
            let path = environment_file(alias, request["sessionId"].as_str().unwrap());
            if let Err(error) = fs::remove_file(path) {
                assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
            }
            Ok(json!({}))
        }
        _ => panic!("unexpected synthetic operation {operation}"),
    }
}

fn wait_until(description: &str, limit: Duration, mut satisfied: impl FnMut() -> bool) {
    let deadline = Instant::now() + limit;
    while !satisfied() {
        assert!(Instant::now() < deadline, "{description}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_post_connect(expected: usize) {
    wait_until("post-connect worker did not finish", LIMIT, || {
        POST_CONNECT_DONE.load(SeqCst) == expected
    });
}

fn connected_target() -> Option<String> {
    let state = lock().unwrap();
    state.child.as_ref()?;
    state.summary.target.as_ref().map(|value| value.id.clone())
}

#[test]
fn connection_lifecycle() {
    use ssh_auth::{acceptance_auth::stage, cancel, finish};

    let normal = stage("normal", target("fixture-a"), fingerprint("fixture-a"));
    assert!(finish("normal").is_ok());
    wait_for_post_connect(1);
    assert_eq!(connected_target().as_deref(), Some("fixture-a"));
    assert_eq!(
        finish("normal").err().as_deref(),
        Some("sshAuthSessionMissing")
    );
    assert_eq!(normal.load(SeqCst), 0);
    disconnect(true).unwrap();
    assert_eq!(normal.load(SeqCst), 1);
    println!("PASS normal completion, duplicate receipt and single release");

    let cancelled = stage("cancelled", target("fixture-a"), fingerprint("fixture-a"));
    cancel("cancelled").unwrap();
    assert_eq!(
        finish("cancelled").err().as_deref(),
        Some("sshAuthSessionMissing")
    );
    assert!(connected_target().is_none());
    assert_eq!(cancelled.load(SeqCst), 1);
    println!("PASS cancel before session handoff");

    let late = stage("late", target("fixture-a"), fingerprint("fixture-a"));
    let (arrived, release) = pause_once("late");
    let task = thread::spawn(|| finish("late"));
    arrived.recv_timeout(LIMIT).unwrap();
    assert_eq!(
        cancel("late").err().as_deref(),
        Some("sshAuthSessionMissing")
    );
    disconnect(true).unwrap();
    release.send(()).unwrap();
    assert!(task.join().unwrap().is_ok());
    wait_for_post_connect(2);
    assert_eq!(connected_target().as_deref(), Some("fixture-a"));
    assert_eq!(late.load(SeqCst), 0);
    disconnect(true).unwrap();
    assert_eq!(late.load(SeqCst), 1);
    println!("OBSERVED DEFECT cancelled/ disconnected completion reacquired Store");

    let (arrived, release) = pause_once("a");
    let previous = stage("a", target("fixture-a"), fingerprint("fixture-a"));
    let task = thread::spawn(|| finish("a"));
    arrived.recv_timeout(LIMIT).unwrap();
    let next = stage("b", target("fixture-b"), fingerprint("fixture-b"));
    release.send(()).unwrap();
    assert!(task.join().unwrap().is_ok());
    wait_for_post_connect(3);
    assert_eq!(connected_target().as_deref(), Some("fixture-a"));
    assert_eq!(finish("b").err().as_deref(), Some("alreadyConnected"));
    assert_eq!(next.load(SeqCst), 1);
    disconnect(true).unwrap();
    assert_eq!(previous.load(SeqCst), 1);
    println!("OBSERVED DEFECT previous target blocked newer target");

    let (arrived, release) = pause_once("exit");
    let _closed = stage("exit", target("fixture-a"), fingerprint("fixture-a"));
    let task = thread::spawn(|| finish("exit"));
    arrived.recv_timeout(LIMIT).unwrap();
    shutdown();
    assert!(is_stopping());
    assert_eq!(report().status, Status::Disconnected);
    release.send(()).unwrap();
    assert!(task.join().unwrap().is_ok());
    assert_eq!(report().status, Status::Connected);
    assert_eq!(
        tray_state::snapshot().connection,
        tray_state::Connection::Connected
    );
    disconnect(true).unwrap();
    println!("OBSERVED DEFECT already dispatched completion republished Connected after STOPPING");
}

struct Transport;
impl ssh::ManagedSsh for Transport {
    fn is_running(&mut self) -> BridgeResult<bool> {
        Ok(true)
    }
}

fn install_post_connect(alias: &str, port: u16) -> (u64, PathBuf) {
    let endpoint = ProxyEndpoint {
        host: "127.0.0.1".into(),
        port: 1,
        protocol: ProxyProtocol::Http,
    };
    let relay = authenticated_relay::AuthenticatedRelay::start(
        endpoint.clone(),
        authenticated_relay::RelayMode::General(ProxyProtocol::Http),
    )
    .unwrap();
    let file = environment_file(alias, relay.session_id());
    let mut state = lock().unwrap();
    state.connection_generation += 1;
    state.child = Some(Box::new(Transport));
    state.target_fingerprint = Some(fingerprint(alias));
    state.proxy_relay = Some(relay);
    state.summary = Summary {
        status: Status::Connected,
        target: Some(target(alias)),
        proxy: Some(Endpoint {
            local: endpoint,
            remote_port: port,
        }),
        ..Summary::default()
    };
    prepare_post_connect(&mut state.summary);
    (state.connection_generation, file)
}

#[test]
fn background_lifecycle() {
    use settings::ProfileSyncState::{Conflict, Synced};
    use tool_adapter::{RemoteToolId::Codex, RemoteToolInstallState};

    let (generation, original_file) = install_post_connect("fixture-a", 27891);
    start_post_connect(generation);
    wait_for_post_connect(1);
    assert_eq!(fs::read_to_string(&original_file).unwrap(), "27891");
    {
        let mut state = lock().unwrap();
        assert_eq!(state.summary.session_environment_state, RuntimeState::Ready);
        assert_eq!(state.summary.post_connect_status, PostConnectStatus::Ready);
        assert_eq!(
            state.summary.tool_installations[0].state,
            RemoteToolInstallState::Missing
        );
        let profile = ProfileSyncTask::claim(&mut state, Codex).unwrap();
        drop(state);
        finish_profile_sync(profile, Conflict);
        finish_profile_sync(profile, Synced);
        let state = lock().unwrap();
        assert!(state.profile_syncing.is_none());
        assert_eq!(state.summary.codex_profile_state, Conflict);
    }
    disconnect(true).unwrap();
    disconnect(true).unwrap();
    assert!(!original_file.exists());
    let count = OPERATIONS
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, op)| op == "session-env-remove")
        .count();
    assert_eq!(count, 1);
    println!("PASS normal environment, discovery and Profile error cleanup, idempotent disconnect");

    let (env_arrived, env_release) = pause_once("session-env-apply:fixture-a");
    let (tools_arrived, tools_release) = pause_once("tool-discover:fixture-a");
    let (generation, old_file) = install_post_connect("fixture-a", 27892);
    start_post_connect(generation);
    env_arrived.recv_timeout(LIMIT).unwrap();
    tools_arrived.recv_timeout(LIMIT).unwrap();
    let old_profile = ProfileSyncTask::claim(&mut lock().unwrap(), Codex).unwrap();
    disconnect(true).unwrap();
    assert!(!old_file.exists());
    let (generation, new_file) = install_post_connect("fixture-b", 27893);
    start_post_connect(generation);
    wait_for_post_connect(2);
    let new_profile = ProfileSyncTask::claim(&mut lock().unwrap(), Codex).unwrap();
    finish_profile_sync(old_profile, Conflict);
    tools_release.send(()).unwrap();
    env_release.send(()).unwrap();
    wait_for_post_connect(3);
    let state = lock().unwrap();
    assert_eq!(state.summary.target.as_ref().unwrap().id, "fixture-b");
    assert_eq!(state.summary.session_environment_state, RuntimeState::Ready);
    assert_eq!(
        state.summary.tool_installations[0].state,
        RemoteToolInstallState::Missing
    );
    assert_eq!(state.profile_syncing, Some(new_profile));
    assert!(state
        .summary
        .timings
        .iter()
        .all(|phase| phase.phase != "remoteTools.discovery" || phase.outcome == "success"));
    drop(state);
    assert_eq!(fs::read_to_string(&new_file).unwrap(), "27893");
    assert_eq!(fs::read_to_string(&old_file).unwrap(), "27892");
    let operations = OPERATIONS.lock().unwrap();
    let last_a: Vec<_> = operations
        .iter()
        .filter(|(id, op)| id == "fixture-a" && op.starts_with("session-env"))
        .map(|(_, op)| op.as_str())
        .collect();
    assert!(last_a.ends_with(&["session-env-remove", "session-env-apply"]));
    drop(operations);
    finish_profile_sync(new_profile, Synced);
    disconnect(true).unwrap();
    assert!(!new_file.exists());
    fs::remove_file(old_file).unwrap();
    println!("PASS old discovery and Profile receipts were isolated from the newer session");
    println!("OBSERVED DEFECT old environment writer recreated its file after cleanup");
}

mod process_exit {
    use super::*;
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt, process::Child};
    use windows::Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
    };

    struct Witness(HANDLE);
    impl Drop for Witness {
        fn drop(&mut self) {
            // This handle was opened for one of the isolated fixture processes.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    struct AppChild(Child);
    impl Drop for AppChild {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }

    struct ManagedResource {
        child: Option<ssh::OwnedChild>,
        dir: PathBuf,
        name: &'static str,
        pause: bool,
    }
    impl ssh::ManagedSsh for ManagedResource {
        fn is_running(&mut self) -> BridgeResult<bool> {
            self.child.as_mut().unwrap().is_running()
        }
    }
    impl Drop for ManagedResource {
        fn drop(&mut self) {
            mark(&self.dir, &format!("{}.drop-started", self.name));
            if self.pause {
                wait_until("test release for occupied destructor", LIMIT, || {
                    self.dir.join("release").exists()
                });
            }
            drop(self.child.take());
            fs::remove_file(self.dir.join(format!("{}.owned", self.name))).unwrap();
            mark(&self.dir, &format!("{}.drop-done", self.name));
        }
    }

    fn mark(dir: &Path, name: &str) {
        let path = dir.join(name);
        let temp = path.with_extension("writing");
        fs::write(&temp, b"fixture").unwrap();
        fs::rename(temp, path).unwrap();
    }

    fn command(name: &str, dir: &Path) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                &format!("{}::{name}", module_path!().split_once("::").unwrap().1),
                "--nocapture",
            ])
            .env("ROUTESPAN_R46_EXIT_DIR", dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    fn exclusive(path: &Path) -> std::io::Result<fs::File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(path)
    }

    #[test]
    fn controlled_resource_process() {
        let dir = PathBuf::from(std::env::var_os("ROUTESPAN_R46_EXIT_DIR").unwrap());
        let name = std::env::var("ROUTESPAN_R46_RESOURCE").unwrap();
        let _lease = exclusive(&dir.join(format!("{name}.lock"))).unwrap();
        mark(&dir, &format!("{name}.ready"));
        // Fallback deadline if the app or its observer fails to reap this fixture.
        thread::sleep(Duration::from_secs(20));
    }

    fn managed_resource(dir: &Path, name: &'static str, pause: bool) -> (u32, ManagedResource) {
        let child = ssh::OwnedChild::spawn(
            command("controlled_resource_process", dir).env("ROUTESPAN_R46_RESOURCE", name),
        )
        .unwrap();
        wait_until("resource process did not start", LIMIT, || {
            dir.join(format!("{name}.ready")).exists()
        });
        mark(dir, &format!("{name}.owned"));
        let pid = child.child.id();
        (
            pid,
            ManagedResource {
                child: Some(child),
                dir: dir.into(),
                name,
                pause,
            },
        )
    }

    #[test]
    fn application_process() {
        let dir = PathBuf::from(std::env::var_os("ROUTESPAN_R46_EXIT_DIR").unwrap());
        let mode = std::env::var("ROUTESPAN_R46_EXIT_MODE").unwrap();
        let (first, primary) = managed_resource(&dir, "primary", mode == "busy");
        let (second, additional) = managed_resource(&dir, "additional", false);
        let upstream = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let relay = authenticated_relay::AuthenticatedRelay::start(
            ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: upstream.local_addr().unwrap().port(),
                protocol: ProxyProtocol::Http,
            },
            authenticated_relay::RelayMode::General(ProxyProtocol::Http),
        )
        .unwrap();
        let port = relay.port();
        {
            let mut state = lock().unwrap();
            state.summary.status = Status::Connected;
            state.child = Some(Box::new(primary));
            state.additional_tunnels.push(Box::new(additional));
            state.proxy_relay = Some(relay);
        }
        assert_eq!(report().status, Status::Connected);
        assert_eq!(
            tray_state::snapshot().connection,
            tray_state::Connection::Connected
        );

        wait_for_exit_request(&dir, &mode, first, second, port);
        run_exit_callback(&dir);
        std::process::exit(0);
    }

    fn wait_for_exit_request(dir: &Path, mode: &str, first: u32, second: u32, port: u16) {
        if mode == "busy" {
            let (begun, active) = mpsc::channel();
            let worker_dir = dir.to_owned();
            thread::spawn(move || {
                let _work = crate::desktop::updates::bridge_operation().unwrap();
                begun.send(()).unwrap();
                wait_until("release of ongoing test operation", LIMIT, || {
                    worker_dir.join("release").exists()
                });
                mark(&worker_dir, "work.done");
            });
            active.recv_timeout(LIMIT).unwrap();
            mark(dir, "work.started");
        }
        if mode == "contended" {
            let (held, acquired) = mpsc::channel();
            let dir = dir.to_owned();
            thread::spawn(move || {
                let _owned = lock().unwrap();
                held.send(()).unwrap();
                wait_until("test release for held Store", LIMIT, || {
                    dir.join("release").exists()
                });
            });
            acquired.recv_timeout(LIMIT).unwrap();
        }
        fs::write(dir.join("processes"), format!("{first} {second} {port}")).unwrap();
        mark(dir, "app.ready");
        wait_until("observer did not request exit", LIMIT, || {
            dir.join("quit").exists()
        });
    }

    fn run_exit_callback(dir: &Path) {
        shutdown();
        shutdown();
        let status = format!(
            "{:?} {:?}",
            report().status,
            tray_state::snapshot().connection
        );
        fs::write(dir.join("callback.result"), status).unwrap();
    }

    fn observe(mode: &str) {
        let dir = scratch().join(mode);
        fs::create_dir(&dir).unwrap();
        let path = dir.join("app.log");
        let output = fs::File::create(&path).unwrap();
        let mut app = AppChild(
            command("application_process", &dir)
                .env("ROUTESPAN_R46_EXIT_MODE", mode)
                .stdout(output.try_clone().unwrap())
                .stderr(output)
                .spawn()
                .unwrap(),
        );
        wait_until("application process did not start", LIMIT, || {
            if let Some(status) = app.0.try_wait().unwrap() {
                panic!(
                    "application process exited before Ready ({status}): {}",
                    fs::read_to_string(&path).unwrap()
                );
            }
            dir.join("app.ready").exists()
        });
        let manifest = fs::read_to_string(dir.join("processes")).unwrap();
        let ids: Vec<u32> = manifest
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let witnesses: Vec<_> = ids[..2]
            .iter()
            .map(|id| {
                // The application publishes PIDs of its own test children before exit.
                let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, *id) }.unwrap();
                assert_eq!(unsafe { WaitForSingleObject(handle, 0) }, WAIT_TIMEOUT);
                Witness(handle)
            })
            .collect();
        let relay = SocketAddr::from(([127, 0, 0, 1], ids[2].try_into().unwrap()));
        TcpStream::connect_timeout(&relay, Duration::from_millis(250)).unwrap();
        for name in ["primary", "additional"] {
            assert_eq!(
                exclusive(&dir.join(format!("{name}.lock")))
                    .unwrap_err()
                    .raw_os_error(),
                Some(32)
            );
        }

        mark(&dir, "quit");
        let mut status = None;
        wait_until("application process did not exit", LIMIT, || {
            status = app.0.try_wait().unwrap();
            status.is_some()
        });
        assert!(
            status.unwrap().success(),
            "{}",
            fs::read_to_string(&path).unwrap()
        );
        let exit_state = fs::read_to_string(dir.join("callback.result")).unwrap();
        // OS resource observations occur after the fixture app process is gone.
        for witness in witnesses {
            assert_eq!(
                unsafe { WaitForSingleObject(witness.0, 1500) },
                WAIT_OBJECT_0
            );
        }
        for name in ["primary", "additional"] {
            drop(exclusive(&dir.join(format!("{name}.lock"))).unwrap());
        }
        assert!(TcpStream::connect_timeout(&relay, Duration::from_millis(250)).is_err());

        let completed = dir.join("shutdown.done").exists();
        let primary_started = dir.join("primary.drop-started").exists();
        let primary_done = dir.join("primary.drop-done").exists();
        let additional_started = dir.join("additional.drop-started").exists();
        let additional_done = dir.join("additional.drop-done").exists();
        assert_eq!(dir.join("primary.owned").exists(), !primary_done);
        assert_eq!(dir.join("additional.owned").exists(), !additional_done);
        println!("{mode}: exit state={exit_state}, worker={completed}, primary={primary_started}/{primary_done}, additional={additional_started}/{additional_done}, children/handles/listener gone");
        match mode {
            "normal" => {
                assert_eq!(exit_state, "Disconnected Disconnected");
                assert!(
                    completed
                        && primary_started
                        && primary_done
                        && additional_started
                        && additional_done
                );
            }
            "busy" => {
                assert_eq!(exit_state, "Disconnected Disconnected");
                assert!(dir.join("work.started").exists());
                assert!(!dir.join("work.done").exists());
                assert!(
                    !completed
                        && primary_started
                        && !primary_done
                        && !additional_started
                        && !additional_done
                );
                println!("OBSERVED DEFECT application exit abandoned an active destructor");
            }
            "contended" => {
                assert_eq!(exit_state, "Connected Connected");
                assert!(
                    !completed
                        && !primary_started
                        && !primary_done
                        && !additional_started
                        && !additional_done
                );
                println!(
                    "OBSERVED DEFECT application exit abandoned cleanup before Store retirement"
                );
            }
            _ => panic!("unknown test mode"),
        }
        mark(&dir, "release");
    }

    pub(super) fn run() {
        for mode in ["normal", "busy", "contended"] {
            observe(mode);
        }
    }
}

#[test]
fn application_exit() {
    process_exit::run();
}
