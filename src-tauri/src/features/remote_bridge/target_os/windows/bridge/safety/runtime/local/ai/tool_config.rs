use super::ServiceBinding;
use crate::features::remote_bridge::{tool_adapter, tool_adapter::RemoteToolId, BridgeResult};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Weak};

pub(in super::super::super::super) struct ToolConfigPreview {
    owner: Weak<ServiceBinding>,
    source_hash: Option<[u8; 32]>,
    content: Vec<u8>,
    pub filename: &'static str,
    pub token_env: &'static str,
}

pub(in super::super::super::super) struct AppliedToolConfig {
    content: Option<Vec<u8>>,
}

pub(in super::super::super::super) struct ToolConfigApply<'a> {
    pub preview: ToolConfigPreview,
    pub source: Option<&'a [u8]>,
    pub occupied: bool,
    pub confirmed: bool,
}

impl ToolConfigPreview {
    pub(super) fn new(
        owner: &Arc<ServiceBinding>,
        tool: RemoteToolId,
        version: &str,
        source: Option<&[u8]>,
        port: u16,
    ) -> BridgeResult<Self> {
        let adapter = tool_adapter::by_id(tool);
        if !adapter.detect(version) {
            return Err("cliUnsupported".into());
        }
        let original = source
            .map(|bytes| std::str::from_utf8(bytes).map_err(|_| "configConflict"))
            .transpose()?;
        let preview = adapter.preview(port, None);
        let (filename, token_env, content) = match tool {
            RemoteToolId::Codex => (
                "config.toml",
                "PROXYENV_SESSION_TOKEN",
                codex(original, &preview.content)?,
            ),
            RemoteToolId::Claude => (
                "settings.json",
                "ANTHROPIC_AUTH_TOKEN",
                claude(original, &preview.content)?,
            ),
        };
        Ok(Self {
            owner: Arc::downgrade(owner),
            source_hash: source.map(digest),
            content,
            filename,
            token_env,
        })
    }

    pub(super) fn belongs_to(&self, owner: &Arc<ServiceBinding>) -> bool {
        self.owner.ptr_eq(&Arc::downgrade(owner))
    }

    pub(super) fn apply(
        self,
        source: Option<&[u8]>,
        occupied: bool,
        confirmed: bool,
    ) -> BridgeResult<AppliedToolConfig> {
        if !confirmed {
            return Err("confirmationRequired".into());
        }
        if occupied || source.map(digest) != self.source_hash {
            return Err("configConflict".into());
        }
        Ok(AppliedToolConfig {
            content: Some(self.content),
        })
    }
}

impl AppliedToolConfig {
    pub(in super::super::super::super) fn content(&self) -> Option<&[u8]> {
        self.content.as_deref()
    }

    pub(in super::super::super::super) fn restore(
        &mut self,
        observed: Option<&[u8]>,
    ) -> BridgeResult<()> {
        if let (Some(expected), Some(observed)) = (self.content(), observed) {
            if observed != expected {
                return Err("configConflict".into());
            }
        }
        self.content = None;
        Ok(())
    }
}

fn codex(original: Option<&str>, preview: &str) -> BridgeResult<Vec<u8>> {
    if let Some(original) = original {
        let original: toml::Table = toml::from_str(original).map_err(|_| "configConflict")?;
        if original.get("model_provider").and_then(toml::Value::as_str) == Some("proxyenv_bridge")
            || original
                .get("model_providers")
                .and_then(toml::Value::as_table)
                .is_some_and(|providers| providers.contains_key("proxyenv_bridge"))
        {
            return Err("configConflict".into());
        }
    }
    let mut overlay: toml::Table = toml::from_str(preview).map_err(|_| "configConflict")?;
    overlay.remove("model");
    overlay.remove("model_catalog_json");
    let provider = overlay
        .get_mut("model_providers")
        .and_then(toml::Value::as_table_mut)
        .and_then(|providers| providers.get_mut("proxyenv_bridge"))
        .and_then(toml::Value::as_table_mut)
        .ok_or("configConflict")?;
    provider.insert(
        "env_key".into(),
        toml::Value::String("PROXYENV_SESSION_TOKEN".into()),
    );
    Ok(toml::to_string(&overlay)
        .map_err(|_| "configConflict")?
        .into_bytes())
}

fn claude(original: Option<&str>, preview: &str) -> BridgeResult<Vec<u8>> {
    if let Some(original) = original {
        let original: Value = serde_json::from_str(original).map_err(|_| "configConflict")?;
        let original = original.as_object().ok_or("configConflict")?;
        if original.contains_key("apiKeyHelper") {
            return Err("configConflict".into());
        }
        if let Some(environment) = original.get("env") {
            let environment = environment.as_object().ok_or("configConflict")?;
            if [
                "ANTHROPIC_BASE_URL",
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_AUTH_TOKEN",
            ]
            .iter()
            .any(|key| environment.contains_key(*key))
            {
                return Err("configConflict".into());
            }
        }
    }
    let mut overlay: Value = serde_json::from_str(preview).map_err(|_| "configConflict")?;
    overlay
        .get_mut("env")
        .and_then(Value::as_object_mut)
        .ok_or("configConflict")?
        .remove("ANTHROPIC_AUTH_TOKEN");
    Ok(serde_json::to_vec(&overlay).map_err(|_| "configConflict")?)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
