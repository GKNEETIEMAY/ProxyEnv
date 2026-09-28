//! Restore transport only: keep session tokens, ports and client configuration.
use super::*;
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    Idle,
    Waiting,
    Retrying,
    AttentionRequired,
}

pub(super) struct Retry {
    due: Instant,
    attempts: u32,
    paused: bool,
}
impl Retry {
    pub(super) fn new() -> Self {
        Self {
            due: Instant::now(),
            attempts: 0,
            paused: false,
        }
    }
    fn failed(&mut self, code: &str) {
        self.attempts = self.attempts.saturating_add(1);
        self.paused = !transient(code);
        self.due = Instant::now() + Duration::from_secs(delay(self.attempts));
    }
}
fn delay(attempts: u32) -> u64 {
    (2_u64 << attempts.min(5)).min(60)
}
fn transient(code: &str) -> bool {
    matches!(
        code,
        "sshFailed"
            | "sshTimeout"
            | "forwardDenied"
            | "portInUse"
            | "processFailed"
            | "remoteFailed"
    )
}

struct Job {
    generation: u64,
    request: Request,
    fingerprint: String,
    endpoints: Vec<(u16, String, u16)>,
}

// Called by the existing background monitor even when the window is hidden.
pub(super) fn tick() {
    if STOPPING.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let job = {
        let Ok(mut state) = store().try_lock() else {
            return;
        };
        refresh(&mut state);
        if state.child.is_some()
            || state
                .reconnect
                .as_ref()
                .is_none_or(|r| r.paused || Instant::now() < r.due)
        {
            return;
        }
        let Some(request) = state.last_request.clone() else {
            return;
        };
        let Some(fingerprint) = state.target_fingerprint.clone() else {
            return;
        };
        let Ok(endpoints) = forwarding_endpoints(
            &state.summary,
            state.proxy_relay.as_ref(),
            state.ai_relay.as_ref(),
        ) else {
            return;
        };
        state.summary.reconnect_state = State::Retrying;
        Job {
            generation: state.connection_generation,
            request,
            fingerprint,
            endpoints,
        }
    };
    // No Store lock during network I/O: Disconnect can cancel this attempt.
    let result = restore_transport(&job);
    let Ok(mut state) = lock() else {
        return;
    };
    if STOPPING.load(std::sync::atomic::Ordering::SeqCst) || !current(&state, job.generation) {
        return;
    }
    match result {
        Ok(child) => {
            state.child = Some(Box::new(child));
            state.reconnect = None;
            state.summary.reconnect_state = State::Idle;
            state.summary.error = None;
            state.reachable = true;
            state.ssh_auth.authenticated = true;
            state.summary.ssh_auth = state.ssh_auth;
            refresh(&mut state);
        }
        Err(code) => {
            let retry = state.reconnect.as_mut().unwrap();
            retry.failed(&code);
            let paused = retry.paused;
            state.summary.reconnect_state = if paused {
                State::AttentionRequired
            } else {
                State::Waiting
            };
            state.summary.status = if paused {
                Status::Error
            } else {
                Status::Connecting
            };
            state.summary.error = Some(code.clone());
            if paused {
                if code == "sshAuth" {
                    credential_cache::clear_if_matches(&job.fingerprint);
                }
                drop(state);
                crate::desktop::notifications::bridge_attention();
            }
        }
    }
}

fn current(state: &Store, generation: u64) -> bool {
    state.connection_generation == generation && state.reconnect.is_some() && state.child.is_none()
}

fn restore_transport(job: &Job) -> BridgeResult<ssh::OwnedChild> {
    if ssh::fingerprint(&job.request.target_id)? != job.fingerprint {
        return Err("sshConfigChanged".into());
    }
    // Never choose another active proxy, target or port during recovery.
    if job.request.proxy_port.is_some() {
        let active = active::snapshot().map_err(|_| "activeChanged")?;
        if active.revision != job.request.expected_revision {
            return Err("activeChanged".into());
        }
    }
    // Auth probe returns classified auth/host-key errors without prompts. It
    // does not require free ports: the old server-side sshd may still be exiting.
    ssh::reconnect_probe(&job.request.target_id)?;
    {
        let state = lock()?;
        if !current(&state, job.generation) {
            return Err("cancelled".into());
        }
    }
    let mut child = ssh::tunnel(&job.request, &job.endpoints)?;
    let ports: Vec<_> = job.endpoints.iter().map(|entry| entry.0).collect();
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
            &job.request.target_id,
            json!({"operation":"verify", "ports":ports}),
        ) {
            Ok(_) => {
                if ssh::fingerprint(&job.request.target_id)? != job.fingerprint {
                    return Err("sshConfigChanged".into());
                }
                return Ok(child);
            }
            Err(code) if !transient(&code) => return Err(code),
            Err(_) => {}
        }
    }
    Err("forwardDenied".into())
}

pub(super) fn cancel(state: &mut Store) {
    state.connection_generation = state.connection_generation.wrapping_add(1);
    state.last_request = None;
    state.reconnect = None;
    state.summary.reconnect_state = State::Idle;
    if state.child.is_none() {
        state.proxy_relay = None;
        state.ai_relay = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct ExitedSsh;
    impl ssh::ManagedSsh for ExitedSsh {
        fn is_running(&mut self) -> BridgeResult<bool> {
            Ok(false)
        }
    }
    #[test]
    fn transport_loss_preserves_live_relay_and_session_until_explicit_cancel() {
        let relay = authenticated_relay::AuthenticatedRelay::start(
            ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: 9,
                protocol: ProxyProtocol::Http,
            },
            authenticated_relay::RelayMode::AiHttp,
        )
        .unwrap();
        let original_port = relay.port();
        let original_token = relay.token();
        let request = Request {
            target_id: "test".into(),
            proxy_port: None,
            cc_port: Some(15721),
            cc_local_port: 9,
            expected_revision: 0,
        };
        let mut state = Store {
            child: Some(Box::new(ExitedSsh)),
            last_request: Some(request),
            ai_relay: Some(relay),
            ..Store::default()
        };
        refresh(&mut state);
        assert!(state.child.is_none());
        assert_eq!(state.summary.status, Status::Connecting);
        assert_eq!(state.summary.reconnect_state, State::Waiting);
        let retained = state.ai_relay.as_ref().unwrap();
        assert!(retained.is_running());
        assert_eq!(retained.port(), original_port);
        assert!(*retained.token() == *original_token);
        cancel(&mut state);
        assert!(state.ai_relay.is_none());
        assert!(state.last_request.is_none());
        assert!(state.reconnect.is_none());
        refresh(&mut state);
        assert!(state.reconnect.is_none());
    }
    #[test]
    fn unestablished_transport_does_not_begin_automatic_reconnect() {
        let mut state = Store {
            child: Some(Box::new(ExitedSsh)),
            ..Store::default()
        };
        refresh(&mut state);
        assert_eq!(state.summary.status, Status::Disconnected);
        assert!(state.reconnect.is_none());
    }
    #[test]
    fn backoff_is_bounded() {
        assert_eq!([1, 2, 3, 4, 5, 99].map(delay), [4, 8, 16, 32, 60, 60]);
    }
    #[test]
    fn only_transport_errors_retry() {
        for code in ["sshFailed", "sshTimeout", "forwardDenied", "portInUse"] {
            assert!(transient(code));
        }
        for code in [
            "sshAuth",
            "hostKey",
            "sshConfigChanged",
            "activeChanged",
            "unsafeBinding",
        ] {
            assert!(!transient(code));
        }
    }
    #[test]
    fn explicit_cancel_invalidates_in_flight_job() {
        let mut state = Store {
            reconnect: Some(Retry::new()),
            ..Store::default()
        };
        assert!(current(&state, 0));
        cancel(&mut state);
        assert!(!current(&state, 0));
        state.reconnect = Some(Retry::new());
        assert!(!current(&state, 0));
    }
    #[test]
    fn authentication_failure_pauses_instead_of_repeating_passwords() {
        let mut retry = Retry::new();
        retry.failed("sshAuth");
        assert!(retry.paused);
    }
}
