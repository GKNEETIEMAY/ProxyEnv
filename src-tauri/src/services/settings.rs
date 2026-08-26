use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    error::{ProxyEnvError, Result},
    features::proxy::ProxyVariable,
};

use super::local_file;

const MAX_SETTINGS_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LanguagePreference {
    System,
    #[serde(rename = "zh-CN")]
    Chinese,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "ko")]
    Korean,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

fn default_proxy_variables() -> Vec<ProxyVariable> {
    vec![ProxyVariable::Http, ProxyVariable::Https]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettings {
    pub language: LanguagePreference,
    pub theme: ThemePreference,
    pub launch_at_startup: bool,
    pub silent_start: bool,
    pub close_to_tray: bool,
    #[serde(default = "default_proxy_variables")]
    pub proxy_variables: Vec<ProxyVariable>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: LanguagePreference::System,
            theme: ThemePreference::System,
            launch_at_startup: false,
            silent_start: false,
            close_to_tray: true,
            proxy_variables: default_proxy_variables(),
        }
    }
}

impl AppSettings {
    pub fn resolved_language(self) -> &'static str {
        match self.language {
            LanguagePreference::Chinese => "zh-CN",
            LanguagePreference::English => "en",
            LanguagePreference::Japanese => "ja",
            LanguagePreference::Korean => "ko",
            LanguagePreference::System => locale_code(&system_locale()),
        }
    }
}

fn locale_code(locale: &str) -> &'static str {
    let locale = locale.to_ascii_lowercase();
    if locale == "zh-cn" || locale == "zh-sg" || locale.starts_with("zh-hans") {
        "zh-CN"
    } else if locale.starts_with("ja") {
        "ja"
    } else if locale.starts_with("ko") {
        "ko"
    } else {
        "en"
    }
}

#[cfg(windows)]
fn system_locale() -> String {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;

    // Windows documents LOCALE_NAME_MAX_LENGTH as 85 characters.
    let mut buffer = [0u16; 85];
    let length = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    if length <= 1 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..length as usize - 1])
}

#[cfg(not(windows))]
fn system_locale() -> String {
    std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
}

fn settings_path() -> Result<PathBuf> {
    dirs::data_local_dir()
        .map(|path| path.join("ProxyEnv").join("settings.json"))
        .ok_or_else(|| ProxyEnvError::SettingsWrite("local data directory is unavailable".into()))
}

pub fn load() -> Result<AppSettings> {
    let path = settings_path().map_err(|error| ProxyEnvError::SettingsRead(error.to_string()))?;
    let Some(bytes) = local_file::safe_read(&path, MAX_SETTINGS_BYTES)
        .map_err(|error| ProxyEnvError::SettingsRead(error.to_string()))?
    else {
        return Ok(AppSettings::default());
    };
    decode_settings(&bytes).map_err(ProxyEnvError::SettingsRead)
}

pub fn save(settings: &AppSettings) -> Result<()> {
    validate_settings(settings).map_err(ProxyEnvError::SettingsWrite)?;
    let destination = settings_path()?;
    let directory = destination
        .parent()
        .ok_or_else(|| ProxyEnvError::SettingsWrite("settings directory is unavailable".into()))?;
    fs::create_dir_all(directory)
        .map_err(|error| ProxyEnvError::SettingsWrite(error.to_string()))?;
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|error| ProxyEnvError::SettingsWrite(error.to_string()))?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(ProxyEnvError::SettingsWrite(
            "settings exceed the size limit".into(),
        ));
    }
    local_file::atomic_write(&destination, &bytes, "settings")
        .map_err(|error| ProxyEnvError::SettingsWrite(error.to_string()))
}

fn decode_settings(bytes: &[u8]) -> std::result::Result<AppSettings, String> {
    if bytes.len() as u64 > MAX_SETTINGS_BYTES {
        return Err("settings exceed the size limit".into());
    }
    let mut settings: AppSettings =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if settings.proxy_variables.is_empty() {
        settings.proxy_variables = default_proxy_variables();
    }
    validate_settings(&settings)?;
    Ok(settings)
}

fn validate_settings(settings: &AppSettings) -> std::result::Result<(), String> {
    for (index, variable) in settings.proxy_variables.iter().enumerate() {
        if settings.proxy_variables[..index].contains(variable) {
            return Err("settings contain duplicate proxy variables".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe_and_non_intrusive() {
        let settings = AppSettings::default();
        assert!(!settings.launch_at_startup);
        assert!(!settings.silent_start);
        assert!(settings.close_to_tray);
        assert_eq!(
            settings.proxy_variables,
            vec![ProxyVariable::Http, ProxyVariable::Https]
        );
    }

    #[test]
    fn unsupported_traditional_chinese_locales_fall_back_to_english() {
        assert_eq!(locale_code("zh-TW"), "en");
        assert_eq!(locale_code("zh-HK"), "en");
        assert_eq!(locale_code("zh-Hans-CN"), "zh-CN");
    }

    #[test]
    fn settings_schema_rejects_unknown_fields_and_duplicate_variables() {
        let unknown = br#"{
            "language":"system","theme":"system","launchAtStartup":false,
            "silentStart":false,"closeToTray":true,"proxyVariables":["http"],
            "unexpected":true
        }"#;
        assert!(decode_settings(unknown).is_err());

        let duplicates = br#"{
            "language":"system","theme":"system","launchAtStartup":false,
            "silentStart":false,"closeToTray":true,"proxyVariables":["http","http"]
        }"#;
        assert!(decode_settings(duplicates).is_err());
    }

    #[test]
    fn oversized_settings_are_rejected_before_parsing() {
        assert!(decode_settings(&vec![b' '; MAX_SETTINGS_BYTES as usize + 1]).is_err());
    }
}
