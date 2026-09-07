use super::BridgeResult;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use sysinfo::System;

#[derive(Debug, Clone)]
pub struct Session {
    pub name: String,
    pub host: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub compatible: bool,
}

fn configured_path(process: &sysinfo::Process) -> Option<PathBuf> {
    let command = process.cmd();
    command.windows(2).find_map(|pair| {
        (pair[0].to_string_lossy().eq_ignore_ascii_case("-i")).then(|| PathBuf::from(&pair[1]))
    })
}

pub fn config_paths() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let system = System::new_all();
    for process in system.processes().values() {
        if process
            .name()
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains("mobaxterm")
        {
            if let Some(path) = configured_path(process) {
                candidates.push(path);
            }
            if let Some(parent) = process.exe().and_then(Path::parent) {
                candidates.push(parent.join("MobaXterm.ini"));
            }
        }
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join("Documents/MobaXterm/MobaXterm.ini"));
    }
    if let Some(config) = dirs::config_dir() {
        candidates.push(config.join("MobaXterm/MobaXterm.ini"));
    }
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter_map(|path| path.canonicalize().ok())
        .filter(|path| path.is_file() && seen.insert(path.clone()))
        .collect()
}

pub fn sessions(path: &Path) -> BridgeResult<Vec<Session>> {
    let metadata = std::fs::metadata(path).map_err(|_| "mobaConfigInvalid")?;
    if metadata.len() > 1024 * 1024 {
        return Err("mobaConfigInvalid".into());
    }
    let input = std::fs::read_to_string(path).map_err(|_| "mobaConfigInvalid")?;
    Ok(sessions_from(&input))
}

pub fn sessions_from(input: &str) -> Vec<Session> {
    let mut in_bookmarks = false;
    let mut sessions = Vec::new();
    for raw in input.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            let section = &line[1..line.len() - 1];
            in_bookmarks = section == "Bookmarks"
                || section.strip_prefix("Bookmarks_").is_some_and(|suffix| {
                    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
                });
            continue;
        }
        if !in_bookmarks || line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let Some((name, raw_value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || name.len() > 160 || ["SubRep", "ImgNum"].contains(&name) {
            continue;
        }
        let Some((_, payload)) = raw_value.split_once('%') else {
            sessions.push(Session {
                name: name.into(),
                host: None,
                user: None,
                port: None,
                compatible: false,
            });
            continue;
        };
        let fields: Vec<_> = payload.split('%').collect();
        let host = fields
            .first()
            .map(|value| value.trim())
            .filter(|value| super::ssh::safe_host(value))
            .map(str::to_owned);
        let port = fields
            .get(1)
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|value| *value > 0);
        let user = fields
            .get(2)
            .map(|value| value.trim())
            .filter(|value| super::ssh::safe_name(value))
            .map(str::to_owned);
        let compatible =
            raw_value.starts_with("#109#") && host.is_some() && port.is_some() && user.is_some();
        sessions.push(Session {
            name: name.into(),
            host,
            user,
            port,
            compatible,
        });
    }
    sessions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_bookmark_ssh_sessions_without_credentials() {
        let input = "[Passwords]\nsecret=value\n[Bookmarks]\nSubRep=Servers\nGPU Server=#109#0%gpu.example.test%22%dev%%-1%-1%%%%%0\nBroken=#109#0%bad host%22%root%%\n[Other]\nIgnored=#109#0%other%22%dev%%";
        let sessions = sessions_from(input);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name, "GPU Server");
        assert_eq!(sessions[0].host.as_deref(), Some("gpu.example.test"));
        assert!(sessions[0].compatible);
        assert!(!sessions[1].compatible);
        assert!(!format!("{sessions:?}").contains("secret"));
    }
}
