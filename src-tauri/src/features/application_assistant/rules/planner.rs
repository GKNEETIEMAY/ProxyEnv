use std::{fs, path::PathBuf};

use crate::features::proxy::{ProxyEndpoint, ProxyProtocol};

use super::{
    loader::load_bundled,
    matcher::{match_executable, RuleMatchResult},
    schema::{ApplicationRule, RuleConfigFormat, RuleValueKind},
};

const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ConfigValue {
    String(String),
    Number(serde_json::Number),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RuleInspectionState {
    Ready,
    AlreadyCurrent,
    NoMatchingRule,
    AmbiguousRule,
    ActiveProxyMissing,
    UnsupportedProxyProtocol,
    FileMissing,
    ReadFailed,
    ParseFailed,
    FieldMissing,
    UnsupportedFieldValue,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RuleInspection {
    pub state: RuleInspectionState,
    pub rule_id: Option<String>,
}

pub fn inspect_application(
    executable_path: &std::path::Path,
    endpoint: Option<&ProxyEndpoint>,
) -> RuleInspection {
    let catalog = load_bundled();
    match match_executable(executable_path, &catalog.rules) {
        RuleMatchResult::None => inspection_state(RuleInspectionState::NoMatchingRule, None),
        RuleMatchResult::Ambiguous(_) => inspection_state(RuleInspectionState::AmbiguousRule, None),
        RuleMatchResult::Exact(rule) => {
            let Some(endpoint) = endpoint else {
                return inspection_state(
                    RuleInspectionState::ActiveProxyMissing,
                    Some(rule.id.clone()),
                );
            };
            inspect_rule(rule, endpoint, &EnvironmentRoots::from_process())
        }
    }
}

pub(super) fn inspect_rule(
    rule: &ApplicationRule,
    endpoint: &ProxyEndpoint,
    roots: &EnvironmentRoots,
) -> RuleInspection {
    let new_value = match value_for_endpoint(rule.config.value_kind, endpoint) {
        Ok(value) => value,
        Err(state) => return inspection_state(state, Some(rule.id.clone())),
    };
    let target_file = match locate_config(rule, roots) {
        Ok(path) => path,
        Err(state) => return inspection_state(state, Some(rule.id.clone())),
    };
    let content = match read_config(&target_file) {
        Ok(content) => content,
        Err(state) => {
            return inspection_state(state, Some(rule.id.clone()));
        }
    };
    let old_value = match inspect_field(rule, &content) {
        Ok(value) => value,
        Err(state) => {
            return inspection_state(state, Some(rule.id.clone()));
        }
    };
    if old_value == new_value {
        return inspection_state(RuleInspectionState::AlreadyCurrent, Some(rule.id.clone()));
    }
    RuleInspection {
        state: RuleInspectionState::Ready,
        rule_id: Some(rule.id.clone()),
    }
}

fn inspection_state(state: RuleInspectionState, rule_id: Option<String>) -> RuleInspection {
    RuleInspection { state, rule_id }
}

#[derive(Debug, Clone, Default)]
pub(super) struct EnvironmentRoots {
    app_data: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    user_profile: Option<PathBuf>,
}

impl EnvironmentRoots {
    pub(super) fn from_process() -> Self {
        Self {
            app_data: std::env::var_os("APPDATA").map(PathBuf::from),
            local_app_data: std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
            user_profile: std::env::var_os("USERPROFILE").map(PathBuf::from),
        }
    }

    pub(super) fn expand(&self, declared_path: &str) -> Option<PathBuf> {
        for (token, root) in [
            ("%APPDATA%", self.app_data.as_ref()),
            ("%LOCALAPPDATA%", self.local_app_data.as_ref()),
            ("%USERPROFILE%", self.user_profile.as_ref()),
        ] {
            let Some(prefix) = declared_path.get(..token.len()) else {
                continue;
            };
            if prefix.eq_ignore_ascii_case(token) {
                let root = root?.as_path();
                if !root.is_absolute() {
                    return None;
                }
                let relative = declared_path[token.len()..].trim_start_matches(['\\', '/']);
                return Some(root.join(relative.replace('\\', "/")));
            }
        }
        None
    }
}

fn locate_config(
    rule: &ApplicationRule,
    roots: &EnvironmentRoots,
) -> Result<PathBuf, RuleInspectionState> {
    for declared_path in &rule.config.paths {
        let Some(path) = roots.expand(declared_path) else {
            continue;
        };
        match fs::symlink_metadata(&path) {
            Ok(metadata) if is_safe_regular_file(&metadata) => return Ok(path),
            Ok(_) => return Err(RuleInspectionState::ReadFailed),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(RuleInspectionState::ReadFailed),
        }
    }
    Err(RuleInspectionState::FileMissing)
}

pub(super) fn is_safe_regular_file(metadata: &fs::Metadata) -> bool {
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return false;
        }
    }
    true
}

pub(super) fn read_config(path: &std::path::Path) -> Result<String, RuleInspectionState> {
    let metadata = fs::metadata(path).map_err(|_| RuleInspectionState::ReadFailed)?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(RuleInspectionState::ReadFailed);
    }
    fs::read_to_string(path).map_err(|_| RuleInspectionState::ReadFailed)
}

pub(super) fn inspect_field(
    rule: &ApplicationRule,
    content: &str,
) -> Result<ConfigValue, RuleInspectionState> {
    match rule.config.format {
        RuleConfigFormat::Json => {
            let value: serde_json::Value =
                serde_json::from_str(content).map_err(|_| RuleInspectionState::ParseFailed)?;
            let value = inspect_json_path(&value, &rule.config.field)
                .ok_or(RuleInspectionState::FieldMissing)?;
            config_value_from_json(value)
        }
        RuleConfigFormat::Yaml => {
            let value: serde_yaml::Value =
                serde_yaml::from_str(content).map_err(|_| RuleInspectionState::ParseFailed)?;
            let value = inspect_yaml_path(&value, &rule.config.field)
                .ok_or(RuleInspectionState::FieldMissing)?;
            let value =
                serde_json::to_value(value).map_err(|_| RuleInspectionState::ParseFailed)?;
            config_value_from_json(&value)
        }
        RuleConfigFormat::Toml => {
            let value: toml::Value =
                toml::from_str(content).map_err(|_| RuleInspectionState::ParseFailed)?;
            let value = inspect_toml_path(&value, &rule.config.field)
                .ok_or(RuleInspectionState::FieldMissing)?;
            let value =
                serde_json::to_value(value).map_err(|_| RuleInspectionState::ParseFailed)?;
            config_value_from_json(&value)
        }
        RuleConfigFormat::Ini => inspect_ini_field(content, &rule.config.field),
    }
}

fn inspect_json_path<'a>(
    root: &'a serde_json::Value,
    field: &[String],
) -> Option<&'a serde_json::Value> {
    field
        .iter()
        .try_fold(root, |value, segment| value.get(segment))
}

fn inspect_yaml_path<'a>(
    root: &'a serde_yaml::Value,
    field: &[String],
) -> Option<&'a serde_yaml::Value> {
    field.iter().try_fold(root, |value, segment| {
        value.get(serde_yaml::Value::String(segment.clone()))
    })
}

fn inspect_toml_path<'a>(root: &'a toml::Value, field: &[String]) -> Option<&'a toml::Value> {
    field
        .iter()
        .try_fold(root, |value, segment| value.get(segment))
}

fn inspect_ini_field(content: &str, field: &[String]) -> Result<ConfigValue, RuleInspectionState> {
    if field.len() != 2 {
        return Err(RuleInspectionState::ParseFailed);
    }
    let expected_section = &field[0];
    let expected_key = &field[1];
    let mut current_section = None::<String>;
    let mut matched_value = None::<String>;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with([';', '#']) {
            continue;
        }
        if line.starts_with('[') {
            if !line.ends_with(']') || line.len() < 3 {
                return Err(RuleInspectionState::ParseFailed);
            }
            current_section = Some(line[1..line.len() - 1].trim().to_owned());
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(RuleInspectionState::ParseFailed);
        };
        let in_section = current_section
            .as_deref()
            .is_some_and(|section| section.eq_ignore_ascii_case(expected_section));
        if in_section && key.trim().eq_ignore_ascii_case(expected_key) {
            if matched_value.is_some() {
                return Err(RuleInspectionState::ParseFailed);
            }
            matched_value = Some(normalize_ini_string(value.trim()));
        }
    }
    matched_value
        .map(ConfigValue::String)
        .ok_or(RuleInspectionState::FieldMissing)
}

fn normalize_ini_string(value: &str) -> String {
    if value.len() >= 2 {
        let first = value.as_bytes()[0];
        let last = value.as_bytes()[value.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return value[1..value.len() - 1].to_owned();
        }
    }
    value.to_owned()
}

fn config_value_from_json(value: &serde_json::Value) -> Result<ConfigValue, RuleInspectionState> {
    match value {
        serde_json::Value::String(value) => Ok(ConfigValue::String(value.clone())),
        serde_json::Value::Number(value) => Ok(ConfigValue::Number(value.clone())),
        serde_json::Value::Bool(value) => Ok(ConfigValue::Boolean(*value)),
        serde_json::Value::Null => Ok(ConfigValue::Null),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err(RuleInspectionState::UnsupportedFieldValue)
        }
    }
}

fn value_for_endpoint(
    kind: RuleValueKind,
    endpoint: &ProxyEndpoint,
) -> Result<ConfigValue, RuleInspectionState> {
    let host = format_host(endpoint.host.trim());
    if host.is_empty() || endpoint.port == 0 || matches!(endpoint.protocol, ProxyProtocol::Unknown)
    {
        return Err(RuleInspectionState::UnsupportedProxyProtocol);
    }
    match kind {
        RuleValueKind::HttpProxyUrl
            if matches!(
                endpoint.protocol,
                ProxyProtocol::Http | ProxyProtocol::Mixed
            ) =>
        {
            Ok(ConfigValue::String(format!(
                "http://{host}:{}",
                endpoint.port
            )))
        }
        RuleValueKind::Socks5ProxyUrl
            if matches!(
                endpoint.protocol,
                ProxyProtocol::Socks5 | ProxyProtocol::Mixed
            ) =>
        {
            Ok(ConfigValue::String(format!(
                "socks5://{host}:{}",
                endpoint.port
            )))
        }
        RuleValueKind::Host => Ok(ConfigValue::String(endpoint.host.trim().to_owned())),
        RuleValueKind::Port => Ok(ConfigValue::Number(serde_json::Number::from(endpoint.port))),
        RuleValueKind::HttpProxyUrl | RuleValueKind::Socks5ProxyUrl => {
            Err(RuleInspectionState::UnsupportedProxyProtocol)
        }
    }
}

fn format_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::features::application_assistant::rules::schema::{RuleConfig, RuleMatcher};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "proxyenv-rule-planner-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn write(&self, file_name: &str, content: &str) -> PathBuf {
            let path = self.0.join(file_name);
            fs::write(&path, content).unwrap();
            path
        }

        fn roots(&self) -> EnvironmentRoots {
            EnvironmentRoots {
                app_data: Some(self.0.clone()),
                local_app_data: None,
                user_profile: None,
            }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn rule(
        format: RuleConfigFormat,
        file_name: &str,
        field: &[&str],
        value_kind: RuleValueKind,
    ) -> ApplicationRule {
        ApplicationRule {
            schema_version: 1,
            id: "example-app".into(),
            display_name: "Example App".into(),
            matcher: RuleMatcher {
                process_names: vec!["example.exe".into()],
            },
            config: RuleConfig {
                format,
                paths: vec![format!(r"%APPDATA%\{file_name}")],
                field: field.iter().map(|value| (*value).to_owned()).collect(),
                value_kind,
            },
            restart_required: true,
        }
    }

    fn endpoint(protocol: ProxyProtocol) -> ProxyEndpoint {
        ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: 7897,
            protocol,
        }
    }

    #[test]
    fn detects_a_different_value_without_writing_the_config() {
        let directory = TestDirectory::new();
        let target = directory.write(
            "settings.json",
            r#"{"network":{"proxy":"http://127.0.0.1:7890"}}"#,
        );
        let rule = rule(
            RuleConfigFormat::Json,
            "settings.json",
            &["network", "proxy"],
            RuleValueKind::HttpProxyUrl,
        );

        let preview = inspect_rule(&rule, &endpoint(ProxyProtocol::Mixed), &directory.roots());

        assert_eq!(preview.state, RuleInspectionState::Ready);
        assert_eq!(preview.rule_id.as_deref(), Some(rule.id.as_str()));
        assert_eq!(
            fs::read_to_string(target).unwrap(),
            r#"{"network":{"proxy":"http://127.0.0.1:7890"}}"#
        );
    }

    #[test]
    fn reports_a_missing_declared_file_without_scanning() {
        let directory = TestDirectory::new();
        let rule = rule(
            RuleConfigFormat::Json,
            "missing.json",
            &["proxy"],
            RuleValueKind::HttpProxyUrl,
        );

        let preview = inspect_rule(&rule, &endpoint(ProxyProtocol::Http), &directory.roots());

        assert_eq!(preview.state, RuleInspectionState::FileMissing);
    }

    #[test]
    fn stops_when_the_declared_field_is_missing() {
        let directory = TestDirectory::new();
        directory.write("settings.json", r#"{"network":{}}"#);
        let rule = rule(
            RuleConfigFormat::Json,
            "settings.json",
            &["network", "proxy"],
            RuleValueKind::HttpProxyUrl,
        );

        let preview = inspect_rule(&rule, &endpoint(ProxyProtocol::Http), &directory.roots());

        assert_eq!(preview.state, RuleInspectionState::FieldMissing);
    }

    #[test]
    fn stops_on_parse_failure() {
        let directory = TestDirectory::new();
        directory.write("settings.json", "{not valid json");
        let rule = rule(
            RuleConfigFormat::Json,
            "settings.json",
            &["proxy"],
            RuleValueKind::HttpProxyUrl,
        );

        let preview = inspect_rule(&rule, &endpoint(ProxyProtocol::Http), &directory.roots());

        assert_eq!(preview.state, RuleInspectionState::ParseFailed);
    }

    #[test]
    fn reports_when_the_existing_value_already_matches() {
        let directory = TestDirectory::new();
        directory.write("settings.json", r#"{"proxy":"http://127.0.0.1:7897"}"#);
        let rule = rule(
            RuleConfigFormat::Json,
            "settings.json",
            &["proxy"],
            RuleValueKind::HttpProxyUrl,
        );

        let preview = inspect_rule(&rule, &endpoint(ProxyProtocol::Mixed), &directory.roots());

        assert_eq!(preview.state, RuleInspectionState::AlreadyCurrent);
    }

    #[test]
    fn inspects_yaml_toml_and_ini_structurally() {
        let directory = TestDirectory::new();
        let cases = [
            (
                RuleConfigFormat::Yaml,
                "settings.yaml",
                "network:\n  proxy: http://127.0.0.1:7890\n",
                vec!["network", "proxy"],
            ),
            (
                RuleConfigFormat::Toml,
                "settings.toml",
                "[network]\nproxy = \"http://127.0.0.1:7890\"\n",
                vec!["network", "proxy"],
            ),
            (
                RuleConfigFormat::Ini,
                "settings.ini",
                "[network]\nproxy = http://127.0.0.1:7890\n",
                vec!["network", "proxy"],
            ),
        ];

        for (format, file_name, content, field) in cases {
            directory.write(file_name, content);
            let rule = rule(format, file_name, &field, RuleValueKind::HttpProxyUrl);
            let preview = inspect_rule(&rule, &endpoint(ProxyProtocol::Http), &directory.roots());
            assert_eq!(preview.state, RuleInspectionState::Ready, "{file_name}");
            assert_eq!(
                inspect_field(&rule, content).unwrap(),
                ConfigValue::String("http://127.0.0.1:7890".into()),
                "{file_name}"
            );
        }
    }

    #[test]
    fn generates_only_fixed_endpoint_value_kinds() {
        let mixed = endpoint(ProxyProtocol::Mixed);
        assert_eq!(
            value_for_endpoint(RuleValueKind::Socks5ProxyUrl, &mixed),
            Ok(ConfigValue::String("socks5://127.0.0.1:7897".into()))
        );
        assert_eq!(
            value_for_endpoint(RuleValueKind::Host, &mixed),
            Ok(ConfigValue::String("127.0.0.1".into()))
        );
        assert_eq!(
            value_for_endpoint(RuleValueKind::Port, &mixed),
            Ok(ConfigValue::Number(serde_json::Number::from(7897)))
        );
        assert_eq!(
            value_for_endpoint(
                RuleValueKind::HttpProxyUrl,
                &endpoint(ProxyProtocol::Socks5)
            ),
            Err(RuleInspectionState::UnsupportedProxyProtocol)
        );
    }

    #[test]
    fn expands_only_absolute_allowlisted_environment_roots() {
        let roots = EnvironmentRoots {
            app_data: Some(PathBuf::from("relative")),
            local_app_data: None,
            user_profile: None,
        };
        assert!(roots.expand(r"%APPDATA%\settings.json").is_none());
        assert!(roots.expand(r"%TEMP%\settings.json").is_none());
    }
}
