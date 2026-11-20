#[path = "relay/authenticated_relay.rs"]
mod authenticated_relay;
mod capabilities;
pub use capabilities::{set_capability, CapabilityChange};
#[allow(dead_code)]
#[path = "relay/codex_relay.rs"]
mod codex_relay;
mod config_transaction;
use config_transaction::{config_apply_locked, config_preview_locked, verify_tool_for_generation};
pub use config_transaction::{config_restore, config_restore_preview};
pub(crate) mod connections;
#[path = "ssh/credential_cache.rs"]
pub(crate) mod credential_cache;
pub mod diagnostics;
pub mod events;
pub mod extension;
pub mod logging;
pub mod observations;
pub mod security;
use events::{Action as EventAction, Component as EventComponent, Outcome as EventOutcome};
use observations::{DiagnosticSnapshot, DiagnosticState, ErrorCategory};
mod local_model;
pub(crate) mod mobaxterm;
mod models;
pub use models::{
    RemoteTarget, RemoteTargetCompatibility, RemoteTargetSource, Request, SshAuthMethod,
    SshAuthMode, SshAuthState,
};
mod port_allocation;
use port_allocation::{derived_port, first_available_port, resolve_port_pair};
mod projection;
use projection::{project_report, projected_session_command, sync_tool_states};
mod reconnect;
pub(crate) mod settings;
mod skills;
mod ssh;
#[path = "ssh/ssh_auth.rs"]
pub mod ssh_auth;
mod target_os;
pub use target_os::windows::WindowsTargetProbe;
pub mod tool_adapter;
pub mod tool_discovery;
mod tool_state;
use tool_state::{
    automatic_tool_setup_allowed, discovery_context_current, invalidate_tool_discovery,
};
pub(crate) mod tray_state;
pub(crate) mod vscode;
mod vscode_network;
#[cfg(test)]
use super::proxy::ProxyVariable;
use super::proxy::{active, plan, ProxyEndpoint, ProxyProtocol};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    net::{SocketAddr, TcpStream},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

pub type BridgeResult<T> = std::result::Result<T, String>;

pub const DEFAULT_CC_REMOTE_PORT: u16 = 15_721;

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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PostConnectStatus {
    #[default]
    Idle,
    Preparing,
    Ready,
    Partial,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeState {
    #[default]
    Pending,
    Preparing,
    Ready,
    Warning,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PhaseTiming {
    pub phase: String,
    pub duration_ms: u64,
    pub outcome: String,
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
    pub reconnect_state: reconnect::State,
    pub status: Status,
    pub target: Option<RemoteTarget>,
    pub proxy: Option<Endpoint>,
    pub runtime_expected_proxy_port: Option<u16>,
    pub runtime_proxy_match: RuntimeProxyMatch,
    pub cc: Option<Endpoint>,
    pub proxy_status: Option<Status>,
    pub cc_status: Option<Status>,
    pub active_proxy_revision: Option<u64>,
    pub environment: String,
    pub codex_configured: bool,
    pub claude_configured: bool,
    pub codex_profile_state: settings::ProfileSyncState,
    pub claude_profile_state: settings::ProfileSyncState,
    #[serde(skip)]
    pub(crate) codex_verification: tool_adapter::RemoteToolVerification,
    #[serde(skip)]
    pub(crate) claude_verification: tool_adapter::RemoteToolVerification,
    pub tools: Vec<tool_adapter::RemoteToolState>,
    pub tool_installations: Vec<tool_adapter::RemoteToolInstallation>,
    pub install_capabilities: tool_discovery::RemoteInstallCapabilities,
    pub codex_extension: Option<String>,
    pub claude_extension: Option<String>,
    pub error: Option<String>,
    pub ssh_auth: SshAuthState,
    pub post_connect_status: PostConnectStatus,
    pub session_environment_state: RuntimeState,
    pub session_environment_command: Option<String>,
    pub vscode_state: RuntimeState,
    pub codex_state: RuntimeState,
    pub claude_state: RuntimeState,
    pub skills_state: RuntimeState,
    pub post_connect_error: Option<String>,
    pub timings: Vec<PhaseTiming>,
    pub diagnostics: DiagnosticSnapshot,
    pub security: security::SecuritySnapshot,
}
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub tool_installations: Vec<tool_adapter::RemoteToolInstallation>,
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
    pub reconnect_state: reconnect::State,
    pub ssh_auth_method: SshAuthMethod,
    pub runtime_proxy_match: RuntimeProxyMatch,
    pub post_connect_status: PostConnectStatus,
    pub session_environment_state: RuntimeState,
    pub vscode_state: RuntimeState,
    pub codex_state: RuntimeState,
    pub claude_state: RuntimeState,
    pub skills_state: RuntimeState,
    pub post_connect_error_category: Option<ErrorCategory>,
    pub phase_timings: Vec<PhaseTiming>,
    pub diagnostics: DiagnosticSnapshot,
    pub security: security::SecuritySnapshot,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPreview {
    pub id: String,
    pub tool: tool_adapter::RemoteToolId,
    pub path: String,
    pub before: String,
    pub after: String,
    pub version: String,
    pub launch: String,
    pub alias: String,
    pub restore: bool,
    pub existing_config: bool,
    pub route_update: bool,
    pub permission_hardening: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolVerificationResult {
    pub tool: tool_adapter::RemoteToolId,
    pub verification: tool_adapter::RemoteToolVerification,
}
struct Pending {
    preview: ConfigPreview,
    hash: String,
    target_id: String,
    port: u16,
    backup_hash: Option<String>,
    target_fingerprint: String,
    local_profile: Option<LocalToolProfile>,
}

#[derive(Clone)]
enum LocalToolProfile {
    Codex(local_model::LocalCodexProfile),
    Claude(local_model::LocalClaudeProfile),
}

impl LocalToolProfile {
    fn routes_to(&self, port: u16) -> bool {
        match self {
            Self::Codex(profile) => profile.route_port == Some(port),
            Self::Claude(profile) => profile.route_port == Some(port),
        }
    }
    fn can_replace(&self, cached: Option<&Self>) -> bool {
        // Missing Claude settings are valid on first setup, not while CC Switch
        // is replacing an existing file. Keep the last known good remote profile.
        match (self, cached) {
            (Self::Claude(next), Some(Self::Claude(previous))) => next.can_replace(previous),
            _ => true,
        }
    }
    fn inspect(tool: tool_adapter::RemoteToolId) -> Result<Self, local_model::ProfileError> {
        match tool {
            tool_adapter::RemoteToolId::Codex => {
                local_model::inspect_codex_profile().map(Self::Codex)
            }
            tool_adapter::RemoteToolId::Claude => {
                local_model::inspect_claude_profile().map(Self::Claude)
            }
        }
    }

    fn hash(&self) -> &str {
        match self {
            Self::Codex(profile) => &profile.hash,
            Self::Claude(profile) => &profile.hash,
        }
    }

    fn model(&self) -> Option<&str> {
        match self {
            Self::Codex(profile) => Some(&profile.model),
            Self::Claude(profile) => profile.model.as_deref(),
        }
    }

    fn stamp_changed(&self) -> Result<bool, local_model::ProfileError> {
        match self {
            Self::Codex(profile) => local_model::profile_stamp_changed(profile),
            Self::Claude(profile) => local_model::claude_profile_stamp_changed(profile),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortAllocation {
    pub proxy_port: u16,
    pub cc_port: u16,
    pub runtime_expected_proxy_port: Option<u16>,
    pub runtime_port_conflict: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeProxyMatch {
    #[default]
    Unknown,
    Matched,
    Mismatch,
    NotSet,
    Disabled,
    InvalidSettings,
    ReadFailed,
    UnsupportedProxy,
    NotCompared,
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
struct Store {
    events: events::EventStore,
    last_request: Option<Request>,
    reconnect: Option<reconnect::Retry>,
    connection_generation: u64,
    summary: Summary,
    child: Option<Box<dyn ssh::ManagedSsh>>,
    additional_tunnels: Vec<Box<dyn ssh::ManagedSsh>>,
    paused_proxy: Option<Endpoint>,
    paused_cc: Option<Endpoint>,
    paused_tools: Vec<tool_adapter::RemoteToolId>,
    capability_vscode_managed: bool,
    proxy_relay: Option<authenticated_relay::AuthenticatedRelay>,
    ai_relay: Option<authenticated_relay::AuthenticatedRelay>,
    pending: Option<Pending>,
    extension_pending: Option<extension::Pending>,
    cc_detected: bool,
    reachable: bool,
    target_fingerprint: Option<String>,
    tool_discovery_fingerprint: Option<String>,
    tool_discovery_ticket: u64,
    // Explicit per-tool OFF survives discovery and transport recovery in this bridge.
    disabled_tools: Vec<tool_adapter::RemoteToolId>,
    proxy_status: Status,
    cc_status: Status,
    ssh_auth: SshAuthState,
    follow_local_codex_profile: bool,
    local_codex_profile: Option<local_model::LocalCodexProfile>,
    local_claude_profile: Option<local_model::LocalClaudeProfile>,
    profile_syncing: bool,
    connect_started_at: Option<Instant>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            events: events::EventStore::default(),
            last_request: None,
            reconnect: None,
            connection_generation: 0,
            summary: Summary::default(),
            child: None,
            additional_tunnels: Vec::new(),
            paused_proxy: None,
            paused_cc: None,
            paused_tools: Vec::new(),
            capability_vscode_managed: false,
            proxy_relay: None,
            ai_relay: None,
            pending: None,
            extension_pending: None,
            cc_detected: false,
            reachable: false,
            target_fingerprint: None,
            tool_discovery_fingerprint: None,
            tool_discovery_ticket: 0,
            disabled_tools: Vec::new(),
            proxy_status: Status::Disconnected,
            cc_status: Status::Disconnected,
            ssh_auth: SshAuthState::default(),
            follow_local_codex_profile: true,
            local_codex_profile: None,
            local_claude_profile: None,
            profile_syncing: false,
            connect_started_at: None,
        }
    }
}
// Every Store access uses this guard, including monitor/reconnect try_lock paths.
// Publish read models on release, never while a mutation is incomplete.
#[derive(Default)]
struct StateStore {
    inner: Mutex<Store>,
    tray: tray_state::Cache,
    report: Mutex<Report>,
}
struct StoreGuard<'a> {
    inner: std::sync::MutexGuard<'a, Store>,
    tray: &'a tray_state::Cache,
    report: &'a Mutex<Report>,
}
impl StateStore {
    fn lock(&self) -> BridgeResult<StoreGuard<'_>> {
        self.inner
            .lock()
            .map(|inner| StoreGuard {
                inner,
                tray: &self.tray,
                report: &self.report,
            })
            .map_err(|_| "stateUnavailable".into())
    }
    fn try_lock(&self) -> BridgeResult<StoreGuard<'_>> {
        self.inner
            .try_lock()
            .map(|inner| StoreGuard {
                inner,
                tray: &self.tray,
                report: &self.report,
            })
            .map_err(|_| "stateUnavailable".into())
    }
    fn report(&self) -> Report {
        self.report
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
impl std::ops::Deref for StoreGuard<'_> {
    type Target = Store;
    fn deref(&self) -> &Store {
        &self.inner
    }
}
impl std::ops::DerefMut for StoreGuard<'_> {
    fn deref_mut(&mut self) -> &mut Store {
        &mut self.inner
    }
}
impl Drop for StoreGuard<'_> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            // Cache only the existing allowlisted projection, never the private Store.
            let report = project_report(&self.inner);
            *self.report.lock().unwrap_or_else(|e| e.into_inner()) = report;
            self.tray.publish(&self.inner);
        }
    }
}
static STORE: OnceLock<StateStore> = OnceLock::new();
static STOPPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn store() -> &'static StateStore {
    STORE.get_or_init(StateStore::default)
}
fn lock() -> BridgeResult<StoreGuard<'static>> {
    store().lock()
}
pub(crate) fn update_activity() -> BridgeResult<crate::desktop::updates::Activity> {
    let mut state = store().try_lock().map_err(|_| "updateBusy")?;
    if ssh_auth::update_busy()?
        || state.profile_syncing
        || state.summary.post_connect_status == PostConnectStatus::Preparing
        || state.summary.status == Status::Connecting
        || state.reconnect.is_some()
    {
        return Err("updateBusy".into());
    }
    let connected = match state.child.as_mut() {
        Some(child) => child.is_running().map_err(|_| "stateUnavailable")?,
        None => false,
    };
    if state.child.is_some() && !connected {
        return Err("updateStateChanged".into());
    }
    Ok(crate::desktop::updates::Activity {
        generation: state.connection_generation,
        connected,
    })
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
#[cfg(test)]
fn remote_environment(endpoint: &Endpoint) -> BridgeResult<String> {
    let remote = ProxyEndpoint {
        host: "127.0.0.1".into(),
        port: endpoint.remote_port,
        protocol: endpoint.local.protocol,
    };
    let mut lines = vec!["unset HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY".to_owned()];
    lines.extend(
        plan::build_proxy_environment_plan(
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
        .collect::<Vec<_>>(),
    );
    lines.push("export NO_PROXY=localhost,127.0.0.1,::1".to_owned());
    Ok(lines.join("\n"))
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
    let additional_failed = state
        .additional_tunnels
        .iter_mut()
        .any(|child| !matches!(child.is_running(), Ok(true)));
    if let Some(child) = state.child.as_mut() {
        if additional_failed || !matches!(child.is_running(), Ok(true)) {
            state.additional_tunnels.clear();
            state.child = None;
            if state.last_request.is_some() {
                state.reconnect = Some(reconnect::Retry::new());
                state.summary.reconnect_state = reconnect::State::Waiting;
                state.summary.status = Status::Connecting;
            } else {
                state.proxy_relay = None;
                state.ai_relay = None;
                state.summary.status = Status::Disconnected;
            }
            state.ssh_auth.authenticated = false;
            state.summary.ssh_auth = state.ssh_auth;
            state.proxy_status = Status::Disconnected;
            state.cc_status = Status::Disconnected;
            state.summary.proxy_status = state.summary.proxy.as_ref().map(|_| Status::Disconnected);
            state.summary.cc_status = state.summary.cc.as_ref().map(|_| Status::Disconnected);
            invalidate_tool_verification(&mut state.summary);
            state.summary.diagnostics.general_proxy_egress.reset();
            state.summary.diagnostics.server_direct.reset();
            state.summary.diagnostics.server_internet = None;
            state.events.push(
                EventComponent::Ssh,
                EventAction::TransportLost,
                EventOutcome::Warning,
                Some(ErrorCategory::Ssh),
                None,
            );
            state.reachable = false;
            state.pending = None;
            state.extension_pending = None;
            state.local_codex_profile = None;
            state.local_claude_profile = None;
            state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
            state.summary.codex_profile_state = settings::ProfileSyncState::NotStarted;
            state.profile_syncing = false;
            return;
        }
        let current = active::snapshot().ok();
        let proxy_available = state
            .summary
            .proxy
            .as_ref()
            .is_none_or(|e| listening(&e.local))
            && state.summary.proxy.as_ref().is_none_or(|_| {
                state
                    .proxy_relay
                    .as_ref()
                    .is_some_and(|relay| relay.is_running())
            });
        let cc_available = state
            .summary
            .cc
            .as_ref()
            .is_none_or(|e| listening(&e.local))
            && state.summary.cc.as_ref().is_none_or(|_| {
                state
                    .ai_relay
                    .as_ref()
                    .is_some_and(|relay| relay.is_running())
            });
        state.proxy_status = observed_status(&state.summary, current.as_ref(), proxy_available);
        state.cc_status = if cc_available {
            Status::Connected
        } else {
            Status::Unavailable
        };
        state.summary.proxy_status = state.summary.proxy.as_ref().map(|_| state.proxy_status);
        state.summary.cc_status = state.summary.cc.as_ref().map(|_| state.cc_status);
        if let Some(target) = state.summary.target.as_ref() {
            state.summary.runtime_expected_proxy_port = runtime_expected_port(
                target,
                state.summary.proxy.as_ref().map(|e| e.local.protocol),
            );
            state.summary.runtime_proxy_match = runtime_proxy_match(
                state.summary.runtime_expected_proxy_port,
                state
                    .summary
                    .proxy
                    .as_ref()
                    .map(|endpoint| endpoint.remote_port),
            );
        }
        let endpoints_available = proxy_available && cc_available;
        state.summary.status =
            observed_status(&state.summary, current.as_ref(), endpoints_available);
        // A stale general proxy must not invalidate an independent AI route.
        if state.summary.proxy_status != Some(Status::Connected) {
            state.summary.diagnostics.general_proxy_egress.reset();
        }
        if state.summary.cc_status != Some(Status::Connected) {
            invalidate_tool_verification(&mut state.summary);
        }
        observe_user_proxy_setting(&mut state.summary);
    }
}

fn remote_request(
    operation: &str,
    adapter: &dyn tool_adapter::RemoteToolAdapter,
    port: u16,
    profile: Option<&LocalToolProfile>,
    session_token: Option<&str>,
) -> serde_json::Value {
    let mut request = json!({
        "operation": operation,
        "tool": adapter.id().as_str(),
        "port": port,
    });
    if let Some(profile) = profile {
        request["profileHash"] = json!(profile.hash());
        match profile {
            LocalToolProfile::Codex(profile) => {
                request["profileModelBase64"] = json!(base64(&profile.model));
                request["profileCatalogBase64"] = json!(base64_bytes(&profile.catalog_bytes));
            }
            LocalToolProfile::Claude(profile) => {
                request["profileSettingsBase64"] = json!(base64_bytes(&profile.settings_bytes));
            }
        }
    }
    if let Some(session_token) = session_token {
        request["sessionToken"] = json!(session_token);
    }
    request
}

fn base64(value: &str) -> String {
    base64_bytes(value.as_bytes())
}

fn base64_bytes(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(TABLE[((value >> 18) & 63) as usize] as char);
        output.push(TABLE[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn timing(phase: &str, started: Instant, success: bool) -> PhaseTiming {
    PhaseTiming {
        phase: phase.into(),
        duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        outcome: if success { "success" } else { "failed" }.into(),
    }
}

fn invalidate_tool_verification(summary: &mut Summary) {
    summary.diagnostics.ai_route_verification.reset();
    for adapter in tool_adapter::adapters() {
        if adapter.configured(summary) {
            adapter.verify(summary, tool_adapter::RemoteToolVerification::VerifyPending);
        }
    }
    sync_tool_states(summary);
}

fn finish_profile_sync(tool: tool_adapter::RemoteToolId, value: settings::ProfileSyncState) {
    if let Ok(mut state) = store().lock() {
        state.profile_syncing = false;
        if tool == tool_adapter::RemoteToolId::Codex {
            state.summary.codex_profile_state = value;
        } else {
            state.summary.claude_profile_state = value;
        }
    }
}

fn cached_profile(state: &Store, tool: tool_adapter::RemoteToolId) -> Option<LocalToolProfile> {
    match tool {
        tool_adapter::RemoteToolId::Codex => state
            .local_codex_profile
            .clone()
            .map(LocalToolProfile::Codex),
        tool_adapter::RemoteToolId::Claude => state
            .local_claude_profile
            .clone()
            .map(LocalToolProfile::Claude),
    }
}

fn set_cached_profile(state: &mut Store, profile: Option<LocalToolProfile>) {
    match profile {
        Some(LocalToolProfile::Codex(profile)) => state.local_codex_profile = Some(profile),
        Some(LocalToolProfile::Claude(profile)) => state.local_claude_profile = Some(profile),
        None => {}
    }
}

fn clear_cached_profile(state: &mut Store, tool: tool_adapter::RemoteToolId) {
    match tool {
        tool_adapter::RemoteToolId::Codex => state.local_codex_profile = None,
        tool_adapter::RemoteToolId::Claude => state.local_claude_profile = None,
    }
}

fn profile_sync_enabled(state: &Store, tool: tool_adapter::RemoteToolId) -> bool {
    tool_adapter::by_id(tool).configured(&state.summary)
        && (tool != tool_adapter::RemoteToolId::Codex || state.follow_local_codex_profile)
}

fn sync_changed_profile(tool: tool_adapter::RemoteToolId) {
    let profile = match LocalToolProfile::inspect(tool) {
        Ok(profile) => profile,
        Err(_) => {
            finish_profile_sync(tool, settings::ProfileSyncState::InvalidLocalProfile);
            return;
        }
    };
    let snapshot = {
        let Ok(mut state) = store().lock() else {
            return;
        };
        if state.child.is_none()
            || state.ai_relay.is_none()
            || !profile_sync_enabled(&state, tool)
            || state.pending.is_some()
        {
            state.profile_syncing = false;
            return;
        }
        let cached = cached_profile(&state, tool);
        if !profile.can_replace(cached.as_ref()) {
            state.profile_syncing = false;
            state.summary.claude_profile_state = settings::ProfileSyncState::LocalChanged;
            return;
        }
        let Some(local_port) = state.summary.cc.as_ref().map(|route| route.local.port) else {
            state.profile_syncing = false;
            return;
        };
        if !profile.routes_to(local_port) {
            let changed = cached
                .as_ref()
                .is_none_or(|cached| cached.routes_to(local_port));
            set_cached_profile(&mut state, Some(profile));
            state.profile_syncing = false;
            if tool == tool_adapter::RemoteToolId::Codex {
                state.summary.codex_profile_state = settings::ProfileSyncState::RemoteUnavailable;
            } else {
                state.summary.claude_profile_state = settings::ProfileSyncState::RemoteUnavailable;
            }
            if changed {
                record_inactive_tool_route(&mut state, tool);
            }
            return;
        }
        if cached
            .is_some_and(|cached| cached.routes_to(local_port) && cached.hash() == profile.hash())
        {
            set_cached_profile(&mut state, Some(profile));
            state.profile_syncing = false;
            if tool == tool_adapter::RemoteToolId::Codex
                && state.summary.codex_profile_state != settings::ProfileSyncState::RestartRequired
            {
                state.summary.codex_profile_state = settings::ProfileSyncState::Synced;
            } else if tool == tool_adapter::RemoteToolId::Claude
                && state.summary.claude_profile_state != settings::ProfileSyncState::RestartRequired
            {
                state.summary.claude_profile_state = settings::ProfileSyncState::Synced;
            }
            return;
        }
        let Some(target) = state.summary.target.as_ref() else {
            state.profile_syncing = false;
            return;
        };
        let Some(endpoint) = state.summary.cc.as_ref() else {
            state.profile_syncing = false;
            return;
        };
        let Some(session_token) = state.ai_relay.as_ref().map(|relay| relay.token()) else {
            state.profile_syncing = false;
            return;
        };
        (
            target.id.clone(),
            endpoint.remote_port,
            state.target_fingerprint.clone(),
            session_token,
            state.connection_generation,
        )
    };
    // Debounce the parsed snapshot too, not only the initial mtime observation.
    // An in-flight provider save must never become a remote replacement.
    std::thread::sleep(Duration::from_millis(500));
    if profile.stamp_changed().unwrap_or(true) {
        finish_profile_sync(tool, settings::ProfileSyncState::LocalChanged);
        return;
    }
    let adapter = tool_adapter::by_id(tool);
    let preview = match ssh::remote(
        &snapshot.0,
        remote_request(
            "preview",
            adapter,
            snapshot.1,
            Some(&profile),
            Some(snapshot.3.as_str()),
        ),
    ) {
        Ok(value) => value,
        Err(code) => {
            finish_profile_sync(
                tool,
                if code == "configConflict" || code == "remoteProfileConflict" {
                    settings::ProfileSyncState::Conflict
                } else {
                    settings::ProfileSyncState::RemoteUnavailable
                },
            );
            return;
        }
    };
    if preview["configured"].as_bool() == Some(true)
        && preview["profileHash"].as_str() == Some(profile.hash())
        && preview["previousPort"].as_u64() == Some(u64::from(snapshot.1))
    {
        if let Ok(mut state) = store().lock() {
            if state.connection_generation != snapshot.4
                || !profile_sync_enabled(&state, tool)
                || state
                    .ai_relay
                    .as_ref()
                    .is_none_or(|relay| relay.token().as_str() != snapshot.3.as_str())
            {
                state.profile_syncing = false;
                return;
            }
            set_cached_profile(&mut state, Some(profile));
            state.profile_syncing = false;
            if tool == tool_adapter::RemoteToolId::Codex
                && state.summary.codex_profile_state != settings::ProfileSyncState::RestartRequired
            {
                state.summary.codex_profile_state = settings::ProfileSyncState::Synced;
            } else if tool == tool_adapter::RemoteToolId::Claude
                && state.summary.claude_profile_state != settings::ProfileSyncState::RestartRequired
            {
                state.summary.claude_profile_state = settings::ProfileSyncState::Synced;
            }
        }
        return;
    }
    let Some(expected_hash) = preview["expectedHash"].as_str() else {
        finish_profile_sync(tool, settings::ProfileSyncState::RemoteUnavailable);
        return;
    };
    let mut apply_request = remote_request(
        "apply",
        adapter,
        snapshot.1,
        Some(&profile),
        Some(snapshot.3.as_str()),
    );
    apply_request["expectedHash"] = json!(expected_hash);
    apply_request["repairPermissions"] = json!(false);
    if profile.stamp_changed().unwrap_or(true) {
        finish_profile_sync(tool, settings::ProfileSyncState::LocalChanged);
        return;
    }
    {
        let Ok(mut state) = store().lock() else {
            return;
        };
        if state.connection_generation != snapshot.4
            || state.summary.cc.as_ref().map(|route| route.remote_port) != Some(snapshot.1)
            || state
                .ai_relay
                .as_ref()
                .is_none_or(|relay| relay.token().as_str() != snapshot.3.as_str())
            || !profile_sync_enabled(&state, tool)
        {
            state.profile_syncing = false;
            return;
        }
    }
    // apply already performs atomic replacement, readback and route/session/
    // profile validation remotely. Use that receipt instead of a third SSH trip.
    let applied = match ssh::remote(&snapshot.0, apply_request) {
        Ok(value) => value,
        Err(code) => {
            finish_profile_sync(
                tool,
                if code == "configConflict" || code == "remoteProfileConflict" {
                    settings::ProfileSyncState::Conflict
                } else {
                    settings::ProfileSyncState::RemoteUnavailable
                },
            );
            return;
        }
    };
    let verified = applied["configured"].as_bool() == Some(true)
        && applied["profileHash"].as_str() == Some(profile.hash())
        && applied["previousPort"].as_u64() == Some(u64::from(snapshot.1));
    if !verified {
        finish_profile_sync(tool, settings::ProfileSyncState::Conflict);
        return;
    }
    if profile.stamp_changed().unwrap_or(true) {
        finish_profile_sync(tool, settings::ProfileSyncState::LocalChanged);
        return;
    }
    if let Ok(mut state) = store().lock() {
        let same_bridge = state.child.is_some()
            && state.connection_generation == snapshot.4
            && state.ai_relay.is_some()
            && profile_sync_enabled(&state, tool)
            && state
                .summary
                .target
                .as_ref()
                .map(|target| target.id.as_str())
                == Some(snapshot.0.as_str())
            && state.target_fingerprint == snapshot.2;
        state.profile_syncing = false;
        if same_bridge {
            set_cached_profile(&mut state, Some(profile));
            adapter.apply(&mut state.summary);
            sync_tool_states(&mut state.summary);
            if tool == tool_adapter::RemoteToolId::Codex {
                state.summary.codex_profile_state = settings::ProfileSyncState::RestartRequired;
            } else {
                state.summary.claude_profile_state = settings::ProfileSyncState::RestartRequired;
            }
        } else if tool == tool_adapter::RemoteToolId::Codex && !state.follow_local_codex_profile {
            clear_cached_profile(&mut state, tool);
            state.summary.codex_profile_state = settings::ProfileSyncState::Disabled;
        } else if state.child.is_none() || state.ai_relay.is_none() {
            clear_cached_profile(&mut state, tool);
            if tool == tool_adapter::RemoteToolId::Codex {
                state.summary.codex_profile_state = settings::ProfileSyncState::NotStarted;
            } else {
                state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
            }
        } else if tool == tool_adapter::RemoteToolId::Codex {
            state.summary.codex_profile_state = settings::ProfileSyncState::RemoteUnavailable;
        } else {
            state.summary.claude_profile_state = settings::ProfileSyncState::RemoteUnavailable;
        }
    }
}

fn exposed_summary(summary: &Summary) -> Summary {
    let mut snapshot = summary.clone();
    sync_tool_states(&mut snapshot);
    snapshot
}

fn observe_user_proxy_setting(snapshot: &mut Summary) {
    // Observe default user settings even when unrelated VS Code profiles exist.
    // This is configuration evidence only, never extension connectivity proof.
    let setting = vscode::user_proxy_setting();
    snapshot.runtime_expected_proxy_port = match setting {
        vscode::UserProxySetting::Configured(port) => Some(port),
        _ => None,
    };
    snapshot.runtime_proxy_match = match setting {
        vscode::UserProxySetting::Configured(port) => {
            if snapshot.proxy_status == Some(Status::Connected)
                && snapshot.proxy.as_ref().is_some_and(|endpoint| {
                    matches!(
                        endpoint.local.protocol,
                        ProxyProtocol::Http | ProxyProtocol::Mixed
                    )
                })
            {
                runtime_proxy_match(
                    Some(port),
                    snapshot.proxy.as_ref().map(|endpoint| endpoint.remote_port),
                )
            } else {
                RuntimeProxyMatch::NotCompared
            }
        }
        vscode::UserProxySetting::NotSet => RuntimeProxyMatch::NotSet,
        vscode::UserProxySetting::Disabled => RuntimeProxyMatch::Disabled,
        vscode::UserProxySetting::InvalidSettings => RuntimeProxyMatch::InvalidSettings,
        vscode::UserProxySetting::ReadFailed => RuntimeProxyMatch::ReadFailed,
        vscode::UserProxySetting::UnsupportedProxy => RuntimeProxyMatch::UnsupportedProxy,
    };
}

fn exposed_state_summary(state: &Store) -> Summary {
    let mut snapshot = exposed_summary(&state.summary);
    observe_user_proxy_setting(&mut snapshot);
    // Read-only projection: displaying or copying an already-ready command
    // must not launch SSH or prepare env.sh again.
    snapshot.session_environment_command = projected_session_command(
        &state.summary,
        state.child.is_some(),
        state.proxy_relay.as_ref().map(|relay| relay.session_id()),
    );
    snapshot
}

fn profile_poll_order(last: tool_adapter::RemoteToolId) -> [tool_adapter::RemoteToolId; 2] {
    use tool_adapter::RemoteToolId::{Claude, Codex};
    match last {
        Codex => [Claude, Codex],
        Claude => [Codex, Claude],
    }
}

pub fn start_monitor() {
    skills::start_monitor();
    if let Ok(mut state) = store().lock() {
        state.follow_local_codex_profile = settings::load()
            .map(|value| value.follow_local_codex_profile)
            .unwrap_or(true);
    }
    std::thread::spawn(|| {
        let mut last_profile = tool_adapter::RemoteToolId::Claude;
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let Ok(_operation) = crate::desktop::updates::bridge_operation() else {
                continue;
            };
            ssh_auth::cleanup();
            reconnect::tick();
            let candidate = {
                let Ok(mut state) = store().try_lock() else {
                    continue;
                };
                refresh(&mut state);
                if state.profile_syncing || state.pending.is_some() {
                    continue;
                }
                // Profile polling exists only while the bridge and at least one
                // shared remote client profile are active.
                if state.child.is_none() || state.ai_relay.is_none() {
                    state.local_codex_profile = None;
                    state.local_claude_profile = None;
                    state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
                    state.summary.codex_profile_state = if state.follow_local_codex_profile {
                        settings::ProfileSyncState::NotStarted
                    } else {
                        settings::ProfileSyncState::Disabled
                    };
                    continue;
                }
                let mut changed = None;
                for tool in profile_poll_order(last_profile) {
                    if !profile_sync_enabled(&state, tool) {
                        clear_cached_profile(&mut state, tool);
                        if tool == tool_adapter::RemoteToolId::Codex {
                            state.summary.codex_profile_state = if state.follow_local_codex_profile
                            {
                                settings::ProfileSyncState::NotStarted
                            } else {
                                settings::ProfileSyncState::Disabled
                            };
                        } else {
                            state.summary.claude_profile_state =
                                settings::ProfileSyncState::NotStarted;
                        }
                        continue;
                    }
                    match cached_profile(&state, tool) {
                        Some(profile) => match profile.stamp_changed() {
                            Ok(false) => {}
                            Ok(true) => {
                                if tool == tool_adapter::RemoteToolId::Codex {
                                    state.summary.codex_profile_state =
                                        settings::ProfileSyncState::LocalChanged;
                                } else {
                                    state.summary.claude_profile_state =
                                        settings::ProfileSyncState::LocalChanged;
                                }
                                changed = Some(tool);
                                break;
                            }
                            Err(_) => {
                                if tool == tool_adapter::RemoteToolId::Codex {
                                    state.summary.codex_profile_state =
                                        settings::ProfileSyncState::InvalidLocalProfile;
                                } else {
                                    state.summary.claude_profile_state =
                                        settings::ProfileSyncState::InvalidLocalProfile;
                                }
                            }
                        },
                        None => {
                            changed = Some(tool);
                            break;
                        }
                    }
                }
                if changed.is_some() {
                    state.profile_syncing = true;
                }
                changed
            };
            let Some(candidate) = candidate else { continue };
            // A missing/invalid Codex profile must not starve Claude (or vice versa).
            last_profile = candidate;
            // Coalesce editor save sequences (temporary file, rename, metadata update)
            // before opening and parsing either profile file.
            std::thread::sleep(Duration::from_millis(500));
            sync_changed_profile(candidate);
        }
    });
}

pub use skills::SkillView;

pub fn skill_views() -> BridgeResult<Vec<SkillView>> {
    skills::views()
}

pub fn enable_skill(id: String) -> BridgeResult<SkillView> {
    let started = Instant::now();
    let generation = lock()?.connection_generation;
    let result = skills::enable(id);
    if let Ok(mut state) = lock() {
        if state.connection_generation == generation && state.child.is_some() {
            record_result(
                &mut state,
                EventComponent::Skills,
                EventAction::Enable,
                &result,
                started,
            );
        }
    }
    result
}

pub fn disable_skill(id: String) -> BridgeResult<SkillView> {
    let started = Instant::now();
    let generation = lock()?.connection_generation;
    let result = skills::disable(id);
    if let Ok(mut state) = lock() {
        if state.connection_generation == generation && state.child.is_some() {
            record_result(
                &mut state,
                EventComponent::Skills,
                EventAction::Disable,
                &result,
                started,
            );
        }
    }
    result
}
pub fn summary() -> BridgeResult<Summary> {
    let mut state = lock()?;
    if state.tool_discovery_fingerprint.is_some() {
        let fingerprint = state
            .summary
            .target
            .as_ref()
            .and_then(|target| ssh::fingerprint(&target.id).ok());
        invalidate_tool_discovery(&mut state, fingerprint.as_deref());
    }
    Ok(exposed_state_summary(&state))
}

fn record_inactive_tool_route(state: &mut Store, tool: tool_adapter::RemoteToolId) {
    state.events.push(
        if tool == tool_adapter::RemoteToolId::Codex {
            EventComponent::Codex
        } else {
            EventComponent::Claude
        },
        EventAction::ToolSetup,
        EventOutcome::Warning,
        Some(ErrorCategory::LocalRouteInactive),
        None,
    );
}

fn configure_discovered_tools(target_id: &str, fingerprint: &str, generation: u64, ticket: u64) {
    for adapter in tool_adapter::adapters() {
        let Ok(mut state) = lock() else { return };
        refresh(&mut state);
        if !discovery_context_current(&state, target_id, fingerprint, generation, ticket)
            || state.tool_discovery_fingerprint.as_deref() != Some(fingerprint)
        {
            return;
        }
        // Serialize with manual transactions; never replace their pending preview.
        if state.pending.is_some() || state.extension_pending.is_some() || state.profile_syncing {
            return;
        }
        let tool = adapter.id();
        if !automatic_tool_setup_allowed(&state, tool) {
            if tool == tool_adapter::RemoteToolId::Codex {
                state.summary.codex_state = RuntimeState::Ready;
            } else {
                state.summary.claude_state = RuntimeState::Ready;
            }
            continue;
        }
        let started = Instant::now();
        // The AI capability is the user's opt-in. Reuse the exact manual
        // preview/apply/read-back/rollback transaction, including plugin fallback.
        let result = config_preview_locked(&mut state, *adapter)
            .and_then(|preview| config_apply_locked(&mut state, preview.id));
        let route_inactive = result
            .as_ref()
            .err()
            .is_some_and(|code| code == "localRouteInactive");
        let runtime = if result.is_ok() || route_inactive {
            RuntimeState::Ready
        } else {
            RuntimeState::Warning
        };
        if tool == tool_adapter::RemoteToolId::Codex {
            state.summary.codex_state = runtime;
        } else {
            state.summary.claude_state = runtime;
        }
        let mut entry = timing(&format!("{}.setup", tool.as_str()), started, result.is_ok());
        if route_inactive {
            entry.outcome = "unsupported".into();
        }
        state.events.push(
            if tool == tool_adapter::RemoteToolId::Codex {
                EventComponent::Codex
            } else {
                EventComponent::Claude
            },
            EventAction::ToolSetup,
            if result.is_ok() {
                EventOutcome::Success
            } else if route_inactive {
                EventOutcome::Warning
            } else {
                EventOutcome::Failed
            },
            result
                .as_ref()
                .err()
                .map(|code| ErrorCategory::from_code(code)),
            Some(entry.duration_ms),
        );
        if let Err(code) = result {
            if !route_inactive {
                state.summary.post_connect_error.get_or_insert(code);
            }
        }
        state.summary.timings.push(entry);
    }
}

pub fn discover_tools(
    target_id: String,
) -> BridgeResult<tool_discovery::RemoteToolDiscoverySnapshot> {
    discover_tools_for_generation(target_id, None)
}

fn discover_tools_for_generation(
    target_id: String,
    expected_generation: Option<u64>,
) -> BridgeResult<tool_discovery::RemoteToolDiscoverySnapshot> {
    let _operation = crate::desktop::updates::bridge_operation()?;
    let target = targets()?
        .into_iter()
        .find(|target| target.id == target_id && target.available)
        .ok_or("targetUnsupported")?;
    let fingerprint = ssh::fingerprint(&target.id)?;
    let (generation, ticket, connected) = {
        let mut state = lock()?;
        let connected = state.child.is_some()
            && state
                .summary
                .target
                .as_ref()
                .is_some_and(|target| target.id == target_id);
        if expected_generation
            .is_some_and(|generation| generation != state.connection_generation || !connected)
        {
            return Err("stateUnavailable".into());
        }
        if connected && state.target_fingerprint.as_deref() != Some(&fingerprint) {
            return Err("sshConfigChanged".into());
        }
        if connected {
            state.tool_discovery_ticket = state.tool_discovery_ticket.wrapping_add(1);
            state.tool_discovery_fingerprint = Some(fingerprint.clone());
            state.summary.tool_installations = tool_adapter::unknown_installations();
            for tool in &mut state.summary.tool_installations {
                tool.state = tool_adapter::RemoteToolInstallState::Checking;
                tool.extension_state = tool_adapter::RemoteToolInstallState::Checking;
            }
        }
        (
            state.connection_generation,
            state.tool_discovery_ticket,
            connected,
        )
    };
    let started = Instant::now();
    let result = ssh::remote(&target_id, json!({"operation":"tool-discover"}))
        .and_then(tool_discovery::parse)
        .map(|mut snapshot| {
            // Reuse the read-only Remote SSH extension detector, not configuration status.
            if let Ok(inspection) = extension::discover(&target_id) {
                tool_discovery::project_extensions(&mut snapshot.tools, &inspection);
            }
            snapshot
        });
    if ssh::fingerprint(&target_id).ok().as_ref() != Some(&fingerprint) {
        return Err("sshConfigChanged".into());
    }
    if connected {
        let mut state = lock()?;
        if !discovery_context_current(&state, &target_id, &fingerprint, generation, ticket) {
            return Err("stateUnavailable".into());
        }
        match &result {
            Ok(snapshot) => {
                state.summary.tool_installations = snapshot.tools.clone();
                state.summary.install_capabilities = snapshot.capabilities.clone();
                for tool in &snapshot.tools {
                    let warning = matches!(
                        tool.state,
                        tool_adapter::RemoteToolInstallState::Unsupported
                            | tool_adapter::RemoteToolInstallState::Broken
                    );
                    state.events.push(
                        if tool.tool == tool_adapter::RemoteToolId::Codex {
                            EventComponent::Codex
                        } else {
                            EventComponent::Claude
                        },
                        EventAction::ToolDiscovery,
                        if warning {
                            EventOutcome::Warning
                        } else {
                            EventOutcome::Success
                        },
                        if warning {
                            Some(ErrorCategory::Unsupported)
                        } else {
                            None
                        },
                        Some(started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)),
                    );
                }
            }
            Err(code) => {
                state.summary.tool_installations = tool_adapter::unknown_installations();
                if state.summary.cc.is_some() {
                    state.summary.codex_state = RuntimeState::Warning;
                    state.summary.claude_state = RuntimeState::Warning;
                }
                let error = if code == "dependencyMissing" {
                    tool_adapter::RemoteToolInstallError::DependencyMissing
                } else if code.starts_with("ssh") {
                    tool_adapter::RemoteToolInstallError::SshUnavailable
                } else {
                    tool_adapter::RemoteToolInstallError::RemoteUnsupported
                };
                for tool in &mut state.summary.tool_installations {
                    tool.checked_at = Some(observations::timestamp());
                    tool.error_code = Some(error);
                }
                state.events.push(
                    EventComponent::Ssh,
                    EventAction::ToolDiscovery,
                    EventOutcome::Warning,
                    Some(ErrorCategory::from_code(code)),
                    None,
                );
            }
        }
        drop(state);
        if result.is_ok() {
            configure_discovered_tools(&target_id, &fingerprint, generation, ticket);
        }
    }
    result
}

pub fn recent_events(limit: usize) -> BridgeResult<Vec<events::BridgeEvent>> {
    // A UI snapshot must not wait behind an SSH operation.
    let state = store().try_lock().map_err(|_| "stateUnavailable")?;
    Ok(state.events.recent(limit))
}

pub fn model_settings() -> BridgeResult<settings::RemoteBridgeSettingsView> {
    let state = lock()?;
    let mut view = settings::current_view(None)?;
    view.profile_state = state.summary.codex_profile_state;
    if let Some(profile) = state.local_codex_profile.as_ref() {
        view.model = Some(profile.model.clone());
        view.profile_hash = Some(profile.hash.clone());
    }
    Ok(view)
}

pub fn save_model_settings(
    settings: settings::RemoteBridgeSettings,
) -> BridgeResult<settings::RemoteBridgeSettingsView> {
    let follow = settings.follow_local_codex_profile;
    let mut view = settings::save(settings, None)?;
    let mut state = lock()?;
    state.follow_local_codex_profile = follow;
    if !follow {
        state.local_codex_profile = None;
        state.summary.codex_profile_state = settings::ProfileSyncState::Disabled;
    } else if state.child.is_some() && state.ai_relay.is_some() && state.summary.codex_configured {
        state.summary.codex_profile_state = settings::ProfileSyncState::LocalChanged;
    } else {
        state.summary.codex_profile_state = settings::ProfileSyncState::NotStarted;
    }
    view.profile_state = state.summary.codex_profile_state;
    Ok(view)
}
pub fn report() -> Report {
    // Report generation reads cached observations only: no SSH or socket probes.
    store().report()
}

pub fn targets() -> BridgeResult<Vec<RemoteTarget>> {
    ssh::targets()
}
pub fn add_connection(input: connections::ManualConnectionInput) -> BridgeResult<RemoteTarget> {
    let connection = connections::add(input)?;
    Ok(connections::target(&connection))
}
pub fn update_connection(
    id: String,
    input: connections::ManualConnectionInput,
) -> BridgeResult<RemoteTarget> {
    let connection = connections::update(&id, input)?;
    Ok(connections::target(&connection))
}
pub fn remove_connection(id: String) -> BridgeResult<()> {
    connections::remove(&id)
}
pub fn launch_mobaxterm(target_id: String) -> BridgeResult<()> {
    let (connected_target, _) = connected_terminal_context()?;
    if connected_target != target_id {
        return Err("bridgeUnavailable".into());
    }
    ssh::launch_mobaxterm_target(&target_id)
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
    allocate_ports(target_id, true)
}

pub fn probe_target_os(target_id: String) -> BridgeResult<WindowsTargetProbe> {
    #[cfg(windows)]
    {
        ssh::probe_windows_target(&target_id)
    }
    #[cfg(not(windows))]
    {
        let _ = target_id;
        Err("targetUnsupported".into())
    }
}
pub fn check_remote_network(target_id: String) -> BridgeResult<RemoteNetworkObservation> {
    check_remote_network_for_generation(target_id, None)
}
fn check_remote_network_for_generation(
    target_id: String,
    expected_generation: Option<u64>,
) -> BridgeResult<RemoteNetworkObservation> {
    let started = Instant::now();
    let (generation, ticket) = {
        let mut state = lock()?;
        if expected_generation.is_some_and(|expected| {
            state.connection_generation != expected || state.child.is_none()
        }) {
            return Err("bridgeUnavailable".into());
        }
        let ticket = if state.child.is_some()
            && state.summary.target.as_ref().map(|target| &target.id) == Some(&target_id)
        {
            Some(state.summary.diagnostics.server_direct.begin())
        } else {
            None
        };
        (state.connection_generation, ticket)
    };
    let result: BridgeResult<RemoteNetworkObservation> = (|| {
        ssh::validate_target(&target_id)?;
        let value = ssh::remote(&target_id, json!({"operation":"internet"}))?;
        let server_internet = match value.get("internet").and_then(|entry| entry.as_str()) {
            Some("reachable") => RemoteInternetState::Reachable,
            Some("unreachable") => RemoteInternetState::Unreachable,
            Some("unknown") => RemoteInternetState::Unknown,
            _ => return Err("remoteUnsupported".into()),
        };
        Ok(RemoteNetworkObservation { server_internet })
    })();
    if let Ok(mut state) = lock() {
        refresh(&mut state);
        if state.connection_generation == generation
            && state.child.is_some()
            && state.summary.target.as_ref().map(|target| &target.id) == Some(&target_id)
        {
            if let Some(ticket) = ticket {
                let error = match &result {
                    Ok(observation) => match observation.server_internet {
                        RemoteInternetState::Reachable => None,
                        RemoteInternetState::Unreachable => Some(ErrorCategory::Network),
                        RemoteInternetState::Unknown => Some(ErrorCategory::Unknown),
                    },
                    Err(code) => Some(ErrorCategory::from_code(code)),
                };
                if state.summary.diagnostics.server_direct.finish(
                    ticket,
                    error,
                    elapsed_ms(started),
                ) {
                    state.summary.diagnostics.server_internet =
                        result.as_ref().ok().map(|value| value.server_internet);
                }
            }
            let outcome = match &result {
                Ok(RemoteNetworkObservation {
                    server_internet: RemoteInternetState::Reachable,
                }) => EventOutcome::Success,
                Ok(_) => EventOutcome::Warning,
                Err(_) => EventOutcome::Failed,
            };
            state.events.push(
                EventComponent::Ssh,
                EventAction::ServerDirect,
                outcome,
                result
                    .as_ref()
                    .err()
                    .map(|code| ErrorCategory::from_code(code)),
                Some(elapsed_ms(started)),
            );
        }
    }
    result
}
fn runtime_expected_port(
    target: &RemoteTarget,
    local_protocol: Option<ProxyProtocol>,
) -> Option<u16> {
    if target.can_open_vscode
        && matches!(
            local_protocol,
            Some(ProxyProtocol::Http | ProxyProtocol::Mixed)
        )
    {
        vscode::user_proxy_port()
    } else {
        None
    }
}

fn runtime_proxy_match(expected: Option<u16>, actual: Option<u16>) -> RuntimeProxyMatch {
    match (expected, actual) {
        (Some(expected), Some(actual)) if expected == actual => RuntimeProxyMatch::Matched,
        (Some(_), Some(_)) => RuntimeProxyMatch::Mismatch,
        _ => RuntimeProxyMatch::Unknown,
    }
}

pub fn allocate_ports(target_id: String, prefer_defaults: bool) -> BridgeResult<PortAllocation> {
    let target = ssh::target(&target_id)?;
    let fingerprint = ssh::fingerprint(&target_id)?;
    let local = active::snapshot()
        .ok()
        .and_then(|context| context.available_candidate().cloned());
    let runtime_expected_proxy_port =
        runtime_expected_port(&target, local.as_ref().map(|candidate| candidate.protocol));
    let local_port = local.as_ref().map(|candidate| candidate.port);
    resolve_port_pair(
        &fingerprint,
        local_port,
        runtime_expected_proxy_port,
        prefer_defaults,
        |candidate| match ssh::remote(&target_id, json!({"operation":"check","ports":[candidate]}))
        {
            Ok(_) => Ok(true),
            Err(code) if code == "portInUse" => Ok(false),
            Err(code) => Err(code),
        },
    )
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

// Discovery is setup-only. Established tunnels retain their original endpoint.
pub fn discover_cc(preferred_port: u16) -> BridgeResult<CcDetection> {
    port(preferred_port)?;
    let listeners = super::proxy::listeners::enumerate().map_err(|_| "stateUnavailable")?;
    let processes = super::proxy::processes::enumerate();
    let ports = cc_route_ports(&listeners, &processes, preferred_port);
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_millis(350))
        .build()
        .map_err(|_| "stateUnavailable")?;
    let found = ports
        .into_iter()
        .take(8)
        .find(|candidate| cc_route_healthy(&client, *candidate));
    let result = CcDetection {
        state: if found.is_some() {
            CcDetectionState::Confirmed
        } else if listeners
            .iter()
            .any(|listener| listener.port == preferred_port)
        {
            CcDetectionState::ListeningUnknown
        } else {
            CcDetectionState::NotDetected
        },
        local_port: found.unwrap_or(preferred_port),
    };
    lock()?.cc_detected = result.state == CcDetectionState::Confirmed;
    Ok(result)
}

fn cc_route_ports(
    listeners: &[super::proxy::listeners::TcpListener],
    processes: &std::collections::HashMap<u32, super::proxy::processes::RunningProcess>,
    preferred_port: u16,
) -> Vec<u16> {
    let mut ports: Vec<u16> = listeners
        .iter()
        .filter(|listener| listener.host == "127.0.0.1" && listener.port >= 1024)
        .filter(|listener| {
            listener.pids.iter().any(|pid| {
                processes.get(pid).is_some_and(|process| {
                    process
                        .name
                        .replace(['-', '_', ' '], "")
                        .contains("ccswitch")
                })
            })
        })
        .map(|listener| listener.port)
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports.sort_by_key(|candidate| (*candidate != preferred_port, *candidate));
    ports
}

fn cc_route_healthy(client: &reqwest::blocking::Client, local_port: u16) -> bool {
    use std::io::Read;
    let Ok(response) = client
        .get(format!("http://127.0.0.1:{local_port}/health"))
        .send()
    else {
        return false;
    };
    if response.status() != reqwest::StatusCode::OK {
        return false;
    }
    let mut bytes = Vec::new();
    if response.take(4097).read_to_end(&mut bytes).is_err() || bytes.len() > 4096 {
        return false;
    }
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .is_some_and(|body| {
            body.get("status").and_then(|status| status.as_str()) == Some("healthy")
        })
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
    // Authenticated relays must never expose their session credential to the
    // WebView. The managed terminal loads a private remote env file instead.
    let environment = String::new();
    let runtime_expected_proxy_port = runtime_expected_port(
        &target,
        proxy.as_ref().map(|endpoint| endpoint.local.protocol),
    );
    let runtime_proxy_match = runtime_proxy_match(runtime_expected_proxy_port, request.proxy_port);
    let mut summary = Summary {
        target: Some(target),
        proxy,
        runtime_expected_proxy_port,
        runtime_proxy_match,
        cc,
        environment,
        active_proxy_revision: request.proxy_port.map(|_| context.revision),
        ..Summary::default()
    };
    sync_tool_states(&mut summary);
    Ok(summary)
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
    pub proxy_relay: Option<authenticated_relay::AuthenticatedRelay>,
    pub ai_relay: Option<authenticated_relay::AuthenticatedRelay>,
}

fn relays_for(
    summary: &Summary,
) -> BridgeResult<(
    Option<authenticated_relay::AuthenticatedRelay>,
    Option<authenticated_relay::AuthenticatedRelay>,
)> {
    let ai = summary
        .cc
        .as_ref()
        .map(|endpoint| {
            authenticated_relay::AuthenticatedRelay::start(
                endpoint.local.clone(),
                authenticated_relay::RelayMode::AiHttp,
            )
        })
        .transpose()?;
    let ai_route = ai
        .as_ref()
        .zip(summary.cc.as_ref())
        .map(|(relay, endpoint)| relay.ai_route(endpoint.remote_port));
    let proxy = summary
        .proxy
        .as_ref()
        .map(|endpoint| {
            authenticated_relay::AuthenticatedRelay::start_with_ai_route(
                endpoint.local.clone(),
                authenticated_relay::RelayMode::General(endpoint.local.protocol),
                ai_route,
            )
        })
        .transpose()?;
    Ok((proxy, ai))
}

fn forwarding_endpoints(
    summary: &Summary,
    proxy_relay: Option<&authenticated_relay::AuthenticatedRelay>,
    ai_relay: Option<&authenticated_relay::AuthenticatedRelay>,
) -> BridgeResult<Vec<(u16, String, u16)>> {
    let mut endpoints = Vec::new();
    if let Some(endpoint) = summary.proxy.as_ref() {
        let relay = proxy_relay.ok_or("relayUnavailable")?;
        endpoints.push((endpoint.remote_port, "127.0.0.1".into(), relay.port()));
    }
    if let Some(endpoint) = summary.cc.as_ref() {
        let relay = ai_relay.ok_or("relayUnavailable")?;
        endpoints.push((endpoint.remote_port, "127.0.0.1".into(), relay.port()));
    }
    Ok(endpoints)
}

fn apply_session_environment(
    target_id: &str,
    summary: &Summary,
    session_id: Option<&str>,
) -> BridgeResult<()> {
    let Some(endpoint) = summary.proxy.as_ref() else {
        return Ok(());
    };
    let session_id = session_id.ok_or("relayUnavailable")?;
    ssh::remote(
        target_id,
        json!({
            "operation": "session-env-apply",
            "sessionId": session_id,
            "port": endpoint.remote_port,
            "protocol": endpoint.local.protocol,
        }),
    )?;
    Ok(())
}

fn remove_session_environment(target_id: &str, session_id: Option<&str>) {
    let Some(session_id) = session_id else {
        return;
    };
    let _ = ssh::remote(
        target_id,
        json!({
            "operation": "session-env-remove",
            "sessionId": session_id,
        }),
    );
}

enum PostConnectResult {
    ToolDiscovery(
        BridgeResult<tool_discovery::RemoteToolDiscoverySnapshot>,
        PhaseTiming,
    ),
    Environment(BridgeResult<()>, PhaseTiming),
    Vscode(BridgeResult<()>, PhaseTiming),
}

fn prepare_post_connect(summary: &mut Summary) {
    summary.tool_installations = tool_adapter::unknown_installations();
    for tool in &mut summary.tool_installations {
        tool.state = tool_adapter::RemoteToolInstallState::Checking;
        tool.extension_state = tool_adapter::RemoteToolInstallState::Checking;
    }
    summary.post_connect_status = PostConnectStatus::Preparing;
    summary.session_environment_state = if summary.proxy.is_some() {
        RuntimeState::Preparing
    } else {
        RuntimeState::Ready
    };
    summary.vscode_state = if summary
        .target
        .as_ref()
        .is_some_and(|target| target.can_open_vscode)
    {
        RuntimeState::Preparing
    } else {
        RuntimeState::Ready
    };
    let tool_state = if summary.cc.is_some() {
        RuntimeState::Preparing
    } else {
        RuntimeState::Ready
    };
    summary.codex_state = tool_state;
    summary.claude_state = tool_state;
    // Skill discovery and sync use their own monitored transaction and never
    // gate transport readiness.
    summary.skills_state = RuntimeState::Ready;
    summary.post_connect_error = None;
}

fn start_post_connect(generation: u64) {
    start_auto_proxy_test(generation);
    std::thread::spawn(move || {
        let post_connect_started = Instant::now();
        let job = {
            let Ok(state) = lock() else {
                return;
            };
            if state.connection_generation != generation || state.child.is_none() {
                return;
            }
            let Some(target_id) = state
                .summary
                .target
                .as_ref()
                .map(|target| target.id.clone())
            else {
                return;
            };
            (
                target_id,
                state.summary.clone(),
                state
                    .proxy_relay
                    .as_ref()
                    .map(|relay| relay.session_id().to_owned()),
                state
                    .ai_relay
                    .as_ref()
                    .or(state.proxy_relay.as_ref())
                    .map(|relay| relay.session_id().to_owned()),
                state.target_fingerprint.clone(),
            )
        };
        let (target_id, summary, proxy_session, vscode_session, fingerprint) = job;
        let (sender, receiver) = std::sync::mpsc::channel();
        {
            let discovery_sender = sender.clone();
            let discovery_target = target_id.clone();
            std::thread::spawn(move || {
                let started = Instant::now();
                let result = discover_tools_for_generation(discovery_target, Some(generation));
                let entry = timing("remoteTools.discovery", started, result.is_ok());
                let _ = discovery_sender.send(PostConnectResult::ToolDiscovery(result, entry));
            });
            if summary.proxy.is_some() {
                let sender = sender.clone();
                let target_id = target_id.clone();
                let summary = summary.clone();
                let proxy_session = proxy_session.clone();
                std::thread::spawn(move || {
                    let started = Instant::now();
                    let result =
                        apply_session_environment(&target_id, &summary, proxy_session.as_deref());
                    let _ = sender.send(PostConnectResult::Environment(
                        result.clone(),
                        timing("sessionEnv.apply", started, result.is_ok()),
                    ));
                });
            }
            if summary
                .target
                .as_ref()
                .is_some_and(|target| target.can_open_vscode)
            {
                let sender = sender.clone();
                let target_id = target_id.clone();
                let vscode_session = vscode_session.clone();
                let fingerprint = fingerprint.clone();
                let proxy = summary.proxy.clone();
                std::thread::spawn(move || {
                    let started = Instant::now();
                    let result = match (vscode_session.as_deref(), fingerprint.as_deref()) {
                        (Some(session_id), Some(fingerprint)) => refresh_owned_vscode_network(
                            &target_id,
                            session_id,
                            fingerprint,
                            proxy.as_ref(),
                        ),
                        _ => Err("relayUnavailable".into()),
                    };
                    let _ = sender.send(PostConnectResult::Vscode(
                        result.clone(),
                        timing("vscodeNetwork.refresh", started, result.is_ok()),
                    ));
                });
            }
        }
        drop(sender);

        for result in receiver {
            let Ok(mut state) = lock() else {
                return;
            };
            if state.connection_generation != generation || state.child.is_none() {
                return;
            }
            match result {
                PostConnectResult::ToolDiscovery(result, entry) => {
                    // CLI states are observations, not bridge or configuration failures.
                    if result.is_err() {
                        state
                            .summary
                            .post_connect_error
                            .get_or_insert_with(|| "remoteUnsupported".into());
                    }
                    state.summary.timings.push(entry);
                }
                PostConnectResult::Environment(result, entry) => {
                    state.events.push(
                        EventComponent::SessionEnvironment,
                        EventAction::EnvironmentSetup,
                        if result.is_ok() {
                            EventOutcome::Success
                        } else {
                            EventOutcome::Failed
                        },
                        result
                            .as_ref()
                            .err()
                            .map(|code| ErrorCategory::from_code(code)),
                        Some(entry.duration_ms),
                    );
                    state.summary.session_environment_state = if result.is_ok() {
                        RuntimeState::Ready
                    } else {
                        RuntimeState::Warning
                    };
                    if let Err(code) = result {
                        state.summary.post_connect_error.get_or_insert(code);
                    }
                    state.summary.timings.push(entry);
                }
                PostConnectResult::Vscode(result, entry) => {
                    state.events.push(
                        EventComponent::Vscode,
                        EventAction::VscodeSetup,
                        if result.is_ok() {
                            EventOutcome::Success
                        } else {
                            EventOutcome::Failed
                        },
                        result
                            .as_ref()
                            .err()
                            .map(|code| ErrorCategory::from_code(code)),
                        Some(entry.duration_ms),
                    );
                    state.summary.vscode_state = if result.is_ok() {
                        RuntimeState::Ready
                    } else {
                        RuntimeState::Warning
                    };
                    if let Err(code) = result {
                        state.summary.post_connect_error.get_or_insert(code);
                    }
                    state.summary.timings.push(entry);
                }
            }
        }

        if let Ok(mut state) = lock() {
            if state.connection_generation != generation || state.child.is_none() {
                return;
            }
            let warning = [
                state.summary.session_environment_state,
                state.summary.vscode_state,
                state.summary.codex_state,
                state.summary.claude_state,
                state.summary.skills_state,
            ]
            .into_iter()
            .any(|value| value == RuntimeState::Warning);
            state.summary.post_connect_status = if warning {
                PostConnectStatus::Partial
            } else {
                PostConnectStatus::Ready
            };
            state
                .summary
                .timings
                .push(timing("postConnect.total", post_connect_started, !warning));
        }
    });
}

pub(super) struct InteractiveConnectCompletion {
    pub request: Request,
    pub summary: Summary,
    pub fingerprint: String,
    pub process: ssh_auth::PtyProcess,
    pub auth: SshAuthState,
    pub auth_duration_ms: u64,
    pub core_duration_ms: u64,
    pub proxy_relay: Option<authenticated_relay::AuthenticatedRelay>,
    pub ai_relay: Option<authenticated_relay::AuthenticatedRelay>,
}

pub(super) fn prepare_interactive_connect(
    request: &Request,
) -> BridgeResult<InteractiveConnectPlan> {
    let started = Instant::now();
    {
        let state = lock()?;
        if state.child.is_some() {
            return Err("alreadyConnected".into());
        }
    }
    let fingerprint = ssh::fingerprint(&request.target_id)?;
    let mut summary = preview(request)?;
    let (proxy_relay, ai_relay) = relays_for(&summary)?;
    let endpoints = forwarding_endpoints(&summary, proxy_relay.as_ref(), ai_relay.as_ref())?;
    summary
        .timings
        .push(timing("bridge.prepare", started, true));
    Ok(InteractiveConnectPlan {
        summary,
        fingerprint,
        endpoints,
        proxy_relay,
        ai_relay,
    })
}

pub(super) fn mark_interactive_connecting() -> BridgeResult<()> {
    let mut state = lock()?;
    if state.child.is_some() {
        return Err("alreadyConnected".into());
    }
    reconnect::cancel(&mut state);
    state.disabled_tools.clear();
    state.pending = None;
    state.extension_pending = None;
    state.summary.status = Status::Connecting;
    state.summary.error = None;
    state.connect_started_at = Some(Instant::now());
    state.events.clear();
    state.events.push(
        EventComponent::Ssh,
        EventAction::Connect,
        EventOutcome::Started,
        None,
        None,
    );
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
    completion: InteractiveConnectCompletion,
) -> BridgeResult<Summary> {
    let InteractiveConnectCompletion {
        request,
        mut summary,
        fingerprint,
        process,
        auth,
        auth_duration_ms,
        core_duration_ms,
        proxy_relay,
        ai_relay,
    } = completion;
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
    summary.timings.push(PhaseTiming {
        phase: "ssh.authenticate".into(),
        duration_ms: auth_duration_ms,
        outcome: "success".into(),
    });
    summary.timings.push(PhaseTiming {
        phase: "coreBridge.total".into(),
        duration_ms: core_duration_ms,
        outcome: "success".into(),
    });
    summary.timings.push(PhaseTiming {
        phase: "bridge.forwardReady".into(),
        duration_ms: core_duration_ms,
        outcome: "success".into(),
    });
    prepare_post_connect(&mut summary);
    let mut state = lock()?;
    if state.child.is_some() {
        return Err("alreadyConnected".into());
    }
    if let Some(started) = state.connect_started_at.take() {
        summary.timings.push(timing("bridge.total", started, true));
    }
    state.summary = summary;
    state.last_request = Some(request);
    state.proxy_status = Status::Connected;
    state.cc_status = Status::Connected;
    state.child = Some(Box::new(process));
    state.proxy_relay = proxy_relay;
    state.ai_relay = ai_relay;
    state.reachable = true;
    state.target_fingerprint = Some(fingerprint);
    state.ssh_auth = auth;
    state.events.push(
        EventComponent::Ssh,
        EventAction::Connect,
        EventOutcome::Success,
        None,
        Some(core_duration_ms),
    );
    let generation = state.connection_generation;
    let snapshot = exposed_state_summary(&state);
    drop(state);
    start_post_connect(generation);
    Ok(snapshot)
}

pub fn connect(request: Request, confirmed: bool) -> BridgeResult<Summary> {
    if !confirmed {
        return Err("confirmationRequired".into());
    }
    let mut state = lock()?;
    if state.child.is_some() {
        return Err("alreadyConnected".into());
    }
    reconnect::cancel(&mut state);
    state.disabled_tools.clear();
    state.pending = None;
    let connect_started = Instant::now();
    state.events.clear();
    state.events.push(
        EventComponent::Ssh,
        EventAction::Connect,
        EventOutcome::Started,
        None,
        None,
    );
    let result = (|| {
        state.extension_pending = None;
        let fingerprint = ssh::fingerprint(&request.target_id)?;
        let mut next = preview(&request)?;
        state.summary.status = Status::Connecting;
        let (proxy_relay, ai_relay) = relays_for(&next)?;
        let endpoints = forwarding_endpoints(&next, proxy_relay.as_ref(), ai_relay.as_ref())?;
        let ports: Vec<_> = endpoints.iter().map(|e| e.0).collect();
        ssh::remote(
            &request.target_id,
            json!({"operation":"check","ports":ports}),
        )?;
        let forward_started = Instant::now();
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
        next.timings
            .push(timing("bridge.forwardReady", forward_started, true));
        next.timings
            .push(timing("coreBridge.total", connect_started, true));
        prepare_post_connect(&mut next);
        state.summary = next;
        state.last_request = Some(request.clone());
        state.proxy_status = Status::Connected;
        state.cc_status = Status::Connected;
        state.child = Some(Box::new(child));
        state.proxy_relay = proxy_relay;
        state.ai_relay = ai_relay;
        state.ssh_auth = SshAuthState {
            mode: SshAuthMode::NonInteractive,
            method: ssh::non_interactive_auth_method(&request.target_id),
            authenticated: true,
            password_stored: false,
        };
        state.summary.ssh_auth = state.ssh_auth;
        state.reachable = true;
        state.target_fingerprint = Some(fingerprint);
        Ok(exposed_state_summary(&state))
    })();
    if let Err(code) = &result {
        state.summary.status = Status::Error;
        state.summary.error = Some(code.clone());
    }
    record_result(
        &mut state,
        EventComponent::Ssh,
        EventAction::Connect,
        &result,
        connect_started,
    );
    let generation = state.connection_generation;
    drop(state);
    if result.is_ok() {
        start_post_connect(generation);
    }
    result
}
pub fn disconnect(confirmed: bool) -> BridgeResult<Summary> {
    if !confirmed {
        return Err("confirmationRequired".into());
    }
    let mut state = lock()?;
    // Invalidate in-flight recovery before revoking credentials or cleanup.
    state.connection_generation = state.connection_generation.wrapping_add(1);
    state.last_request = None;
    state.reconnect = None;
    state.summary.reconnect_state = reconnect::State::Idle;
    let target_id = state
        .summary
        .target
        .as_ref()
        .map(|target| target.id.clone());
    let session_id = state
        .proxy_relay
        .as_ref()
        .map(|relay| relay.session_id().to_owned());
    // Revoke both local capabilities before any potentially slow SSH cleanup.
    state.proxy_relay = None;
    state.ai_relay = None;
    let vscode_restore_error = restore_vscode_network(&mut state).err();
    if let Some(target_id) = target_id.as_deref() {
        remove_session_environment(target_id, session_id.as_deref());
    }
    state.child = None;
    state.pending = None;
    state.extension_pending = None;
    state.profile_syncing = false;
    state.additional_tunnels.clear();
    state.paused_proxy = None;
    state.paused_cc = None;
    state.paused_tools.clear();
    state.capability_vscode_managed = false;
    state.local_codex_profile = None;
    state.local_claude_profile = None;
    state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
    state.summary.codex_profile_state = if state.follow_local_codex_profile {
        settings::ProfileSyncState::NotStarted
    } else {
        settings::ProfileSyncState::Disabled
    };
    state.summary.status = Status::Disconnected;
    state.summary.proxy_status = state.summary.proxy.as_ref().map(|_| Status::Disconnected);
    state.summary.cc_status = state.summary.cc.as_ref().map(|_| Status::Disconnected);
    state.proxy_status = Status::Disconnected;
    state.cc_status = Status::Disconnected;
    invalidate_tool_verification(&mut state.summary);
    state.summary.diagnostics.general_proxy_egress.reset();
    state.summary.diagnostics.server_direct.reset();
    state.summary.diagnostics.server_internet = None;
    state.summary.error = vscode_restore_error;
    let cleanup_error = state.summary.error.as_deref().map(ErrorCategory::from_code);
    state.events.push(
        EventComponent::Ssh,
        EventAction::Disconnect,
        if cleanup_error.is_some() {
            EventOutcome::Warning
        } else {
            EventOutcome::Success
        },
        cleanup_error,
        None,
    );
    credential_cache::clear();
    Ok(exposed_state_summary(&state))
}

pub fn retry_reconnect() -> BridgeResult<Summary> {
    let mut state = lock()?;
    refresh(&mut state);
    reconnect::resume(&mut state)?;
    Ok(exposed_state_summary(&state))
}
pub fn clear_session_credential() {
    credential_cache::clear();
    if let Ok(mut state) = lock() {
        state.ssh_auth.password_stored = false;
        state.summary.ssh_auth.password_stored = false;
        state.events.push(
            EventComponent::Ssh,
            EventAction::CredentialClear,
            EventOutcome::Success,
            None,
            None,
        );
    }
}
pub fn shutdown() {
    STOPPING.store(true, std::sync::atomic::Ordering::SeqCst);
    // Never hold up application exit behind an SSH timeout. Windows closes all
    // outstanding kill-on-close jobs even when an operation owns the mutex.
    if let Ok(mut state) = store().try_lock() {
        state.child = None;
        state.additional_tunnels.clear();
        state.proxy_relay = None;
        state.ai_relay = None;
        state.profile_syncing = false;
        state.local_codex_profile = None;
        state.local_claude_profile = None;
        state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
        state.events.clear();
    }
    credential_cache::clear();
    ssh_auth::shutdown();
    logging::shutdown();
}
pub fn test() -> BridgeResult<()> {
    test_for_generation(None)
}
fn test_for_generation(expected_generation: Option<u64>) -> BridgeResult<()> {
    test_proxy_egress(expected_generation, false)
}

fn should_auto_proxy_test(summary: &Summary, connected: bool) -> bool {
    capability_ready(connected, summary.proxy_status)
        && summary.proxy.is_some()
        && summary.diagnostics.general_proxy_egress.state == DiagnosticState::NotTested
}

fn start_auto_proxy_test(generation: u64) {
    // Independent of post-connect setup and paid model verification.
    std::thread::spawn(move || {
        let _ = test_proxy_egress(Some(generation), true);
    });
}

fn test_proxy_egress(expected_generation: Option<u64>, only_unchecked: bool) -> BridgeResult<()> {
    let _operation = crate::desktop::updates::bridge_operation()?;
    let started = Instant::now();
    let (endpoint, target_id, fingerprint, generation, ticket) = {
        let mut state = lock()?;
        if expected_generation.is_some_and(|expected| state.connection_generation != expected) {
            return Err("bridgeUnavailable".into());
        }
        refresh(&mut state);
        // Check and claim under the same lock: duplicate background work must
        // neither repeat a completed probe nor replace an in-flight result.
        if only_unchecked && !should_auto_proxy_test(&state.summary, state.child.is_some()) {
            return Ok(());
        }
        if !capability_ready(state.child.is_some(), state.summary.proxy_status) {
            return Err("bridgeUnavailable".into());
        }
        if state.summary.diagnostics.general_proxy_egress.state == DiagnosticState::Testing {
            return Err("bridgeBusy".into());
        }
        let endpoint = state.summary.proxy.clone().ok_or("proxyUnavailable")?;
        let target_id = state
            .summary
            .target
            .as_ref()
            .map(|target| target.id.clone())
            .ok_or("invalidTarget")?;
        let fingerprint = state.target_fingerprint.clone().ok_or("sshConfigChanged")?;
        if ssh::fingerprint(&target_id)? != fingerprint {
            return Err("sshConfigChanged".into());
        }
        let ticket = state.summary.diagnostics.general_proxy_egress.begin();
        state.events.push(
            EventComponent::GeneralProxy,
            EventAction::EgressTest,
            EventOutcome::Started,
            None,
            None,
        );
        (
            endpoint,
            target_id,
            fingerprint,
            state.connection_generation,
            ticket,
        )
    };
    // Keep UI snapshots and Disconnect available while the existing probe runs.
    let result = ssh::remote(
        &target_id,
        json!({
            "operation":"test",
            "port":endpoint.remote_port,
            "protocol":endpoint.local.protocol
        }),
    )
    .and_then(|_| {
        if ssh::fingerprint(&target_id)? == fingerprint {
            Ok(())
        } else {
            Err("sshConfigChanged".into())
        }
    });
    let mut state = lock()?;
    refresh(&mut state);
    if state.connection_generation != generation
        || state.summary.target.as_ref().map(|target| &target.id) != Some(&target_id)
        || state.target_fingerprint.as_ref() != Some(&fingerprint)
        || state.summary.proxy.as_ref().is_none_or(|current| {
            current.remote_port != endpoint.remote_port || current.local != endpoint.local
        })
        || !capability_ready(state.child.is_some(), state.summary.proxy_status)
        || !state
            .summary
            .diagnostics
            .general_proxy_egress
            .current(ticket)
    {
        return Err("bridgeUnavailable".into());
    }
    state.summary.diagnostics.general_proxy_egress.finish(
        ticket,
        result
            .as_ref()
            .err()
            .map(|code| ErrorCategory::from_code(code)),
        elapsed_ms(started),
    );
    record_result(
        &mut state,
        EventComponent::GeneralProxy,
        EventAction::EgressTest,
        &result,
        started,
    );
    result
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn record_result<T>(
    state: &mut Store,
    component: EventComponent,
    action: EventAction,
    result: &BridgeResult<T>,
    started: Instant,
) {
    state.events.push(
        component,
        action,
        if result.is_ok() {
            EventOutcome::Success
        } else {
            EventOutcome::Failed
        },
        result
            .as_ref()
            .err()
            .map(|code| ErrorCategory::from_code(code)),
        Some(elapsed_ms(started)),
    );
}
fn connected_terminal_context() -> BridgeResult<(String, String)> {
    let (target_id, target_fingerprint) = {
        let mut state = lock()?;
        refresh(&mut state);
        if state.child.is_none() {
            return Err("bridgeUnavailable".into());
        }
        let target_id = state
            .summary
            .target
            .as_ref()
            .map(|target| target.id.clone())
            .ok_or("invalidTarget")?;
        (target_id, state.target_fingerprint.clone())
    };
    if Some(ssh::fingerprint(&target_id)?) != target_fingerprint {
        return Err("sshConfigChanged".into());
    }
    let fingerprint = target_fingerprint.ok_or("sshConfigChanged")?;
    Ok((target_id, fingerprint))
}
fn proxy_terminal_context() -> BridgeResult<(String, String, String)> {
    let (target_id, fingerprint) = connected_terminal_context()?;
    let mut state = lock()?;
    refresh(&mut state);
    if !capability_ready(state.child.is_some(), state.summary.proxy_status)
        || state
            .summary
            .target
            .as_ref()
            .map(|target| target.id.as_str())
            != Some(target_id.as_str())
        || state.target_fingerprint.as_deref() != Some(fingerprint.as_str())
    {
        return Err("bridgeUnavailable".into());
    }
    state.summary.proxy.as_ref().ok_or("proxyUnavailable")?;
    let session_id = state
        .proxy_relay
        .as_ref()
        .map(|relay| relay.session_id().to_owned())
        .ok_or("relayUnavailable")?;
    // The core bridge is shown before optional background setup completes.
    // Never hand a terminal a source command for an unprepared env.sh.
    if state.summary.session_environment_state != RuntimeState::Ready {
        let result = apply_session_environment(&target_id, &state.summary, Some(&session_id));
        state.summary.session_environment_state = if result.is_ok() {
            RuntimeState::Ready
        } else {
            RuntimeState::Warning
        };
        result?;
    }
    Ok((target_id, session_id, fingerprint))
}
pub fn launch_proxy_terminal() -> BridgeResult<()> {
    let (target_id, session_id, fingerprint) = proxy_terminal_context()?;
    ssh::launch_managed_terminal(&target_id, &session_id, &fingerprint)
}

fn restore_vscode_network(state: &mut Store) -> BridgeResult<()> {
    if let Some(target) = &state.summary.target {
        vscode_network::restore(&target.id)?;
    }
    Ok(())
}

fn refresh_owned_vscode_network(
    target_id: &str,
    session_id: &str,
    fingerprint: &str,
    proxy: Option<&Endpoint>,
) -> BridgeResult<()> {
    if !vscode_network::is_managed(target_id)? {
        return Ok(());
    }
    vscode_network::apply(target_id, session_id, fingerprint, proxy)
}

/// Configure only the connected remote VS Code Server, never local user settings.
/// The user invokes this through Open in VS Code; credentials stay out of IPC.
pub fn open_vscode(target: String) -> BridgeResult<Option<String>> {
    let mut state = lock()?;
    refresh(&mut state);
    if state.child.is_none()
        || state.summary.target.as_ref().map(|item| item.id.as_str()) != Some(target.as_str())
    {
        return Err("bridgeUnavailable".into());
    }
    if !state
        .summary
        .target
        .as_ref()
        .is_some_and(|item| item.can_open_vscode)
    {
        return Err("vscodeConfigMismatch".into());
    }
    let fingerprint = ssh::fingerprint(&target)?;
    if state.target_fingerprint.as_deref() != Some(fingerprint.as_str()) {
        return Err("sshConfigChanged".into());
    }
    if state.summary.proxy.is_none() && state.summary.cc.is_none() {
        return vscode::open(target).map(|_| None);
    }
    // Apply the remote VS Code network isolation before the window starts. If the
    // server already exists, launching first lets extensions inherit stale proxy
    // settings and can trigger a proxy-authentication loop until the window reloads.
    open_vscode_with_setup(
        || vscode::open(target.clone()),
        || {
            let relay = state
                .ai_relay
                .as_ref()
                .or(state.proxy_relay.as_ref())
                .ok_or("bridgeUnavailable")?;
            vscode_network::apply(
                &target,
                relay.session_id(),
                &fingerprint,
                state.summary.proxy.as_ref(),
            )?;
            Ok(())
        },
    )
}

fn open_vscode_with_setup(
    launch: impl FnOnce() -> BridgeResult<()>,
    configure: impl FnOnce() -> BridgeResult<()>,
) -> BridgeResult<Option<String>> {
    match configure() {
        Ok(()) => {
            launch()?;
            Ok(None)
        }
        // Opening the reviewed SSH target is still useful when the optional
        // pre-launch configuration connection cannot authenticate or is
        // transiently unavailable. VS Code owns its Remote-SSH authentication,
        // so surface the isolation failure as a warning instead of blocking the
        // launch. Security and configuration validation failures remain fatal.
        Err(code) if vscode_setup_allows_launch(&code) => {
            launch()?;
            Ok(Some(code))
        }
        Err(code) => Err(code),
    }
}

fn vscode_setup_allows_launch(code: &str) -> bool {
    matches!(
        code,
        "vscodeServerMissing"
            | "remoteNodeMissing"
            | "remoteNodeUnsupported"
            | "sshAuth"
            | "sshFailed"
            | "sshTimeout"
    )
}

#[cfg(test)]
mod vscode_launch_tests {
    use super::*;

    #[test]
    fn remote_setup_precedes_launch() {
        let calls = std::cell::RefCell::new(Vec::new());
        let warning = open_vscode_with_setup(
            || {
                calls.borrow_mut().push("launch");
                Ok(())
            },
            || {
                calls.borrow_mut().push("configure");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(warning, None);
        assert_eq!(*calls.borrow(), ["configure", "launch"]);
    }

    #[test]
    fn unsafe_remote_setup_prevents_launch() {
        let opened = std::cell::Cell::new(false);
        assert_eq!(
            open_vscode_with_setup(
                || {
                    opened.set(true);
                    Ok(())
                },
                || Err("unsafePath".into()),
            )
            .unwrap_err(),
            "unsafePath"
        );
        assert!(!opened.get());
    }

    #[test]
    fn missing_vscode_server_allows_bootstrap_with_warning() {
        let opened = std::cell::Cell::new(false);
        let warning = open_vscode_with_setup(
            || {
                opened.set(true);
                Ok(())
            },
            || Err("vscodeServerMissing".into()),
        )
        .unwrap();
        assert!(opened.get());
        assert_eq!(warning.as_deref(), Some("vscodeServerMissing"));
    }

    #[test]
    fn setup_transport_failure_does_not_block_remote_ssh_launch() {
        for code in [
            "sshAuth",
            "sshFailed",
            "sshTimeout",
            "remoteNodeMissing",
            "remoteNodeUnsupported",
        ] {
            let opened = std::cell::Cell::new(false);
            let warning = open_vscode_with_setup(
                || {
                    opened.set(true);
                    Ok(())
                },
                || Err(code.into()),
            )
            .unwrap();
            assert!(opened.get(), "{code} should not block VS Code");
            assert_eq!(warning.as_deref(), Some(code));
        }
    }

    #[test]
    fn failed_launch_is_reported_after_successful_setup() {
        assert_eq!(
            open_vscode_with_setup(|| Err("vscodeMissing".into()), || Ok(())).unwrap_err(),
            "vscodeMissing"
        );
    }
}
fn capability_ready(ssh_running: bool, status: Option<Status>) -> bool {
    ssh_running && status == Some(Status::Connected)
}

pub fn session_environment_command() -> BridgeResult<String> {
    let (_, session_id, _) = proxy_terminal_context()?;
    Ok(target_os::linux::session_environment_source_command(
        &session_id,
    ))
}
pub fn launch_manual_terminal() -> BridgeResult<()> {
    // AI-only bridges also need a plain remote terminal, not proxy exports.
    let (target_id, fingerprint) = connected_terminal_context()?;
    ssh::launch_manual_terminal(&target_id, &fingerprint)
}
pub fn config_preview(tool: String) -> BridgeResult<ConfigPreview> {
    let adapter = tool_adapter::by_name(&tool)?;
    let mut state = lock()?;
    refresh(&mut state);
    config_preview_locked(&mut state, adapter)
}

pub fn config_apply(id: String, confirmed: bool) -> BridgeResult<()> {
    if !confirmed {
        return Err("confirmationRequired".into());
    }
    let mut state = lock()?;
    refresh(&mut state);
    config_apply_locked(&mut state, id)
}

pub fn verify_tool(tool: String) -> BridgeResult<ToolVerificationResult> {
    verify_tool_for_generation(tool, None)
}
#[cfg(test)]
mod tests;
