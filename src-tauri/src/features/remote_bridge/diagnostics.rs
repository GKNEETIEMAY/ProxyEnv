//! Explicit coordination only. Every test still uses its original implementation.
use super::{
    observations::DiagnosticSnapshot, BridgeResult, RemoteInternetState, RuntimeState, Status,
};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(false);
struct RunGuard;
impl RunGuard {
    fn acquire() -> BridgeResult<Self> {
        RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "bridgeBusy")?;
        Ok(Self)
    }
}
impl Drop for RunGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnostics {
    pub running: bool,
    pub observations: DiagnosticSnapshot,
    pub ssh: Status,
    pub vscode: RuntimeState,
    pub server_internet: Option<RemoteInternetState>,
}

pub fn snapshot() -> BridgeResult<RuntimeDiagnostics> {
    // Cached read: never opens a socket or launches SSH.
    let state = super::store().try_lock().map_err(|_| "stateUnavailable")?;
    Ok(project_snapshot(
        &state.summary,
        RUNNING.load(Ordering::Acquire),
    ))
}
fn project_snapshot(summary: &super::Summary, running: bool) -> RuntimeDiagnostics {
    RuntimeDiagnostics {
        running,
        observations: summary.diagnostics.clone(),
        ssh: summary.status,
        vscode: summary.vscode_state,
        server_internet: summary.diagnostics.server_internet,
    }
}
fn should_verify(summary: &super::Summary, tool: &super::tool_adapter::RemoteToolState) -> bool {
    let runtime = match tool.id {
        super::tool_adapter::RemoteToolId::Codex => summary.codex_state,
        super::tool_adapter::RemoteToolId::Claude => summary.claude_state,
    };
    tool.configured
        && super::tool_adapter::by_id(tool.id).configured(summary)
        && tool.verification_supported
        && summary.cc_status == Some(Status::Connected)
        && !matches!(runtime, RuntimeState::Pending | RuntimeState::Preparing)
}

fn run_parallel(jobs: Vec<Box<dyn FnOnce() + Send + '_>>) -> BridgeResult<()> {
    std::thread::scope(|scope| {
        let tasks: Vec<_> = jobs.into_iter().map(|job| scope.spawn(job)).collect();
        tasks
            .into_iter()
            .try_for_each(|task| task.join().map_err(|_| "remoteFailed".to_owned()))
    })
}

pub fn run_all() -> BridgeResult<RuntimeDiagnostics> {
    let _guard = RunGuard::acquire()?;
    let (generation, target_id, summary) = {
        let mut state = super::lock()?;
        super::refresh(&mut state);
        if state.child.is_none() || state.summary.status != Status::Connected {
            return Err("bridgeUnavailable".into());
        }
        (
            state.connection_generation,
            state
                .summary
                .target
                .as_ref()
                .ok_or("invalidTarget")?
                .id
                .clone(),
            super::exposed_summary(&state.summary),
        )
    };
    super::logging::debug_diagnostics();
    // A failed lane must not prevent the other lane from being checked.
    let mut jobs: Vec<Box<dyn FnOnce() + Send>> = vec![Box::new(move || {
        let _ = super::check_remote_network_for_generation(target_id, Some(generation));
    })];
    if summary.proxy_status == Some(Status::Connected) {
        jobs.push(Box::new(move || {
            let _ = super::test_for_generation(Some(generation));
        }));
    }
    for tool in &summary.tools {
        if should_verify(&summary, tool) {
            let id = tool.id;
            jobs.push(Box::new(move || {
                // The original verifier rechecks the toggle and generation.
                let _ = super::verify_tool_for_generation(id.as_str().into(), Some(generation));
            }));
        }
    }
    run_parallel(jobs)?;
    // Validate and project while owning the same lock; never return another session's snapshot.
    let state = super::lock()?;
    if state.connection_generation != generation || state.child.is_none() {
        return Err("bridgeUnavailable".into());
    }
    Ok(project_snapshot(&state.summary, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_run_is_rejected_before_any_ssh_probe_or_state_change() {
        let generation = super::super::lock()
            .unwrap()
            .connection_generation
            .wrapping_add(1);
        assert_eq!(
            super::super::test_for_generation(Some(generation)).unwrap_err(),
            "bridgeUnavailable"
        );
        assert_eq!(
            super::super::verify_tool_for_generation("claude".into(), Some(generation))
                .unwrap_err(),
            "bridgeUnavailable"
        );
        assert_eq!(
            super::super::check_remote_network_for_generation(
                "nonexistent-target".into(),
                Some(generation)
            )
            .unwrap_err(),
            "bridgeUnavailable"
        );
    }
    #[test]
    fn duplicate_runs_are_rejected_and_guard_always_releases() {
        let guard = RunGuard::acquire().unwrap();
        assert!(RunGuard::acquire().is_err());
        drop(guard);
        assert!(RunGuard::acquire().is_ok());
    }
    #[test]
    fn verification_skips_unsupported_disabled_and_preparing_tools() {
        let mut summary = super::super::Summary {
            claude_configured: true,
            codex_configured: true,
            cc_status: Some(Status::Connected),
            ..Default::default()
        };
        super::super::sync_tool_states(&mut summary);
        assert!(summary
            .tools
            .iter()
            .all(|tool| !should_verify(&summary, tool)));
        summary.claude_state = RuntimeState::Ready;
        let claude = summary
            .tools
            .iter()
            .find(|tool| tool.id.as_str() == "claude")
            .unwrap();
        assert!(should_verify(&summary, claude));
        let codex = summary
            .tools
            .iter()
            .find(|tool| tool.id.as_str() == "codex")
            .unwrap();
        summary.codex_state = RuntimeState::Ready;
        assert!(!should_verify(&summary, codex));
        summary.claude_configured = false;
        assert!(
            !should_verify(&summary, claude),
            "a stale tool snapshot cannot override its off switch"
        );
        summary.claude_configured = true;
        summary.cc_status = Some(Status::Disconnected);
        assert!(!should_verify(&summary, claude));
    }

    #[test]
    fn independent_checks_start_concurrently_and_all_complete() {
        use std::sync::{atomic::AtomicUsize, Arc, Barrier};
        let barrier = Arc::new(Barrier::new(3));
        let completed = Arc::new(AtomicUsize::new(0));
        let jobs = (0..3)
            .map(|_| {
                let barrier = barrier.clone();
                let completed = completed.clone();
                Box::new(move || {
                    barrier.wait();
                    completed.fetch_add(1, Ordering::AcqRel);
                }) as Box<dyn FnOnce() + Send>
            })
            .collect();
        run_parallel(jobs).unwrap();
        assert_eq!(completed.load(Ordering::Acquire), 3);
    }
}
