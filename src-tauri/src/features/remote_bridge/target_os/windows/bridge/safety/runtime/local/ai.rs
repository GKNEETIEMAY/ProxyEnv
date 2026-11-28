mod tool_config;
pub(in super::super::super) use tool_config::ToolConfigApply;

use super::{LocalRuntime, Phase, Task};
use crate::features::{
    proxy::{ProxyEndpoint, ProxyProtocol},
    remote_bridge::{
        authenticated_relay::{AuthenticatedRelay, RelayMode},
        cc_service::{CcDetection, CcDetectionState, CcServiceState},
        target_os::WindowsAiState,
        tool_adapter::RemoteToolId,
        BridgeResult,
    },
};
use std::sync::Arc;

use crate::features::remote_bridge::target_os::windows_bridge::safety::{
    policy::ClockSample, WindowsBridgeInput,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct ServiceBinding {
    port: u16,
    revision: u64,
}

impl ServiceBinding {
    fn recognized(service: CcDetection) -> Option<Self> {
        (service.state == CcDetectionState::Confirmed
            && service.service == CcServiceState::Recognized
            && service.local_port >= 1024
            && service.revision > 0)
            .then_some(Self {
                port: service.local_port,
                revision: service.revision,
            })
    }
}

pub(in super::super::super) struct AiPreparation(Arc<ServiceBinding>);

pub(super) enum AiSession {
    Inactive(WindowsAiState),
    Preparing(Arc<ServiceBinding>),
    Prepared {
        binding: Arc<ServiceBinding>,
        relay: AuthenticatedRelay,
    },
}

impl Default for AiSession {
    fn default() -> Self {
        Self::Inactive(WindowsAiState::NotEnabled)
    }
}

impl AiSession {
    fn state(&self) -> WindowsAiState {
        match self {
            Self::Inactive(state) => *state,
            Self::Preparing(_) => WindowsAiState::Preparing,
            Self::Prepared { .. } => WindowsAiState::Prepared,
        }
    }

    pub(super) fn close(&mut self, state: WindowsAiState) -> WindowsAiState {
        if self.state() == WindowsAiState::CleanupIncomplete {
            return WindowsAiState::CleanupIncomplete;
        }
        if let Self::Prepared { mut relay, .. } = std::mem::replace(self, Self::Inactive(state)) {
            relay.set_enabled(false);
            if !relay.close() {
                *self = Self::Inactive(WindowsAiState::CleanupIncomplete);
            }
        }
        self.state()
    }
}

impl LocalRuntime<'_, '_> {
    pub(in super::super::super) fn prepare_ai(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        service: CcDetection,
        confirmed: bool,
        clock: impl FnMut() -> ClockSample,
    ) -> BridgeResult<AiPreparation> {
        if !self.runtime.owns(task) {
            return Err("windowsBridgeContextChanged".into());
        }
        if !self.transport_verified || !self.poll(task, current, clock) {
            return Err("bridgeUnavailable".into());
        }
        if matches!(
            self.ai.state(),
            WindowsAiState::Preparing
                | WindowsAiState::Prepared
                | WindowsAiState::CleanupIncomplete
        ) {
            return Err("bridgeBusy".into());
        }
        if !confirmed {
            self.ai.close(WindowsAiState::AwaitingAuthorization);
            return Err("confirmationRequired".into());
        }
        let Some(binding) = ServiceBinding::recognized(service) else {
            self.ai.close(WindowsAiState::Unavailable);
            return Err("ccUnavailable".into());
        };
        let binding = Arc::new(binding);
        self.ai = AiSession::Preparing(Arc::clone(&binding));
        Ok(AiPreparation(binding))
    }

    // The detached coordinator is only driven by local fixtures. It keeps the
    // existing authenticated relay in its own lane and issues no route to SSH.
    pub(in super::super::super) fn finish_ai<'context>(
        &mut self,
        task: &Task,
        attempt: AiPreparation,
        current: impl Fn() -> WindowsBridgeInput<'context>,
        mut service: impl FnMut() -> CcDetection,
        mut clock: impl FnMut() -> ClockSample,
    ) -> BridgeResult<()> {
        let token = attempt.0;
        if !self.runtime.owns(task)
            || !matches!(&self.ai, AiSession::Preparing(active) if Arc::ptr_eq(active, &token))
        {
            return Err("windowsBridgeContextChanged".into());
        }
        if !self.poll(task, current(), &mut clock) {
            return Err("bridgeUnavailable".into());
        }
        if ServiceBinding::recognized(service()) != Some(*token) {
            self.ai.close(WindowsAiState::Invalidated);
            return Err("ccUnavailable".into());
        }
        let endpoint = ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: token.port,
            protocol: ProxyProtocol::Http,
        };
        let relay = AuthenticatedRelay::start(endpoint, RelayMode::AiHttp).inspect_err(|_| {
            self.ai.close(WindowsAiState::Unavailable);
        })?;
        if !self.poll(task, current(), clock) {
            drop(relay);
            return Err("bridgeUnavailable".into());
        }
        if ServiceBinding::recognized(service()) != Some(*token) {
            drop(relay);
            self.ai.close(WindowsAiState::Invalidated);
            return Err("ccUnavailable".into());
        }
        self.ai = AiSession::Prepared {
            binding: token,
            relay,
        };
        Ok(())
    }

    pub(in super::super::super) fn poll_ai(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        service: CcDetection,
        clock: impl FnMut() -> ClockSample,
    ) -> Option<WindowsAiState> {
        if !self.runtime.owns(task) {
            return None;
        }
        if self.poll(task, current, clock) {
            let observed = ServiceBinding::recognized(service);
            let invalid = match &self.ai {
                AiSession::Preparing(binding) => observed != Some(**binding),
                AiSession::Prepared { binding, relay } => {
                    !relay.is_running() || observed != Some(**binding)
                }
                AiSession::Inactive(_) => false,
            };
            if invalid {
                self.ai.close(WindowsAiState::Invalidated);
            }
        }
        Some(self.ai.state())
    }

    pub(in super::super::super) fn stop_ai(&mut self, task: &Task) -> Option<WindowsAiState> {
        if !self.runtime.owns(task) {
            return None;
        }
        if matches!(
            self.runtime.phase,
            Phase::Closed(_) | Phase::ClosedAbnormally(_)
        ) {
            return Some(self.ai.state());
        }
        Some(self.ai.close(WindowsAiState::Closed))
    }

    pub(in super::super::super) fn preview_ai_tool(
        &self,
        task: &Task,
        now: ClockSample,
        tool: RemoteToolId,
        version: &str,
        source: Option<&[u8]>,
    ) -> BridgeResult<tool_config::ToolConfigPreview> {
        if !self.available(task, now) {
            return Err("bridgeUnavailable".into());
        }
        let AiSession::Prepared { binding, relay } = &self.ai else {
            return Err("bridgeUnavailable".into());
        };
        if !relay.is_running() {
            return Err("bridgeUnavailable".into());
        }
        tool_config::ToolConfigPreview::new(binding, tool, version, source, relay.port())
    }

    pub(in super::super::super) fn apply_ai_tool(
        &mut self,
        task: &Task,
        current: WindowsBridgeInput<'_>,
        service: CcDetection,
        clock: impl FnMut() -> ClockSample,
        change: ToolConfigApply<'_>,
    ) -> BridgeResult<tool_config::AppliedToolConfig> {
        let AiSession::Prepared { binding, .. } = &self.ai else {
            return Err("bridgeUnavailable".into());
        };
        if !self.runtime.owns(task) || !change.preview.belongs_to(binding) {
            return Err("windowsBridgeContextChanged".into());
        }
        if self.poll_ai(task, current, service, clock) != Some(WindowsAiState::Prepared) {
            return Err("bridgeUnavailable".into());
        }
        change
            .preview
            .apply(change.source, change.occupied, change.confirmed)
    }

    #[cfg(test)]
    pub(in super::super::super) fn ai_relay_fixture(&self) -> Option<&AuthenticatedRelay> {
        match &self.ai {
            AiSession::Prepared { relay, .. } => Some(relay),
            _ => None,
        }
    }
}
