use super::ServiceBinding;
use crate::features::remote_bridge::{
    authenticated_relay::AuthenticatedRelay,
    target_os::windows_bridge::safety::local_files::{delete_by_handle, FileId, PrivateDirectory},
    tool_adapter,
    tool_adapter::RemoteToolId,
    BridgeResult,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{Arc, Weak},
};
use windows::Win32::{
    Foundation::{GENERIC_READ, GENERIC_WRITE},
    Storage::FileSystem::{DELETE, FILE_SHARE_READ},
};

// A local test listener cannot be used as a verified remote access endpoint.
pub(super) struct LocalRelayToolEndpoint(u16);

impl LocalRelayToolEndpoint {
    pub(super) fn from_local_relay(relay: &AuthenticatedRelay) -> Self {
        Self(relay.port())
    }
}

pub(in super::super::super::super) struct ToolConfigPreview {
    owner: Weak<ServiceBinding>,
    source_hash: Option<[u8; 32]>,
    content: Vec<u8>,
    pub filename: &'static str,
    pub token_env: &'static str,
}

pub(in super::super::super::super) struct AppliedToolConfig {
    owner: Weak<ServiceBinding>,
    source_hash: Option<[u8; 32]>,
    filename: &'static str,
    content: Option<Vec<u8>>,
    overlay: Option<OwnedOverlay>,
}

struct OwnedOverlay {
    directory: PrivateDirectory,
    file: File,
    id: FileId,
    complete: bool,
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
        endpoint: LocalRelayToolEndpoint,
    ) -> BridgeResult<Self> {
        let adapter = tool_adapter::by_id(tool);
        if !adapter.detect(version) {
            return Err("cliUnsupported".into());
        }
        let original = source
            .map(|bytes| std::str::from_utf8(bytes).map_err(|_| "configConflict"))
            .transpose()?;
        let preview = adapter.preview(endpoint.0, None);
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
            owner: self.owner,
            source_hash: self.source_hash,
            filename: self.filename,
            content: Some(self.content),
            overlay: None,
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
        if self.overlay.is_some() {
            return Err("configConflict".into());
        }
        if let (Some(expected), Some(observed)) = (self.content(), observed) {
            if observed != expected {
                return Err("configConflict".into());
            }
        }
        self.content = None;
        Ok(())
    }

    pub(in super::super::super::super) fn materialize(
        &mut self,
        directory: &Path,
        source: Option<&[u8]>,
    ) -> BridgeResult<()> {
        self.write_overlay(directory, source, |file, bytes| {
            file.write_all(bytes)?;
            file.sync_all()
        })
    }

    #[cfg(test)]
    pub(in super::super::super::super) fn materialize_partial_fixture(
        &mut self,
        directory: &Path,
        source: Option<&[u8]>,
    ) -> BridgeResult<()> {
        self.write_overlay(directory, source, |file, bytes| {
            file.write_all(&bytes[..bytes.len() / 2])?;
            Err(io::ErrorKind::WriteZero.into())
        })
    }

    fn write_overlay(
        &mut self,
        directory: &Path,
        source: Option<&[u8]>,
        write: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> BridgeResult<()> {
        let _owner = self.owner.upgrade().ok_or("windowsBridgeContextChanged")?;
        if self.overlay.is_some() || source.map(digest) != self.source_hash {
            return Err("configConflict".into());
        }
        let bytes = self.content.as_deref().ok_or("configConflict")?;
        let directory = PrivateDirectory::new(directory, false, GENERIC_READ.0)
            .map_err(|_| "configConflict")?;
        let file = directory
            .file(
                self.filename,
                GENERIC_READ.0 | GENERIC_WRITE.0 | DELETE.0,
                FILE_SHARE_READ,
                true,
            )
            .map_err(|_| "configConflict")?;
        let id = directory
            .validate(&file, false)
            .map_err(|_| "configConflict")?;
        let owned = self.overlay.insert(OwnedOverlay {
            directory,
            file,
            id,
            complete: false,
        });
        if write(&mut owned.file, bytes).is_err() {
            // Retain the open handle for retry if owned partial output cannot be unlinked.
            self.restore_materialized()?;
            return Err("stateUnavailable".into());
        }
        owned.complete = true;
        Ok(())
    }

    pub(in super::super::super::super) fn restore_materialized(&mut self) -> BridgeResult<()> {
        let Some(overlay) = &mut self.overlay else {
            return Ok(());
        };
        if overlay
            .directory
            .validate(&overlay.directory.directory, true)
            .ok()
            != Some(overlay.directory.id)
            || overlay.directory.validate(&overlay.file, false).ok() != Some(overlay.id)
        {
            return Err("configConflict".into());
        }
        if overlay.complete {
            let expected = self.content.as_deref().ok_or("configConflict")?;
            let mut observed = Vec::new();
            overlay
                .file
                .seek(SeekFrom::Start(0))
                .map_err(|_| "configConflict")?;
            (&mut overlay.file)
                .take(expected.len() as u64 + 1)
                .read_to_end(&mut observed)
                .map_err(|_| "configConflict")?;
            if observed != expected {
                return Err("configConflict".into());
            }
        }
        delete_by_handle(&overlay.file).map_err(|_| "configConflict")?;
        self.overlay = None;
        self.content = None;
        Ok(())
    }
}

impl Drop for AppliedToolConfig {
    fn drop(&mut self) {
        if self.restore_materialized().is_err() {
            tracing::warn!("AI configuration cleanup incomplete");
        }
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
