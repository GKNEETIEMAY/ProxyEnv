use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ApplicationRule {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    #[serde(rename = "match")]
    pub matcher: RuleMatcher,
    pub config: RuleConfig,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RuleMatcher {
    pub process_names: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RuleConfig {
    pub format: RuleConfigFormat,
    pub paths: Vec<String>,
    pub field: Vec<String>,
    pub value_kind: RuleValueKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleConfigFormat {
    Json,
    Yaml,
    Toml,
    Ini,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleValueKind {
    HttpProxyUrl,
    Socks5ProxyUrl,
    Host,
    Port,
}

#[derive(Debug, Clone, Eq, Error, PartialEq)]
pub enum RuleValidationError {
    #[error("unsupported rule schema version: {0}")]
    UnsupportedSchemaVersion(u32),
    #[error("rule id must use lowercase ASCII letters, numbers, and hyphens")]
    InvalidId,
    #[error("rule display name cannot be empty")]
    EmptyDisplayName,
    #[error("rule must declare at least one exact process name")]
    EmptyProcessNames,
    #[error("invalid exact process name: {0}")]
    InvalidProcessName(String),
    #[error("rule must declare at least one fixed config path")]
    EmptyPaths,
    #[error("invalid rule config path: {0}")]
    InvalidPath(String),
    #[error("rule config field path cannot be empty")]
    EmptyField,
    #[error("INI rules must declare field as exactly [section, key]")]
    InvalidIniSelector,
    #[error("invalid rule config field segment: {0}")]
    InvalidFieldSegment(String),
}

impl ApplicationRule {
    pub fn validate(&self) -> Result<(), RuleValidationError> {
        if self.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(RuleValidationError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        if !is_valid_id(&self.id) {
            return Err(RuleValidationError::InvalidId);
        }
        if self.display_name.trim().is_empty() {
            return Err(RuleValidationError::EmptyDisplayName);
        }
        if self.matcher.process_names.is_empty() {
            return Err(RuleValidationError::EmptyProcessNames);
        }
        for process_name in &self.matcher.process_names {
            if !is_valid_process_name(process_name) {
                return Err(RuleValidationError::InvalidProcessName(
                    process_name.clone(),
                ));
            }
        }
        if self.config.paths.is_empty() {
            return Err(RuleValidationError::EmptyPaths);
        }
        for path in &self.config.paths {
            if !is_valid_config_path(path) {
                return Err(RuleValidationError::InvalidPath(path.clone()));
            }
        }
        if self.config.field.is_empty() {
            return Err(RuleValidationError::EmptyField);
        }
        if matches!(self.config.format, RuleConfigFormat::Ini) && self.config.field.len() != 2 {
            return Err(RuleValidationError::InvalidIniSelector);
        }
        for segment in &self.config.field {
            if !is_valid_field_segment(segment) {
                return Err(RuleValidationError::InvalidFieldSegment(segment.clone()));
            }
        }
        Ok(())
    }
}

fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_valid_process_name(process_name: &str) -> bool {
    let trimmed = process_name.trim();
    !trimmed.is_empty()
        && trimmed == process_name
        && !trimmed.contains(['\\', '/', ':', '*', '?', '[', ']', '%', '\0'])
        && trimmed.to_ascii_lowercase().ends_with(".exe")
}

fn is_valid_config_path(path: &str) -> bool {
    const ALLOWED_PREFIXES: [&str; 3] = ["%APPDATA%", "%LOCALAPPDATA%", "%USERPROFILE%"];

    let Some(prefix) = ALLOWED_PREFIXES.iter().find(|prefix| {
        path.len() > prefix.len()
            && path
                .get(..prefix.len())
                .is_some_and(|value| value.eq_ignore_ascii_case(prefix))
    }) else {
        return false;
    };
    let remainder = &path[prefix.len()..];
    if !remainder.starts_with(['\\', '/'])
        || remainder.ends_with(['\\', '/'])
        || remainder.contains(['%', ':', '*', '?', '[', ']', '\0'])
    {
        return false;
    }
    remainder
        .split(['\\', '/'])
        .skip(1)
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn is_valid_field_segment(segment: &str) -> bool {
    let trimmed = segment.trim();
    !trimmed.is_empty()
        && trimmed == segment
        && !trimmed.contains(['\\', '/', '\0'])
        && trimmed != "."
        && trimmed != ".."
}
