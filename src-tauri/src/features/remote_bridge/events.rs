//! Session-local, bounded, allowlisted events. No free-form payload is accepted.
use super::observations::{timestamp, ErrorCategory};
use serde::Serialize;
use std::collections::VecDeque;

const CAPACITY: usize = 100;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Component {
    Ssh,
    GeneralProxy,
    AiRoute,
    SessionEnvironment,
    Vscode,
    Codex,
    Claude,
    Skills,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Connect,
    Disconnect,
    TransportLost,
    Reconnect,
    ServerDirect,
    EgressTest,
    ToolVerify,
    EnvironmentSetup,
    VscodeSetup,
    ToolSetup,
    CredentialClear,
    Enable,
    Disable,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Success,
    Warning,
    Failed,
    Started,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEvent {
    pub timestamp: u64,
    pub level: Level,
    pub component: Component,
    pub action: Action,
    pub outcome: Outcome,
    pub error_code: Option<ErrorCategory>,
    pub duration_ms: Option<u64>,
}

#[derive(Default)]
pub(super) struct EventStore {
    events: VecDeque<BridgeEvent>,
}
impl EventStore {
    pub fn clear(&mut self) {
        self.events.clear();
    }
    pub fn push(
        &mut self,
        component: Component,
        action: Action,
        outcome: Outcome,
        error_code: Option<ErrorCategory>,
        duration_ms: Option<u64>,
    ) {
        if self.events.len() == CAPACITY {
            self.events.pop_front();
        }
        let event = BridgeEvent {
            timestamp: timestamp(),
            component,
            action,
            outcome,
            error_code,
            duration_ms,
            level: match outcome {
                Outcome::Failed => Level::Error,
                Outcome::Warning => Level::Warning,
                _ => Level::Info,
            },
        };
        super::logging::record(&event);
        self.events.push_back(event);
    }
    pub fn recent(&self, limit: usize) -> Vec<BridgeEvent> {
        self.events
            .iter()
            .rev()
            .take(limit.min(CAPACITY))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn events_are_bounded_newest_first_and_clearable() {
        let mut queue = EventStore::default();
        for duration in 0..150 {
            queue.push(
                Component::GeneralProxy,
                Action::EgressTest,
                Outcome::Success,
                None,
                Some(duration),
            );
        }
        let events = queue.recent(usize::MAX);
        assert_eq!(events.len(), CAPACITY);
        assert_eq!(events.first().unwrap().duration_ms, Some(149));
        assert_eq!(events.last().unwrap().duration_ms, Some(50));
        assert_eq!(queue.recent(2).len(), 2);
        assert_eq!(queue.recent(0).len(), 0);
        queue.clear();
        assert!(queue.recent(100).is_empty());
    }
    #[test]
    fn serialized_events_have_only_safe_fields() {
        let mut queue = EventStore::default();
        queue.push(
            Component::AiRoute,
            Action::ToolVerify,
            Outcome::Failed,
            Some(ErrorCategory::Authentication),
            Some(2),
        );
        let value = serde_json::to_value(queue.recent(1)).unwrap();
        let event = value[0].as_object().unwrap();
        assert_eq!(event.len(), 7);
        assert_eq!(event["level"], "error");
        assert_eq!(event["errorCode"], "authentication");
        for key in [
            "token", "password", "body", "host", "path", "username", "headers",
        ] {
            assert!(!event.contains_key(key));
        }
    }
}
