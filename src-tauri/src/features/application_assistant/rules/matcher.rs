use std::path::Path;

use super::schema::ApplicationRule;

#[derive(Debug, Clone, PartialEq)]
pub enum RuleMatchResult<'a> {
    None,
    Exact(&'a ApplicationRule),
    Ambiguous(Vec<&'a ApplicationRule>),
}

pub fn match_executable<'a>(
    executable_path: &Path,
    rules: &'a [ApplicationRule],
) -> RuleMatchResult<'a> {
    let Some(process_name) = executable_path.file_name().and_then(|name| name.to_str()) else {
        return RuleMatchResult::None;
    };
    let matches = rules
        .iter()
        .filter(|rule| {
            rule.matcher
                .process_names
                .iter()
                .any(|expected| process_names_equal(process_name, expected))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => RuleMatchResult::None,
        [rule] => RuleMatchResult::Exact(rule),
        _ => RuleMatchResult::Ambiguous(matches),
    }
}

#[cfg(windows)]
fn process_names_equal(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}

#[cfg(not(windows))]
fn process_names_equal(actual: &str, expected: &str) -> bool {
    actual == expected
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::features::application_assistant::rules::schema::{
        RuleConfig, RuleConfigFormat, RuleMatcher, RuleValueKind,
    };

    fn rule(id: &str, process_name: &str) -> ApplicationRule {
        ApplicationRule {
            schema_version: 1,
            id: id.into(),
            display_name: id.into(),
            matcher: RuleMatcher {
                process_names: vec![process_name.into()],
            },
            config: RuleConfig {
                format: RuleConfigFormat::Json,
                paths: vec![r"%APPDATA%\Example\config.json".into()],
                field: vec!["proxy".into()],
                value_kind: RuleValueKind::HttpProxyUrl,
            },
            restart_required: true,
        }
    }

    #[test]
    fn reports_zero_matches() {
        let rules = vec![rule("other", "other.exe")];
        assert_eq!(
            match_executable(Path::new(r"C:\Apps\example.exe"), &rules),
            RuleMatchResult::None
        );
    }

    #[test]
    fn returns_one_exact_match() {
        let rules = vec![rule("example", "example.exe")];
        let result = match_executable(Path::new(r"C:\Apps\example.exe"), &rules);
        assert!(matches!(result, RuleMatchResult::Exact(rule) if rule.id == "example"));
    }

    #[test]
    fn marks_multiple_exact_matches_as_ambiguous() {
        let rules = vec![
            rule("example-one", "example.exe"),
            rule("example-two", "example.exe"),
        ];
        let result = match_executable(Path::new(r"C:\Apps\example.exe"), &rules);
        assert!(matches!(result, RuleMatchResult::Ambiguous(matches) if matches.len() == 2));
    }

    #[cfg(windows)]
    #[test]
    fn matches_windows_executable_names_case_insensitively() {
        let rules = vec![rule("example", "Example.EXE")];
        let executable = PathBuf::from(r"C:\Apps\EXAMPLE.exe");
        assert!(matches!(
            match_executable(&executable, &rules),
            RuleMatchResult::Exact(_)
        ));
    }
}
