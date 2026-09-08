pub mod extension;
mod mobaxterm;
mod ssh;
pub mod ssh_auth;
pub(crate) mod vscode;
use super::proxy::{active, plan, ProxyEndpoint, ProxyProtocol, ProxyVariable};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    net::{SocketAddr, TcpStream},
    sync::{Mutex, OnceLock},
    time::Duration,
};

pub type BridgeResult<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemoteTargetSource {
    Openssh,
    Vscode,
    Mobaxterm,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemoteTargetCompatibility {
    Compatible,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SshAuthMode {
    #[default]
    NonInteractive,
    Interactive,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SshAuthMethod {
    IdentityFile,
    Agent,
    Password,
    KeyboardInteractive,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SshAuthState {
    pub mode: SshAuthMode,
    pub method: SshAuthMethod,
    pub authenticated: bool,
    pub password_stored: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTarget {
    pub id: String,
    pub display_name: String,
    pub source: RemoteTargetSource,
    pub source_label: String,
    pub config_path: String,
    pub ssh_alias: Option<String>,
    pub host: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub available: bool,
    pub compatibility: RemoteTargetCompatibility,
    pub unavailable_reason: Option<String>,
    pub can_open_vscode: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub target_id: String,
    pub proxy_port: Option<u16>,
    pub cc_port: Option<u16>,
    pub cc_local_port: u16,
    pub expected_revision: u64,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Stale,
    Unavailable,
    Error,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub local: ProxyEndpoint,
    pub remote_port: u16,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub status: Status,
    pub target: Option<RemoteTarget>,
    pub proxy: Option<Endpoint>,
    pub cc: Option<Endpoint>,
    pub proxy_status: Option<Status>,
    pub cc_status: Option<Status>,
    pub active_proxy_revision: Option<u64>,
    pub environment: String,
    pub codex_configured: bool,
    pub claude_configured: bool,
    pub codex_extension: Option<String>,
    pub claude_extension: Option<String>,
    pub error: Option<String>,
    pub ssh_auth: SshAuthState,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub configured: bool,
    pub reachable: bool,
    pub status: Status,
    pub protocol: Option<ProxyProtocol>,
    pub proxy_port: Option<u16>,
    pub proxy_status: Option<Status>,
    pub cc_detected: bool,
    pub cc_port: Option<u16>,
    pub cc_status: Option<Status>,
    pub codex_configured: bool,
    pub claude_configured: bool,
    pub codex_extension: Option<String>,
    pub claude_extension: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPreview {
    pub id: String,
    pub tool: String,
    pub path: String,
    pub before: String,
    pub after: String,
    pub version: String,
    pub launch: String,
    pub alias: String,
    pub restore: bool,
    pub onboarding_required: bool,
}
struct Pending {
    preview: ConfigPreview,
    hash: String,
    target_id: String,
    port: u16,
    backup_hash: Option<String>,
    target_fingerprint: String,
    state_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortAllocation {
    pub proxy_port: u16,
    pub cc_port: u16,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CcDetectionState {
    Confirmed,
    ListeningUnknown,
    NotDetected,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcDetection {
    pub state: CcDetectionState,
    pub local_port: u16,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemoteInternetState {
    Reachable,
    Unreachable,
    Unknown,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteNetworkObservation {
    pub server_internet: RemoteInternetState,
}
#[derive(Default)]
struct Store {
    summary: Summary,
    child: Option<Box<dyn ssh::ManagedSsh>>,
    pending: Option<Pending>,
    extension_pending: Option<extension::Pending>,
    cc_detected: bool,
    reachable: bool,
    target_fingerprint: Option<String>,
    proxy_status: Status,
    cc_status: Status,
    ssh_auth: SshAuthState,
}
static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
fn store() -> &'static Mutex<Store> {
    STORE.get_or_init(|| Mutex::new(Store::default()))
}
fn lock() -> BridgeResult<std::sync::MutexGuard<'static, Store>> {
    store().lock().map_err(|_| "stateUnavailable".into())
}
fn listening(endpoint: &ProxyEndpoint) -> bool {
    endpoint.host.parse().ok().is_some_and(|ip| {
        TcpStream::connect_timeout(
            &SocketAddr::new(ip, endpoint.port),
            Duration::from_millis(350),
        )
        .is_ok()
    })
}
fn port(value: u16) -> BridgeResult<()> {
    if value >= 1024 {
        Ok(())
    } else {
        Err("invalidPort".into())
    }
}
fn remote_environment(endpoint: &Endpoint) -> BridgeResult<String> {
    let remote = ProxyEndpoint {
        host: "127.0.0.1".into(),
        port: endpoint.remote_port,
        protocol: endpoint.local.protocol,
    };
    Ok(plan::build_proxy_environment_plan(
        &remote,
        &[
            ProxyVariable::Http,
            ProxyVariable::Https,
            ProxyVariable::All,
        ],
    )
    .map_err(|_| "proxyUnavailable")?
    .variables
    .into_iter()
    .filter_map(|entry| {
        entry.value.map(|value| {
            // Share the existing protocol/variable mapping; remote SOCKS DNS
            // must also travel through the tunnel, as specified for the bridge.
            format!(
                "export {}={}",
                entry.name.to_uppercase(),
                value.replacen("socks5://", "socks5h://", 1)
            )
        })
    })
    .collect::<Vec<_>>()
    .join("\n"))
}
fn observed_status(
    summary: &Summary,
    current: Option<&active::ActiveProxyContext>,
    endpoints_available: bool,
) -> Status {
    if summary.proxy.is_some() && current.is_none_or(|c| !c.available) {
        Status::Unavailable
    } else if summary.proxy.is_some()
        && current.map(|c| c.revision) != summary.active_proxy_revision
    {
        Status::Stale
    } else if !endpoints_available {
        Status::Unavailable
    } else {
        Status::Connected
    }
}
fn refresh(state: &mut Store) {
    if let Some(child) = state.child.as_mut() {
        if !matches!(child.is_running(), Ok(true)) {
            state.child = None;
            state.summary.status = Status::Disconnected;
            state.proxy_status = Status::Disconnected;
            state.cc_status = Status::Disconnected;
            state.summary.proxy_status = state.summary.proxy.as_ref().map(|_| Status::Disconnected);
            state.summary.cc_status = state.summary.cc.as_ref().map(|_| Status::Disconnected);
            state.reachable = false;
            state.pending = None;
            state.extension_pending = None;
            return;
        }
        let current = active::snapshot().ok();
        let proxy_available = state
            .summary
            .proxy
            .as_ref()
            .is_none_or(|e| listening(&e.local));
        let cc_available = state
            .summary
            .cc
            .as_ref()
            .is_none_or(|e| listening(&e.local));
        state.proxy_status = observed_status(&state.summary, current.as_ref(), proxy_available);
        state.cc_status = if cc_available {
            Status::Connected
        } else {
            Status::Unavailable
        };
        state.summary.proxy_status = state.summary.proxy.as_ref().map(|_| state.proxy_status);
        state.summary.cc_status = state.summary.cc.as_ref().map(|_| state.cc_status);
        let endpoints_available = proxy_available && cc_available;
        state.summary.status =
            observed_status(&state.summary, current.as_ref(), endpoints_available);
    }
}
pub fn start_monitor() {
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(2));
        ssh_auth::cleanup();
        if let Ok(mut state) = store().try_lock() {
            refresh(&mut state);
        }
    });
}
pub fn summary() -> BridgeResult<Summary> {
    Ok(lock()?.summary.clone())
}
pub fn report() -> Report {
    // Report generation reads cached observations only: no SSH or socket probes.
    let Ok(state) = store().try_lock() else {
        return Report::default();
    };
    let s = &state.summary;
    Report {
        configured: s.target.is_some(),
        reachable: state.reachable,
        status: s.status,
        protocol: s.proxy.as_ref().map(|e| e.local.protocol),
        proxy_port: s.proxy.as_ref().map(|e| e.remote_port),
        proxy_status: s.proxy.as_ref().map(|_| state.proxy_status),
        cc_detected: state.cc_detected,
        cc_port: s.cc.as_ref().map(|e| e.remote_port),
        cc_status: s.cc.as_ref().map(|_| state.cc_status),
        codex_configured: s.codex_configured,
        claude_configured: s.claude_configured,
        codex_extension: s.codex_extension.clone(),
        claude_extension: s.claude_extension.clone(),
    }
}
pub fn targets() -> BridgeResult<Vec<RemoteTarget>> {
    ssh::targets()
}
pub fn check(target_id: String) -> BridgeResult<PortAllocation> {
    ssh::validate_target(&target_id)?;
    ssh::remote(&target_id, json!({"operation":"check","ports":[]}))?;
    let mut state = lock()?;
    state.reachable = true;
    state.ssh_auth = SshAuthState {
        mode: SshAuthMode::NonInteractive,
        method: ssh::non_interactive_auth_method(&target_id),
        authenticated: true,
        password_stored: false,
    };
    state.summary.ssh_auth = state.ssh_auth;
    drop(state);
    allocate_ports(target_id)
}
pub fn check_remote_network(target_id: String) -> BridgeResult<RemoteNetworkObservation> {
    ssh::validate_target(&target_id)?;
    let value = ssh::remote(&target_id, json!({"operation":"internet"}))?;
    let server_internet = match value.get("internet").and_then(|entry| entry.as_str()) {
        Some("reachable") => RemoteInternetState::Reachable,
        Some("unreachable") => RemoteInternetState::Unreachable,
        Some("unknown") => RemoteInternetState::Unknown,
        _ => return Err("remoteUnsupported".into()),
    };
    Ok(RemoteNetworkObservation { server_internet })
}
pub fn allocate_ports(target_id: String) -> BridgeResult<PortAllocation> {
    ssh::validate_target(&target_id)?;
    for _ in 0..24 {
        let mut bytes = [0u8; 4];
        getrandom::fill(&mut bytes).map_err(|_| "stateUnavailable")?;
        let proxy_port = 20_000 + (u16::from_le_bytes([bytes[0], bytes[1]]) % 40_001);
        let cc_port = 20_000 + (u16::from_le_bytes([bytes[2], bytes[3]]) % 40_001);
        if proxy_port == cc_port {
            continue;
        }
        match ssh::remote(
            &target_id,
            json!({"operation":"check","ports":[proxy_port,cc_port]}),
        ) {
            Ok(_) => {
                return Ok(PortAllocation {
                    proxy_port,
                    cc_port,
                })
            }
            Err(code) if code == "portInUse" => continue,
            Err(code) => return Err(code),
        }
    }
    Err("portAllocationFailed".into())
}
pub fn detect_cc(local_port: u16) -> BridgeResult<CcDetection> {
    port(local_port)?;
    let listener = super::proxy::listeners::enumerate()
        .map_err(|_| "stateUnavailable")?
        .into_iter()
        .find(|listener| listener.port == local_port);
    let state = if let Some(listener) = listener {
        let processes = super::proxy::processes::enumerate();
        if listener.pids.iter().any(|pid| {
            processes.get(pid).is_some_and(|process| {
                let name = process.name.replace(['-', '_', ' '], "");
                name.contains("ccswitch")
            })
        }) {
            CcDetectionState::Confirmed
        } else {
            CcDetectionState::ListeningUnknown
        }
    } else {
        CcDetectionState::NotDetected
    };
    lock()?.cc_detected = state != CcDetectionState::NotDetected;
    Ok(CcDetection { state, local_port })
}
pub fn preview(request: &Request) -> BridgeResult<Summary> {
    let target = ssh::target(&request.target_id)?;
    if !target.available || target.compatibility != RemoteTargetCompatibility::Compatible {
        return Err(target
            .unavailable_reason
            .unwrap_or_else(|| "targetUnsupported".into()));
    }
    if request.proxy_port.is_none() && request.cc_port.is_none() {
        return Err("noCapability".into());
    }
    if request.proxy_port.is_some() && request.proxy_port == request.cc_port {
        return Err("portInUse".into());
    }
    let context = active::snapshot().map_err(|_| "stateUnavailable")?;
    let proxy = if let Some(remote_port) = request.proxy_port {
        port(remote_port)?;
        if context.revision != request.expected_revision {
            return Err("activeChanged".into());
        }
        let candidate = context.available_candidate().ok_or("proxyUnavailable")?;
        let local = plan::validate_and_normalize_endpoint(&active::endpoint(candidate))
            .map_err(|_| "proxyUnavailable")?;
        if !listening(&local) {
            return Err("proxyUnavailable".into());
        }
        Some(Endpoint { local, remote_port })
    } else {
        None
    };
    let cc = if let Some(remote_port) = request.cc_port {
        port(remote_port)?;
        port(request.cc_local_port)?;
        let local = ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: request.cc_local_port,
            protocol: ProxyProtocol::Http,
        };
        if !listening(&local) {
            return Err("ccUnavailable".into());
        }
        Some(Endpoint { local, remote_port })
    } else {
        None
    };
    let environment = proxy
        .as_ref()
        .map(remote_environment)
        .transpose()?
        .unwrap_or_default();
    Ok(Summary {
        target: Some(target),
        proxy,
        cc,
        environment,
        active_proxy_revision: request.proxy_port.map(|_| context.revision),
        ..Summary::default()
    })
}

pub(super) fn complete_interactive_check(auth: SshAuthState) {
    if let Ok(mut state) = lock() {
        state.reachable = true;
        state.ssh_auth = auth;
        state.summary.ssh_auth = auth;
    }
}

pub(super) struct InteractiveConnectPlan {
    pub summary: Summary,
    pub fingerprint: String,
    pub endpoints: Vec<(u16, String, u16)>,
}

pub(super) fn prepare_interactive_connect(
    request: &Request,
) -> BridgeResult<InteractiveConnectPlan> {
    {
        let state = lock()?;
        if state.child.is_some() {
            return Err("alreadyConnected".into());
        }
    }
    let fingerprint = ssh::fingerprint(&request.target_id)?;
    let summary = preview(request)?;
    let endpoints = summary
        .proxy
        .iter()
        .chain(summary.cc.iter())
        .map(|endpoint| {
            (
                endpoint.remote_port,
                endpoint.local.host.clone(),
                endpoint.local.port,
            )
        })
        .collect();
    Ok(InteractiveConnectPlan {
        summary,
        fingerprint,
        endpoints,
    })
}

pub(super) fn mark_interactive_connecting() -> BridgeResult<()> {
    let mut state = lock()?;
    if state.child.is_some() {
        return Err("alreadyConnected".into());
    }
    state.pending = None;
    state.extension_pending = None;
    state.summary.status = Status::Connecting;
    state.summary.error = None;
    Ok(())
}

pub(super) fn cancel_interactive_connect() {
    if let Ok(mut state) = lock() {
        if state.child.is_none() && state.summary.status == Status::Connecting {
            state.summary.status = Status::Disconnected;
            state.summary.error = None;
        }
    }
}

pub(super) fn complete_interactive_connect(
    request: Request,
    mut summary: Summary,
    fingerprint: String,
    process: ssh_auth::PtyProcess,
    auth: SshAuthState,
) -> BridgeResult<Summary> {
    if ssh::fingerprint(&request.target_id)? != fingerprint {
        return Err("sshConfigChanged".into());
    }
    if request.proxy_port.is_some() {
        preview(&request)?;
    }
    summary.status = Status::Connected;
    summary.proxy_status = summary.proxy.as_ref().map(|_| Status::Connected);
    summary.cc_status = summary.cc.as_ref().map(|_| Status::Connected);
    summary.ssh_auth = auth;
    let mut state = lock()?;
    if state.child.is_some() {
        return Err("alreadyConnected".into());
    }
    state.summary = summary;
    state.proxy_status = Status::Connected;
    state.cc_status = Status::Connected;
    state.child = Some(Box::new(process));
    state.reachable = true;
    state.target_fingerprint = Some(fingerprint);
    state.ssh_auth = auth;
    Ok(state.summary.clone())
}

pub fn connect(request: Request, confirmed: bool) -> BridgeResult<Summary> {
    if !confirmed {
        return Err("confirmationRequired".into());
    }
    let mut state = lock()?;
    if state.child.is_some() {
        return Err("alreadyConnected".into());
    }
    state.pending = None;
    let result = (|| {
        state.extension_pending = None;
        let fingerprint = ssh::fingerprint(&request.target_id)?;
        let mut next = preview(&request)?;
        state.summary.status = Status::Connecting;
        let endpoints: Vec<_> = next
            .proxy
            .iter()
            .chain(next.cc.iter())
            .map(|e| (e.remote_port, e.local.host.clone(), e.local.port))
            .collect();
        let ports: Vec<_> = endpoints.iter().map(|e| e.0).collect();
        ssh::remote(
            &request.target_id,
            json!({"operation":"check","ports":ports}),
        )?;
        let mut child = ssh::tunnel(&request, &endpoints)?;
        let mut verified = false;
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(400));
            if child
                .child
                .try_wait()
                .map_err(|_| "processFailed")?
                .is_some()
            {
                return Err("forwardDenied".into());
            }
            match ssh::remote(
                &request.target_id,
                json!({"operation":"verify","ports":ports}),
            ) {
                Ok(_) => {
                    verified = true;
                    break;
                }
                Err(code) if code == "unsafeBinding" => return Err(code),
                Err(_) => (),
            }
        }
        if !verified {
            return Err("forwardDenied".into());
        }
        if ssh::fingerprint(&request.target_id)? != fingerprint {
            return Err("sshConfigChanged".into());
        }
        // Recheck captured revision after the asynchronous SSH handshake.
        if request.proxy_port.is_some() {
            preview(&request)?;
        }
        next.status = Status::Connected;
        next.proxy_status = next.proxy.as_ref().map(|_| Status::Connected);
        next.cc_status = next.cc.as_ref().map(|_| Status::Connected);
        state.summary = next;
        state.proxy_status = Status::Connected;
        state.cc_status = Status::Connected;
        state.child = Some(Box::new(child));
        state.ssh_auth = SshAuthState {
            mode: SshAuthMode::NonInteractive,
            method: ssh::non_interactive_auth_method(&request.target_id),
            authenticated: true,
            password_stored: false,
        };
        state.summary.ssh_auth = state.ssh_auth;
        state.reachable = true;
        state.target_fingerprint = Some(fingerprint);
        Ok(state.summary.clone())
    })();
    if let Err(code) = &result {
        state.summary.status = Status::Error;
        state.summary.error = Some(code.clone());
    }
    result
}
pub fn disconnect(confirmed: bool) -> BridgeResult<Summary> {
    if !confirmed {
        return Err("confirmationRequired".into());
    }
    let mut state = lock()?;
    state.child = None;
    state.pending = None;
    state.extension_pending = None;
    state.summary.status = Status::Disconnected;
    state.summary.proxy_status = state.summary.proxy.as_ref().map(|_| Status::Disconnected);
    state.summary.cc_status = state.summary.cc.as_ref().map(|_| Status::Disconnected);
    state.proxy_status = Status::Disconnected;
    state.cc_status = Status::Disconnected;
    Ok(state.summary.clone())
}
pub fn shutdown() {
    // Never hold up application exit behind an SSH timeout. Windows closes all
    // outstanding kill-on-close jobs even when an operation owns the mutex.
    if let Ok(mut state) = store().try_lock() {
        state.child = None;
    }
    ssh_auth::shutdown();
}
pub fn test() -> BridgeResult<()> {
    let mut state = lock()?;
    refresh(&mut state);
    if state.summary.status != Status::Connected {
        return Err("bridgeUnavailable".into());
    }
    let endpoint = state.summary.proxy.as_ref().ok_or("proxyUnavailable")?;
    let target_id = state
        .summary
        .target
        .as_ref()
        .map(|target| target.id.as_str())
        .ok_or("invalidTarget")?;
    if Some(ssh::fingerprint(target_id)?) != state.target_fingerprint {
        return Err("sshConfigChanged".into());
    }
    ssh::remote(
        target_id,
        json!({"operation":"test","port":endpoint.remote_port,"protocol":endpoint.local.protocol}),
    )?;
    Ok(())
}
fn overlay(tool: &str, port: u16) -> String {
    if tool == "codex" {
        format!("# ProxyEnv Remote Bridge\nmodel_provider = \"proxyenv_bridge\"\n\n[model_providers.proxyenv_bridge]\nname = \"ProxyEnv CC Switch\"\nbase_url = \"http://127.0.0.1:{port}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\n")
    } else {
        format!("{{\"env\":{{\"ANTHROPIC_BASE_URL\":\"http://127.0.0.1:{port}\",\"ANTHROPIC_AUTH_TOKEN\":\"PROXY_MANAGED\"}}}}\n")
    }
}
pub fn config_preview(tool: String) -> BridgeResult<ConfigPreview> {
    if !["codex", "claude"].contains(&tool.as_str()) {
        return Err("invalidRequest".into());
    }
    let mut state = lock()?;
    refresh(&mut state);
    if state.child.is_none() || state.summary.cc.is_none() {
        return Err("bridgeUnavailable".into());
    }
    let target_id = state
        .summary
        .target
        .as_ref()
        .map(|target| target.id.clone())
        .ok_or("invalidTarget")?;
    let target_fingerprint = ssh::fingerprint(&target_id)?;
    if state.target_fingerprint.as_ref() != Some(&target_fingerprint) {
        return Err("sshConfigChanged".into());
    }
    if state
        .summary
        .cc
        .as_ref()
        .is_none_or(|e| !listening(&e.local))
    {
        return Err("ccUnavailable".into());
    }
    let port = state
        .summary
        .cc
        .as_ref()
        .ok_or("ccUnavailable")?
        .remote_port;
    let value = ssh::remote(
        &target_id,
        json!({"operation":"preview","tool":tool,"port":port}),
    )?;
    let hash = value["expectedHash"]
        .as_str()
        .ok_or("remoteFailed")?
        .to_string();
    let state_hash = value["stateHash"]
        .as_str()
        .ok_or("remoteFailed")?
        .to_string();
    let onboarding_required = value["onboardingRequired"]
        .as_bool()
        .ok_or("remoteFailed")?;
    let before = match value["previousPort"].as_u64() {
        Some(p) if (1024..=65535).contains(&p) => overlay(&tool, p as u16),
        None if value["previousPort"].is_null() => String::new(),
        _ => return Err("remoteFailed".into()),
    };
    let version = value["version"]
        .as_str()
        .filter(|s| s.len() < 32 && s.bytes().all(|b| b.is_ascii_digit() || b == b'.'))
        .ok_or("remoteFailed")?
        .to_owned();
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| "stateUnavailable")?;
    let preview = ConfigPreview {
        id: hex::encode(nonce),
        tool: tool.clone(),
        path: if tool == "codex" {
            "~/.codex/proxyenv_bridge.config.toml"
        } else {
            "~/.claude/proxyenv-bridge.json"
        }
        .into(),
        before,
        after: overlay(&tool, port),
        version,
        launch: if tool == "codex" {
            "codex --profile proxyenv_bridge"
        } else {
            "claude --settings \"$HOME/.claude/proxyenv-bridge.json\""
        }
        .into(),
        alias: target_id.clone(),
        restore: false,
        onboarding_required,
    };
    state.pending = Some(Pending {
        preview: preview.clone(),
        hash,
        target_id,
        port,
        backup_hash: None,
        target_fingerprint,
        state_hash,
    });
    Ok(preview)
}
pub fn config_apply(id: String, confirmed: bool) -> BridgeResult<()> {
    if !confirmed {
        return Err("confirmationRequired".into());
    }
    let mut state = lock()?;
    refresh(&mut state);
    if state.child.is_none() {
        return Err("bridgeUnavailable".into());
    }
    let pending = state
        .pending
        .take()
        .filter(|p| p.preview.id == id && !p.preview.restore)
        .ok_or("configConflict")?;
    if ssh::fingerprint(&pending.target_id)? != pending.target_fingerprint {
        return Err("sshConfigChanged".into());
    }
    if state
        .summary
        .cc
        .as_ref()
        .is_none_or(|e| !listening(&e.local))
    {
        return Err("ccUnavailable".into());
    }
    ssh::remote(
        &pending.target_id,
        json!({"operation":"apply","tool":pending.preview.tool,"port":pending.port,"expectedHash":pending.hash,"stateHash":pending.state_hash}),
    )?;
    if pending.preview.tool == "codex" {
        state.summary.codex_configured = true;
    } else {
        state.summary.claude_configured = true;
    }
    Ok(())
}
pub fn config_restore_preview(target_id: String, tool: String) -> BridgeResult<ConfigPreview> {
    if !["codex", "claude"].contains(&tool.as_str()) {
        return Err("invalidRequest".into());
    }
    let target_fingerprint = ssh::fingerprint(&target_id)?;
    let mut state = lock()?;
    let value = ssh::remote(
        &target_id,
        json!({"operation":"restore-preview","tool":tool}),
    )?;
    let content = |key: &str| -> BridgeResult<String> {
        match value[key].as_u64() {
            Some(p) if (1024..=65535).contains(&p) => Ok(overlay(&tool, p as u16)),
            None if value[key].is_null() => Ok(String::new()),
            _ => Err("remoteFailed".into()),
        }
    };
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| "stateUnavailable")?;
    let preview = ConfigPreview {
        id: hex::encode(nonce),
        alias: target_id.clone(),
        tool: tool.clone(),
        restore: true,
        onboarding_required: false,
        path: if tool == "codex" {
            "~/.codex/proxyenv_bridge.config.toml"
        } else {
            "~/.claude/proxyenv-bridge.json"
        }
        .into(),
        before: content("previousPort")?,
        after: content("originalPort")?,
        version: String::new(),
        launch: String::new(),
    };
    state.pending = Some(Pending {
        preview: preview.clone(),
        target_id,
        port: 25721,
        hash: value["expectedHash"].as_str().ok_or("remoteFailed")?.into(),
        backup_hash: Some(value["backupHash"].as_str().ok_or("remoteFailed")?.into()),
        target_fingerprint,
        state_hash: "absent".into(),
    });
    Ok(preview)
}
pub fn config_restore(id: String, confirmed: bool) -> BridgeResult<()> {
    if !confirmed {
        return Err("confirmationRequired".into());
    }
    let mut state = lock()?;
    let pending = state
        .pending
        .take()
        .filter(|p| p.preview.id == id && p.preview.restore)
        .ok_or("configConflict")?;
    if ssh::fingerprint(&pending.target_id)? != pending.target_fingerprint {
        return Err("sshConfigChanged".into());
    }
    ssh::remote(
        &pending.target_id,
        json!({"operation":"restore","tool":pending.preview.tool,"expectedHash":pending.hash,"backupHash":pending.backup_hash}),
    )?;
    if state.summary.target.as_ref().map(|target| &target.id) == Some(&pending.target_id) {
        if pending.preview.tool == "codex" {
            state.summary.codex_configured = false;
        } else {
            state.summary.claude_configured = false;
        }
    }
    state.pending = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_environment_obeys_protocol_and_uses_remote_socks_dns() {
        let endpoint = |protocol| Endpoint {
            local: ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: 10809,
                protocol,
            },
            remote_port: 17897,
        };
        assert_eq!(
            remote_environment(&endpoint(ProxyProtocol::Http)).unwrap(),
            "export HTTP_PROXY=http://127.0.0.1:17897\nexport HTTPS_PROXY=http://127.0.0.1:17897"
        );
        assert_eq!(
            remote_environment(&endpoint(ProxyProtocol::Socks5)).unwrap(),
            "export ALL_PROXY=socks5h://127.0.0.1:17897"
        );
        assert_eq!(
            remote_environment(&endpoint(ProxyProtocol::Mixed))
                .unwrap()
                .lines()
                .count(),
            3
        );
        assert!(remote_environment(&endpoint(ProxyProtocol::Unknown)).is_err());
    }
    #[test]
    fn dedicated_overlays_have_only_bridge_fields() {
        let codex: toml::Value = toml::from_str(&overlay("codex", 25721)).unwrap();
        assert_eq!(codex["model_provider"].as_str(), Some("proxyenv_bridge"));
        assert_eq!(codex.as_table().unwrap().len(), 2);
        let claude: serde_json::Value = serde_json::from_str(&overlay("claude", 25721)).unwrap();
        assert_eq!(claude["env"]["ANTHROPIC_AUTH_TOKEN"], "PROXY_MANAGED");
    }
    #[test]
    fn remote_ports_exclude_privileged_range() {
        assert!(port(1023).is_err());
        assert!(port(1024).is_ok());
        assert!(port(65535).is_ok());
    }
    #[test]
    fn revision_changes_never_retarget_an_existing_tunnel() {
        let original = Endpoint {
            local: ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: 10809,
                protocol: ProxyProtocol::Mixed,
            },
            remote_port: 17897,
        };
        let summary = Summary {
            proxy: Some(original),
            active_proxy_revision: Some(8),
            ..Summary::default()
        };
        let mut context = active::ActiveProxyContext {
            available: true,
            revision: 8,
            ..Default::default()
        };
        assert_eq!(
            observed_status(&summary, Some(&context), true),
            Status::Connected
        );
        context.revision = 9;
        assert_eq!(
            observed_status(&summary, Some(&context), true),
            Status::Stale
        );
        context.available = false;
        assert_eq!(
            observed_status(&summary, Some(&context), true),
            Status::Unavailable
        );
        assert_eq!(summary.proxy.unwrap().local.port, 10809);
        assert_eq!(
            observed_status(&Summary::default(), None, true),
            Status::Connected
        );
        assert_eq!(
            observed_status(&Summary::default(), None, false),
            Status::Unavailable
        );
    }
    #[test]
    fn request_cannot_override_local_endpoint_or_add_commands() {
        let valid = json!({"targetId":"openssh|example|dev-vm","proxyPort":23841,"ccPort":null,"ccLocalPort":15721,"expectedRevision":8});
        assert!(serde_json::from_value::<Request>(valid.clone()).is_ok());
        let mut value = valid;
        value["localHost"] = json!("0.0.0.0");
        assert!(serde_json::from_value::<Request>(value).is_err());
        for invalid in [-1, 65536] {
            let value = json!({"targetId":"openssh|example|dev-vm","proxyPort":invalid,"ccPort":null,"ccLocalPort":15721,"expectedRevision":8});
            assert!(serde_json::from_value::<Request>(value).is_err());
        }
    }
    #[test]
    fn report_does_not_expose_target_or_paths() {
        let value = serde_json::to_string(&Report::default()).unwrap();
        for forbidden in ["alias", "host", "path", "before", "after", "environment"] {
            assert!(!value.contains(forbidden));
        }
    }
}
