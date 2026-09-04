//! The only owner of proxy selection. Discovery may change observations, never a selection.
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use crate::error::{ProxyEnvError, Result};

use super::{
    models::DetectionSource, ProxyCandidate, ProxyEndpoint, ProxyEnvironmentService,
    ProxyEnvironmentStatus, ProxyProtocol, ProxyVariable,
};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SelectionSource {
    #[default]
    Auto,
    User,
    SystemProxy,
    Manual,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveProxyContext {
    pub selected_candidate_id: Option<String>,
    /// Includes host, port, protocol, client name and PID; retained when unavailable.
    pub candidate: Option<ProxyCandidate>,
    pub selection_source: SelectionSource,
    pub available: bool,
    pub revision: u64,
}

impl ActiveProxyContext {
    pub fn available_candidate(&self) -> Option<&ProxyCandidate> {
        self.candidate.as_ref().filter(|_| self.available)
    }

    fn require_candidate(&self, expected_revision: Option<u64>) -> Result<&ProxyCandidate> {
        if expected_revision.is_some_and(|revision| revision != self.revision) {
            return Err(ProxyEnvError::ActiveProxyChanged);
        }
        self.available_candidate()
            .ok_or(ProxyEnvError::ActiveProxyMissing)
    }
}

#[derive(Default)]
struct ActiveProxyStore {
    context: ActiveProxyContext,
    candidates: Vec<ProxyCandidate>,
}

impl ActiveProxyStore {
    fn select(&mut self, candidate: ProxyCandidate, source: SelectionSource) {
        self.context = ActiveProxyContext {
            selected_candidate_id: Some(candidate.id.clone()),
            available: usable(&candidate),
            candidate: Some(candidate),
            selection_source: source,
            revision: self.context.revision + 1,
        };
    }

    fn reconcile(&mut self, candidates: Vec<ProxyCandidate>, manual: Option<ProxyCandidate>) {
        self.candidates = candidates;
        if let Some(previous) = self.context.candidate.as_ref() {
            let current = if self.context.selection_source == SelectionSource::Manual {
                manual.as_ref()
            } else {
                self.candidates
                    .iter()
                    .find(|candidate| same_identity(previous, candidate))
            };
            let available = current.is_some_and(usable);
            if available != self.context.available {
                self.context.revision += 1;
            }
            self.context.available = available;
            if let Some(candidate) = current.filter(|candidate| usable(candidate)) {
                self.context.candidate = Some(candidate.clone());
            } else if let Some(candidate) = self.context.candidate.as_mut() {
                candidate.listening = false;
            }
            return;
        }
        // Detector ranking is consulted only before the first selection in this session.
        if let Some(candidate) = self
            .candidates
            .iter()
            .find(|candidate| usable(candidate))
            .cloned()
        {
            let source = if candidate
                .source
                .iter()
                .any(|source| matches!(source, DetectionSource::WindowsSystemProxy))
            {
                SelectionSource::SystemProxy
            } else {
                SelectionSource::Auto
            };
            self.select(candidate, source);
        }
    }

    fn refresh(&mut self) {
        let candidates = super::detect().unwrap_or_default();
        let manual = if self.context.selection_source == SelectionSource::Manual {
            self.context.candidate.clone().map(|mut candidate| {
                candidate.listening = super::inspect_endpoint(&endpoint(&candidate))
                    .is_ok_and(|inspection| inspection.listening && inspection.protocol_matches);
                candidate
            })
        } else {
            None
        };
        self.reconcile(candidates, manual);
    }

    fn status(&self, selected: &[ProxyVariable]) -> Result<ProxyEnvironmentStatus> {
        let mut status = ProxyEnvironmentService::status(selected, self.context.candidate.clone())?;
        status.active_proxy = self.context.clone();
        status.candidates = self.candidates.clone();
        Ok(status)
    }
}

fn usable(candidate: &ProxyCandidate) -> bool {
    candidate.listening && candidate.protocol != ProxyProtocol::Unknown
}

fn same_identity(previous: &ProxyCandidate, current: &ProxyCandidate) -> bool {
    previous.id == current.id
        && previous.host == current.host
        && previous.port == current.port
        && previous.protocol == current.protocol
        && previous.pid == current.pid
        && previous.process_name == current.process_name
}

pub fn endpoint(candidate: &ProxyCandidate) -> ProxyEndpoint {
    ProxyEndpoint {
        host: candidate.host.clone(),
        port: candidate.port,
        protocol: candidate.protocol,
    }
}

static STORE: OnceLock<Mutex<ActiveProxyStore>> = OnceLock::new();

fn with_store<T>(operation: impl FnOnce(&mut ActiveProxyStore) -> Result<T>) -> Result<T> {
    let mut store = STORE
        .get_or_init(|| Mutex::new(ActiveProxyStore::default()))
        .lock()
        .map_err(|_| ProxyEnvError::Detection("active proxy state is unavailable".into()))?;
    store.refresh();
    operation(&mut store)
}

pub fn context() -> Result<ActiveProxyContext> {
    with_store(|store| Ok(store.context.clone()))
}

pub fn status(selected: &[ProxyVariable]) -> Result<ProxyEnvironmentStatus> {
    with_store(|store| store.status(selected))
}

pub fn select(candidate_id: &str, selected: &[ProxyVariable]) -> Result<ProxyEnvironmentStatus> {
    with_store(|store| {
        let candidate = store
            .candidates
            .iter()
            .find(|candidate| candidate.id == candidate_id && usable(candidate))
            .cloned()
            .ok_or(ProxyEnvError::ActiveProxyMissing)?;
        store.select(candidate, SelectionSource::User);
        store.status(selected)
    })
}

/// Selection and a synchronous operation are serialized, so a write/launch cannot change target mid-flight.
pub fn with_current<T>(
    expected_revision: Option<u64>,
    operation: impl FnOnce(&ProxyCandidate) -> Result<T>,
) -> Result<T> {
    with_store(|store| operation(store.context.require_candidate(expected_revision)?))
}

/// Manual override is an explicit user action and also becomes the global selection.
pub fn sync_manual(
    endpoint: &ProxyEndpoint,
    selected: &[ProxyVariable],
) -> Result<ProxyEnvironmentStatus> {
    let endpoint = super::plan::validate_and_normalize_endpoint(endpoint)?;
    with_store(|store| {
        let listening = super::inspect_endpoint(&endpoint)?;
        ProxyEnvironmentService::sync_manual(&endpoint, selected)?;
        store.select(
            ProxyCandidate {
                id: format!(
                    "manual:{}:{}:{:?}",
                    endpoint.host, endpoint.port, endpoint.protocol
                ),
                client_name: None,
                icon_key: None,
                process_name: None,
                pid: None,
                host: endpoint.host,
                port: endpoint.port,
                protocol: endpoint.protocol,
                source: Vec::new(),
                confidence: super::models::Confidence::Low,
                listening: listening.listening && listening.protocol_matches,
            },
            SelectionSource::Manual,
        );
        store.status(selected)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(port: u16, pid: u32) -> ProxyCandidate {
        ProxyCandidate {
            id: format!("127.0.0.1:{port}"),
            host: "127.0.0.1".into(),
            port,
            protocol: ProxyProtocol::Mixed,
            pid: Some(pid),
            process_name: Some(format!("client-{pid}")),
            client_name: None,
            icon_key: None,
            source: Vec::new(),
            confidence: super::super::models::Confidence::High,
            listening: true,
        }
    }

    #[test]
    fn starts_with_ranked_recommendation_and_records_source() {
        let mut store = ActiveProxyStore::default();
        let mut recommended = candidate(10809, 2);
        recommended.source = vec![DetectionSource::WindowsSystemProxy];
        store.reconcile(vec![recommended, candidate(7897, 1)], None);
        assert_eq!(store.context.candidate.unwrap().port, 10809);
        assert_eq!(store.context.selection_source, SelectionSource::SystemProxy);
    }

    #[test]
    fn manual_client_selection_survives_ranking_changes() {
        let mut store = ActiveProxyStore::default();
        let clash = candidate(7897, 1);
        let v2ray = candidate(10809, 2);
        store.reconcile(vec![v2ray.clone(), clash.clone()], None);
        store.select(clash.clone(), SelectionSource::User);
        let revision = store.context.revision;
        for candidates in [vec![v2ray.clone(), clash.clone()], vec![clash, v2ray]] {
            store.reconcile(candidates, None);
            assert_eq!(
                endpoint(store.context.require_candidate(Some(revision)).unwrap()).port,
                7897
            );
            assert_eq!(store.context.selection_source, SelectionSource::User);
        }
    }

    #[test]
    fn missing_selection_is_retained_and_never_falls_back() {
        let mut store = ActiveProxyStore::default();
        store.select(candidate(7897, 1), SelectionSource::User);
        let revision = store.context.revision;
        store.reconcile(vec![candidate(10809, 2)], None);
        assert!(!store.context.available);
        assert_eq!(store.context.candidate.as_ref().unwrap().port, 7897);
        assert!(store.context.require_candidate(None).is_err());
        assert!(store.context.require_candidate(Some(revision)).is_err());
        store.reconcile(vec![candidate(10809, 2)], None);
        assert!(!store.context.available);
    }

    #[test]
    fn endpoint_reuse_by_another_process_requires_reselection() {
        let mut store = ActiveProxyStore::default();
        store.select(candidate(7897, 1), SelectionSource::User);
        store.reconcile(vec![candidate(7897, 3)], None);
        assert!(!store.context.available);
    }

    #[test]
    fn stale_operation_is_rejected_after_explicit_switch() {
        let mut store = ActiveProxyStore::default();
        store.select(candidate(7897, 1), SelectionSource::User);
        let revision = store.context.revision;
        store.select(candidate(10809, 2), SelectionSource::User);
        assert!(matches!(
            store.context.require_candidate(Some(revision)),
            Err(ProxyEnvError::ActiveProxyChanged)
        ));
    }

    #[test]
    fn automatic_selection_is_not_replaced_when_it_disappears() {
        let mut store = ActiveProxyStore::default();
        store.reconcile(vec![candidate(7897, 1)], None);
        store.reconcile(vec![candidate(10809, 2)], None);
        assert!(!store.context.available);
        assert_eq!(store.context.candidate.unwrap().port, 7897);
    }

    #[test]
    fn unknown_protocol_is_not_an_initial_recommendation() {
        let mut store = ActiveProxyStore::default();
        let mut unknown = candidate(7897, 1);
        unknown.protocol = ProxyProtocol::Unknown;
        store.reconcile(vec![unknown], None);
        assert!(store.context.selected_candidate_id.is_none());
        assert!(!store.context.available);
        store.reconcile(vec![candidate(10809, 2)], None);
        assert_eq!(store.context.require_candidate(None).unwrap().port, 10809);
    }

    #[test]
    fn changed_protocol_does_not_silently_replace_selected_protocol() {
        let mut store = ActiveProxyStore::default();
        store.select(candidate(7897, 1), SelectionSource::User);
        let mut changed = candidate(7897, 1);
        changed.protocol = ProxyProtocol::Socks5;
        store.reconcile(vec![changed], None);
        assert!(!store.context.available);
        assert_eq!(
            store.context.candidate.unwrap().protocol,
            ProxyProtocol::Mixed
        );
    }

    #[test]
    fn environment_plan_and_serialized_context_use_the_selected_endpoint() {
        let mut store = ActiveProxyStore::default();
        store.reconcile(vec![candidate(10809, 2), candidate(7897, 1)], None);
        store.select(candidate(7897, 1), SelectionSource::User);
        let selected = endpoint(store.context.require_candidate(None).unwrap());
        let plan = super::super::plan::build_proxy_environment_plan(
            &selected,
            &[ProxyVariable::Http, ProxyVariable::Https],
        )
        .unwrap();
        for entry in plan.variables.iter().filter(|entry| entry.value.is_some()) {
            assert_eq!(entry.value.as_deref(), Some("http://127.0.0.1:7897"));
        }
        let serialized = serde_json::to_value(&store.context).unwrap();
        assert_eq!(serialized["candidate"]["port"], 7897);
        assert_eq!(serialized["selectionSource"], "user");
        assert_eq!(serialized["available"], true);
    }

    #[test]
    fn manual_endpoint_survives_detection_and_can_recover_without_switching() {
        let mut store = ActiveProxyStore::default();
        let manual = candidate(9000, 1);
        store.select(manual.clone(), SelectionSource::Manual);
        store.reconcile(vec![candidate(7897, 2)], None);
        assert!(!store.context.available);
        store.reconcile(vec![candidate(7897, 2)], Some(manual));
        assert_eq!(store.context.require_candidate(None).unwrap().port, 9000);
        assert_eq!(store.context.selection_source, SelectionSource::Manual);
    }
}
