//! Remote - SSH integration; reads only the default VS Code user settings file.
//! No extension settings, credentials, keys or remote services are modified.
use super::{ssh, BridgeResult};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use sysinfo::System;

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
    let system = System::new_all();
    candidates.extend(system.processes().values().filter_map(|process| {
        is_vscode_process_name(process.name())
            .then(|| process.exe().map(Path::to_path_buf))
            .flatten()
    }));
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

fn is_vscode_process_name(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    name.eq_ignore_ascii_case("Code.exe") || name.eq_ignore_ascii_case("Code - Insiders.exe")
}

pub fn open(target: String) -> BridgeResult<()> {
    let selected = ssh::target(&target)?;
    if !selected.can_open_vscode {
        return Err("vscodeConfigMismatch".into());
    }
    let alias = selected
        .ssh_alias
        .as_deref()
        .ok_or("vscodeConfigMismatch")?;
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

pub fn open_settings() -> BridgeResult<()> {
    let settings = settings_path()
        .filter(|path| path.is_file())
        .ok_or("vscodeConfigInvalid")?;
    let executable = executable().ok_or("vscodeMissing")?;
    let mut command = Command::new(executable);
    command
        .args(["--new-window"])
        .arg(settings)
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

pub fn open_target_config(target: String) -> BridgeResult<()> {
    let path = ssh::target_config_path(&target)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut command = if let Some(executable) = executable() {
            let mut command = Command::new(executable);
            command.arg("--new-window");
            command
        } else {
            let notepad = PathBuf::from(
                std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
            )
            .join("System32/notepad.exe");
            if !notepad.is_file() {
                return Err("processFailed".into());
            }
            Command::new(notepad)
        };
        command
            .arg(path)
            .env_remove("ELECTRON_RUN_AS_NODE")
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "processFailed")?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("processFailed".into())
    }
}

pub fn reveal_target_config(target: String) -> BridgeResult<()> {
    let path = ssh::target_config_path(&target)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        Command::new("explorer.exe")
            .arg(format!("/select,{}", path.display()))
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "processFailed")?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("processFailed".into())
    }
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

    #[test]
    fn vscode_process_names_are_matched_without_requiring_a_default_install_path() {
        assert!(is_vscode_process_name(std::ffi::OsStr::new("Code.exe")));
        assert!(is_vscode_process_name(std::ffi::OsStr::new("code.exe")));
        assert!(is_vscode_process_name(std::ffi::OsStr::new(
            "Code - Insiders.exe"
        )));
        assert!(!is_vscode_process_name(std::ffi::OsStr::new("Cursor.exe")));
    }
}
