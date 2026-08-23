use std::{fs, ops::Range, path::Path};

use chrono::Utc;
use serde::Serialize;

use crate::features::proxy::ProxyEndpoint;

use super::{
    backup::{RuleBackup, RuleBackupStore},
    file_io::{atomic_write, operation_id, sha256_hex},
    loader::load_bundled,
    matcher::{match_executable, RuleMatchResult},
    planner::{
        inspect_field, preview_rule, read_config, validate_target_for_rule, ConfigValue,
        EnvironmentRoots, RuleChangePlan, RulePreviewState,
    },
    schema::{ApplicationRule, RuleConfigFormat},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleApplyState {
    Applied,
    ConfirmationRequired,
    Conflict,
    PreviewUnavailable,
    BackupFailed,
    WriteFailed,
    VerificationFailed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleApplyResult {
    pub state: RuleApplyState,
    pub backup: Option<RuleBackup>,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleRestoreState {
    Restored,
    ConfirmationRequired,
    BackupMissing,
    Conflict,
    WriteFailed,
    VerificationFailed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleRestoreResult {
    pub state: RuleRestoreState,
    pub backup_id: Option<String>,
    pub restart_required: bool,
}

pub fn apply_application(
    executable_path: &Path,
    endpoint: Option<&ProxyEndpoint>,
    expected_plan: &RuleChangePlan,
    confirmed: bool,
) -> RuleApplyResult {
    if !confirmed {
        return apply_result(RuleApplyState::ConfirmationRequired, None, false);
    }
    let Some(endpoint) = endpoint else {
        return apply_result(RuleApplyState::PreviewUnavailable, None, false);
    };
    let catalog = load_bundled();
    let rule = match match_executable(executable_path, &catalog.rules) {
        RuleMatchResult::Exact(rule) => rule,
        RuleMatchResult::Ambiguous(_) => {
            return apply_result(RuleApplyState::Conflict, None, false);
        }
        RuleMatchResult::None => {
            return apply_result(RuleApplyState::PreviewUnavailable, None, false);
        }
    };
    let Some(store) = RuleBackupStore::production() else {
        return apply_result(RuleApplyState::BackupFailed, None, rule.restart_required);
    };
    apply_rule_with_store(
        rule,
        endpoint,
        expected_plan,
        &EnvironmentRoots::from_process(),
        &store,
        true,
    )
}

pub fn restore_rule_change(backup_id: &str, confirmed: bool) -> RuleRestoreResult {
    if !confirmed {
        return restore_result(
            RuleRestoreState::ConfirmationRequired,
            Some(backup_id.to_owned()),
            false,
        );
    }
    let Some(store) = RuleBackupStore::production() else {
        return restore_result(RuleRestoreState::BackupMissing, None, false);
    };
    let Ok(backup) = store.load(backup_id) else {
        return restore_result(RuleRestoreState::BackupMissing, None, false);
    };
    let catalog = load_bundled();
    let mut matching_rules = catalog
        .rules
        .iter()
        .filter(|rule| rule.id == backup.rule_id);
    let Some(rule) = matching_rules.next() else {
        return restore_result(RuleRestoreState::Conflict, Some(backup.id), false);
    };
    if matching_rules.next().is_some() {
        return restore_result(RuleRestoreState::Conflict, Some(backup.id), false);
    }
    restore_rule_with_store(rule, &backup, &EnvironmentRoots::from_process(), true)
}

fn apply_rule_with_store(
    rule: &ApplicationRule,
    endpoint: &ProxyEndpoint,
    expected_plan: &RuleChangePlan,
    roots: &EnvironmentRoots,
    store: &RuleBackupStore,
    confirmed: bool,
) -> RuleApplyResult {
    if !confirmed {
        return apply_result(
            RuleApplyState::ConfirmationRequired,
            None,
            rule.restart_required,
        );
    }
    let preview = preview_rule(rule, endpoint, roots);
    if !matches!(preview.state, RulePreviewState::Ready) {
        return apply_result(RuleApplyState::Conflict, None, rule.restart_required);
    }
    let Some(current_plan) = preview.plan else {
        return apply_result(RuleApplyState::Conflict, None, rule.restart_required);
    };
    if &current_plan != expected_plan
        || !validate_target_for_rule(rule, &current_plan.target_file, roots)
    {
        return apply_result(RuleApplyState::Conflict, None, rule.restart_required);
    }

    let Ok(original_content) = read_config(&current_plan.target_file) else {
        return apply_result(RuleApplyState::Conflict, None, rule.restart_required);
    };
    if inspect_field(rule, &original_content) != Ok(current_plan.old_value.clone()) {
        return apply_result(RuleApplyState::Conflict, None, rule.restart_required);
    }
    let Ok(updated_content) =
        update_existing_field(rule, &original_content, &current_plan.new_value)
    else {
        return apply_result(RuleApplyState::WriteFailed, None, rule.restart_required);
    };
    if inspect_field(rule, &updated_content) != Ok(current_plan.new_value.clone()) {
        return apply_result(
            RuleApplyState::VerificationFailed,
            None,
            rule.restart_required,
        );
    }

    let original_bytes = original_content.as_bytes();
    let updated_bytes = updated_content.as_bytes();
    let before_hash = sha256_hex(original_bytes);
    let after_hash = sha256_hex(updated_bytes);
    let backup = RuleBackup {
        id: String::new(),
        rule_id: rule.id.clone(),
        target_file: current_plan.target_file.clone(),
        field_path: current_plan.field_path.clone(),
        old_value: current_plan.old_value.clone(),
        applied_value: current_plan.new_value.clone(),
        before_hash: before_hash.clone(),
        after_hash: after_hash.clone(),
        created_at: Utc::now().to_rfc3339(),
    };
    let Ok(backup) = store.create(backup) else {
        return apply_result(RuleApplyState::BackupFailed, None, rule.restart_required);
    };

    if current_hash(&current_plan.target_file).as_deref() != Some(before_hash.as_str()) {
        store.remove(&backup.id);
        return apply_result(RuleApplyState::Conflict, None, rule.restart_required);
    }
    let write_id = operation_id();
    if atomic_write(&current_plan.target_file, updated_bytes, &write_id).is_err() {
        store.remove(&backup.id);
        return apply_result(RuleApplyState::WriteFailed, None, rule.restart_required);
    }
    if verify_field_and_hash(
        rule,
        &current_plan.target_file,
        &current_plan.new_value,
        &after_hash,
    ) {
        return apply_result(RuleApplyState::Applied, Some(backup), rule.restart_required);
    }

    let rolled_back = rollback_if_unchanged(
        &current_plan.target_file,
        &after_hash,
        original_bytes,
        &write_id,
    );
    if rolled_back {
        store.remove(&backup.id);
    }
    apply_result(
        RuleApplyState::VerificationFailed,
        (!rolled_back).then_some(backup),
        rule.restart_required,
    )
}

fn restore_rule_with_store(
    rule: &ApplicationRule,
    backup: &RuleBackup,
    roots: &EnvironmentRoots,
    confirmed: bool,
) -> RuleRestoreResult {
    if !confirmed {
        return restore_result(
            RuleRestoreState::ConfirmationRequired,
            Some(backup.id.clone()),
            rule.restart_required,
        );
    }
    if backup.rule_id != rule.id
        || backup.field_path != rule.config.field
        || !validate_target_for_rule(rule, &backup.target_file, roots)
    {
        return restore_result(
            RuleRestoreState::Conflict,
            Some(backup.id.clone()),
            rule.restart_required,
        );
    }
    let Ok(current_content) = read_config(&backup.target_file) else {
        return restore_result(
            RuleRestoreState::Conflict,
            Some(backup.id.clone()),
            rule.restart_required,
        );
    };
    if inspect_field(rule, &current_content) != Ok(backup.applied_value.clone()) {
        return restore_result(
            RuleRestoreState::Conflict,
            Some(backup.id.clone()),
            rule.restart_required,
        );
    }
    let Ok(restored_content) = update_existing_field(rule, &current_content, &backup.old_value)
    else {
        return restore_result(
            RuleRestoreState::WriteFailed,
            Some(backup.id.clone()),
            rule.restart_required,
        );
    };
    if inspect_field(rule, &restored_content) != Ok(backup.old_value.clone()) {
        return restore_result(
            RuleRestoreState::VerificationFailed,
            Some(backup.id.clone()),
            rule.restart_required,
        );
    }

    let current_bytes = current_content.as_bytes();
    let restored_bytes = restored_content.as_bytes();
    let current_hash = sha256_hex(current_bytes);
    let restored_hash = sha256_hex(restored_bytes);
    if self::current_hash(&backup.target_file).as_deref() != Some(current_hash.as_str()) {
        return restore_result(
            RuleRestoreState::Conflict,
            Some(backup.id.clone()),
            rule.restart_required,
        );
    }
    let write_id = operation_id();
    if atomic_write(&backup.target_file, restored_bytes, &write_id).is_err() {
        return restore_result(
            RuleRestoreState::WriteFailed,
            Some(backup.id.clone()),
            rule.restart_required,
        );
    }
    if verify_field_and_hash(rule, &backup.target_file, &backup.old_value, &restored_hash) {
        return restore_result(
            RuleRestoreState::Restored,
            Some(backup.id.clone()),
            rule.restart_required,
        );
    }
    rollback_if_unchanged(
        &backup.target_file,
        &restored_hash,
        current_bytes,
        &write_id,
    );
    restore_result(
        RuleRestoreState::VerificationFailed,
        Some(backup.id.clone()),
        rule.restart_required,
    )
}

fn apply_result(
    state: RuleApplyState,
    backup: Option<RuleBackup>,
    restart_required: bool,
) -> RuleApplyResult {
    RuleApplyResult {
        state,
        backup,
        restart_required,
    }
}

fn restore_result(
    state: RuleRestoreState,
    backup_id: Option<String>,
    restart_required: bool,
) -> RuleRestoreResult {
    RuleRestoreResult {
        state,
        backup_id,
        restart_required,
    }
}

fn current_hash(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|bytes| sha256_hex(&bytes))
}

fn verify_field_and_hash(
    rule: &ApplicationRule,
    path: &Path,
    expected_value: &ConfigValue,
    expected_hash: &str,
) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    if sha256_hex(&bytes) != expected_hash {
        return false;
    }
    let Ok(content) = String::from_utf8(bytes) else {
        return false;
    };
    inspect_field(rule, &content) == Ok(expected_value.clone())
}

fn rollback_if_unchanged(
    path: &Path,
    expected_hash: &str,
    original: &[u8],
    write_id: &str,
) -> bool {
    if current_hash(path).as_deref() == Some(expected_hash) {
        let rollback_id = format!("{write_id}-rollback");
        return atomic_write(path, original, &rollback_id).is_ok();
    }
    false
}

fn update_existing_field(
    rule: &ApplicationRule,
    content: &str,
    new_value: &ConfigValue,
) -> Result<String, ()> {
    match rule.config.format {
        RuleConfigFormat::Json => {
            let mut document: serde_json::Value = serde_json::from_str(content).map_err(|_| ())?;
            let target = json_target_mut(&mut document, &rule.config.field).ok_or(())?;
            *target = config_to_json(new_value);
            let serialized = serde_json::to_string_pretty(&document).map_err(|_| ())?;
            Ok(preserve_final_newline(content, serialized))
        }
        RuleConfigFormat::Yaml => {
            let mut document: serde_yaml::Value = serde_yaml::from_str(content).map_err(|_| ())?;
            let target = yaml_target_mut(&mut document, &rule.config.field).ok_or(())?;
            *target = serde_yaml::to_value(new_value).map_err(|_| ())?;
            let serialized = serde_yaml::to_string(&document).map_err(|_| ())?;
            Ok(preserve_final_newline(content, serialized))
        }
        RuleConfigFormat::Toml => {
            let mut document: toml::Value = toml::from_str(content).map_err(|_| ())?;
            let target = toml_target_mut(&mut document, &rule.config.field).ok_or(())?;
            *target = config_to_toml(new_value)?;
            let serialized = toml::to_string_pretty(&document).map_err(|_| ())?;
            Ok(preserve_final_newline(content, serialized))
        }
        RuleConfigFormat::Ini => update_ini_field(content, &rule.config.field, new_value),
    }
}

fn json_target_mut<'a>(
    root: &'a mut serde_json::Value,
    field: &[String],
) -> Option<&'a mut serde_json::Value> {
    let (last, parents) = field.split_last()?;
    let parent = parents
        .iter()
        .try_fold(root, |value, segment| value.get_mut(segment))?;
    parent.get_mut(last)
}

fn yaml_target_mut<'a>(
    root: &'a mut serde_yaml::Value,
    field: &[String],
) -> Option<&'a mut serde_yaml::Value> {
    let (last, parents) = field.split_last()?;
    let parent = parents.iter().try_fold(root, |value, segment| {
        value.get_mut(serde_yaml::Value::String(segment.clone()))
    })?;
    parent.get_mut(serde_yaml::Value::String(last.clone()))
}

fn toml_target_mut<'a>(root: &'a mut toml::Value, field: &[String]) -> Option<&'a mut toml::Value> {
    let (last, parents) = field.split_last()?;
    let parent = parents
        .iter()
        .try_fold(root, |value, segment| value.get_mut(segment))?;
    parent.get_mut(last)
}

fn config_to_json(value: &ConfigValue) -> serde_json::Value {
    match value {
        ConfigValue::String(value) => serde_json::Value::String(value.clone()),
        ConfigValue::Number(value) => serde_json::Value::Number(value.clone()),
        ConfigValue::Boolean(value) => serde_json::Value::Bool(*value),
        ConfigValue::Null => serde_json::Value::Null,
    }
}

fn config_to_toml(value: &ConfigValue) -> Result<toml::Value, ()> {
    match value {
        ConfigValue::String(value) => Ok(toml::Value::String(value.clone())),
        ConfigValue::Number(value) => value
            .as_i64()
            .map(toml::Value::Integer)
            .or_else(|| value.as_f64().map(toml::Value::Float))
            .ok_or(()),
        ConfigValue::Boolean(value) => Ok(toml::Value::Boolean(*value)),
        ConfigValue::Null => Err(()),
    }
}

fn preserve_final_newline(original: &str, mut serialized: String) -> String {
    if original.ends_with('\n') {
        if !serialized.ends_with('\n') {
            serialized.push('\n');
        }
    } else {
        while serialized.ends_with(['\r', '\n']) {
            serialized.pop();
        }
    }
    serialized
}

#[derive(Debug, Clone, Copy)]
enum IniQuote {
    None,
    Single,
    Double,
}

fn update_ini_field(
    content: &str,
    field: &[String],
    new_value: &ConfigValue,
) -> Result<String, ()> {
    let (range, quote) = find_ini_value_range(content, field)?;
    let replacement = ini_value(new_value, quote)?;
    let mut updated = content.to_owned();
    updated.replace_range(range, &replacement);
    Ok(updated)
}

fn find_ini_value_range(content: &str, field: &[String]) -> Result<(Range<usize>, IniQuote), ()> {
    if field.len() != 2 {
        return Err(());
    }
    let mut current_section = None::<String>;
    let mut matched = None::<(Range<usize>, IniQuote)>;
    let mut offset = 0usize;
    for chunk in content.split_inclusive('\n') {
        let line = chunk
            .strip_suffix('\n')
            .unwrap_or(chunk)
            .strip_suffix('\r')
            .unwrap_or_else(|| chunk.strip_suffix('\n').unwrap_or(chunk));
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with([';', '#']) {
            offset += chunk.len();
            continue;
        }
        if trimmed.starts_with('[') {
            if !trimmed.ends_with(']') || trimmed.len() < 3 {
                return Err(());
            }
            current_section = Some(trimmed[1..trimmed.len() - 1].trim().to_owned());
            offset += chunk.len();
            continue;
        }
        let Some(equal_index) = line.find('=') else {
            return Err(());
        };
        let in_section = current_section
            .as_deref()
            .is_some_and(|section| section.eq_ignore_ascii_case(&field[0]));
        if in_section && line[..equal_index].trim().eq_ignore_ascii_case(&field[1]) {
            if matched.is_some() {
                return Err(());
            }
            let value_region = &line[equal_index + 1..];
            let leading = value_region.len() - value_region.trim_start().len();
            let trailing = value_region.len() - value_region.trim_end().len();
            let start = offset + equal_index + 1 + leading;
            let end = offset + line.len() - trailing;
            let raw_value = &content[start..end];
            let quote = if raw_value.starts_with('"') && raw_value.ends_with('"') {
                IniQuote::Double
            } else if raw_value.starts_with('\'') && raw_value.ends_with('\'') {
                IniQuote::Single
            } else {
                IniQuote::None
            };
            matched = Some((start..end, quote));
        }
        offset += chunk.len();
    }
    matched.ok_or(())
}

fn ini_value(value: &ConfigValue, quote: IniQuote) -> Result<String, ()> {
    let raw = match value {
        ConfigValue::String(value) => value.clone(),
        ConfigValue::Number(value) => value.to_string(),
        ConfigValue::Boolean(value) => value.to_string(),
        ConfigValue::Null => return Err(()),
    };
    if raw.contains(['\r', '\n']) {
        return Err(());
    }
    match quote {
        IniQuote::None => Ok(raw),
        IniQuote::Single if !raw.contains('\'') => Ok(format!("'{raw}'")),
        IniQuote::Double if !raw.contains('"') => Ok(format!("\"{raw}\"")),
        IniQuote::Single | IniQuote::Double => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::features::{
        application_assistant::rules::schema::{RuleConfig, RuleMatcher, RuleValueKind},
        proxy::ProxyProtocol,
    };

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
                "proxyenv-rule-apply-{}-{timestamp}-{sequence}",
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
            EnvironmentRoots::with_app_data(self.0.clone())
        }

        fn store(&self) -> RuleBackupStore {
            RuleBackupStore::at(self.0.join("backups"))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn rule(format: RuleConfigFormat, file_name: &str, field: &[&str]) -> ApplicationRule {
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
                value_kind: RuleValueKind::HttpProxyUrl,
            },
            restart_required: true,
        }
    }

    fn endpoint() -> ProxyEndpoint {
        ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: 7897,
            protocol: ProxyProtocol::Mixed,
        }
    }

    fn ready_plan(rule: &ApplicationRule, roots: &EnvironmentRoots) -> RuleChangePlan {
        preview_rule(rule, &endpoint(), roots).plan.unwrap()
    }

    #[test]
    fn confirmed_apply_creates_backup_writes_and_verifies_then_restores() {
        let directory = TestDirectory::new();
        let target = directory.write(
            "settings.json",
            r#"{"network":{"proxy":"http://127.0.0.1:7890"},"other":true}"#,
        );
        let rule = rule(
            RuleConfigFormat::Json,
            "settings.json",
            &["network", "proxy"],
        );
        let roots = directory.roots();
        let store = directory.store();
        let plan = ready_plan(&rule, &roots);

        let applied = apply_rule_with_store(&rule, &endpoint(), &plan, &roots, &store, true);

        assert_eq!(applied.state, RuleApplyState::Applied);
        let backup = applied.backup.unwrap();
        assert_eq!(store.load(&backup.id).unwrap(), backup);
        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(
            inspect_field(&rule, &content),
            Ok(ConfigValue::String("http://127.0.0.1:7897".into()))
        );
        assert!(content.contains("\"other\": true"));

        let unconfirmed = restore_rule_with_store(&rule, &backup, &roots, false);
        assert_eq!(unconfirmed.state, RuleRestoreState::ConfirmationRequired);
        assert_eq!(
            inspect_field(&rule, &fs::read_to_string(&target).unwrap()),
            Ok(ConfigValue::String("http://127.0.0.1:7897".into()))
        );

        let restored = restore_rule_with_store(&rule, &backup, &roots, true);

        assert_eq!(restored.state, RuleRestoreState::Restored);
        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(
            inspect_field(&rule, &content),
            Ok(ConfigValue::String("http://127.0.0.1:7890".into()))
        );
        assert!(content.contains("\"other\": true"));
        assert!(!fs::read_dir(&directory.0).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".proxyenv-")
        }));
    }

    #[test]
    fn stale_or_unconfirmed_plans_never_write() {
        let directory = TestDirectory::new();
        let target = directory.write("settings.json", r#"{"proxy":"http://127.0.0.1:7890"}"#);
        let rule = rule(RuleConfigFormat::Json, "settings.json", &["proxy"]);
        let roots = directory.roots();
        let store = directory.store();
        let plan = ready_plan(&rule, &roots);

        let unconfirmed = apply_rule_with_store(&rule, &endpoint(), &plan, &roots, &store, false);
        assert_eq!(unconfirmed.state, RuleApplyState::ConfirmationRequired);
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            r#"{"proxy":"http://127.0.0.1:7890"}"#
        );

        fs::write(&target, r#"{"proxy":"http://127.0.0.1:7895"}"#).unwrap();
        let conflict = apply_rule_with_store(&rule, &endpoint(), &plan, &roots, &store, true);
        assert_eq!(conflict.state, RuleApplyState::Conflict);
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            r#"{"proxy":"http://127.0.0.1:7895"}"#
        );
    }

    #[test]
    fn restore_conflicts_when_current_field_changed_or_disappeared() {
        let directory = TestDirectory::new();
        let target = directory.write("settings.json", r#"{"proxy":"http://127.0.0.1:7890"}"#);
        let rule = rule(RuleConfigFormat::Json, "settings.json", &["proxy"]);
        let roots = directory.roots();
        let store = directory.store();
        let plan = ready_plan(&rule, &roots);
        let backup = apply_rule_with_store(&rule, &endpoint(), &plan, &roots, &store, true)
            .backup
            .unwrap();

        for changed in [
            r#"{"proxy":"http://127.0.0.1:7999"}"#,
            r#"{"other":true}"#,
            "{invalid json",
        ] {
            fs::write(&target, changed).unwrap();
            let result = restore_rule_with_store(&rule, &backup, &roots, true);
            assert_eq!(result.state, RuleRestoreState::Conflict);
            assert_eq!(fs::read_to_string(&target).unwrap(), changed);
        }
    }

    #[test]
    fn applies_yaml_toml_and_ini_without_creating_new_fields() {
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
                "; keep this comment\r\n[network]\r\nproxy = \"http://127.0.0.1:7890\"\r\n",
                vec!["network", "proxy"],
            ),
        ];

        for (format, file_name, content, field) in cases {
            let target = directory.write(file_name, content);
            let rule = rule(format, file_name, &field);
            let roots = directory.roots();
            let store = directory.store();
            let plan = ready_plan(&rule, &roots);
            let result = apply_rule_with_store(&rule, &endpoint(), &plan, &roots, &store, true);
            assert_eq!(result.state, RuleApplyState::Applied, "{file_name}");
            let updated = fs::read_to_string(target).unwrap();
            assert_eq!(
                inspect_field(&rule, &updated),
                Ok(ConfigValue::String("http://127.0.0.1:7897".into())),
                "{file_name}"
            );
            if matches!(format, RuleConfigFormat::Ini) {
                assert!(updated.contains("; keep this comment\r\n"));
                assert!(updated.contains("proxy = \"http://127.0.0.1:7897\""));
            }
        }
    }
}
