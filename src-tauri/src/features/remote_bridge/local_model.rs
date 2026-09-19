use crate::services::local_file;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

const MAX_CODEX_CONFIG_BYTES: u64 = 128 * 1024;
const MAX_CODEX_CATALOG_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CLAUDE_CONFIG_BYTES: u64 = 128 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileError {
    Missing,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCodexProfile {
    pub model: String,
    pub catalog_path: PathBuf,
    pub catalog_bytes: Vec<u8>,
    pub hash: String,
    pub stamp: LocalCodexProfileStamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalCodexProfileStamp {
    config_modified: u128,
    config_len: u64,
    catalog_modified: u128,
    catalog_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalClaudeProfile {
    pub model: Option<String>,
    pub settings_bytes: Vec<u8>,
    pub hash: String,
    pub stamp: LocalClaudeProfileStamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalClaudeProfileStamp {
    exists: bool,
    modified: u128,
    len: u64,
}

fn safe_text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
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

fn file_stamp(path: &Path) -> Result<(u128, u64), ProfileError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ProfileError::Missing)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || is_reparse_point(&metadata)
    {
        return Err(ProfileError::Invalid);
    }
    let modified = metadata
        .modified()
        .map_err(|_| ProfileError::Invalid)?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProfileError::Invalid)?
        .as_nanos();
    Ok((modified, metadata.len()))
}

fn optional_file_stamp(path: &Path) -> Result<LocalClaudeProfileStamp, ProfileError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file()
                || metadata.file_type().is_symlink()
                || is_reparse_point(&metadata)
            {
                return Err(ProfileError::Invalid);
            }
            let modified = metadata
                .modified()
                .map_err(|_| ProfileError::Invalid)?
                .duration_since(UNIX_EPOCH)
                .map_err(|_| ProfileError::Invalid)?
                .as_nanos();
            Ok(LocalClaudeProfileStamp {
                exists: true,
                modified,
                len: metadata.len(),
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LocalClaudeProfileStamp {
            exists: false,
            modified: 0,
            len: 0,
        }),
        Err(_) => Err(ProfileError::Invalid),
    }
}

fn safe_claude_env_key(key: &str) -> bool {
    key == "ANTHROPIC_MODEL"
        || key == "CLAUDE_CODE_SUBAGENT_MODEL"
        || key == "CLAUDE_CODE_EFFORT_LEVEL"
        || key == "ANTHROPIC_CUSTOM_MODEL_OPTION"
        || ["OPUS", "SONNET", "HAIKU", "FABLE"].iter().any(|tier| {
            ["MODEL", "MODEL_NAME"]
                .iter()
                .any(|field| key == format!("ANTHROPIC_DEFAULT_{tier}_{field}"))
        })
}

fn safe_model_text(value: &serde_json::Value) -> bool {
    value.as_str().is_some_and(|text| {
        safe_text(text, 512)
            && !text.contains("://")
            && !text.contains('@')
            && !text.contains(['\r', '\n'])
    })
}

fn validate_claude_field(key: &str, value: &serde_json::Value) -> bool {
    match key {
        "model" | "effortLevel" => safe_model_text(value),
        "alwaysThinkingEnabled" => value.is_boolean(),
        "availableModels" => value
            .as_array()
            .is_some_and(|items| items.len() <= 128 && items.iter().all(safe_model_text)),
        "modelOverrides" => value.as_object().is_some_and(|items| {
            items.len() <= 128
                && items
                    .iter()
                    .all(|(name, model)| safe_text(name, 256) && safe_model_text(model))
        }),
        _ => false,
    }
}

pub fn inspect_claude_profile() -> Result<LocalClaudeProfile, ProfileError> {
    let home = dirs::home_dir().ok_or(ProfileError::Missing)?;
    inspect_claude_profile_at(&home)
}

fn inspect_claude_profile_at(home: &Path) -> Result<LocalClaudeProfile, ProfileError> {
    let config_path = home.join(".claude").join("settings.json");
    let stamp = optional_file_stamp(&config_path)?;
    let root = match local_file::safe_read(&config_path, MAX_CLAUDE_CONFIG_BYTES)
        .map_err(|_| ProfileError::Invalid)?
    {
        Some(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)
            .map_err(|_| ProfileError::Invalid)?,
        None => serde_json::json!({}),
    };
    let source = root.as_object().ok_or(ProfileError::Invalid)?;
    let mut projected = serde_json::Map::new();
    for key in [
        "model",
        "availableModels",
        "modelOverrides",
        "effortLevel",
        "alwaysThinkingEnabled",
    ] {
        if let Some(value) = source.get(key) {
            if !validate_claude_field(key, value) {
                return Err(ProfileError::Invalid);
            }
            projected.insert(key.to_owned(), value.clone());
        }
    }
    let mut projected_env = serde_json::Map::new();
    if let Some(env) = source.get("env") {
        let env = env.as_object().ok_or(ProfileError::Invalid)?;
        for (key, value) in env {
            if safe_claude_env_key(key) {
                if !safe_model_text(value) {
                    return Err(ProfileError::Invalid);
                }
                projected_env.insert(key.clone(), value.clone());
            }
        }
    }
    if !projected_env.is_empty() {
        projected.insert("env".to_owned(), serde_json::Value::Object(projected_env));
    }
    let model = projected
        .get("model")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            projected
                .get("env")
                .and_then(serde_json::Value::as_object)
                .and_then(|env| env.get("ANTHROPIC_MODEL"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::to_owned);
    let settings_bytes = serde_json::to_vec(&serde_json::Value::Object(projected))
        .map_err(|_| ProfileError::Invalid)?;
    let hash = hex::encode(Sha256::digest(&settings_bytes));
    Ok(LocalClaudeProfile {
        model,
        settings_bytes,
        hash,
        stamp,
    })
}

pub fn claude_profile_stamp_changed(profile: &LocalClaudeProfile) -> Result<bool, ProfileError> {
    let home = dirs::home_dir().ok_or(ProfileError::Missing)?;
    Ok(profile.stamp != optional_file_stamp(&home.join(".claude").join("settings.json"))?)
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

pub fn inspect_codex_profile() -> Result<LocalCodexProfile, ProfileError> {
    let home = dirs::home_dir().ok_or(ProfileError::Missing)?;
    inspect_codex_profile_at(&home)
}

fn inspect_codex_profile_at(home: &Path) -> Result<LocalCodexProfile, ProfileError> {
    let directory = home.join(".codex");
    let config_path = directory.join("config.toml");
    let config_bytes = local_file::safe_read(&config_path, MAX_CODEX_CONFIG_BYTES)
        .map_err(|_| ProfileError::Invalid)?
        .ok_or(ProfileError::Missing)?;
    let config_text = std::str::from_utf8(&config_bytes).map_err(|_| ProfileError::Invalid)?;
    let config = toml::from_str::<toml::Value>(config_text).map_err(|_| ProfileError::Invalid)?;
    let root = config.as_table().ok_or(ProfileError::Invalid)?;
    let model = root
        .get("model")
        .and_then(toml::Value::as_str)
        .filter(|value| safe_text(value, 256))
        .ok_or(ProfileError::Missing)?
        .to_owned();
    let catalog_name = root
        .get("model_catalog_json")
        .and_then(toml::Value::as_str)
        .filter(|value| safe_text(value, 1024))
        .ok_or(ProfileError::Missing)?;
    let catalog_path = safe_catalog_path(&directory, catalog_name).ok_or(ProfileError::Invalid)?;
    let catalog_bytes = local_file::safe_read(&catalog_path, MAX_CODEX_CATALOG_BYTES)
        .map_err(|_| ProfileError::Invalid)?
        .ok_or(ProfileError::Missing)?;
    let catalog: serde_json::Value =
        serde_json::from_slice(&catalog_bytes).map_err(|_| ProfileError::Invalid)?;
    let models = catalog
        .as_object()
        .and_then(|object| object.get("models"))
        .and_then(serde_json::Value::as_array)
        .ok_or(ProfileError::Invalid)?;
    let contains_model = models.iter().any(|entry| {
        entry
            .as_object()
            .and_then(|object| {
                object
                    .get("slug")
                    .or_else(|| object.get("model"))
                    .or_else(|| object.get("id"))
            })
            .and_then(serde_json::Value::as_str)
            == Some(model.as_str())
    });
    if !contains_model {
        return Err(ProfileError::Invalid);
    }
    let (config_modified, config_len) = file_stamp(&config_path)?;
    let (catalog_modified, catalog_len) = file_stamp(&catalog_path)?;
    let hash = hex::encode(Sha256::digest(
        [model.as_bytes(), b"\0", catalog_bytes.as_slice()].concat(),
    ));
    Ok(LocalCodexProfile {
        model,
        catalog_path,
        catalog_bytes,
        hash,
        stamp: LocalCodexProfileStamp {
            config_modified,
            config_len,
            catalog_modified,
            catalog_len,
        },
    })
}

/// The two-second bridge poll only reads metadata for the two known files. It
/// never opens or parses their contents unless this reports a change.
pub fn profile_stamp_changed(profile: &LocalCodexProfile) -> Result<bool, ProfileError> {
    let home = dirs::home_dir().ok_or(ProfileError::Missing)?;
    let config_path = home.join(".codex").join("config.toml");
    let (config_modified, config_len) = file_stamp(&config_path)?;
    let (catalog_modified, catalog_len) = file_stamp(&profile.catalog_path)?;
    Ok(profile.stamp
        != LocalCodexProfileStamp {
            config_modified,
            config_len,
            catalog_modified,
            catalog_len,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn claude_profile_projects_only_safe_model_selection() {
        let root = fixture("model = \"fixture\"\n", "unused.json", None);
        let path = root.join(".claude/settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"model":"模型 A","availableModels":["模型 A","sonnet"],"modelOverrides":{"sonnet":"模型 A"},"env":{"ANTHROPIC_DEFAULT_SONNET_MODEL":"模型 A","ANTHROPIC_DEFAULT_SECRET_MODEL":"private-model","ANTHROPIC_CUSTOM_MODEL_OPTION_TOKEN":"private-option","ANTHROPIC_AUTH_TOKEN":"private-token","ANTHROPIC_BASE_URL":"https://upstream.example"},"permissions":{"allow":["private"]}}"#,
        )
        .unwrap();
        let profile = inspect_claude_profile_at(&root).unwrap();
        let projected: serde_json::Value = serde_json::from_slice(&profile.settings_bytes).unwrap();
        assert_eq!(projected["model"], "模型 A");
        assert_eq!(projected["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"], "模型 A");
        assert!(projected["env"].get("ANTHROPIC_AUTH_TOKEN").is_none());
        assert!(projected["env"].get("ANTHROPIC_BASE_URL").is_none());
        assert!(projected["env"]
            .get("ANTHROPIC_DEFAULT_SECRET_MODEL")
            .is_none());
        assert!(projected["env"]
            .get("ANTHROPIC_CUSTOM_MODEL_OPTION_TOKEN")
            .is_none());
        assert!(projected.get("permissions").is_none());
        assert_eq!(profile.model.as_deref(), Some("模型 A"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn claude_profile_rejects_nested_or_url_model_values_and_handles_missing_file() {
        let root = fixture("model = \"fixture\"\n", "unused.json", None);
        assert_eq!(
            inspect_claude_profile_at(&root).unwrap().settings_bytes,
            b"{}"
        );
        let path = root.join(".claude/settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        for value in [
            r#"{"modelOverrides":{"sonnet":{"token":"private"}}}"#,
            r#"{"env":{"ANTHROPIC_DEFAULT_SONNET_MODEL":"https://upstream.example"}}"#,
        ] {
            fs::write(&path, value).unwrap();
            assert_eq!(inspect_claude_profile_at(&root), Err(ProfileError::Invalid));
        }
        fs::remove_dir_all(root).unwrap();
    }

    fn fixture(config: &str, catalog_name: &str, catalog: Option<&str>) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "proxyenv-codex-profile-{}-{}",
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

    #[test]
    fn captures_selected_model_and_preserves_catalog_bytes_opaquely() {
        let catalog =
            r#"{"models":[{"slug":"deepseek-flash","future":{"opaque":true}}],"futureRoot":[1,2]}"#;
        let root = fixture(
            "model = \"deepseek-flash\"\nmodel_catalog_json = \"models.json\"\n",
            "models.json",
            Some(catalog),
        );
        let profile = inspect_codex_profile_at(&root).unwrap();
        assert_eq!(profile.model, "deepseek-flash");
        assert_eq!(profile.catalog_bytes, catalog.as_bytes());
        assert!(!profile.hash.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn supports_nested_catalog_but_rejects_unsafe_or_missing_profile() {
        let catalog = r#"{"models":[{"id":"kimi-k2.5"}]}"#;
        let root = fixture(
            "model = \"kimi-k2.5\"\nmodel_catalog_json = \"nested/models.json\"\n",
            "nested/models.json",
            Some(catalog),
        );
        assert!(inspect_codex_profile_at(&root).is_ok());
        fs::remove_dir_all(root).unwrap();

        for path in ["../outside.json", "C:/outside.json", "./models.json"] {
            let root = fixture(
                &format!("model = \"kimi\"\nmodel_catalog_json = \"{path}\"\n"),
                "unused.json",
                None,
            );
            assert_eq!(inspect_codex_profile_at(&root), Err(ProfileError::Invalid));
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn rejects_invalid_catalog_and_model_not_present_in_catalog() {
        let root = fixture(
            "model = \"missing\"\nmodel_catalog_json = \"models.json\"\n",
            "models.json",
            Some(r#"{"models":[{"slug":"other"}]}"#),
        );
        assert_eq!(inspect_codex_profile_at(&root), Err(ProfileError::Invalid));
        fs::remove_dir_all(root).unwrap();

        let root = fixture(
            "model = \"broken\"\nmodel_catalog_json = \"models.json\"\n",
            "models.json",
            Some("not json"),
        );
        assert_eq!(inspect_codex_profile_at(&root), Err(ProfileError::Invalid));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_model_catalog_and_oversized_catalog() {
        let root = fixture(
            "model_catalog_json = \"models.json\"\n",
            "models.json",
            Some(r#"{"models":[]}"#),
        );
        assert_eq!(inspect_codex_profile_at(&root), Err(ProfileError::Missing));
        fs::remove_dir_all(root).unwrap();

        let root = fixture(
            "model = \"deepseek\"\nmodel_catalog_json = \"missing.json\"\n",
            "unused.json",
            None,
        );
        assert_eq!(inspect_codex_profile_at(&root), Err(ProfileError::Invalid));
        fs::remove_dir_all(root).unwrap();

        let root = fixture(
            "model = \"deepseek\"\nmodel_catalog_json = \"models.json\"\n",
            "models.json",
            Some("{}"),
        );
        fs::write(
            root.join(".codex/models.json"),
            vec![b' '; MAX_CODEX_CATALOG_BYTES as usize + 1],
        )
        .unwrap();
        assert_eq!(inspect_codex_profile_at(&root), Err(ProfileError::Invalid));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_catalog() {
        use std::os::unix::fs::symlink;
        let root = fixture(
            "model = \"deepseek\"\nmodel_catalog_json = \"models.json\"\n",
            "unused.json",
            None,
        );
        let outside = root.join("outside.json");
        fs::write(&outside, r#"{"models":[{"slug":"deepseek"}]}"#).unwrap();
        symlink(&outside, root.join(".codex/models.json")).unwrap();
        assert_eq!(inspect_codex_profile_at(&root), Err(ProfileError::Invalid));
        fs::remove_dir_all(root).unwrap();
    }
}
