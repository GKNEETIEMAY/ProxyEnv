use super::{capability_ready, tool_adapter, tool_discovery, Store};

pub(super) fn invalidate_tool_discovery(state: &mut Store, fingerprint: Option<&str>) {
    if state.tool_discovery_fingerprint.is_some()
        && state.tool_discovery_fingerprint.as_deref() != fingerprint
    {
        state.tool_discovery_ticket = state.tool_discovery_ticket.wrapping_add(1);
        state.tool_discovery_fingerprint = None;
        state.summary.tool_installations = tool_adapter::unknown_installations();
        state.summary.install_capabilities = tool_discovery::RemoteInstallCapabilities::default();
    }
}

pub(super) fn discovery_context_current(
    state: &Store,
    target_id: &str,
    fingerprint: &str,
    generation: u64,
    ticket: u64,
) -> bool {
    state.child.is_some()
        && state.connection_generation == generation
        && state.tool_discovery_ticket == ticket
        && state
            .summary
            .target
            .as_ref()
            .is_some_and(|target| target.id == target_id)
        && state.target_fingerprint.as_deref() == Some(fingerprint)
}

pub(super) fn automatic_tool_setup_allowed(
    state: &Store,
    tool: tool_adapter::RemoteToolId,
) -> bool {
    use tool_adapter::RemoteToolInstallState as I;
    capability_ready(state.child.is_some(), state.summary.cc_status)
        && state.summary.cc.is_some()
        && !state.disabled_tools.contains(&tool)
        && !tool_adapter::by_id(tool).configured(&state.summary)
        && state.summary.tool_installations.iter().any(|entry| {
            entry.tool == tool
                && entry.state != I::Checking
                && entry.extension_state != I::Checking
                && (entry.state == I::Installed || entry.extension_state == I::Installed)
        })
}
