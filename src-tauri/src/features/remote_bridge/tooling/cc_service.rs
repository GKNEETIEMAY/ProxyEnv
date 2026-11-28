use super::{lock, port, BridgeResult};
use crate::features::proxy::{
    listeners::{self, TcpListener},
    processes::{self, RunningProcess},
};
use serde::Serialize;
use std::{collections::HashMap, io::Read, time::Duration};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CcDetectionState {
    Confirmed,
    ListeningUnknown,
    NotDetected,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CcServiceState {
    NotRunning,
    NotListening,
    Uncertain,
    Recognized,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcDetection {
    pub state: CcDetectionState,
    pub local_port: u16,
    pub service: CcServiceState,
    pub revision: u64,
}

impl CcDetection {
    fn at_requested_port(self, port: u16) -> Self {
        if self.local_port == port {
            return self;
        }
        // A newer discovery may publish a different endpoint while this exact
        // check was running. Its health is evidence only for its own port.
        Self {
            state: CcDetectionState::ListeningUnknown,
            local_port: port,
            service: CcServiceState::Uncertain,
            revision: self.revision,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Observation {
    service: CcServiceState,
    local_port: u16,
    owner: Option<u32>,
    observed_listener: bool,
}

#[derive(Default)]
pub(super) struct CcServiceTracker {
    next_ticket: u64,
    published_ticket: u64,
    revision: u64,
    last: Option<Observation>,
}

impl CcServiceTracker {
    fn begin(&mut self) -> u64 {
        self.next_ticket += 1;
        self.next_ticket
    }

    fn publish(&mut self, ticket: u64, observed: Observation) -> CcDetection {
        if ticket > self.published_ticket {
            if self.last != Some(observed) {
                self.revision += 1;
            }
            self.published_ticket = ticket;
            self.last = Some(observed);
        }
        let current = self.last.unwrap_or(observed);
        let state = match (current.service, current.observed_listener) {
            (CcServiceState::Recognized, _) => CcDetectionState::Confirmed,
            (_, true) => CcDetectionState::ListeningUnknown,
            _ => CcDetectionState::NotDetected,
        };
        CcDetection {
            state,
            local_port: current.local_port,
            service: current.service,
            revision: self.revision,
        }
    }
}

pub fn detect_cc(local_port: u16) -> BridgeResult<CcDetection> {
    check(local_port, false)
}

// Setup may discover a new port. Established tunnels keep the bound endpoint.
pub fn discover_cc(preferred_port: u16) -> BridgeResult<CcDetection> {
    check(preferred_port, true)
}

fn check(preferred_port: u16, discover: bool) -> BridgeResult<CcDetection> {
    port(preferred_port)?;
    let ticket = lock()?.cc_service.begin();
    let observed = observe_host(preferred_port, discover).unwrap_or(Observation {
        service: CcServiceState::Unavailable,
        local_port: preferred_port,
        owner: None,
        observed_listener: false,
    });
    let mut state = lock()?;
    let result = state.cc_service.publish(ticket, observed);
    state.cc_detected = result.state == CcDetectionState::Confirmed;
    Ok(if discover {
        result
    } else {
        result.at_requested_port(preferred_port)
    })
}

pub(super) fn endpoint_healthy(local_port: u16) -> BridgeResult<bool> {
    Ok(observe_host(local_port, false)?.service == CcServiceState::Recognized)
}

fn observe_host(preferred_port: u16, discover: bool) -> BridgeResult<Observation> {
    let listeners = listeners::enumerate().map_err(|_| "stateUnavailable")?;
    let processes = processes::enumerate();
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_millis(350))
        .build()
        .map_err(|_| "stateUnavailable")?;
    Ok(observe(
        &listeners,
        &processes,
        preferred_port,
        discover,
        |port| cc_route_healthy(&client, port),
    ))
}

fn is_cc_switch(process: &RunningProcess) -> bool {
    process
        .name
        .trim_end_matches(".exe")
        .replace(['-', '_', ' '], "")
        == "ccswitch"
}

fn owns_loopback(listener: &TcpListener, processes: &HashMap<u32, RunningProcess>) -> Option<u32> {
    if listener.host != "127.0.0.1" || listener.port < 1024 {
        return None;
    }
    let [pid] = *listener.pids.as_slice() else {
        return None;
    };
    processes.get(&pid).is_some_and(is_cc_switch).then_some(pid)
}

pub(super) fn cc_route_ports(
    listeners: &[TcpListener],
    processes: &HashMap<u32, RunningProcess>,
    preferred_port: u16,
) -> Vec<u16> {
    let mut ports: Vec<_> = listeners
        .iter()
        .filter(|listener| owns_loopback(listener, processes).is_some())
        .map(|listener| listener.port)
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports.sort_by_key(|candidate| (*candidate != preferred_port, *candidate));
    ports
}

fn observe(
    listeners: &[TcpListener],
    processes: &HashMap<u32, RunningProcess>,
    preferred_port: u16,
    discover: bool,
    mut healthy: impl FnMut(u16) -> bool,
) -> Observation {
    let ports = cc_route_ports(listeners, processes, preferred_port);
    let candidates = ports
        .into_iter()
        .filter(|port| discover || *port == preferred_port);
    let mut observed_owned_listener = false;
    for port in candidates.take(8) {
        observed_owned_listener = true;
        if healthy(port) {
            return Observation {
                service: CcServiceState::Recognized,
                local_port: port,
                owner: listeners
                    .iter()
                    .filter(|listener| listener.port == port)
                    .find_map(|listener| owns_loopback(listener, processes)),
                observed_listener: true,
            };
        }
    }
    let observed_listener = observed_owned_listener
        || listeners
            .iter()
            .any(|listener| listener.port == preferred_port);
    let service = if observed_owned_listener {
        CcServiceState::Unavailable
    } else if observed_listener {
        CcServiceState::Uncertain
    } else if processes.values().any(is_cc_switch) {
        CcServiceState::NotListening
    } else {
        CcServiceState::NotRunning
    };
    Observation {
        service,
        local_port: preferred_port,
        owner: None,
        observed_listener,
    }
}

pub(super) fn cc_route_healthy(client: &reqwest::blocking::Client, local_port: u16) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cc_capability_distinguishes_owned_health_and_versions_the_current_observation() {
        let cc_process = |pid| RunningProcess {
            pid,
            name: "cc-switch.exe".into(),
        };
        let processes = HashMap::from([(1, cc_process(1)), (2, cc_process(2))]);
        let listener = |port, pid| TcpListener {
            host: "127.0.0.1".into(),
            port,
            pids: vec![pid],
        };
        let checked =
            |listeners: &[TcpListener], up| observe(listeners, &processes, 15721, true, |_| up);
        let mut tracker = CcServiceTracker::default();
        let absent = tracker.begin();
        assert_eq!(
            tracker
                .publish(
                    absent,
                    observe(&[], &HashMap::new(), 15721, true, |_| false)
                )
                .service,
            CcServiceState::NotRunning
        );
        assert_eq!(checked(&[], true).service, CcServiceState::NotListening);
        assert_eq!(
            checked(&[listener(15721, 42)], true).service,
            CcServiceState::Uncertain
        );
        let latest = tracker.begin();
        let ready = tracker.publish(latest, checked(&[listener(15721, 1)], true));
        assert_eq!(ready.state, CcDetectionState::Confirmed);
        let unchanged = tracker.begin();
        assert_eq!(
            tracker
                .publish(unchanged, checked(&[listener(15721, 1)], true))
                .revision,
            ready.revision
        );
        let stale = tracker.begin();
        let replaced = tracker.begin();
        let changed = tracker.publish(replaced, checked(&[listener(15721, 2)], true));
        assert!(changed.revision > ready.revision);
        assert_eq!(
            tracker
                .publish(stale, checked(&[listener(15721, 1)], true))
                .revision,
            changed.revision
        );
        let moved = tracker.begin();
        let result = tracker.publish(moved, checked(&[listener(15723, 2)], true));
        assert_eq!(result.local_port, 15723);
        assert!(result.revision > changed.revision);
        let exact = result.at_requested_port(15721);
        assert_eq!(exact.local_port, 15721);
        assert_eq!(exact.state, CcDetectionState::ListeningUnknown);
        assert_eq!(exact.service, CcServiceState::Uncertain);
        let failed = tracker.begin();
        let result = tracker.publish(failed, checked(&[listener(15723, 2)], false));
        assert_eq!(result.service, CcServiceState::Unavailable);
        assert_eq!(result.state, CcDetectionState::ListeningUnknown);
    }
}
