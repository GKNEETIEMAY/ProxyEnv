use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::features::proxy::{ProxyEndpoint, ProxyProtocol};

use super::{
    loader::load_bundled,
    matcher::{match_executable, RuleMatchResult},
    schema::{ApplicationRule, RuleConfigFormat, RuleValueKind},
};

const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ConfigValue {
    String(String),
    Number(serde_json::Number),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleChangePlan {
    pub rule_id: String,
    pub target_file: PathBuf,
    pub field_path: Vec<String>,
    pub old_value: ConfigValue,
    pub new_value: ConfigValue,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RulePreviewState {
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleChangePreview {
    pub state: RulePreviewState,
    pub rule_id: Option<String>,
    pub target_file: Option<PathBuf>,
    pub plan: Option<RuleChangePlan>,
}

pub fn preview_application(
    executable_path: &std::path::Path,
    endpoint: Option<&ProxyEndpoint>,
) -> RuleChangePreview {
    let catalog = load_bundled();
    match match_executable(executable_path, &catalog.rules) {
        RuleMatchResult::None => preview_state(RulePreviewState::NoMatchingRule, None, None),
        RuleMatchResult::Ambiguous(_) => preview_state(RulePreviewState::AmbiguousRule, None, None),
        RuleMatchResult::Exact(rule) => {
            let Some(endpoint) = endpoint else {
                return preview_state(
                    RulePreviewState::ActiveProxyMissing,
                    Some(rule.id.clone()),
                    None,
                );
            };
            preview_rule(rule, endpoint, &EnvironmentRoots::from_process())
        }
    }
}

pub(super) fn preview_rule(
    rule: &ApplicationRule,
    endpoint: &ProxyEndpoint,
    roots: &EnvironmentRoots,
) -> RuleChangePreview {
    let new_value = match value_for_endpoint(rule.config.value_kind, endpoint) {
        Ok(value) => value,
        Err(state) => return preview_state(state, Some(rule.id.clone()), None),
    };
    let target_file = match locate_config(rule, roots) {
        Ok(path) => path,
        Err(state) => return preview_state(state, Some(rule.id.clone()), None),
    };
    let content = match read_config(&target_file) {
        Ok(content) => content,
        Err(state) => {
            return preview_state(state, Some(rule.id.clone()), Some(target_file));
        }
    };
    let old_value = match inspect_field(rule, &content) {
        Ok(value) => value,
        Err(state) => {
            return preview_state(state, Some(rule.id.clone()), Some(target_file));
        }
    };
    if old_value == new_value {
        return preview_state(
            RulePreviewState::AlreadyCurrent,
            Some(rule.id.clone()),
            Some(target_file),
        );
    }
    RuleChangePreview {
        state: RulePreviewState::Ready,
        rule_id: Some(rule.id.clone()),
        target_file: Some(target_file.clone()),
        plan: Some(RuleChangePlan {
            rule_id: rule.id.clone(),
            target_file,
            field_path: rule.config.field.clone(),
            old_value,
            new_value,
            restart_required: rule.restart_required,
        }),
    }
}

fn preview_state(
    state: RulePreviewState,
    rule_id: Option<String>,
    target_file: Option<PathBuf>,
) -> RuleChangePreview {
    RuleChangePreview {
        state,
        rule_id,
        target_file,
        plan: None,
    }
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

    #[cfg(test)]
    pub(super) fn with_app_data(path: PathBuf) -> Self {
        Self {
            app_data: Some(path),
            local_app_data: None,
            user_profile: None,
        }
    }
}

fn locate_config(
    rule: &ApplicationRule,
    roots: &EnvironmentRoots,
) -> Result<PathBuf, RulePreviewState> {
    for declared_path in &rule.config.paths {
        let Some(path) = roots.expand(declared_path) else {
            continue;
        };
        match fs::symlink_metadata(&path) {
            Ok(metadata) if is_safe_regular_file(&metadata) => return Ok(path),
            Ok(_) => return Err(RulePreviewState::ReadFailed),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(RulePreviewState::ReadFailed),
        }
    }
    Err(RulePreviewState::FileMissing)
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

pub(super) fn validate_target_for_rule(
    rule: &ApplicationRule,
    target: &std::path::Path,
    roots: &EnvironmentRoots,
) -> bool {
    let Ok(metadata) = fs::symlink_metadata(target) else {
        return false;
    };
    if !is_safe_regular_file(&metadata) {
        return false;
    }
    let Ok(canonical_target) = fs::canonicalize(target) else {
        return false;
    };
    rule.config.paths.iter().any(|declared_path| {
        let Some(expanded) = roots.expand(declared_path) else {
            return false;
        };
        let Some(root) = root_for_declared_path(declared_path, roots) else {
            return false;
        };
        let (Ok(canonical_expanded), Ok(canonical_root)) =
            (fs::canonicalize(expanded), fs::canonicalize(root))
        else {
            return false;
        };
        canonical_expanded == canonical_target && canonical_target.starts_with(canonical_root)
    })
}

fn root_for_declared_path<'a>(
    declared_path: &str,
    roots: &'a EnvironmentRoots,
) -> Option<&'a std::path::Path> {
    for (token, root) in [
        ("%APPDATA%", roots.app_data.as_deref()),
        ("%LOCALAPPDATA%", roots.local_app_data.as_deref()),
        ("%USERPROFILE%", roots.user_profile.as_deref()),
    ] {
        if declared_path
            .get(..token.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(token))
        {
            return root;
        }
    }
    None
}

pub(super) fn read_config(path: &std::path::Path) -> Result<String, RulePreviewState> {
    let metadata = fs::metadata(path).map_err(|_| RulePreviewState::ReadFailed)?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(RulePreviewState::ReadFailed);
    }
    fs::read_to_string(path).map_err(|_| RulePreviewState::ReadFailed)
}

pub(super) fn inspect_field(
    rule: &ApplicationRule,
    content: &str,
) -> Result<ConfigValue, RulePreviewState> {
    match rule.config.format {
        RuleConfigFormat::Json => {
            let value: serde_json::Value =
                serde_json::from_str(content).map_err(|_| RulePreviewState::ParseFailed)?;
            let value = inspect_json_path(&value, &rule.config.field)
                .ok_or(RulePreviewState::FieldMissing)?;
            config_value_from_json(value)
        }
        RuleConfigFormat::Yaml => {
            let value: serde_yaml::Value =
                serde_yaml::from_str(content).map_err(|_| RulePreviewState::ParseFailed)?;
            let value = inspect_yaml_path(&value, &rule.config.field)
                .ok_or(RulePreviewState::FieldMissing)?;
            let value = serde_json::to_value(value).map_err(|_| RulePreviewState::ParseFailed)?;
            config_value_from_json(&value)
        }
        RuleConfigFormat::Toml => {
            let value: toml::Value =
                toml::from_str(content).map_err(|_| RulePreviewState::ParseFailed)?;
            let value = inspect_toml_path(&value, &rule.config.field)
                .ok_or(RulePreviewState::FieldMissing)?;
            let value = serde_json::to_value(value).map_err(|_| RulePreviewState::ParseFailed)?;
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

fn inspect_ini_field(content: &str, field: &[String]) -> Result<ConfigValue, RulePreviewState> {
    if field.len() != 2 {
        return Err(RulePreviewState::ParseFailed);
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
                return Err(RulePreviewState::ParseFailed);
            }
            current_section = Some(line[1..line.len() - 1].trim().to_owned());
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(RulePreviewState::ParseFailed);
        };
        let in_section = current_section
            .as_deref()
            .is_some_and(|section| section.eq_ignore_ascii_case(expected_section));
        if in_section && key.trim().eq_ignore_ascii_case(expected_key) {
            if matched_value.is_some() {
                return Err(RulePreviewState::ParseFailed);
            }
            matched_value = Some(normalize_ini_string(value.trim()));
        }
    }
    matched_value
        .map(ConfigValue::String)
        .ok_or(RulePreviewState::FieldMissing)
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

fn config_value_from_json(value: &serde_json::Value) -> Result<ConfigValue, RulePreviewState> {
    match value {
        serde_json::Value::String(value) => Ok(ConfigValue::String(value.clone())),
        serde_json::Value::Number(value) => Ok(ConfigValue::Number(value.clone())),
        serde_json::Value::Bool(value) => Ok(ConfigValue::Boolean(*value)),
        serde_json::Value::Null => Ok(ConfigValue::Null),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err(RulePreviewState::UnsupportedFieldValue)
        }
    }
}

fn value_for_endpoint(
    kind: RuleValueKind,
    endpoint: &ProxyEndpoint,
) -> Result<ConfigValue, RulePreviewState> {
    let host = format_host(endpoint.host.trim());
    if host.is_empty() || endpoint.port == 0 || matches!(endpoint.protocol, ProxyProtocol::Unknown)
    {
        return Err(RulePreviewState::UnsupportedProxyProtocol);
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
            Err(RulePreviewState::UnsupportedProxyProtocol)
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
    fn builds_a_plan_when_the_field_exists_and_differs() {
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

        let preview = preview_rule(&rule, &endpoint(ProxyProtocol::Mixed), &directory.roots());

        assert_eq!(preview.state, RulePreviewState::Ready);
        let plan = preview.plan.unwrap();
        assert_eq!(plan.target_file, target);
        assert_eq!(
            plan.old_value,
            ConfigValue::String("http://127.0.0.1:7890".into())
        );
        assert_eq!(
            plan.new_value,
            ConfigValue::String("http://127.0.0.1:7897".into())
        );
        assert!(plan.restart_required);
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

        let preview = preview_rule(&rule, &endpoint(ProxyProtocol::Http), &directory.roots());

        assert_eq!(preview.state, RulePreviewState::FileMissing);
        assert!(preview.target_file.is_none());
        assert!(preview.plan.is_none());
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

        let preview = preview_rule(&rule, &endpoint(ProxyProtocol::Http), &directory.roots());

        assert_eq!(preview.state, RulePreviewState::FieldMissing);
        assert!(preview.plan.is_none());
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

        let preview = preview_rule(&rule, &endpoint(ProxyProtocol::Http), &directory.roots());

        assert_eq!(preview.state, RulePreviewState::ParseFailed);
        assert!(preview.plan.is_none());
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

        let preview = preview_rule(&rule, &endpoint(ProxyProtocol::Mixed), &directory.roots());

        assert_eq!(preview.state, RulePreviewState::AlreadyCurrent);
        assert!(preview.plan.is_none());
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
            let preview = preview_rule(&rule, &endpoint(ProxyProtocol::Http), &directory.roots());
            assert_eq!(preview.state, RulePreviewState::Ready, "{file_name}");
            assert_eq!(
                preview.plan.unwrap().old_value,
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
            Err(RulePreviewState::UnsupportedProxyProtocol)
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
