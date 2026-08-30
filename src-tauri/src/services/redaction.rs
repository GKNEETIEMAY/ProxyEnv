use std::{fmt, sync::OnceLock};

use regex::Regex;

pub const REDACTED_PATH: &str = "<local-path>";
pub const REDACTED_PROXY: &str = "<proxy-endpoint>";
pub const REDACTED_PROCESS: &str = "<process>";
pub const REDACTED_SECRET: &str = "<redacted>";

/// Sanitizes untrusted text before it crosses a logging, diagnostic, or error-report boundary.
///
/// Prefer structured summaries over free-form text. Values such as configuration contents,
/// authorization tokens, and other secrets must use [`sensitive`] because their format is not
/// reliably recognizable.
pub fn sanitize(text: &str) -> String {
    let mut sanitized = proxy_url_regex()
        .replace_all(text, REDACTED_PROXY)
        .into_owned();
    sanitized = endpoint_regex()
        .replace_all(&sanitized, REDACTED_PROXY)
        .into_owned();
    sanitized = loopback_regex()
        .replace_all(&sanitized, REDACTED_PROXY)
        .into_owned();
    sanitized = windows_path_regex()
        .replace_all(&sanitized, REDACTED_PATH)
        .into_owned();
    sanitized = unix_path_regex()
        .replace_all(&sanitized, REDACTED_PATH)
        .into_owned();
    sanitized = environment_path_regex()
        .replace_all(&sanitized, REDACTED_PATH)
        .into_owned();
    sanitized = process_regex()
        .replace_all(&sanitized, REDACTED_PROCESS)
        .into_owned();
    sanitized = process_id_regex()
        .replace_all(&sanitized, "PID <redacted>")
        .into_owned();

    for username in local_usernames() {
        sanitized = replace_case_insensitive(&sanitized, username, "<user>");
    }
    sanitized
}

/// A display/debug wrapper for text that must be sanitized before diagnostics are emitted.
pub fn safe_text(text: &str) -> SafeText<'_> {
    SafeText(text)
}

pub struct SafeText<'a>(&'a str);

impl fmt::Display for SafeText<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&sanitize(self.0))
    }
}

impl fmt::Debug for SafeText<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SafeText")
            .field(&sanitize(self.0))
            .finish()
    }
}

/// A format-agnostic wrapper for values whose contents must never enter logs or reports.
pub fn sensitive<T>(value: &T) -> Sensitive<'_, T> {
    Sensitive(value)
}

pub struct Sensitive<'a, T>(&'a T);

impl<T> fmt::Display for Sensitive<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = self.0;
        formatter.write_str(REDACTED_SECRET)
    }
}

impl<T> fmt::Debug for Sensitive<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = self.0;
        formatter.write_str(REDACTED_SECRET)
    }
}

fn proxy_url_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?:https?|socks5h?|mixed)://(?:[^\s/@]+(?::[^\s/@]*)?@)?(?:localhost|(?:\d{1,3}\.){3}\d{1,3}|\[[0-9a-f:]+\])(?::\d{1,5})?",
        )
        .expect("proxy URL redaction regex must compile")
    })
}

fn endpoint_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)(?:\b(?:localhost|(?:\d{1,3}\.){3}\d{1,3}):\d{1,5}\b|\[[0-9a-f:]+\]:\d{1,5})",
        )
        .expect("proxy endpoint redaction regex must compile")
    })
}

fn loopback_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)(?:\blocalhost\b|\b127(?:\.\d{1,3}){3}\b|::1)")
            .expect("loopback redaction regex must compile")
    })
}

fn windows_path_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?i)(?:\b[a-z]:[\\/]|\\\\)[^\r\n\t\"'<>|]+"#)
            .expect("Windows path redaction regex must compile")
    })
}

fn unix_path_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?i)/(?:home|users|private|var/folders|tmp)/[^\r\n\t\"'<>|]+"#)
            .expect("Unix path redaction regex must compile")
    })
}

fn environment_path_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?i)%[a-z_][a-z0-9_]*%[\\/][^\s\"']+"#)
            .expect("environment path redaction regex must compile")
    })
}

fn process_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b[a-z0-9_.-]+\.exe\b").expect("process redaction regex must compile")
    })
}

fn process_id_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(?:pid|process\s+id)\s*[:=]?\s*\d+\b")
            .expect("process ID redaction regex must compile")
    })
}

fn local_usernames() -> &'static Vec<String> {
    static USERNAMES: OnceLock<Vec<String>> = OnceLock::new();
    USERNAMES.get_or_init(|| {
        ["USERNAME", "USER"]
            .into_iter()
            .filter_map(|name| std::env::var(name).ok())
            .filter(|value| value.len() >= 3 && !value.eq_ignore_ascii_case("user"))
            .fold(Vec::new(), |mut values, value| {
                if !values.iter().any(|known| value.eq_ignore_ascii_case(known)) {
                    values.push(value);
                }
                values
            })
    })
}

fn replace_case_insensitive(text: &str, needle: &str, replacement: &str) -> String {
    Regex::new(&format!("(?i){}", regex::escape(needle)))
        .expect("escaped username redaction regex must compile")
        .replace_all(text, replacement)
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_paths_proxy_endpoints_processes_and_ids() {
        let input = r#"failed to launch "C:\Users\Alice\Apps\Code.exe" with http://name:secret@127.0.0.1:7897 (PID 4420); config=/home/alice/.config/app.yaml"#;
        let output = sanitize(input);

        for sensitive in [
            "Alice",
            "Code.exe",
            "name",
            "secret",
            "127.0.0.1",
            "7897",
            "4420",
            "/home/alice",
        ] {
            assert!(!output.contains(sensitive), "leaked {sensitive}: {output}");
        }
        assert!(output.contains(REDACTED_PATH));
        assert!(output.contains(REDACTED_PROXY));
    }

    #[test]
    fn redacts_ipv6_and_environment_relative_paths() {
        let output = sanitize(r"proxy [::1]:1080; config %APPDATA%\Example\config.yaml");
        assert!(!output.contains("1080"));
        assert!(!output.contains("APPDATA"));
        assert!(output.contains(REDACTED_PROXY));
        assert!(output.contains(REDACTED_PATH));
    }

    #[test]
    fn sensitive_values_never_use_their_display_or_debug_implementation() {
        let secret = "subscription-token";
        assert_eq!(sensitive(&secret).to_string(), REDACTED_SECRET);
        assert_eq!(format!("{:?}", sensitive(&secret)), REDACTED_SECRET);
    }
}
