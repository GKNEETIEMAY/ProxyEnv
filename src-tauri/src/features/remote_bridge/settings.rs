use super::{
    local_model::{self, CompatibilityRule},
    BridgeResult,
};
use crate::services::local_file;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{collections::HashSet, fs, path::PathBuf};

const MAX_SETTINGS_BYTES: u64 = 128 * 1024;
const MAX_RULES: usize = 32;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteBridgeSettings {
    #[serde(default = "default_follow_local")]
    pub follow_local_codex_model: bool,
    #[serde(default)]
    pub compatibility_rules: Vec<CompatibilityRule>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityRuleStatus {
    #[serde(flatten)]
    pub rule: CompatibilityRule,
    pub validation_state: local_model::CompatibilityValidationState,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBridgeSettingsView {
    pub follow_local_codex_model: bool,
    pub compatibility_rules: Vec<CompatibilityRuleStatus>,
    pub local_model_state: local_model::ModelResolutionState,
    pub display_model: Option<String>,
    pub canonical_model: Option<String>,
}

fn default_follow_local() -> bool {
    true
}

impl Default for RemoteBridgeSettings {
    fn default() -> Self {
        Self {
            follow_local_codex_model: true,
            compatibility_rules: Vec::new(),
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
    let settings = serde_json::from_slice(&bytes).map_err(|_| "stateUnavailable")?;
    validate(&settings)?;
    Ok(settings)
}

pub fn save(settings: RemoteBridgeSettings) -> BridgeResult<RemoteBridgeSettingsView> {
    validate(&settings)?;
    let path = path()?;
    let directory = path.parent().ok_or("stateUnavailable")?;
    fs::create_dir_all(directory).map_err(|_| "stateUnavailable")?;
    let bytes = serde_json::to_vec_pretty(&settings).map_err(|_| "stateUnavailable")?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES {
        return Err("invalidRequest".into());
    }
    local_file::atomic_write(&path, &bytes, "remote-bridge-settings")
        .map_err(|_| "stateUnavailable")?;
    Ok(view(settings))
}

fn validate(settings: &RemoteBridgeSettings) -> BridgeResult<()> {
    if settings.compatibility_rules.len() > MAX_RULES {
        return Err("invalidRequest".into());
    }
    let mut ids = HashSet::new();
    for rule in &settings.compatibility_rules {
        if rule.id.is_empty()
            || rule.id.len() > 64
            || !rule
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || !ids.insert(rule.id.as_str())
            || rule.created_at.len() > 48
            || rule.updated_at.len() > 48
            || local_model::validate_compatibility_rule(rule)
                == local_model::CompatibilityValidationState::Invalid
        {
            return Err("invalidRequest".into());
        }
    }
    Ok(())
}

pub fn view(settings: RemoteBridgeSettings) -> RemoteBridgeSettingsView {
    let automatic = local_model::resolve_codex_model();
    let (display_model, canonical_model) = match &automatic {
        local_model::ModelResolution::Resolved(model) => (
            model.display_model.clone(),
            Some(model.canonical_model.clone()),
        ),
        _ => (None, None),
    };
    let mut duplicate_inputs = HashSet::<String>::new();
    let mut seen_inputs = HashSet::<String>::new();
    for rule in settings
        .compatibility_rules
        .iter()
        .filter(|rule| rule.enabled)
    {
        if !seen_inputs.insert(rule.incoming_model.clone()) {
            duplicate_inputs.insert(rule.incoming_model.clone());
        }
    }
    RemoteBridgeSettingsView {
        follow_local_codex_model: settings.follow_local_codex_model,
        compatibility_rules: settings
            .compatibility_rules
            .into_iter()
            .map(|rule| {
                let validation_state = if duplicate_inputs.contains(rule.incoming_model.as_str()) {
                    local_model::CompatibilityValidationState::Ambiguous
                } else {
                    local_model::validate_compatibility_rule(&rule)
                };
                CompatibilityRuleStatus {
                    rule,
                    validation_state,
                }
            })
            .collect(),
        local_model_state: automatic.state(),
        display_model,
        canonical_model,
    }
}

pub fn current_view() -> BridgeResult<RemoteBridgeSettingsView> {
    load().map(view)
}

pub fn resolve(incoming_model: &str) -> local_model::ModelResolution {
    let Ok(settings) = load() else {
        return local_model::ModelResolution::Invalid;
    };
    if settings.follow_local_codex_model {
        local_model::resolve_for_request(incoming_model, &settings.compatibility_rules)
    } else {
        local_model::ModelResolution::Resolved(local_model::EffectiveModel {
            display_model: Some(incoming_model.to_owned()),
            canonical_model: incoming_model.to_owned(),
            source: local_model::ResolutionSource::LocalEffectiveProfile,
            semantic_hash: hex::encode(sha2::Sha256::digest(incoming_model.as_bytes())),
        })
    }
}
