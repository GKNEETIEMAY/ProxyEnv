use tracing::warn;

use super::schema::ApplicationRule;

include!(concat!(env!("OUT_DIR"), "/bundled_application_rules.rs"));

#[derive(Debug, Default)]
pub struct RuleCatalog {
    pub rules: Vec<ApplicationRule>,
    pub issues: Vec<RuleLoadIssue>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RuleLoadIssue {
    pub source: String,
    pub message: String,
}

pub fn load_bundled() -> RuleCatalog {
    let catalog = load_sources(BUNDLED_RULE_SOURCES.iter().copied());
    for issue in &catalog.issues {
        warn!(
            source = issue.source,
            error = issue.message,
            "disabled invalid bundled application rule"
        );
    }
    catalog
}

fn load_sources<'a>(sources: impl IntoIterator<Item = (&'a str, &'a str)>) -> RuleCatalog {
    let mut catalog = RuleCatalog::default();
    for (source, json) in sources {
        match serde_json::from_str::<ApplicationRule>(json) {
            Ok(rule) => match rule.validate() {
                Ok(()) => catalog.rules.push(rule),
                Err(error) => catalog.issues.push(RuleLoadIssue {
                    source: source.to_owned(),
                    message: error.to_string(),
                }),
            },
            Err(error) => catalog.issues.push(RuleLoadIssue {
                source: source.to_owned(),
                message: error.to_string(),
            }),
        }
    }
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_RULE: &str = r#"{
        "schemaVersion": 1,
        "id": "example-app",
        "displayName": "Example App",
        "match": { "processNames": ["example.exe"] },
        "config": {
            "format": "yaml",
            "paths": ["%APPDATA%\\Example\\config.yaml"],
            "field": ["proxy-url"],
            "valueKind": "httpProxyUrl"
        },
        "restartRequired": true
    }"#;

    fn changed(original: &str, old: &str, new: &str) -> String {
        original.replacen(old, new, 1)
    }

    #[test]
    fn loads_a_valid_schema_v1_rule() {
        let catalog = load_sources([("valid.json", VALID_RULE)]);
        assert_eq!(catalog.rules.len(), 1);
        assert!(catalog.issues.is_empty());
    }

    #[test]
    fn rejects_unknown_schema_versions() {
        let json = changed(VALID_RULE, "\"schemaVersion\": 1", "\"schemaVersion\": 2");
        let catalog = load_sources([("version.json", json.as_str())]);
        assert!(catalog.rules.is_empty());
        assert!(catalog.issues[0]
            .message
            .contains("unsupported rule schema version"));
    }

    #[test]
    fn rejects_script_capabilities_as_unknown_fields() {
        let json = changed(
            VALID_RULE,
            "\"restartRequired\": true",
            "\"script\": \"powershell.exe\", \"restartRequired\": true",
        );
        let catalog = load_sources([("script.json", json.as_str())]);
        assert!(catalog.rules.is_empty());
        assert!(catalog.issues[0].message.contains("unknown field"));
    }

    #[test]
    fn rejects_unsupported_formats() {
        let json = changed(VALID_RULE, "\"format\": \"yaml\"", "\"format\": \"text\"");
        let catalog = load_sources([("format.json", json.as_str())]);
        assert!(catalog.rules.is_empty());
        assert!(catalog.issues[0].message.contains("unknown variant"));
    }

    #[test]
    fn rejects_invalid_path_tokens_and_traversal() {
        for path in [
            r"%TEMP%\Example\config.yaml",
            r"%APPDATA%\..\secret.yaml",
            r"%APPDATA%\Example\*.yaml",
            "应用配置/config.yaml",
        ] {
            let json = changed(
                VALID_RULE,
                r"%APPDATA%\\Example\\config.yaml",
                &path.replace('\\', r"\\"),
            );
            let catalog = load_sources([("path.json", json.as_str())]);
            assert!(catalog.rules.is_empty(), "accepted unsafe path: {path}");
            assert!(catalog.issues[0]
                .message
                .contains("invalid rule config path"));
        }
    }

    #[test]
    fn rejects_unknown_value_kinds() {
        let json = changed(
            VALID_RULE,
            "\"valueKind\": \"httpProxyUrl\"",
            "\"valueKind\": \"template\"",
        );
        let catalog = load_sources([("value-kind.json", json.as_str())]);
        assert!(catalog.rules.is_empty());
        assert!(catalog.issues[0].message.contains("unknown variant"));
    }

    #[test]
    fn rejects_ini_rules_without_an_exact_section_and_key() {
        let json = changed(VALID_RULE, "\"format\": \"yaml\"", "\"format\": \"ini\"");
        let catalog = load_sources([("ini-selector.json", json.as_str())]);
        assert!(catalog.rules.is_empty());
        assert!(catalog.issues[0].message.contains("exactly [section, key]"));
    }
}
