use super::{BridgeResult, Summary};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemoteToolId {
    Codex,
    Claude,
}

impl RemoteToolId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemoteToolRouteMode {
    CcSwitch,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemoteToolCompatibility {
    Compatible,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RemoteToolVerification {
    #[default]
    NotConfigured,
    VerifyPending,
    Verified,
    AuthenticationRequired,
    RouteUnavailable,
    TimedOut,
    Failed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteToolState {
    pub id: RemoteToolId,
    pub display_name: &'static str,
    pub configured: bool,
    pub verification: RemoteToolVerification,
    pub verification_supported: bool,
    pub supported_route_modes: &'static [RemoteToolRouteMode],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteToolPlan {
    pub path: &'static str,
    pub content: String,
    pub launch: &'static str,
}

pub trait RemoteToolAdapter: Sync {
    fn id(&self) -> RemoteToolId;
    fn display_name(&self) -> &'static str;
    fn config_path(&self) -> &'static str;
    fn launch(&self) -> &'static str;
    fn render(&self, port: u16) -> String;
    fn configured(&self, summary: &Summary) -> bool;
    fn set_configured(&self, summary: &mut Summary, configured: bool);
    fn verification(&self, summary: &Summary) -> RemoteToolVerification;
    fn set_verification(&self, summary: &mut Summary, verification: RemoteToolVerification);
    fn compatibility(&self, version: &str) -> RemoteToolCompatibility;
    fn verification_supported(&self) -> bool {
        false
    }

    fn detect(&self, version: &str) -> bool {
        self.compatibility(version) == RemoteToolCompatibility::Compatible
    }

    fn inspect(&self, summary: &Summary) -> RemoteToolState {
        let configured = self.configured(summary);
        RemoteToolState {
            id: self.id(),
            display_name: self.display_name(),
            configured,
            verification: self.verification(summary),
            verification_supported: self.verification_supported(),
            supported_route_modes: self.supported_route_modes(),
        }
    }

    fn preview(&self, port: u16) -> RemoteToolPlan {
        RemoteToolPlan {
            path: self.config_path(),
            content: self.render(port),
            launch: self.launch(),
        }
    }

    fn apply(&self, summary: &mut Summary) {
        self.set_configured(summary, true);
        self.set_verification(summary, RemoteToolVerification::VerifyPending);
    }

    fn restore(&self, summary: &mut Summary) {
        self.set_configured(summary, false);
        self.set_verification(summary, RemoteToolVerification::NotConfigured);
    }

    fn verify(&self, summary: &mut Summary, verification: RemoteToolVerification) {
        if self.configured(summary) && self.verification_supported() {
            self.set_verification(summary, verification);
        }
    }

    fn supported_route_modes(&self) -> &'static [RemoteToolRouteMode] {
        &[RemoteToolRouteMode::CcSwitch]
    }
}

struct CodexCliAdapter;
struct ClaudeCliAdapter;

impl RemoteToolAdapter for CodexCliAdapter {
    fn id(&self) -> RemoteToolId {
        RemoteToolId::Codex
    }

    fn display_name(&self) -> &'static str {
        "Codex CLI"
    }

    fn config_path(&self) -> &'static str {
        "~/.codex/proxyenv_bridge.config.toml"
    }

    fn launch(&self) -> &'static str {
        "codex --profile proxyenv_bridge"
    }

    fn render(&self, port: u16) -> String {
        format!(
            "# ProxyEnv Remote Bridge\nmodel_provider = \"proxyenv_bridge\"\n\n[model_providers.proxyenv_bridge]\nname = \"ProxyEnv CC Switch\"\nbase_url = \"http://127.0.0.1:{port}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\n"
        )
    }

    fn configured(&self, summary: &Summary) -> bool {
        summary.codex_configured
    }

    fn set_configured(&self, summary: &mut Summary, configured: bool) {
        summary.codex_configured = configured;
    }

    fn verification(&self, summary: &Summary) -> RemoteToolVerification {
        summary.codex_verification
    }

    fn set_verification(&self, summary: &mut Summary, verification: RemoteToolVerification) {
        summary.codex_verification = verification;
    }

    fn compatibility(&self, version: &str) -> RemoteToolCompatibility {
        let mut fields = version.split('.');
        let compatible = fields.next() == Some("0")
            && fields
                .next()
                .and_then(|minor| minor.parse::<u16>().ok())
                .is_some_and(|minor| minor >= 134)
            && fields
                .next()
                .and_then(|patch| patch.parse::<u16>().ok())
                .is_some()
            && fields.next().is_none();
        if compatible {
            RemoteToolCompatibility::Compatible
        } else {
            RemoteToolCompatibility::Unsupported
        }
    }
}

impl RemoteToolAdapter for ClaudeCliAdapter {
    fn id(&self) -> RemoteToolId {
        RemoteToolId::Claude
    }

    fn display_name(&self) -> &'static str {
        "Claude Code CLI"
    }

    fn config_path(&self) -> &'static str {
        "~/.claude/proxyenv-bridge.json"
    }

    fn launch(&self) -> &'static str {
        "claude --settings \"$HOME/.claude/proxyenv-bridge.json\""
    }

    fn render(&self, port: u16) -> String {
        format!(
            "{{\"env\":{{\"ANTHROPIC_BASE_URL\":\"http://127.0.0.1:{port}\",\"ANTHROPIC_AUTH_TOKEN\":\"PROXY_MANAGED\"}}}}\n"
        )
    }

    fn configured(&self, summary: &Summary) -> bool {
        summary.claude_configured
    }

    fn set_configured(&self, summary: &mut Summary, configured: bool) {
        summary.claude_configured = configured;
    }

    fn verification(&self, summary: &Summary) -> RemoteToolVerification {
        summary.claude_verification
    }

    fn set_verification(&self, summary: &mut Summary, verification: RemoteToolVerification) {
        summary.claude_verification = verification;
    }

    fn verification_supported(&self) -> bool {
        true
    }

    fn compatibility(&self, version: &str) -> RemoteToolCompatibility {
        let mut fields = version.split('.');
        let compatible = fields.next() == Some("2")
            && fields
                .next()
                .and_then(|minor| minor.parse::<u16>().ok())
                .is_some()
            && fields
                .next()
                .and_then(|patch| patch.parse::<u16>().ok())
                .is_some()
            && fields.next().is_none();
        if compatible {
            RemoteToolCompatibility::Compatible
        } else {
            RemoteToolCompatibility::Unsupported
        }
    }
}

static CODEX: CodexCliAdapter = CodexCliAdapter;
static CLAUDE: ClaudeCliAdapter = ClaudeCliAdapter;
static ADAPTERS: [&dyn RemoteToolAdapter; 2] = [&CODEX, &CLAUDE];

pub fn adapters() -> &'static [&'static dyn RemoteToolAdapter] {
    &ADAPTERS
}

pub fn by_id(id: RemoteToolId) -> &'static dyn RemoteToolAdapter {
    match id {
        RemoteToolId::Codex => &CODEX,
        RemoteToolId::Claude => &CLAUDE,
    }
}

pub fn by_name(id: &str) -> BridgeResult<&'static dyn RemoteToolAdapter> {
    adapters()
        .iter()
        .copied()
        .find(|adapter| adapter.id().as_str() == id)
        .ok_or_else(|| "invalidRequest".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_unique_supported_cli_adapters() {
        assert_eq!(adapters().len(), 2);
        assert_ne!(adapters()[0].id(), adapters()[1].id());
        assert!(by_name("openclaw").is_err());
        for adapter in adapters() {
            assert_eq!(
                adapter.supported_route_modes(),
                &[RemoteToolRouteMode::CcSwitch]
            );
            assert!(!adapter.config_path().is_empty());
            assert!(!adapter.launch().is_empty());
        }
    }

    #[test]
    fn adapters_own_version_compatibility_and_overlay_content() {
        assert!(CODEX.detect("0.134.0"));
        assert!(!CODEX.detect("0.133.9"));
        assert!(CLAUDE.detect("2.1.0"));
        assert!(!CLAUDE.detect("1.9.9"));
        assert!(CODEX.preview(25721).content.contains("model_provider"));
        assert!(CLAUDE.preview(25721).content.contains("ANTHROPIC_BASE_URL"));
    }

    #[test]
    fn configured_state_remains_pending_until_real_verification() {
        let mut summary = Summary::default();
        for adapter in adapters() {
            assert_eq!(
                adapter.verification(&summary),
                RemoteToolVerification::NotConfigured
            );
            adapter.apply(&mut summary);
            let state = adapter.inspect(&summary);
            assert!(state.configured);
            assert_eq!(state.verification, RemoteToolVerification::VerifyPending);
            adapter.restore(&mut summary);
        }
    }

    #[test]
    fn serialized_state_is_allowlisted_and_uses_stable_wire_names() {
        let mut summary = Summary::default();
        CODEX.apply(&mut summary);
        let value = serde_json::to_value(CODEX.inspect(&summary)).expect("serialize tool state");

        assert_eq!(value["id"], "codex");
        assert_eq!(value["displayName"], "Codex CLI");
        assert_eq!(value["configured"], true);
        assert_eq!(value["verification"], "verifyPending");
        assert_eq!(value["verificationSupported"], false);
        assert_eq!(value["supportedRouteModes"][0], "ccSwitch");
        assert_eq!(value.as_object().map(serde_json::Map::len), Some(6));
    }

    #[test]
    fn only_claude_accepts_a_real_verification_result_in_m6() {
        let mut summary = Summary::default();
        CODEX.apply(&mut summary);
        CLAUDE.apply(&mut summary);
        assert!(!CODEX.verification_supported());
        assert!(CLAUDE.verification_supported());

        CLAUDE.verify(&mut summary, RemoteToolVerification::Verified);
        assert_eq!(
            CLAUDE.verification(&summary),
            RemoteToolVerification::Verified
        );
        CODEX.verify(&mut summary, RemoteToolVerification::Verified);
        assert_eq!(
            CODEX.verification(&summary),
            RemoteToolVerification::VerifyPending
        );
    }
}
