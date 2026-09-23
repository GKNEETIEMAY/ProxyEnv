mod authenticated_relay;
#[cfg(test)]
#[allow(dead_code)]
mod codex_relay;
pub(crate) mod connections;
pub(crate) mod credential_cache;
pub mod extension;
mod local_model;
pub(crate) mod mobaxterm;
pub(crate) mod settings;
mod ssh;
pub mod ssh_auth;
pub mod tool_adapter;
pub(crate) mod vscode;
#[cfg(test)]
use super::proxy::ProxyVariable;
use super::proxy::{active, plan, ProxyEndpoint, ProxyProtocol};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    net::{SocketAddr, TcpStream},
    sync::{Mutex, OnceLock},
    time::Duration,
};

pub type BridgeResult<T> = std::result::Result<T, String>;

pub const DEFAULT_CC_REMOTE_PORT: u16 = 15_721;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemoteTargetSource {
    Openssh,
    Vscode,
    Mobaxterm,
    Manual,
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
    pub runtime_expected_proxy_port: Option<u16>,
    pub runtime_proxy_match: RuntimeProxyMatch,
    pub cc: Option<Endpoint>,
    pub proxy_status: Option<Status>,
    pub cc_status: Option<Status>,
    pub active_proxy_revision: Option<u64>,
    pub environment: String,
    pub codex_configured: bool,
    pub claude_configured: bool,
    pub claude_profile_state: settings::ProfileSyncState,
    #[serde(skip)]
    pub(crate) codex_verification: tool_adapter::RemoteToolVerification,
    #[serde(skip)]
    pub(crate) claude_verification: tool_adapter::RemoteToolVerification,
    pub tools: Vec<tool_adapter::RemoteToolState>,
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
    summary: Summary,
    child: Option<Box<dyn ssh::ManagedSsh>>,
    proxy_relay: Option<authenticated_relay::AuthenticatedRelay>,
    ai_relay: Option<authenticated_relay::AuthenticatedRelay>,
    pending: Option<Pending>,
    extension_pending: Option<extension::Pending>,
    cc_detected: bool,
    reachable: bool,
    target_fingerprint: Option<String>,
    proxy_status: Status,
    cc_status: Status,
    ssh_auth: SshAuthState,
    follow_local_codex_profile: bool,
    local_codex_profile: Option<local_model::LocalCodexProfile>,
    local_claude_profile: Option<local_model::LocalClaudeProfile>,
    profile_sync_state: settings::ProfileSyncState,
    profile_syncing: bool,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            summary: Summary::default(),
            child: None,
            proxy_relay: None,
            ai_relay: None,
            pending: None,
            extension_pending: None,
            cc_detected: false,
            reachable: false,
            target_fingerprint: None,
            proxy_status: Status::Disconnected,
            cc_status: Status::Disconnected,
            ssh_auth: SshAuthState::default(),
            follow_local_codex_profile: true,
            local_codex_profile: None,
            local_claude_profile: None,
            profile_sync_state: settings::ProfileSyncState::NotStarted,
            profile_syncing: false,
        }
    }
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
    if let Some(child) = state.child.as_mut() {
        if !matches!(child.is_running(), Ok(true)) {
            state.child = None;
            state.proxy_relay = None;
            state.ai_relay = None;
            state.summary.status = Status::Disconnected;
            state.proxy_status = Status::Disconnected;
            state.cc_status = Status::Disconnected;
            state.summary.proxy_status = state.summary.proxy.as_ref().map(|_| Status::Disconnected);
            state.summary.cc_status = state.summary.cc.as_ref().map(|_| Status::Disconnected);
            invalidate_tool_verification(&mut state.summary);
            state.reachable = false;
            state.pending = None;
            state.extension_pending = None;
            state.local_codex_profile = None;
            state.local_claude_profile = None;
            state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
            state.profile_sync_state = settings::ProfileSyncState::NotStarted;
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
        if state.summary.status != Status::Connected
            || state.summary.cc_status != Some(Status::Connected)
        {
            invalidate_tool_verification(&mut state.summary);
        }
    }
}

fn sync_tool_states(summary: &mut Summary) {
    summary.tools = tool_adapter::adapters()
        .iter()
        .map(|adapter| adapter.inspect(summary))
        .collect();
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

fn refresh_tool_configuration(summary: &mut Summary, session_token: Option<&str>) {
    let Some(target_id) = summary.target.as_ref().map(|target| target.id.clone()) else {
        return;
    };
    let Some(route_port) = summary.cc.as_ref().map(|endpoint| endpoint.remote_port) else {
        for adapter in tool_adapter::adapters() {
            adapter.restore(summary);
        }
        sync_tool_states(summary);
        return;
    };
    for adapter in tool_adapter::adapters() {
        let status = ssh::remote(
            &target_id,
            remote_request("status", *adapter, route_port, None, session_token),
        )
        .ok();
        let mut configured = status
            .as_ref()
            .and_then(|value| value["configured"].as_bool())
            .unwrap_or(false);

        // A reconnect rotates the session token. If this tool is already
        // ProxyEnv-owned and still points at the same route, refresh the
        // allowlisted projection transactionally instead of forcing the user
        // through configuration again. Unowned files are never touched.
        let owned_route = status.as_ref().is_some_and(|value| {
            value["owned"] == true && value["previousPort"].as_u64() == Some(u64::from(route_port))
        });
        if !configured && owned_route {
            if let (Some(session_token), Ok(profile)) =
                (session_token, LocalToolProfile::inspect(adapter.id()))
            {
                let preview = ssh::remote(
                    &target_id,
                    remote_request(
                        "preview",
                        *adapter,
                        route_port,
                        Some(&profile),
                        Some(session_token),
                    ),
                );
                if let Ok(preview) = preview {
                    if let Some(expected_hash) = preview["expectedHash"].as_str() {
                        let mut apply = remote_request(
                            "apply",
                            *adapter,
                            route_port,
                            Some(&profile),
                            Some(session_token),
                        );
                        apply["expectedHash"] = json!(expected_hash);
                        apply["repairPermissions"] =
                            json!(preview["permissionHardening"].as_bool().unwrap_or(false));
                        configured = ssh::remote(&target_id, apply).is_ok();
                    }
                }
            }
        }
        if configured {
            adapter.apply(summary);
        } else {
            adapter.restore(summary);
        }
    }
    sync_tool_states(summary);
}

fn invalidate_tool_verification(summary: &mut Summary) {
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
            state.profile_sync_state = value;
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
        if cached_profile(&state, tool).is_some_and(|cached| cached.hash() == profile.hash()) {
            set_cached_profile(&mut state, Some(profile));
            state.profile_syncing = false;
            if tool == tool_adapter::RemoteToolId::Codex
                && state.profile_sync_state != settings::ProfileSyncState::RestartRequired
            {
                state.profile_sync_state = settings::ProfileSyncState::Synced;
            } else if tool == tool_adapter::RemoteToolId::Claude {
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
        )
    };
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
    if preview["profileHash"].as_str() == Some(profile.hash())
        && preview["previousPort"].as_u64() == Some(u64::from(snapshot.1))
    {
        if let Ok(mut state) = store().lock() {
            set_cached_profile(&mut state, Some(profile));
            state.profile_syncing = false;
            if tool == tool_adapter::RemoteToolId::Codex
                && state.profile_sync_state != settings::ProfileSyncState::RestartRequired
            {
                state.profile_sync_state = settings::ProfileSyncState::Synced;
            } else if tool == tool_adapter::RemoteToolId::Claude {
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
    if let Err(code) = ssh::remote(&snapshot.0, apply_request) {
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
    let verified = ssh::remote(
        &snapshot.0,
        remote_request(
            "preview",
            adapter,
            snapshot.1,
            Some(&profile),
            Some(snapshot.3.as_str()),
        ),
    );
    let verified = matches!(
        verified,
        Ok(value)
            if value["profileHash"].as_str() == Some(profile.hash())
                && value["previousPort"].as_u64() == Some(u64::from(snapshot.1))
    );
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
            if tool == tool_adapter::RemoteToolId::Codex {
                state.profile_sync_state = settings::ProfileSyncState::RestartRequired;
            } else {
                state.summary.claude_profile_state = settings::ProfileSyncState::RestartRequired;
            }
        } else if tool == tool_adapter::RemoteToolId::Codex && !state.follow_local_codex_profile {
            clear_cached_profile(&mut state, tool);
            state.profile_sync_state = settings::ProfileSyncState::Disabled;
        } else if state.child.is_none() || state.ai_relay.is_none() {
            clear_cached_profile(&mut state, tool);
            if tool == tool_adapter::RemoteToolId::Codex {
                state.profile_sync_state = settings::ProfileSyncState::NotStarted;
            } else {
                state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
            }
        } else if tool == tool_adapter::RemoteToolId::Codex {
            state.profile_sync_state = settings::ProfileSyncState::RemoteUnavailable;
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

pub fn start_monitor() {
    if let Ok(mut state) = store().lock() {
        state.follow_local_codex_profile = settings::load()
            .map(|value| value.follow_local_codex_profile)
            .unwrap_or(true);
    }
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(2));
        ssh_auth::cleanup();
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
                state.profile_sync_state = if state.follow_local_codex_profile {
                    settings::ProfileSyncState::NotStarted
                } else {
                    settings::ProfileSyncState::Disabled
                };
                continue;
            }
            let mut changed = None;
            for tool in [
                tool_adapter::RemoteToolId::Codex,
                tool_adapter::RemoteToolId::Claude,
            ] {
                if !profile_sync_enabled(&state, tool) {
                    clear_cached_profile(&mut state, tool);
                    if tool == tool_adapter::RemoteToolId::Codex {
                        state.profile_sync_state = if state.follow_local_codex_profile {
                            settings::ProfileSyncState::NotStarted
                        } else {
                            settings::ProfileSyncState::Disabled
                        };
                    } else {
                        state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
                    }
                    continue;
                }
                match cached_profile(&state, tool) {
                    Some(profile) => match profile.stamp_changed() {
                        Ok(false) => {}
                        Ok(true) => {
                            if tool == tool_adapter::RemoteToolId::Codex {
                                state.profile_sync_state = settings::ProfileSyncState::LocalChanged;
                            } else {
                                state.summary.claude_profile_state =
                                    settings::ProfileSyncState::LocalChanged;
                            }
                            changed = Some(tool);
                            break;
                        }
                        Err(_) => {
                            if tool == tool_adapter::RemoteToolId::Codex {
                                state.profile_sync_state =
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
        // Coalesce editor save sequences (temporary file, rename, metadata update)
        // before opening and parsing either profile file.
        std::thread::sleep(Duration::from_millis(500));
        sync_changed_profile(candidate);
    });
}
pub fn summary() -> BridgeResult<Summary> {
    Ok(exposed_summary(&lock()?.summary))
}

pub fn model_settings() -> BridgeResult<settings::RemoteBridgeSettingsView> {
    let state = lock()?;
    let mut view = settings::current_view(None)?;
    view.profile_state = state.profile_sync_state;
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
        state.profile_sync_state = settings::ProfileSyncState::Disabled;
    } else if state.child.is_some() && state.ai_relay.is_some() && state.summary.codex_configured {
        state.profile_sync_state = settings::ProfileSyncState::LocalChanged;
    } else {
        state.profile_sync_state = settings::ProfileSyncState::NotStarted;
    }
    view.profile_state = state.profile_sync_state;
    Ok(view)
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
pub fn add_connection(input: connections::ManualConnectionInput) -> BridgeResult<RemoteTarget> {
    let connection = connections::add(input)?;
    Ok(connections::target(&connection))
}
pub fn remove_connection(id: String) -> BridgeResult<()> {
    connections::remove(&id)
}
pub fn launch_mobaxterm(target_id: String) -> BridgeResult<()> {
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
fn derived_port(seed: &str, round: u8, lane: u8) -> u16 {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("{seed}\n{round}"));
    let offset = usize::from(lane) * 2;
    20_000 + (u16::from_le_bytes([digest[offset], digest[offset + 1]]) % 40_001)
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

fn first_available_port(
    preferred: impl IntoIterator<Item = u16>,
    excluded: Option<u16>,
    mut available: impl FnMut(u16) -> BridgeResult<bool>,
) -> BridgeResult<u16> {
    for candidate in preferred {
        if Some(candidate) != excluded && available(candidate)? {
            return Ok(candidate);
        }
    }
    Err("portAllocationFailed".into())
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

fn resolve_port_pair(
    fingerprint: &str,
    local_port: Option<u16>,
    runtime_expected_proxy_port: Option<u16>,
    prefer_defaults: bool,
    mut remote_available: impl FnMut(u16) -> BridgeResult<bool>,
) -> BridgeResult<PortAllocation> {
    let mut runtime_port_conflict = false;
    let proxy_port = if let Some(expected) = runtime_expected_proxy_port {
        if remote_available(expected)? {
            expected
        } else {
            runtime_port_conflict = true;
            first_available_port(
                (0..24).map(|round| derived_port(fingerprint, round, 0)),
                None,
                &mut remote_available,
            )?
        }
    } else {
        let preferred = local_port.filter(|_| prefer_defaults).into_iter();
        first_available_port(
            preferred.chain((0..24).map(|round| derived_port(fingerprint, round, 0))),
            None,
            &mut remote_available,
        )?
    };
    let preferred_cc = prefer_defaults
        .then_some(DEFAULT_CC_REMOTE_PORT)
        .into_iter();
    let cc_port = first_available_port(
        preferred_cc.chain((0..24).map(|round| derived_port(fingerprint, round, 1))),
        Some(proxy_port),
        &mut remote_available,
    )?;
    Ok(PortAllocation {
        proxy_port,
        cc_port,
        runtime_expected_proxy_port,
        runtime_port_conflict,
    })
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
    let proxy = summary
        .proxy
        .as_ref()
        .map(|endpoint| {
            authenticated_relay::AuthenticatedRelay::start(
                endpoint.local.clone(),
                authenticated_relay::RelayMode::General(endpoint.local.protocol),
            )
        })
        .transpose()?;
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
    relay: Option<&authenticated_relay::AuthenticatedRelay>,
) -> BridgeResult<()> {
    let Some(endpoint) = summary.proxy.as_ref() else {
        return Ok(());
    };
    let relay = relay.ok_or("relayUnavailable")?;
    let token = relay.token();
    ssh::remote(
        target_id,
        json!({
            "operation": "session-env-apply",
            "sessionId": relay.session_id(),
            "sessionToken": token.as_str(),
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
    let (proxy_relay, ai_relay) = relays_for(&summary)?;
    let endpoints = forwarding_endpoints(&summary, proxy_relay.as_ref(), ai_relay.as_ref())?;
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
    proxy_relay: Option<authenticated_relay::AuthenticatedRelay>,
    ai_relay: Option<authenticated_relay::AuthenticatedRelay>,
) -> BridgeResult<Summary> {
    if ssh::fingerprint(&request.target_id)? != fingerprint {
        return Err("sshConfigChanged".into());
    }
    if request.proxy_port.is_some() {
        preview(&request)?;
    }
    apply_session_environment(&request.target_id, &summary, proxy_relay.as_ref())?;
    summary.status = Status::Connected;
    summary.proxy_status = summary.proxy.as_ref().map(|_| Status::Connected);
    summary.cc_status = summary.cc.as_ref().map(|_| Status::Connected);
    summary.ssh_auth = auth;
    let ai_token = ai_relay.as_ref().map(|relay| relay.token());
    refresh_tool_configuration(&mut summary, ai_token.as_ref().map(|token| token.as_str()));
    let mut state = lock()?;
    if state.child.is_some() {
        return Err("alreadyConnected".into());
    }
    state.summary = summary;
    state.proxy_status = Status::Connected;
    state.cc_status = Status::Connected;
    state.child = Some(Box::new(process));
    state.proxy_relay = proxy_relay;
    state.ai_relay = ai_relay;
    state.reachable = true;
    state.target_fingerprint = Some(fingerprint);
    state.ssh_auth = auth;
    Ok(exposed_summary(&state.summary))
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
        let (proxy_relay, ai_relay) = relays_for(&next)?;
        let endpoints = forwarding_endpoints(&next, proxy_relay.as_ref(), ai_relay.as_ref())?;
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
        apply_session_environment(&request.target_id, &next, proxy_relay.as_ref())?;
        next.status = Status::Connected;
        next.proxy_status = next.proxy.as_ref().map(|_| Status::Connected);
        next.cc_status = next.cc.as_ref().map(|_| Status::Connected);
        let ai_token = ai_relay.as_ref().map(|relay| relay.token());
        refresh_tool_configuration(&mut next, ai_token.as_ref().map(|token| token.as_str()));
        state.summary = next;
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
        Ok(exposed_summary(&state.summary))
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
    if let Some(target_id) = target_id.as_deref() {
        remove_session_environment(target_id, session_id.as_deref());
    }
    state.child = None;
    state.pending = None;
    state.extension_pending = None;
    state.profile_syncing = false;
    state.local_codex_profile = None;
    state.local_claude_profile = None;
    state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
    state.profile_sync_state = if state.follow_local_codex_profile {
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
    credential_cache::clear();
    Ok(exposed_summary(&state.summary))
}
pub fn clear_session_credential() {
    credential_cache::clear();
}
pub fn shutdown() {
    // Never hold up application exit behind an SSH timeout. Windows closes all
    // outstanding kill-on-close jobs even when an operation owns the mutex.
    if let Ok(mut state) = store().try_lock() {
        state.child = None;
        state.proxy_relay = None;
        state.ai_relay = None;
        state.profile_syncing = false;
        state.local_codex_profile = None;
        state.local_claude_profile = None;
        state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
    }
    credential_cache::clear();
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
    let session_token = state
        .proxy_relay
        .as_ref()
        .map(|relay| relay.token())
        .ok_or("proxyUnavailable")?;
    ssh::remote(
        target_id,
        json!({
            "operation":"test",
            "port":endpoint.remote_port,
            "protocol":endpoint.local.protocol,
            "sessionToken":session_token.as_str()
        }),
    )?;
    Ok(())
}
fn proxy_terminal_context() -> BridgeResult<(String, String, String)> {
    let (target_id, session_id, target_fingerprint) = {
        let mut state = lock()?;
        refresh(&mut state);
        if state.summary.status != Status::Connected
            || state.summary.proxy_status != Some(Status::Connected)
        {
            return Err("bridgeUnavailable".into());
        }
        let target_id = state
            .summary
            .target
            .as_ref()
            .map(|target| target.id.clone())
            .ok_or("invalidTarget")?;
        state.summary.proxy.as_ref().ok_or("proxyUnavailable")?;
        let session_id = state
            .proxy_relay
            .as_ref()
            .map(|relay| relay.session_id().to_owned())
            .ok_or("relayUnavailable")?;
        (target_id, session_id, state.target_fingerprint.clone())
    };
    if Some(ssh::fingerprint(&target_id)?) != target_fingerprint {
        return Err("sshConfigChanged".into());
    }
    let fingerprint = target_fingerprint.ok_or("sshConfigChanged")?;
    Ok((target_id, session_id, fingerprint))
}
pub fn launch_proxy_terminal() -> BridgeResult<()> {
    let (target_id, session_id, fingerprint) = proxy_terminal_context()?;
    ssh::launch_managed_terminal(&target_id, &session_id, &fingerprint)
}
pub fn launch_manual_terminal() -> BridgeResult<()> {
    let (target_id, _, fingerprint) = proxy_terminal_context()?;
    ssh::launch_manual_terminal(&target_id, &fingerprint)
}
pub fn config_preview(tool: String) -> BridgeResult<ConfigPreview> {
    let adapter = tool_adapter::by_name(&tool)?;
    let mut state = lock()?;
    refresh(&mut state);
    if state.profile_syncing {
        return Err("configConflict".into());
    }
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
    let local_profile = Some(LocalToolProfile::inspect(adapter.id()).map_err(
        |_| match adapter.id() {
            tool_adapter::RemoteToolId::Codex => "localCodexProfileInvalid",
            tool_adapter::RemoteToolId::Claude => "localClaudeProfileInvalid",
        },
    )?);
    let session_token = state
        .ai_relay
        .as_ref()
        .map(|relay| relay.token())
        .ok_or("ccUnavailable")?;
    let value = ssh::remote(
        &target_id,
        remote_request(
            "preview",
            adapter,
            port,
            local_profile.as_ref(),
            Some(session_token.as_str()),
        ),
    )?;
    if adapter.id() == tool_adapter::RemoteToolId::Codex
        && value["remoteChanged"].as_bool() == Some(true)
    {
        state.profile_sync_state = settings::ProfileSyncState::RemoteChanged;
    } else if adapter.id() == tool_adapter::RemoteToolId::Claude
        && value["remoteChanged"].as_bool() == Some(true)
    {
        state.summary.claude_profile_state = settings::ProfileSyncState::RemoteChanged;
    }
    let hash = value["expectedHash"]
        .as_str()
        .ok_or("remoteFailed")?
        .to_string();
    let existing_config = value["configExists"].as_bool().ok_or("remoteFailed")?;
    let permission_hardening = value["permissionHardening"]
        .as_bool()
        .ok_or("remoteFailed")?;
    let previous_port = value["previousPort"].as_u64();
    let before = match value["previousPort"].as_u64() {
        Some(p) if (1024..=65535).contains(&p) => {
            adapter
                .preview(p as u16, value["remoteModel"].as_str())
                .content
        }
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
    if !adapter.detect(&version) {
        return Err("cliUnsupported".into());
    }
    let plan = adapter.preview(
        port,
        local_profile.as_ref().and_then(LocalToolProfile::model),
    );
    let preview = ConfigPreview {
        id: hex::encode(nonce),
        tool: adapter.id(),
        path: plan.path.into(),
        before,
        after: plan.content,
        version,
        launch: plan.launch.into(),
        alias: target_id.clone(),
        restore: false,
        existing_config,
        route_update: previous_port.is_some_and(|previous| previous != u64::from(port)),
        permission_hardening,
    };
    state.pending = Some(Pending {
        preview: preview.clone(),
        hash,
        target_id,
        port,
        backup_hash: None,
        target_fingerprint,
        local_profile,
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
    let adapter = tool_adapter::by_id(pending.preview.tool);
    let session_token = state
        .ai_relay
        .as_ref()
        .map(|relay| relay.token())
        .ok_or("ccUnavailable")?;
    let applied = ssh::remote(
        &pending.target_id,
        json!({
            "operation":"apply",
            "tool":pending.preview.tool.as_str(),
            "port":pending.port,
            "expectedHash":pending.hash,
            "repairPermissions":pending.preview.permission_hardening,
            "profileModelBase64":pending.local_profile.as_ref().and_then(|profile| match profile { LocalToolProfile::Codex(profile) => Some(base64(&profile.model)), _ => None }),
            "profileCatalogBase64":pending.local_profile.as_ref().and_then(|profile| match profile { LocalToolProfile::Codex(profile) => Some(base64_bytes(&profile.catalog_bytes)), _ => None }),
            "profileSettingsBase64":pending.local_profile.as_ref().and_then(|profile| match profile { LocalToolProfile::Claude(profile) => Some(base64_bytes(&profile.settings_bytes)), _ => None }),
            "profileHash":pending.local_profile.as_ref().map(LocalToolProfile::hash),
            "sessionToken":session_token.as_str()
        }),
    )?;
    let applied_hash = applied["appliedHash"].as_str().ok_or("remoteFailed")?;
    let backup_hash = applied["backupHash"].as_str().ok_or("remoteFailed")?;
    let rollback = || {
        ssh::remote(
            &pending.target_id,
            json!({
                "operation":"restore",
                "tool":pending.preview.tool.as_str(),
                "expectedHash":applied_hash,
                "backupHash":backup_hash,
                "repairPermissions":pending.preview.permission_hardening
            }),
        )
    };
    let verified = match ssh::remote(
        &pending.target_id,
        remote_request(
            "preview",
            adapter,
            pending.port,
            pending.local_profile.as_ref(),
            Some(session_token.as_str()),
        ),
    ) {
        Ok(value) => value,
        Err(_) => {
            return Err(if rollback().is_ok() {
                "writeRolledBack"
            } else {
                "rollbackFailed"
            }
            .into());
        }
    };
    let profile_verified = pending
        .local_profile
        .as_ref()
        .is_none_or(|profile| verified["profileHash"].as_str() == Some(profile.hash()));
    if verified["previousPort"].as_u64() != Some(u64::from(pending.port)) || !profile_verified {
        return Err(if rollback().is_ok() {
            "writeRolledBack"
        } else {
            "rollbackFailed"
        }
        .into());
    }
    tool_adapter::by_id(pending.preview.tool).apply(&mut state.summary);
    if let Some(profile) = pending.local_profile {
        let tool = pending.preview.tool;
        set_cached_profile(&mut state, Some(profile));
        if tool == tool_adapter::RemoteToolId::Codex {
            state.profile_sync_state = settings::ProfileSyncState::Synced;
        } else {
            state.summary.claude_profile_state = settings::ProfileSyncState::Synced;
        }
    }
    sync_tool_states(&mut state.summary);
    Ok(())
}
pub fn config_restore_preview(target_id: String, tool: String) -> BridgeResult<ConfigPreview> {
    let adapter = tool_adapter::by_name(&tool)?;
    let target_fingerprint = ssh::fingerprint(&target_id)?;
    let mut state = lock()?;
    if state.profile_syncing {
        return Err("configConflict".into());
    }
    let value = ssh::remote(
        &target_id,
        json!({"operation":"restore-preview","tool":adapter.id().as_str()}),
    )?;
    let content = |key: &str| -> BridgeResult<String> {
        match value[key].as_u64() {
            Some(p) if (1024..=65535).contains(&p) => Ok(adapter.preview(p as u16, None).content),
            None if value[key].is_null() => Ok(String::new()),
            _ => Err("remoteFailed".into()),
        }
    };
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| "stateUnavailable")?;
    let preview = ConfigPreview {
        id: hex::encode(nonce),
        alias: target_id.clone(),
        tool: adapter.id(),
        restore: true,
        existing_config: value["originalExists"].as_bool().ok_or("remoteFailed")?,
        route_update: false,
        permission_hardening: value["permissionHardening"]
            .as_bool()
            .ok_or("remoteFailed")?,
        path: adapter.config_path().into(),
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
        local_profile: None,
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
        json!({
            "operation":"restore",
            "tool":pending.preview.tool.as_str(),
            "expectedHash":pending.hash,
            "backupHash":pending.backup_hash,
            "repairPermissions":pending.preview.permission_hardening
        }),
    )?;
    if state.summary.target.as_ref().map(|target| &target.id) == Some(&pending.target_id) {
        tool_adapter::by_id(pending.preview.tool).restore(&mut state.summary);
        if pending.preview.tool == tool_adapter::RemoteToolId::Codex {
            state.local_codex_profile = None;
            state.profile_sync_state = settings::ProfileSyncState::NotStarted;
        } else {
            state.local_claude_profile = None;
            state.summary.claude_profile_state = settings::ProfileSyncState::NotStarted;
        }
        sync_tool_states(&mut state.summary);
    }
    state.pending = None;
    Ok(())
}

pub fn verify_tool(tool: String) -> BridgeResult<ToolVerificationResult> {
    let adapter = tool_adapter::by_name(&tool)?;
    if !adapter.verification_supported() {
        return Err("toolVerificationUnsupported".into());
    }
    let (target_id, target_fingerprint, route_port, session_token) = {
        let mut state = lock()?;
        refresh(&mut state);
        if state.child.is_none()
            || state.summary.status != Status::Connected
            || state.summary.cc_status != Some(Status::Connected)
        {
            return Err("bridgeUnavailable".into());
        }
        if !adapter.configured(&state.summary) {
            return Err("toolNotConfigured".into());
        }
        let target_id = state
            .summary
            .target
            .as_ref()
            .map(|target| target.id.clone())
            .ok_or("invalidTarget")?;
        let fingerprint = state.target_fingerprint.clone().ok_or("sshConfigChanged")?;
        let route_port = state
            .summary
            .cc
            .as_ref()
            .map(|endpoint| endpoint.remote_port)
            .ok_or("bridgeUnavailable")?;
        let session_token = state
            .ai_relay
            .as_ref()
            .map(|relay| relay.token())
            .ok_or("bridgeUnavailable")?;
        if ssh::fingerprint(&target_id)? != fingerprint {
            return Err("sshConfigChanged".into());
        }
        adapter.verify(
            &mut state.summary,
            tool_adapter::RemoteToolVerification::VerifyPending,
        );
        sync_tool_states(&mut state.summary);
        (target_id, fingerprint, route_port, session_token)
    };

    let value = ssh::remote(
        &target_id,
        json!({
            "operation":"tool-verify",
            "tool":adapter.id().as_str(),
            "port":route_port,
            "sessionToken":session_token.as_str()
        }),
    )?;
    let verification: tool_adapter::RemoteToolVerification =
        serde_json::from_value(value.get("verification").cloned().ok_or("remoteFailed")?)
            .map_err(|_| "remoteFailed")?;
    if matches!(
        verification,
        tool_adapter::RemoteToolVerification::NotConfigured
            | tool_adapter::RemoteToolVerification::VerifyPending
    ) {
        return Err("remoteFailed".into());
    }

    let mut state = lock()?;
    refresh(&mut state);
    if state.child.is_none()
        || state.summary.status != Status::Connected
        || state.summary.cc_status != Some(Status::Connected)
        || state.summary.target.as_ref().map(|target| &target.id) != Some(&target_id)
        || state.target_fingerprint.as_ref() != Some(&target_fingerprint)
        || ssh::fingerprint(&target_id)? != target_fingerprint
        || !adapter.configured(&state.summary)
    {
        return Err("bridgeUnavailable".into());
    }
    adapter.verify(&mut state.summary, verification);
    sync_tool_states(&mut state.summary);
    Ok(ToolVerificationResult {
        tool: adapter.id(),
        verification,
    })
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
            "unset HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY\nexport HTTP_PROXY=http://127.0.0.1:17897\nexport HTTPS_PROXY=http://127.0.0.1:17897\nexport NO_PROXY=localhost,127.0.0.1,::1"
        );
        assert_eq!(
            remote_environment(&endpoint(ProxyProtocol::Socks5)).unwrap(),
            "unset HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY\nexport ALL_PROXY=socks5h://127.0.0.1:17897\nexport NO_PROXY=localhost,127.0.0.1,::1"
        );
        assert_eq!(
            remote_environment(&endpoint(ProxyProtocol::Mixed))
                .unwrap()
                .lines()
                .count(),
            5
        );
        assert!(remote_environment(&endpoint(ProxyProtocol::Unknown)).is_err());
    }
    #[test]
    fn codex_projection_contains_profile_and_claude_remains_route_only() {
        let codex: toml::Value = toml::from_str(
            &tool_adapter::by_name("codex")
                .unwrap()
                .preview(25721, Some("deepseek-flash"))
                .content,
        )
        .unwrap();
        assert_eq!(codex["model_provider"].as_str(), Some("proxyenv_bridge"));
        assert_eq!(codex["model"].as_str(), Some("deepseek-flash"));
        assert_eq!(
            codex["model_catalog_json"].as_str(),
            Some("proxyenv-codex-model-catalog.json")
        );
        assert_eq!(codex.as_table().unwrap().len(), 4);
        let claude: serde_json::Value = serde_json::from_str(
            &tool_adapter::by_name("claude")
                .unwrap()
                .preview(25721, None)
                .content,
        )
        .unwrap();
        assert_eq!(claude["env"]["ANTHROPIC_AUTH_TOKEN"], "PROXY_MANAGED");
    }
    #[test]
    fn remote_ports_exclude_privileged_range() {
        assert!(port(1023).is_err());
        assert!(port(1024).is_ok());
        assert!(port(65535).is_ok());
    }
    #[test]
    fn remote_port_fallbacks_are_repeatable_and_independent() {
        assert_eq!(DEFAULT_CC_REMOTE_PORT, 15_721);
        assert_eq!(
            derived_port("same-ssh-target", 0, 0),
            derived_port("same-ssh-target", 0, 0)
        );
        assert_ne!(
            derived_port("same-ssh-target", 0, 0),
            derived_port("same-ssh-target", 0, 1)
        );
        assert!((20_000..=60_000).contains(&derived_port("same-ssh-target", 0, 0)));
    }

    #[test]
    fn runtime_proxy_port_and_local_upstream_are_independent() {
        let expected = 7897;
        for active in [expected, 10809, 2080] {
            let allocation =
                resolve_port_pair("fixture", Some(active), Some(expected), true, |_| Ok(true))
                    .unwrap();
            assert_eq!(allocation.proxy_port, expected);
            assert_eq!(allocation.cc_port, DEFAULT_CC_REMOTE_PORT);
            assert!(!allocation.runtime_port_conflict);
        }
        let no_runtime =
            resolve_port_pair("fixture", Some(10809), None, true, |_| Ok(true)).unwrap();
        assert_eq!(no_runtime.proxy_port, 10809);
        assert_eq!(no_runtime.runtime_expected_proxy_port, None);
        let switched =
            resolve_port_pair("fixture", Some(2080), Some(expected), true, |_| Ok(true)).unwrap();
        assert_eq!(switched.proxy_port, expected);
    }

    #[test]
    fn occupied_runtime_proxy_port_falls_back_without_moving_cc() {
        let expected = 7897;
        let fallback = derived_port("fixture", 0, 0);
        let result = resolve_port_pair("fixture", Some(10809), Some(expected), true, |port| {
            Ok(port != expected)
        })
        .unwrap();
        assert_eq!(result.proxy_port, fallback);
        assert!(result.runtime_port_conflict);
        assert_eq!(result.cc_port, DEFAULT_CC_REMOTE_PORT);
        assert_eq!(
            runtime_proxy_match(result.runtime_expected_proxy_port, Some(result.proxy_port)),
            RuntimeProxyMatch::Mismatch
        );
        let cc_conflict = resolve_port_pair("fixture", Some(10809), Some(expected), true, |port| {
            Ok(port != DEFAULT_CC_REMOTE_PORT)
        })
        .unwrap();
        assert_eq!(cc_conflict.proxy_port, expected);
        assert_eq!(
            runtime_proxy_match(
                cc_conflict.runtime_expected_proxy_port,
                Some(cc_conflict.proxy_port)
            ),
            RuntimeProxyMatch::Matched
        );
        assert_eq!(cc_conflict.cc_port, derived_port("fixture", 0, 1));
        assert!(!cc_conflict.runtime_port_conflict);
        let shared_preference = resolve_port_pair(
            "fixture",
            Some(10809),
            Some(DEFAULT_CC_REMOTE_PORT),
            true,
            |_| Ok(true),
        )
        .unwrap();
        assert_eq!(shared_preference.proxy_port, DEFAULT_CC_REMOTE_PORT);
        assert_ne!(shared_preference.cc_port, shared_preference.proxy_port);
        let no_runtime =
            resolve_port_pair("fixture", Some(10809), None, true, |port| Ok(port != 10809))
                .unwrap();
        assert_eq!(no_runtime.proxy_port, fallback);
        assert_eq!(no_runtime.cc_port, DEFAULT_CC_REMOTE_PORT);
        assert_eq!(
            runtime_proxy_match(None, Some(no_runtime.proxy_port)),
            RuntimeProxyMatch::Unknown
        );
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
