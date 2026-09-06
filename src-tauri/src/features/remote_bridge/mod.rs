mod ssh;
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
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub alias: String,
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
    pub alias: Option<String>,
    pub proxy: Option<Endpoint>,
    pub cc: Option<Endpoint>,
    pub active_proxy_revision: Option<u64>,
    pub environment: String,
    pub codex_configured: bool,
    pub claude_configured: bool,
    pub error: Option<String>,
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
}
struct Pending {
    preview: ConfigPreview,
    hash: String,
    alias: String,
    port: u16,
    backup_hash: Option<String>,
    target_fingerprint: String,
}
#[derive(Default)]
struct Store {
    summary: Summary,
    child: Option<ssh::OwnedChild>,
    pending: Option<Pending>,
    cc_detected: bool,
    reachable: bool,
    target_fingerprint: Option<String>,
    proxy_status: Status,
    cc_status: Status,
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
        if !matches!(child.child.try_wait(), Ok(None)) {
            state.child = None;
            state.summary.status = Status::Disconnected;
            state.proxy_status = Status::Disconnected;
            state.cc_status = Status::Disconnected;
            state.reachable = false;
            state.pending = None;
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
        let endpoints_available = proxy_available && cc_available;
        state.summary.status =
            observed_status(&state.summary, current.as_ref(), endpoints_available);
    }
}
pub fn start_monitor() {
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(2));
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
        configured: s.alias.is_some(),
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
    }
}
pub fn targets() -> BridgeResult<Vec<String>> {
    ssh::aliases()
}
pub fn check(alias: String) -> BridgeResult<()> {
    ssh::validate_target(&alias)?;
    ssh::remote(&alias, json!({"operation":"check","ports":[]}))?;
    let mut state = lock()?;
    state.reachable = true;
    Ok(())
}
pub fn detect_cc(local_port: u16) -> BridgeResult<bool> {
    port(local_port)?;
    let available = listening(&ProxyEndpoint {
        host: "127.0.0.1".into(),
        port: local_port,
        protocol: ProxyProtocol::Http,
    });
    lock()?.cc_detected = available;
    Ok(available)
}
pub fn preview(request: &Request) -> BridgeResult<Summary> {
    if ssh::target_parts(&request.alias).is_err() {
        return Err("invalidTarget".into());
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
        alias: Some(request.alias.clone()),
        proxy,
        cc,
        environment,
        active_proxy_revision: request.proxy_port.map(|_| context.revision),
        ..Summary::default()
    })
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
        let fingerprint = ssh::fingerprint(&request.alias)?;
        let mut next = preview(&request)?;
        state.summary.status = Status::Connecting;
        let endpoints: Vec<_> = next
            .proxy
            .iter()
            .chain(next.cc.iter())
            .map(|e| (e.remote_port, e.local.host.clone(), e.local.port))
            .collect();
        let ports: Vec<_> = endpoints.iter().map(|e| e.0).collect();
        ssh::remote(&request.alias, json!({"operation":"check","ports":ports}))?;
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
            match ssh::remote(&request.alias, json!({"operation":"verify","ports":ports})) {
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
        if ssh::fingerprint(&request.alias)? != fingerprint {
            return Err("sshConfigChanged".into());
        }
        // Recheck captured revision after the asynchronous SSH handshake.
        if request.proxy_port.is_some() {
            preview(&request)?;
        }
        next.status = Status::Connected;
        state.summary = next;
        state.proxy_status = Status::Connected;
        state.cc_status = Status::Connected;
        state.child = Some(child);
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
    state.summary.status = Status::Disconnected;
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
}
pub fn test() -> BridgeResult<()> {
    let mut state = lock()?;
    refresh(&mut state);
    if state.summary.status != Status::Connected {
        return Err("bridgeUnavailable".into());
    }
    let endpoint = state.summary.proxy.as_ref().ok_or("proxyUnavailable")?;
    if Some(ssh::fingerprint(
        state.summary.alias.as_deref().ok_or("invalidTarget")?,
    )?) != state.target_fingerprint
    {
        return Err("sshConfigChanged".into());
    }
    ssh::remote(
        state.summary.alias.as_deref().ok_or("invalidTarget")?,
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
    let alias = state.summary.alias.clone().ok_or("invalidTarget")?;
    let target_fingerprint = ssh::fingerprint(&alias)?;
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
        &alias,
        json!({"operation":"preview","tool":tool,"port":port}),
    )?;
    let hash = value["expectedHash"]
        .as_str()
        .ok_or("remoteFailed")?
        .to_string();
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
        alias: alias.clone(),
        restore: false,
    };
    state.pending = Some(Pending {
        preview: preview.clone(),
        hash,
        alias,
        port,
        backup_hash: None,
        target_fingerprint,
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
    if ssh::fingerprint(&pending.alias)? != pending.target_fingerprint {
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
        &pending.alias,
        json!({"operation":"apply","tool":pending.preview.tool,"port":pending.port,"expectedHash":pending.hash}),
    )?;
    if pending.preview.tool == "codex" {
        state.summary.codex_configured = true;
    } else {
        state.summary.claude_configured = true;
    }
    Ok(())
}
pub fn config_restore_preview(alias: String, tool: String) -> BridgeResult<ConfigPreview> {
    if !["codex", "claude"].contains(&tool.as_str()) {
        return Err("invalidRequest".into());
    }
    let target_fingerprint = ssh::fingerprint(&alias)?;
    let mut state = lock()?;
    let value = ssh::remote(&alias, json!({"operation":"restore-preview","tool":tool}))?;
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
        alias: alias.clone(),
        tool: tool.clone(),
        restore: true,
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
        alias,
        port: 25721,
        hash: value["expectedHash"].as_str().ok_or("remoteFailed")?.into(),
        backup_hash: Some(value["backupHash"].as_str().ok_or("remoteFailed")?.into()),
        target_fingerprint,
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
    if ssh::fingerprint(&pending.alias)? != pending.target_fingerprint {
        return Err("sshConfigChanged".into());
    }
    ssh::remote(
        &pending.alias,
        json!({"operation":"restore","tool":pending.preview.tool,"expectedHash":pending.hash,"backupHash":pending.backup_hash}),
    )?;
    if state.summary.alias.as_ref() == Some(&pending.alias) {
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
        let value = json!({"alias":"dev-vm","proxyPort":17897,"ccPort":null,"ccLocalPort":15721,"expectedRevision":8,"localHost":"0.0.0.0"});
        assert!(serde_json::from_value::<Request>(value).is_err());
        for invalid in [-1, 65536] {
            let value = json!({"alias":"dev-vm","proxyPort":invalid,"ccPort":null,"ccLocalPort":15721,"expectedRevision":8});
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
