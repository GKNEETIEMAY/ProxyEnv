use super::{local_model, BridgeResult};
use crate::features::proxy::ProxyEndpoint;
use crate::services::local_file;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const MAX_SETTINGS_BYTES: u64 = 32 * 1024;

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProfileSyncState {
    Disabled,
    #[default]
    NotStarted,
    Synced,
    LocalChanged,
    RemoteChanged,
    Conflict,
    InvalidLocalProfile,
    RemoteUnavailable,
    RestartRequired,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBridgeSettings {
    #[serde(default = "default_follow_profile", alias = "followLocalCodexModel")]
    pub follow_local_codex_profile: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBridgeSettingsView {
    pub follow_local_codex_profile: bool,
    pub profile_state: ProfileSyncState,
    pub model: Option<String>,
    pub profile_hash: Option<String>,
}

fn default_follow_profile() -> bool {
    true
}

impl Default for RemoteBridgeSettings {
    fn default() -> Self {
        Self {
            follow_local_codex_profile: true,
        }
    }
}

fn path() -> BridgeResult<PathBuf> {
    dirs::data_local_dir()
        .map(|path| path.join("ProxyEnv").join("remote-bridge.json"))
        .ok_or_else(|| "stateUnavailable".into())
}

pub fn load() -> BridgeResult<RemoteBridgeSettings> {
    let path = path()?;
    let Some(bytes) =
        local_file::safe_read(&path, MAX_SETTINGS_BYTES).map_err(|_| "stateUnavailable")?
    else {
        return Ok(RemoteBridgeSettings::default());
    };
    serde_json::from_slice(&bytes).map_err(|_| "stateUnavailable".into())
}

pub fn save(
    settings: RemoteBridgeSettings,
    _endpoint: Option<&ProxyEndpoint>,
) -> BridgeResult<RemoteBridgeSettingsView> {
    let path = path()?;
    let directory = path.parent().ok_or("stateUnavailable")?;
    fs::create_dir_all(directory).map_err(|_| "stateUnavailable")?;
    let bytes = serde_json::to_vec_pretty(&settings).map_err(|_| "stateUnavailable")?;
    local_file::atomic_write(&path, &bytes, "remote-bridge-settings")
        .map_err(|_| "stateUnavailable")?;
    Ok(view(settings))
}

fn view(settings: RemoteBridgeSettings) -> RemoteBridgeSettingsView {
    if !settings.follow_local_codex_profile {
        return RemoteBridgeSettingsView {
            follow_local_codex_profile: false,
            profile_state: ProfileSyncState::Disabled,
            model: None,
            profile_hash: None,
        };
    }
    match local_model::inspect_codex_profile() {
        Ok(profile) => RemoteBridgeSettingsView {
            follow_local_codex_profile: true,
            profile_state: ProfileSyncState::NotStarted,
            model: Some(profile.model),
            profile_hash: Some(profile.hash),
        },
        Err(_) => RemoteBridgeSettingsView {
            follow_local_codex_profile: true,
            profile_state: ProfileSyncState::InvalidLocalProfile,
            model: None,
            profile_hash: None,
        },
    }
}

pub fn current_view(_endpoint: Option<&ProxyEndpoint>) -> BridgeResult<RemoteBridgeSettingsView> {
    Ok(view(load()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_mapping_settings_migrate_to_profile_sync_default() {
        let settings: RemoteBridgeSettings = serde_json::from_str(
            r#"{"followLocalCodexModel":true,"routeMappings":[{"id":"old"}]}"#,
        )
        .unwrap();
        assert!(settings.follow_local_codex_profile);
        assert_eq!(
            serde_json::to_value(settings).unwrap(),
            serde_json::json!({"followLocalCodexProfile": true})
        );
    }
}
