use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use chrono::{SecondsFormat, Utc};
use futures_util::future::join_all;
use reqwest::{redirect::Policy, Client, StatusCode};
use serde::Serialize;

use crate::error::{ProxyEnvError, Result};

use super::{plan, probe, protocol_matches, ProxyCandidate, ProxyEndpoint, ProxyProtocol};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const CACHE_TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum ProxyConnectivityState {
    NotTested,
    Testing,
    Reachable,
    Partial,
    Unreachable,
    LocalProxyUnavailable,
    Unknown,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectivityErrorKind {
    ProxyUnavailable,
    ProxyHandshakeFailed,
    ConnectTimeout,
    TlsFailed,
    RemoteRejected,
    HttpStatus,
    NetworkError,
    Unknown,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectivityTargetResult {
    pub name: String,
    pub url: String,
    pub success: bool,
    pub status_code: Option<u16>,
    pub duration_ms: Option<u64>,
    pub error_kind: Option<ConnectivityErrorKind>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConnectivityResult {
    pub state: ProxyConnectivityState,
    pub endpoint: ProxyEndpoint,
    pub local_listener_ok: bool,
    pub protocol_ok: bool,
    pub targets: Vec<ConnectivityTargetResult>,
    pub tested_at: String,
    pub total_duration_ms: u64,
}

#[derive(Clone, Copy)]
enum RequestMethod {
    Get,
    Head,
}

#[derive(Clone, Copy)]
struct ConnectivityTarget {
    name: &'static str,
    url: &'static str,
    method: RequestMethod,
}

const TARGETS: &[ConnectivityTarget] = &[
    ConnectivityTarget {
        name: "Google",
        url: "https://www.google.com/generate_204",
        method: RequestMethod::Get,
    },
    ConnectivityTarget {
        name: "GitHub",
        url: "https://github.com/",
        method: RequestMethod::Head,
    },
    ConnectivityTarget {
        name: "Cloudflare",
        url: "https://www.cloudflare.com/",
        method: RequestMethod::Head,
    },
];

struct CachedConnectivityResult {
    result: ProxyConnectivityResult,
    candidate_fingerprint: String,
    stored_at: Instant,
}

static LAST_RESULT: OnceLock<Mutex<Option<CachedConnectivityResult>>> = OnceLock::new();

pub async fn test_current_proxy(candidate: &ProxyCandidate) -> Result<ProxyConnectivityResult> {
    let started = Instant::now();
    let endpoint = plan::validate_and_normalize_endpoint(&endpoint_from_candidate(candidate))?;
    let tested_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let probe_endpoint = endpoint.clone();
    let (local_listener_ok, detected_protocol) = tauri::async_runtime::spawn_blocking(move || {
        let listening = probe::listening(&probe_endpoint.host, probe_endpoint.port);
        let protocol = if listening {
            probe::protocol(&probe_endpoint.host, probe_endpoint.port)
        } else {
            ProxyProtocol::Unknown
        };
        (listening, protocol)
    })
    .await
    .map_err(|error| ProxyEnvError::Detection(error.to_string()))?;

    if !local_listener_ok {
        return Ok(store_result(
            candidate,
            ProxyConnectivityResult {
                state: ProxyConnectivityState::LocalProxyUnavailable,
                endpoint,
                local_listener_ok: false,
                protocol_ok: false,
                targets: Vec::new(),
                tested_at,
                total_duration_ms: elapsed_ms(started),
            },
        ));
    }

    if matches!(endpoint.protocol, ProxyProtocol::Unknown) {
        return Ok(store_result(
            candidate,
            ProxyConnectivityResult {
                state: ProxyConnectivityState::Unknown,
                endpoint,
                local_listener_ok: true,
                protocol_ok: false,
                targets: Vec::new(),
                tested_at,
                total_duration_ms: elapsed_ms(started),
            },
        ));
    }

    let client = build_client(&endpoint)?;
    let targets = join_all(TARGETS.iter().map(|target| test_target(&client, *target))).await;
    let success_count = targets.iter().filter(|target| target.success).count();
    let state = classify_state(success_count, targets.len());
    let protocol_ok = protocol_matches(endpoint.protocol, detected_protocol) || success_count > 0;

    Ok(store_result(
        candidate,
        ProxyConnectivityResult {
            state,
            endpoint,
            local_listener_ok: true,
            protocol_ok,
            targets,
            tested_at,
            total_duration_ms: elapsed_ms(started),
        },
    ))
}

pub fn get_last_result(
    active_candidate: Option<&ProxyCandidate>,
) -> Option<ProxyConnectivityResult> {
    let cache = LAST_RESULT.get_or_init(|| Mutex::new(None));
    let mut cached = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(candidate) = active_candidate else {
        *cached = None;
        return None;
    };
    let valid = cached
        .as_ref()
        .is_some_and(|entry| cache_is_valid(entry, candidate, Instant::now()));
    if !valid {
        *cached = None;
        return None;
    }
    cached.as_ref().map(|entry| entry.result.clone())
}

fn build_client(endpoint: &ProxyEndpoint) -> Result<Client> {
    let proxy_url = proxy_url(endpoint)?;
    let proxy = reqwest::Proxy::all(&proxy_url)
        .map_err(|error| ProxyEnvError::InvalidProxyEndpoint(error.to_string()))?;
    Client::builder()
        .no_proxy()
        .proxy(proxy)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(Policy::limited(3))
        .build()
        .map_err(|error| ProxyEnvError::Detection(error.to_string()))
}

fn proxy_url(endpoint: &ProxyEndpoint) -> Result<String> {
    let endpoint = plan::validate_and_normalize_endpoint(endpoint)?;
    let scheme = match endpoint.protocol {
        ProxyProtocol::Http | ProxyProtocol::Mixed => "http",
        ProxyProtocol::Socks5 => "socks5h",
        ProxyProtocol::Unknown => {
            return Err(ProxyEnvError::InvalidProxyEndpoint(
                "proxy protocol is unknown".into(),
            ));
        }
    };
    let host = format_host(&endpoint.host);
    Ok(format!("{scheme}://{host}:{}", endpoint.port))
}

async fn test_target(client: &Client, target: ConnectivityTarget) -> ConnectivityTargetResult {
    let started = Instant::now();
    match target_request(client, target).send().await {
        Ok(response) => target_from_status(target, response.status(), elapsed_ms(started)),
        Err(error) => ConnectivityTargetResult {
            name: target.name.into(),
            url: target.url.into(),
            success: false,
            status_code: error.status().map(|status| status.as_u16()),
            duration_ms: Some(elapsed_ms(started)),
            error_kind: Some(classify_request_error(&error)),
        },
    }
}

fn target_request(client: &Client, target: ConnectivityTarget) -> reqwest::RequestBuilder {
    let request = match target.method {
        RequestMethod::Get => client.get(target.url),
        RequestMethod::Head => client.head(target.url),
    };
    request.header(reqwest::header::USER_AGENT, connectivity_user_agent())
}

fn connectivity_user_agent() -> String {
    format!("ProxyEnv/{} ConnectivityCheck", env!("CARGO_PKG_VERSION"))
}

fn target_from_status(
    target: ConnectivityTarget,
    status: StatusCode,
    duration_ms: u64,
) -> ConnectivityTargetResult {
    let success = status.is_success() || status.is_redirection();
    ConnectivityTargetResult {
        name: target.name.into(),
        url: target.url.into(),
        success,
        status_code: Some(status.as_u16()),
        duration_ms: Some(duration_ms),
        error_kind: (!success).then_some(if status == StatusCode::PROXY_AUTHENTICATION_REQUIRED {
            ConnectivityErrorKind::ProxyHandshakeFailed
        } else {
            ConnectivityErrorKind::HttpStatus
        }),
    }
}

fn classify_request_error(error: &reqwest::Error) -> ConnectivityErrorKind {
    if error.is_timeout() {
        return ConnectivityErrorKind::ConnectTimeout;
    }
    classify_error_detail(error.is_connect(), error.is_status(), &error.to_string())
}

fn classify_error_detail(is_connect: bool, is_status: bool, detail: &str) -> ConnectivityErrorKind {
    let detail = detail.to_ascii_lowercase();
    if detail.contains("certificate") || detail.contains("tls") {
        ConnectivityErrorKind::TlsFailed
    } else if detail.contains("proxy")
        && (detail.contains("handshake")
            || detail.contains("tunnel")
            || detail.contains("authentication"))
    {
        ConnectivityErrorKind::ProxyHandshakeFailed
    } else if detail.contains("connection refused") {
        ConnectivityErrorKind::ProxyUnavailable
    } else if detail.contains("connection reset") || detail.contains("connection closed") {
        ConnectivityErrorKind::RemoteRejected
    } else if is_status {
        ConnectivityErrorKind::HttpStatus
    } else if is_connect {
        ConnectivityErrorKind::NetworkError
    } else if detail.is_empty() {
        ConnectivityErrorKind::Unknown
    } else {
        ConnectivityErrorKind::NetworkError
    }
}

fn classify_state(success_count: usize, target_count: usize) -> ProxyConnectivityState {
    match success_count {
        count if count >= 2 => ProxyConnectivityState::Reachable,
        1 => ProxyConnectivityState::Partial,
        0 if target_count > 0 => ProxyConnectivityState::Unreachable,
        _ => ProxyConnectivityState::Unknown,
    }
}

fn store_result(
    candidate: &ProxyCandidate,
    result: ProxyConnectivityResult,
) -> ProxyConnectivityResult {
    let cache = LAST_RESULT.get_or_init(|| Mutex::new(None));
    let mut cached = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *cached = Some(CachedConnectivityResult {
        result: result.clone(),
        candidate_fingerprint: candidate_fingerprint(candidate),
        stored_at: Instant::now(),
    });
    result
}

fn cache_is_valid(
    cached: &CachedConnectivityResult,
    candidate: &ProxyCandidate,
    now: Instant,
) -> bool {
    now.saturating_duration_since(cached.stored_at) <= CACHE_TTL
        && plan::validate_and_normalize_endpoint(&endpoint_from_candidate(candidate))
            .is_ok_and(|endpoint| cached.result.endpoint == endpoint)
        && cached.candidate_fingerprint == candidate_fingerprint(candidate)
        && candidate.listening
}

fn endpoint_from_candidate(candidate: &ProxyCandidate) -> ProxyEndpoint {
    ProxyEndpoint {
        host: candidate.host.trim().to_owned(),
        port: candidate.port,
        protocol: candidate.protocol,
    }
}

fn candidate_fingerprint(candidate: &ProxyCandidate) -> String {
    format!(
        "{}|{}|{}|{}",
        candidate.id,
        candidate
            .pid
            .map_or_else(String::new, |pid| pid.to_string()),
        candidate.process_name.as_deref().unwrap_or_default(),
        candidate.client_name.as_deref().unwrap_or_default()
    )
}

fn format_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::super::models::{Confidence, DetectionSource};
    use super::*;

    fn candidate() -> ProxyCandidate {
        ProxyCandidate {
            id: "127.0.0.1:7897".into(),
            client_name: Some("Test Proxy".into()),
            icon_key: Some("generic-proxy".into()),
            process_name: Some("test-proxy.exe".into()),
            pid: Some(42),
            host: "127.0.0.1".into(),
            port: 7897,
            protocol: ProxyProtocol::Mixed,
            source: vec![DetectionSource::ProcessListener],
            confidence: Confidence::High,
            listening: true,
        }
    }

    fn cached(candidate: &ProxyCandidate, stored_at: Instant) -> CachedConnectivityResult {
        CachedConnectivityResult {
            result: ProxyConnectivityResult {
                state: ProxyConnectivityState::Reachable,
                endpoint: endpoint_from_candidate(candidate),
                local_listener_ok: true,
                protocol_ok: true,
                targets: Vec::new(),
                tested_at: "2026-08-23T00:00:00.000Z".into(),
                total_duration_ms: 10,
            },
            candidate_fingerprint: candidate_fingerprint(candidate),
            stored_at,
        }
    }

    #[test]
    fn classifies_multi_target_results_without_single_site_false_negatives() {
        assert_eq!(classify_state(3, 3), ProxyConnectivityState::Reachable);
        assert_eq!(classify_state(2, 3), ProxyConnectivityState::Reachable);
        assert_eq!(classify_state(1, 3), ProxyConnectivityState::Partial);
        assert_eq!(classify_state(0, 3), ProxyConnectivityState::Unreachable);
    }

    #[test]
    fn builds_explicit_http_and_socks_proxy_urls() {
        let mut endpoint = endpoint_from_candidate(&candidate());
        assert_eq!(proxy_url(&endpoint).unwrap(), "http://127.0.0.1:7897");
        endpoint.protocol = ProxyProtocol::Socks5;
        endpoint.host = "::1".into();
        assert_eq!(proxy_url(&endpoint).unwrap(), "socks5h://[::1]:7897");
        endpoint.host = "192.168.1.10".into();
        assert!(proxy_url(&endpoint).is_err());
        endpoint.host = "::1".into();
        endpoint.protocol = ProxyProtocol::Unknown;
        assert!(proxy_url(&endpoint).is_err());
    }

    #[test]
    fn diagnostic_requests_do_not_include_credentials_or_cookies() {
        let endpoint = endpoint_from_candidate(&candidate());
        let client = build_client(&endpoint).unwrap();
        let request = target_request(&client, TARGETS[0]).build().unwrap();

        assert!(request.headers().get(reqwest::header::COOKIE).is_none());
        assert!(request
            .headers()
            .get(reqwest::header::AUTHORIZATION)
            .is_none());
        assert_eq!(
            request
                .headers()
                .get(reqwest::header::USER_AGENT)
                .unwrap()
                .to_str()
                .unwrap(),
            connectivity_user_agent()
        );
    }

    #[test]
    fn classifies_actionable_error_kinds() {
        assert_eq!(
            classify_error_detail(true, false, "proxy tunnel handshake failed"),
            ConnectivityErrorKind::ProxyHandshakeFailed
        );
        assert_eq!(
            classify_error_detail(true, false, "certificate verify failed"),
            ConnectivityErrorKind::TlsFailed
        );
        assert_eq!(
            classify_error_detail(true, false, "connection refused"),
            ConnectivityErrorKind::ProxyUnavailable
        );
        assert_eq!(
            classify_error_detail(true, false, "connection reset by peer"),
            ConnectivityErrorKind::RemoteRejected
        );
    }

    #[test]
    fn cache_expires_or_invalidates_when_candidate_changes() {
        let active = candidate();
        let now = Instant::now();
        let fresh = cached(&active, now - Duration::from_secs(60));
        assert!(cache_is_valid(&fresh, &active, now));

        let stale = cached(&active, now - Duration::from_secs(301));
        assert!(!cache_is_valid(&stale, &active, now));

        let mut changed_endpoint = active.clone();
        changed_endpoint.port = 7898;
        assert!(!cache_is_valid(&fresh, &changed_endpoint, now));

        let mut changed_process = active.clone();
        changed_process.pid = Some(43);
        assert!(!cache_is_valid(&fresh, &changed_process, now));

        let mut stopped = active;
        stopped.listening = false;
        assert!(!cache_is_valid(&fresh, &stopped, now));
    }
}
