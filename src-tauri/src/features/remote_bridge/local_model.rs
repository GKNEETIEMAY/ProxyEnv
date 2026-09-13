use crate::services::local_file;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

const MAX_CODEX_CONFIG_BYTES: u64 = 128 * 1024;
const MAX_CODEX_CATALOG_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ModelResolutionState {
    Resolved,
    Ambiguous,
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ResolutionSource {
    LocalEffectiveProfile,
    ManualCompatibilityRule,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CompatibilityValidationState {
    Valid,
    Stale,
    Ambiguous,
    Invalid,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveModel {
    pub display_model: Option<String>,
    pub canonical_model: String,
    pub source: ResolutionSource,
    pub semantic_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelResolution {
    Resolved(EffectiveModel),
    Ambiguous,
    Unsupported,
    Invalid,
}

impl ModelResolution {
    pub fn state(&self) -> ModelResolutionState {
        match self {
            Self::Resolved(_) => ModelResolutionState::Resolved,
            Self::Ambiguous => ModelResolutionState::Ambiguous,
            Self::Unsupported => ModelResolutionState::Unsupported,
            Self::Invalid => ModelResolutionState::Invalid,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompatibilityRule {
    pub id: String,
    pub incoming_model: String,
    pub local_display_model: String,
    pub canonical_model: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug)]
struct CatalogModel {
    slug: String,
    display_name: Option<String>,
}

struct LocalContext {
    configured_model: String,
    models: Vec<CatalogModel>,
}

fn safe_text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn parse_catalog(bytes: &[u8]) -> Option<Vec<CatalogModel>> {
    let root: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let models = root.as_object()?.get("models")?.as_array()?;
    if models.is_empty() {
        return None;
    }
    let mut slugs = HashSet::new();
    let mut parsed = Vec::with_capacity(models.len());
    for model in models {
        let model = model.as_object()?;
        let slug = model.get("slug")?.as_str()?;
        if !safe_text(slug, 256) || !slugs.insert(slug.to_owned()) {
            return None;
        }
        let display_name = match model.get("display_name") {
            Some(value) => {
                let value = value.as_str()?;
                if !safe_text(value, 512) {
                    return None;
                }
                Some(value.to_owned())
            }
            None => None,
        };
        parsed.push(CatalogModel {
            slug: slug.to_owned(),
            display_name,
        });
    }
    Some(parsed)
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_: &fs::Metadata) -> bool {
    false
}

fn safe_catalog_path(root: &Path, relative: &str) -> Option<PathBuf> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    let root = root.canonicalize().ok()?;
    let mut current = root.clone();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return None;
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current).ok()?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return None;
        }
    }
    let resolved = current.canonicalize().ok()?;
    resolved.starts_with(&root).then_some(resolved)
}

pub fn resolve_codex_model() -> ModelResolution {
    let Some(home) = dirs::home_dir() else {
        return ModelResolution::Unsupported;
    };
    resolve_codex_model_at(&home)
}

fn resolve_codex_model_at(home: &Path) -> ModelResolution {
    let context = match read_context_at(home) {
        Ok(context) => context,
        Err(resolution) => return resolution,
    };
    resolve_context(&context)
}

fn read_context_at(home: &Path) -> Result<LocalContext, ModelResolution> {
    let directory = home.join(".codex");
    let config_path = directory.join("config.toml");
    let config_bytes = match local_file::safe_read(&config_path, MAX_CODEX_CONFIG_BYTES) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Err(ModelResolution::Unsupported),
        Err(_) => return Err(ModelResolution::Invalid),
    };
    let Ok(config_text) = std::str::from_utf8(&config_bytes) else {
        return Err(ModelResolution::Invalid);
    };
    let Ok(config) = toml::from_str::<toml::Value>(config_text) else {
        return Err(ModelResolution::Invalid);
    };
    let Some(root) = config.as_table() else {
        return Err(ModelResolution::Invalid);
    };
    let configured_model = match root.get("model") {
        Some(value) => match value.as_str().filter(|value| safe_text(value, 256)) {
            Some(value) => value,
            None => return Err(ModelResolution::Invalid),
        },
        None => return Err(ModelResolution::Unsupported),
    };
    let catalog_name = match root.get("model_catalog_json") {
        Some(value) => match value.as_str().filter(|value| safe_text(value, 1024)) {
            Some(value) => value,
            None => return Err(ModelResolution::Invalid),
        },
        None => return Err(ModelResolution::Unsupported),
    };
    let Some(catalog_path) = safe_catalog_path(&directory, catalog_name) else {
        return Err(ModelResolution::Invalid);
    };
    let catalog_bytes = match local_file::safe_read(&catalog_path, MAX_CODEX_CATALOG_BYTES) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Err(ModelResolution::Unsupported),
        Err(_) => return Err(ModelResolution::Invalid),
    };
    let Some(models) = parse_catalog(&catalog_bytes) else {
        return Err(ModelResolution::Invalid);
    };

    Ok(LocalContext {
        configured_model: configured_model.to_owned(),
        models,
    })
}

fn resolve_context(context: &LocalContext) -> ModelResolution {
    let configured_model = context.configured_model.as_str();
    let matched = if let Some(model) = context
        .models
        .iter()
        .find(|model| model.slug == configured_model)
    {
        model
    } else {
        let aliases = context
            .models
            .iter()
            .filter(|model| model.display_name.as_deref() == Some(configured_model))
            .collect::<Vec<_>>();
        match aliases.as_slice() {
            [model] => *model,
            [] if context.models.len() == 1 => {
                // Local routing gateways such as CC Switch may keep a stable
                // request-model alias in config.toml while replacing the
                // single catalog entry with the selected route's display
                // metadata. The configured model remains the request source
                // of truth; the sole catalog entry is descriptive only.
                let model = &context.models[0];
                let semantic_hash = hex::encode(Sha256::digest(
                    serde_json::to_vec(&(
                        configured_model,
                        configured_model,
                        model.slug.as_str(),
                        model.display_name.as_deref(),
                    ))
                    .expect("effective model hash input is serializable"),
                ));
                return ModelResolution::Resolved(EffectiveModel {
                    display_model: model
                        .display_name
                        .clone()
                        .or_else(|| Some(model.slug.clone())),
                    canonical_model: configured_model.to_owned(),
                    source: ResolutionSource::LocalEffectiveProfile,
                    semantic_hash,
                });
            }
            [] => return ModelResolution::Unsupported,
            _ => return ModelResolution::Ambiguous,
        }
    };
    let semantic_hash = hex::encode(Sha256::digest(
        serde_json::to_vec(&(
            configured_model,
            matched.slug.as_str(),
            matched.display_name.as_deref(),
        ))
        .expect("effective model hash input is serializable"),
    ));
    ModelResolution::Resolved(EffectiveModel {
        display_model: matched
            .display_name
            .clone()
            .or_else(|| Some(matched.slug.clone())),
        canonical_model: matched.slug.clone(),
        source: ResolutionSource::LocalEffectiveProfile,
        semantic_hash,
    })
}

pub fn resolve_for_request(incoming_model: &str, rules: &[CompatibilityRule]) -> ModelResolution {
    let Some(home) = dirs::home_dir() else {
        return ModelResolution::Unsupported;
    };
    resolve_for_request_at(&home, incoming_model, rules)
}

pub fn validate_compatibility_rule(rule: &CompatibilityRule) -> CompatibilityValidationState {
    if !safe_text(&rule.incoming_model, 256)
        || !safe_text(&rule.local_display_model, 512)
        || !safe_text(&rule.canonical_model, 256)
    {
        return CompatibilityValidationState::Invalid;
    }
    let Some(home) = dirs::home_dir() else {
        return CompatibilityValidationState::Stale;
    };
    let Ok(context) = read_context_at(&home) else {
        return CompatibilityValidationState::Stale;
    };
    if context.configured_model != rule.local_display_model
        || !context
            .models
            .iter()
            .any(|model| model.slug == rule.canonical_model)
    {
        CompatibilityValidationState::Stale
    } else {
        CompatibilityValidationState::Valid
    }
}

fn resolve_for_request_at(
    home: &Path,
    incoming_model: &str,
    rules: &[CompatibilityRule],
) -> ModelResolution {
    let context = match read_context_at(home) {
        Ok(context) => context,
        Err(resolution) => return resolution,
    };
    match resolve_context(&context) {
        resolved @ ModelResolution::Resolved(_) => resolved,
        automatic => {
            let matches = rules
                .iter()
                .filter(|rule| {
                    rule.enabled
                        && rule.incoming_model == incoming_model
                        && rule.local_display_model == context.configured_model
                        && context
                            .models
                            .iter()
                            .any(|model| model.slug == rule.canonical_model)
                        && safe_text(&rule.canonical_model, 256)
                        && safe_text(&rule.local_display_model, 512)
                })
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [rule] => {
                    let semantic_hash = hex::encode(Sha256::digest(
                        serde_json::to_vec(&(
                            rule.incoming_model.as_str(),
                            rule.local_display_model.as_str(),
                            rule.canonical_model.as_str(),
                        ))
                        .expect("compatibility rule hash input is serializable"),
                    ));
                    ModelResolution::Resolved(EffectiveModel {
                        display_model: Some(rule.local_display_model.clone()),
                        canonical_model: rule.canonical_model.clone(),
                        source: ResolutionSource::ManualCompatibilityRule,
                        semantic_hash,
                    })
                }
                [] => automatic,
                _ => ModelResolution::Ambiguous,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, time::SystemTime};

    fn fixture(config: &str, catalog_name: &str, catalog: Option<&str>) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "proxyenv-local-model-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join(".codex/nested")).unwrap();
        fs::write(root.join(".codex/config.toml"), config).unwrap();
        if let Some(catalog) = catalog {
            fs::write(root.join(".codex").join(catalog_name), catalog).unwrap();
        }
        root
    }

    fn catalog(models: &str) -> String {
        format!(r#"{{"models":[{models}],"future":{{"preserved":true}}}}"#)
    }

    #[test]
    fn resolves_slug_and_unique_display_name_from_configured_catalog() {
        let data = catalog(
            r#"{"slug":"deepseek-v4-pro","display_name":"gpt-5.6-sol","future_capability":{"enabled":true}}"#,
        );
        for configured in ["deepseek-v4-pro", "gpt-5.6-sol"] {
            let config =
                format!("model = \"{configured}\"\nmodel_catalog_json = \"models-v2.json\"\n");
            let root = fixture(&config, "models-v2.json", Some(&data));
            let ModelResolution::Resolved(model) = resolve_codex_model_at(&root) else {
                panic!("model should resolve");
            };
            assert_eq!(model.display_model.as_deref(), Some("gpt-5.6-sol"));
            assert_eq!(model.canonical_model, "deepseek-v4-pro");
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn supports_nested_catalog_and_rejects_unsafe_paths() {
        let data = catalog(r#"{"slug":"kimi-k2.5","display_name":"kimi"}"#);
        let root = fixture(
            "model = \"kimi\"\nmodel_catalog_json = \"nested/models.json\"\n",
            "nested/models.json",
            Some(&data),
        );
        assert!(matches!(
            resolve_codex_model_at(&root),
            ModelResolution::Resolved(_)
        ));
        fs::remove_dir_all(root).unwrap();

        for catalog_path in ["../outside.json", "C:/outside.json", "./models.json"] {
            let config = format!("model = \"kimi\"\nmodel_catalog_json = \"{catalog_path}\"\n");
            let root = fixture(&config, "unused.json", None);
            assert_eq!(resolve_codex_model_at(&root), ModelResolution::Invalid);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn duplicate_alias_is_ambiguous_and_invalid_catalog_is_rejected() {
        let aliases = catalog(
            r#"{"slug":"one","display_name":"shared"},{"slug":"two","display_name":"shared"}"#,
        );
        let root = fixture(
            "model = \"shared\"\nmodel_catalog_json = \"models.json\"\n",
            "models.json",
            Some(&aliases),
        );
        assert_eq!(resolve_codex_model_at(&root), ModelResolution::Ambiguous);
        fs::remove_dir_all(root).unwrap();

        let duplicate = catalog(r#"{"slug":"same"},{"slug":"same"}"#);
        let root = fixture(
            "model = \"same\"\nmodel_catalog_json = \"models.json\"\n",
            "models.json",
            Some(&duplicate),
        );
        assert_eq!(resolve_codex_model_at(&root), ModelResolution::Invalid);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_or_unresolved_local_profile_does_not_guess() {
        for (config, catalog_data) in [
            ("model = \"gpt-5\"\n", None),
            (
                "model = \"unknown\"\nmodel_catalog_json = \"models.json\"\n",
                Some(catalog(r#"{"slug":"one"},{"slug":"two"}"#)),
            ),
            (
                "model_catalog_json = \"models.json\"\n",
                Some(catalog(r#"{"slug":"known"}"#)),
            ),
        ] {
            let root = fixture(config, "models.json", catalog_data.as_deref());
            assert_eq!(resolve_codex_model_at(&root), ModelResolution::Unsupported);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn single_catalog_entry_can_describe_a_gateway_route_alias() {
        let data = catalog(r#"{"slug":"deepseek-flash","display_name":"deepseek-flash"}"#);
        let root = fixture(
            "model = \"deepseek-v4-pro\"\nmodel_catalog_json = \"models.json\"\n",
            "models.json",
            Some(&data),
        );
        let ModelResolution::Resolved(model) = resolve_codex_model_at(&root) else {
            panic!("gateway request model should resolve");
        };
        assert_eq!(model.display_model.as_deref(), Some("deepseek-flash"));
        assert_eq!(model.canonical_model, "deepseek-v4-pro");
        assert_eq!(model.source, ResolutionSource::LocalEffectiveProfile);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn automatic_resolution_wins_and_only_current_valid_manual_mapping_is_used() {
        let data = catalog(
            r#"{"slug":"deepseek-v4-pro","display_name":"gpt-5.6-sol"},{"slug":"kimi-k2.5","display_name":"kimi"}"#,
        );
        let auto_root = fixture(
            "model = \"kimi\"\nmodel_catalog_json = \"models.json\"\n",
            "models.json",
            Some(&data),
        );
        let stale_rule = CompatibilityRule {
            id: "rule".into(),
            incoming_model: "remote-old".into(),
            local_display_model: "gpt-5.6-sol".into(),
            canonical_model: "deepseek-v4-pro".into(),
            enabled: true,
            created_at: "now".into(),
            updated_at: "now".into(),
        };
        let ModelResolution::Resolved(auto) =
            resolve_for_request_at(&auto_root, "remote-old", std::slice::from_ref(&stale_rule))
        else {
            panic!("automatic model should resolve");
        };
        assert_eq!(auto.canonical_model, "kimi-k2.5");
        assert_eq!(auto.source, ResolutionSource::LocalEffectiveProfile);
        fs::remove_dir_all(auto_root).unwrap();

        let manual_root = fixture(
            "model = \"custom-display\"\nmodel_catalog_json = \"models.json\"\n",
            "models.json",
            Some(&data),
        );
        let valid_rule = CompatibilityRule {
            local_display_model: "custom-display".into(),
            ..stale_rule.clone()
        };
        let ModelResolution::Resolved(manual) =
            resolve_for_request_at(&manual_root, "remote-old", &[valid_rule])
        else {
            panic!("manual model should resolve");
        };
        assert_eq!(manual.canonical_model, "deepseek-v4-pro");
        assert_eq!(manual.source, ResolutionSource::ManualCompatibilityRule);
        assert_eq!(
            resolve_for_request_at(&manual_root, "remote-old", &[stale_rule]),
            ModelResolution::Unsupported
        );
        fs::remove_dir_all(manual_root).unwrap();
    }
}
