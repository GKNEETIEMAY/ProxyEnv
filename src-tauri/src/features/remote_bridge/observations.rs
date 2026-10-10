//! Cached diagnostic proof. No endpoints, credentials or free-form error text.
use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticState {
    #[default]
    NotTested,
    Testing,
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCategory {
    Network,
    Ssh,
    Authentication,
    Conflict,
    Unsupported,
    Unavailable,
    InvalidResponse,
    Unknown,
}

impl ErrorCategory {
    pub fn from_code(code: &str) -> Self {
        match code {
            "networkFailed" => Self::Network,
            "sshTimeout" | "sshFailed" | "forwardDenied" | "sshConfigChanged" => Self::Ssh,
            "sshAuth" | "authenticationRequired" => Self::Authentication,
            "configConflict" | "remoteProfileConflict" => Self::Conflict,
            "remoteUnsupported" | "toolVerificationUnsupported" => Self::Unsupported,
            "bridgeUnavailable" | "proxyUnavailable" | "routeUnavailable" | "relayUnavailable"
            | "toolNotConfigured" => Self::Unavailable,
            "remoteFailed" => Self::InvalidResponse,
            _ => Self::Unknown,
        }
    }
}

pub fn timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().try_into().unwrap_or(u64::MAX)
        })
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticObservation {
    pub state: DiagnosticState,
    pub checked_at: Option<u64>,
    pub error_code: Option<ErrorCategory>,
    pub duration_ms: Option<u64>,
    #[serde(skip)]
    revision: u64,
}

impl DiagnosticObservation {
    pub fn begin(&mut self) -> u64 {
        self.reset();
        self.state = DiagnosticState::Testing;
        self.revision
    }

    pub fn reset(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.state = DiagnosticState::NotTested;
        self.checked_at = None;
        self.error_code = None;
        self.duration_ms = None;
    }

    pub fn current(&self, revision: u64) -> bool {
        self.revision == revision && self.state == DiagnosticState::Testing
    }

    pub fn finish(
        &mut self,
        revision: u64,
        error: Option<ErrorCategory>,
        duration_ms: u64,
    ) -> bool {
        if !self.current(revision) {
            return false;
        }
        self.state = if error.is_some() {
            DiagnosticState::Failed
        } else {
            DiagnosticState::Passed
        };
        self.checked_at = Some(timestamp());
        self.error_code = error;
        self.duration_ms = Some(duration_ms);
        true
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSnapshot {
    pub server_direct: DiagnosticObservation,
    pub server_internet: Option<super::RemoteInternetState>,
    pub general_proxy_egress: DiagnosticObservation,
    pub ai_route_verification: DiagnosticObservation,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proof_is_cached_and_invalidated_without_accepting_late_results() {
        let mut observation = DiagnosticObservation::default();
        let first = observation.begin();
        assert_eq!(observation.state, DiagnosticState::Testing);
        assert!(observation.finish(first, None, 42));
        assert_eq!(observation.state, DiagnosticState::Passed);
        assert_eq!(observation.duration_ms, Some(42));
        assert!(observation.checked_at.is_some());
        let stale = observation.begin();
        observation.reset();
        let current = observation.begin();
        assert!(!observation.finish(stale, None, 99));
        assert!(observation.finish(current, Some(ErrorCategory::Network), 5));
        assert_eq!(observation.state, DiagnosticState::Failed);
        assert_eq!(observation.error_code, Some(ErrorCategory::Network));
        let serialized = serde_json::to_string(&observation).unwrap();
        assert!(!serialized.contains("revision"));
    }
    #[test]
    fn arbitrary_errors_are_reduced_to_categories() {
        for raw in [
            "/home/private/settings.json",
            "Authorization: private",
            "https://private.invalid/?token=secret",
        ] {
            assert_eq!(ErrorCategory::from_code(raw), ErrorCategory::Unknown);
        }
    }
}
