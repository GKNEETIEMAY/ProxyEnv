//! Remote - SSH integration; reads only the default VS Code user settings file.
//! No extension settings, credentials, keys or remote services are modified.
use super::{ssh, BridgeResult};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("Code/User/settings.json"))
}

// JSONC comments and trailing commas are syntax, not string contents. Keep this
// small parser local; serde_json still performs all structural validation.
pub fn parse_settings(input: &str) -> BridgeResult<serde_json::Value> {
    let bytes = input.trim_start_matches('\u{feff}').as_bytes();
    let mut cleaned = Vec::with_capacity(bytes.len());
    let (mut i, mut string, mut escape) = (0, false, false);
    while i < bytes.len() {
        let c = bytes[i];
        if string {
            cleaned.push(c);
            if escape {
                escape = false;
            } else if c == b'\\' {
                escape = true;
            } else if c == b'"' {
                string = false;
            }
        } else if c == b'"' {
            string = true;
            cleaned.push(c);
        } else if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            cleaned.push(b'\n');
            continue;
        } else if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 >= bytes.len() {
                return Err("vscodeConfigInvalid".into());
            }
            i += 2;
            cleaned.push(b' ');
            continue;
        } else {
            cleaned.push(c);
        }
        i += 1;
    }
    let mut output = Vec::with_capacity(cleaned.len());
    string = false;
    escape = false;
    for (i, &c) in cleaned.iter().enumerate() {
        if string {
            if escape {
                escape = false;
            } else if c == b'\\' {
                escape = true;
            } else if c == b'"' {
                string = false;
            }
        } else if c == b'"' {
            string = true;
        } else if c == b','
            && cleaned[i + 1..]
                .iter()
                .find(|b| !b.is_ascii_whitespace())
                .is_some_and(|b| *b == b'}' || *b == b']')
        {
            continue;
        }
        output.push(c);
    }
    serde_json::from_slice(&output).map_err(|_| "vscodeConfigInvalid".into())
}

pub fn custom_ssh_config() -> BridgeResult<Option<PathBuf>> {
    let Some(path) = settings_path().filter(|p| p.exists()) else {
        return Ok(None);
    };
    if std::fs::metadata(&path)
        .map_err(|_| "vscodeConfigInvalid")?
        .len()
        > 1024 * 1024
    {
        return Err("vscodeConfigInvalid".into());
    }
    let settings =
        parse_settings(&std::fs::read_to_string(path).map_err(|_| "vscodeConfigInvalid")?)?;
    let Some(value) = settings
        .get("remote.SSH.configFile")
        .filter(|v| !v.is_null())
    else {
        return Ok(None);
    };
    let raw = value.as_str().ok_or("vscodeConfigInvalid")?;
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.chars().any(char::is_control) {
        return Err("vscodeConfigInvalid".into());
    }
    let home = dirs::home_dir().ok_or("vscodeConfigInvalid")?;
    let expanded = if let Some(suffix) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))
    {
        home.join(suffix)
    } else {
        PathBuf::from(raw.replace("${env:USERPROFILE}", &home.to_string_lossy()))
    };
    if !expanded.is_absolute() || expanded.to_string_lossy().contains("${") {
        return Err("vscodeConfigInvalid".into());
    }
    let canonical = expanded.canonicalize().map_err(|_| "vscodeConfigInvalid")?;
    if !canonical.is_file() {
        return Err("vscodeConfigInvalid".into());
    }
    Ok(Some(canonical))
}

fn executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("Programs/Microsoft VS Code/Code.exe"));
    }
    if let Some(programs) = std::env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(programs).join("Microsoft VS Code/Code.exe"));
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for entry in std::env::split_paths(&paths) {
            if entry.join("code.cmd").is_file() {
                if let Some(parent) = entry.parent() {
                    candidates.push(parent.join("Code.exe"));
                }
            }
        }
    }
    candidates
        .into_iter()
        .find(|path| path.is_absolute() && path.is_file())
}

pub fn open(target: String) -> BridgeResult<()> {
    let (alias, config) = ssh::target_parts(&target)?;
    let vscode_config = custom_ssh_config()?;
    let default = dirs::home_dir()
        .map(|p| p.join(".ssh/config"))
        .and_then(|p| p.canonicalize().ok());
    // VS Code resolves the alias itself; never open a different host when it
    // uses another SSH config than the reviewed bridge.
    let selected = config.as_ref().or(default.as_ref());
    let actual = vscode_config.as_ref().or(default.as_ref());
    if selected != actual {
        return Err("vscodeConfigMismatch".into());
    }
    if !ssh::aliases()?.contains(&target) {
        return Err("invalidTarget".into());
    }
    let executable = executable().ok_or("vscodeMissing")?;
    launch(&executable, alias)
}
fn launch(executable: &Path, alias: &str) -> BridgeResult<()> {
    let mut command = Command::new(executable);
    command
        .args(["--new-window", "--remote", &format!("ssh-remote+{alias}")])
        .env_remove("ELECTRON_RUN_AS_NODE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.spawn().map_err(|_| "vscodeMissing")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jsonc_keeps_strings_and_accepts_comments_and_trailing_commas() {
        let data = parse_settings(
            r#"{// comment
          "remote.SSH.configFile": "C:\\dev\\ssh-config", /* block */
          "url":"https://example.test/a/*b*/,", "array":[1,2,],
        }"#,
        )
        .unwrap();
        assert_eq!(data["remote.SSH.configFile"], "C:\\dev\\ssh-config");
        assert_eq!(data["url"], "https://example.test/a/*b*/,");
        assert!(parse_settings("{/* unterminated").is_err());
    }
}
