use super::{
    capability_ready, target_os, tool_adapter, DiagnosticState, ErrorCategory, PhaseTiming, Report,
    RuntimeState, Store, Summary,
};

pub(super) fn project_report(state: &Store) -> Report {
    let s = &state.summary;
    Report {
        tool_installations: if state.tool_discovery_fingerprint == state.target_fingerprint {
            s.tool_installations.clone()
        } else {
            tool_adapter::unknown_installations()
        },
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
        codex_extension: safe_extension_status(s.codex_extension.as_deref()),
        claude_extension: safe_extension_status(s.claude_extension.as_deref()),
        reconnect_state: s.reconnect_state,
        ssh_auth_method: s.ssh_auth.method,
        runtime_proxy_match: s.runtime_proxy_match,
        post_connect_status: s.post_connect_status,
        session_environment_state: s.session_environment_state,
        vscode_state: s.vscode_state,
        codex_state: s.codex_state,
        claude_state: s.claude_state,
        skills_state: s.skills_state,
        post_connect_error_category: s
            .post_connect_error
            .as_deref()
            .map(ErrorCategory::from_code),
        phase_timings: safe_phase_timings(&s.timings),
        diagnostics: s.diagnostics.clone(),
        security: s.security,
    }
}

fn safe_extension_status(value: Option<&str>) -> Option<String> {
    crate::services::redaction::report_label(
        value,
        &["configured", "notConfigured", "conflict", "unknown"],
    )
}

fn safe_phase_timings(timings: &[PhaseTiming]) -> Vec<PhaseTiming> {
    const PHASES: &[&str] = &[
        "bridge.prepare",
        "ssh.authenticate",
        "coreBridge.total",
        "bridge.forwardReady",
        "bridge.total",
        "sessionEnv.apply",
        "vscodeNetwork.refresh",
        "codex.status",
        "codex.preview",
        "codex.apply",
        "claude.status",
        "claude.preview",
        "claude.apply",
        "postConnect.total",
        "remoteTools.discovery",
    ];
    timings
        .iter()
        .take(50)
        .map(|entry| PhaseTiming {
            phase: crate::services::redaction::report_label(Some(&entry.phase), PHASES)
                .unwrap_or_else(|| "unknown".into()),
            duration_ms: entry.duration_ms,
            outcome: crate::services::redaction::report_label(
                Some(&entry.outcome),
                &["success", "failed", "unsupported"],
            )
            .unwrap_or_else(|| "unknown".into()),
        })
        .collect()
}

pub(super) fn sync_tool_states(summary: &mut Summary) {
    if summary.tool_installations.is_empty() {
        summary.tool_installations = tool_adapter::unknown_installations();
    }
    summary.tools = tool_adapter::adapters()
        .iter()
        .map(|adapter| adapter.inspect(summary))
        .collect();
    // Re-applying/restoring a client profile requires a fresh real request.
    if matches!(
        summary.diagnostics.ai_route_verification.state,
        DiagnosticState::Passed | DiagnosticState::Failed
    ) && !summary.tools.iter().any(|tool| {
        tool.configured
            && tool.verification_supported
            && !matches!(
                tool.verification,
                tool_adapter::RemoteToolVerification::NotConfigured
                    | tool_adapter::RemoteToolVerification::VerifyPending
            )
    }) {
        summary.diagnostics.ai_route_verification.reset();
    }
}

pub(super) fn projected_session_command(
    summary: &Summary,
    ssh_running: bool,
    session_id: Option<&str>,
) -> Option<String> {
    if summary.proxy.is_none()
        || !capability_ready(ssh_running, summary.proxy_status)
        || summary.session_environment_state != RuntimeState::Ready
    {
        return None;
    }
    session_id.map(target_os::linux::session_environment_source_command)
}
