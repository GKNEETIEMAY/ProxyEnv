//! A single backend-owned exclusion gate for every bridge entry point and monitor.
//! The lease pauses mutations, not existing SSH/relay traffic. No credentials live here.
use crate::features::remote_bridge::{self as bridge, BridgeResult};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UpdatePhase {
    Release,
    Metadata,
    Download,
    Install,
    Guard,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UpdateOutcome {
    Available,
    Latest,
    Ahead,
    Manual,
    Unpublished,
    Conflict,
    Error,
    Started,
    Success,
    Cancelled,
    RateLimit,
    Timeout,
    Network,
    Tls,
    Proxy,
    Signature,
    InvalidResponse,
    Service,
    Busy,
    StateChanged,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UpdateDiagnostic {
    phase: UpdatePhase,
    outcome: UpdateOutcome,
}
#[tauri::command]
pub(crate) fn record_update_diagnostic(event: UpdateDiagnostic) {
    bridge::logging::record_update(event);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Activity {
    pub generation: u64,
    pub connected: bool,
}

#[derive(Default)]
struct Gate {
    operations: usize,
    next_id: u64,
    lease: Option<(u64, Activity)>,
}
impl Gate {
    fn start_operation(&mut self) -> BridgeResult<()> {
        if self.lease.is_some() {
            return Err("updateInProgress".into());
        }
        self.operations += 1;
        Ok(())
    }
    fn acquire(&mut self, confirmed: bool, activity: Activity) -> BridgeResult<u64> {
        if self.operations != 0 || self.lease.is_some() {
            return Err("updateBusy".into());
        }
        if activity.connected && !confirmed {
            return Err("updateConfirmationRequired".into());
        }
        self.next_id = self.next_id.wrapping_add(1);
        let id = self.next_id;
        self.lease = Some((id, activity));
        Ok(id)
    }
    fn validate(&self, id: u64, current: Activity) -> BridgeResult<()> {
        if self.operations == 0 && self.lease == Some((id, current)) {
            Ok(())
        } else {
            Err("updateStateChanged".into())
        }
    }
    fn release(&mut self, id: u64) {
        if self.lease.is_some_and(|(owner, _)| owner == id) {
            self.lease = None;
        }
    }
}
fn gate() -> &'static Mutex<Gate> {
    static GATE: OnceLock<Mutex<Gate>> = OnceLock::new();
    GATE.get_or_init(|| Mutex::new(Gate::default()))
}

pub(crate) struct BridgeOperation;
pub(crate) fn bridge_changes_busy() -> bool {
    gate()
        .try_lock()
        .map(|state| state.lease.is_some() || state.operations != 0)
        .unwrap_or(true)
}

pub(crate) fn bridge_operation() -> BridgeResult<BridgeOperation> {
    gate()
        .lock()
        .map_err(|_| "stateUnavailable")?
        .start_operation()?;
    Ok(BridgeOperation)
}
impl Drop for BridgeOperation {
    fn drop(&mut self) {
        if let Ok(mut state) = gate().lock() {
            state.operations = state.operations.saturating_sub(1);
        }
    }
}

#[tauri::command]
pub(crate) fn prepare_update_install(confirmed: bool) -> BridgeResult<u64> {
    let mut state = gate().lock().map_err(|_| "stateUnavailable")?;
    if state.operations != 0 || state.lease.is_some() {
        return Err("updateBusy".into());
    }
    // try_lock snapshots fail closed. No blocking network request or Store->gate lock inversion.
    let activity = bridge::update_activity()?;
    state.acquire(confirmed, activity)
}

#[tauri::command]
pub(crate) fn validate_update_install(lease: u64) -> BridgeResult<()> {
    let state = gate().lock().map_err(|_| "stateUnavailable")?;
    state.validate(lease, bridge::update_activity()?)
}

#[tauri::command]
pub(crate) fn release_update_install(lease: u64) -> BridgeResult<()> {
    gate()
        .lock()
        .map_err(|_| "stateUnavailable")?
        .release(lease);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn activity(connected: bool) -> Activity {
        Activity {
            generation: 7,
            connected,
        }
    }

    #[test]
    fn busy_operations_and_unconfirmed_live_bridges_cannot_acquire() {
        let mut gate = Gate::default();
        gate.start_operation().unwrap();
        assert_eq!(gate.acquire(true, activity(true)), Err("updateBusy".into()));
        gate.operations = 0;
        assert_eq!(
            gate.acquire(false, activity(true)),
            Err("updateConfirmationRequired".into())
        );
        assert!(gate.lease.is_none());
    }

    #[test]
    fn lease_blocks_all_new_operations_until_owner_releases_it() {
        let mut gate = Gate::default();
        let id = gate.acquire(true, activity(true)).unwrap();
        assert_eq!(gate.start_operation(), Err("updateInProgress".into()));
        gate.release(id + 1);
        assert!(gate.start_operation().is_err());
        gate.release(id);
        assert!(gate.start_operation().is_ok());
    }

    #[test]
    fn final_check_rejects_stale_lease_transport_loss_or_new_session() {
        let mut gate = Gate::default();
        let id = gate.acquire(true, activity(true)).unwrap();
        assert!(gate.validate(id, activity(true)).is_ok());
        assert!(gate.validate(id + 1, activity(true)).is_err());
        assert!(gate.validate(id, activity(false)).is_err());
        assert!(gate
            .validate(
                id,
                Activity {
                    generation: 8,
                    connected: true
                }
            )
            .is_err());
        gate.release(id);
        assert!(gate.validate(id, activity(true)).is_err());
    }

    #[test]
    fn update_diagnostics_accept_only_typed_phases_and_outcomes() {
        let event = serde_json::from_str::<UpdateDiagnostic>(
            r#"{"phase":"download","outcome":"signature"}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({"phase":"download","outcome":"signature"})
        );
        assert!(serde_json::from_str::<UpdateDiagnostic>(
            r#"{"phase":"download","outcome":"secret-token"}"#
        )
        .is_err());
        assert!(serde_json::from_str::<UpdateDiagnostic>(
            r#"{"phase":"release","outcome":"network","url":"private"}"#
        )
        .is_err());
    }
}
