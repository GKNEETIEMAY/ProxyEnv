use super::{BridgeResult, RemoteTarget, RemoteTargetCompatibility, RemoteTargetSource};
use crate::services::local_file;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const MAX_STORE_BYTES: u64 = 64 * 1024;
const MAX_CONNECTIONS: usize = 64;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ManualAuthentication {
    Automatic,
    IdentityFile,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManualConnectionInput {
    pub display_name: String,
    pub destination: String,
    pub port: u16,
    pub authentication: ManualAuthentication,
    pub identity_file: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManualConnection {
    pub id: String,
    pub display_name: String,
    pub host: String,
    pub user: Option<String>,
    pub port: u16,
    pub identity_file: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConnectionStore {
    #[serde(default)]
    connections: Vec<ManualConnection>,
}

fn path() -> BridgeResult<PathBuf> {
    dirs::data_local_dir()
        .map(|path| path.join("ProxyEnv").join("remote-connections.json"))
        .ok_or_else(|| "stateUnavailable".into())
}

fn load() -> BridgeResult<ConnectionStore> {
    let path = path()?;
    let Some(bytes) =
        local_file::safe_read(&path, MAX_STORE_BYTES).map_err(|_| "stateUnavailable")?
    else {
        return Ok(ConnectionStore::default());
    };
    let store: ConnectionStore = serde_json::from_slice(&bytes).map_err(|_| "stateUnavailable")?;
    if store.connections.len() > MAX_CONNECTIONS {
        return Err("stateUnavailable".into());
    }
    Ok(store)
}

fn persist(store: &ConnectionStore) -> BridgeResult<()> {
    let path = path()?;
    fs::create_dir_all(path.parent().ok_or("stateUnavailable")?).map_err(|_| "stateUnavailable")?;
    let bytes = serde_json::to_vec_pretty(store).map_err(|_| "stateUnavailable")?;
    local_file::atomic_write(&path, &bytes, "remote-connections")
        .map_err(|_| "stateUnavailable".into())
}

fn display_name(value: &str) -> BridgeResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err("invalidTarget".into());
    }
    Ok(value.to_owned())
}

fn destination(value: &str) -> BridgeResult<(Option<String>, String)> {
    let value = value.trim();
    let (user, host) = if let Some((user, host)) = value.rsplit_once('@') {
        if user.contains('@') || !super::ssh::safe_name(user) {
            return Err("invalidTarget".into());
        }
        (Some(user.to_owned()), host)
    } else {
        (None, value)
    };
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    if !super::ssh::safe_host(host) {
        return Err("invalidTarget".into());
    }
    Ok((user, host.to_owned()))
}

fn identity_file(input: &ManualConnectionInput) -> BridgeResult<Option<String>> {
    if input.authentication == ManualAuthentication::Automatic {
        return Ok(None);
    }
    let value = input
        .identity_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("identityFileInvalid")?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("identityFileInvalid".into());
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| "identityFileInvalid")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("identityFileInvalid".into());
    }
    let canonical = path.canonicalize().map_err(|_| "identityFileInvalid")?;
    Ok(Some(canonical.to_string_lossy().into_owned()))
}

fn new_id() -> BridgeResult<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "stateUnavailable")?;
    Ok(format!("manual|{}", hex::encode(bytes)))
}

pub fn add(input: ManualConnectionInput) -> BridgeResult<ManualConnection> {
    if input.port == 0 {
        return Err("invalidTarget".into());
    }
    let mut store = load()?;
    if store.connections.len() >= MAX_CONNECTIONS {
        return Err("connectionLimitReached".into());
    }
    let display_name = display_name(&input.display_name)?;
    let (user, host) = destination(&input.destination)?;
    let connection = ManualConnection {
        id: new_id()?,
        display_name,
        host,
        user,
        port: input.port,
        identity_file: identity_file(&input)?,
    };
    store.connections.push(connection.clone());
    persist(&store)?;
    Ok(connection)
}

pub fn remove(id: &str) -> BridgeResult<()> {
    let mut store = load()?;
    let original_len = store.connections.len();
    store.connections.retain(|connection| connection.id != id);
    if store.connections.len() == original_len {
        return Err("invalidTarget".into());
    }
    persist(&store)
}

pub fn all() -> BridgeResult<Vec<ManualConnection>> {
    Ok(load()?.connections)
}

pub fn target(connection: &ManualConnection) -> RemoteTarget {
    let identity_available = connection.identity_file.as_deref().is_none_or(|path| {
        fs::symlink_metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
    });
    RemoteTarget {
        id: connection.id.clone(),
        display_name: connection.display_name.clone(),
        source: RemoteTargetSource::Manual,
        source_label: "ProxyEnv".into(),
        config_path: "ProxyEnv".into(),
        ssh_alias: None,
        host: Some(connection.host.clone()),
        user: connection.user.clone(),
        port: Some(connection.port),
        identity_file: connection.identity_file.clone(),
        available: identity_available,
        compatibility: if identity_available {
            RemoteTargetCompatibility::Compatible
        } else {
            RemoteTargetCompatibility::Unsupported
        },
        unavailable_reason: (!identity_available).then(|| "identityFileInvalid".into()),
        can_open_vscode: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_host_and_optional_user_without_accepting_shell_syntax() {
        assert_eq!(
            destination("student@lab.example.edu").unwrap(),
            (Some("student".into()), "lab.example.edu".into())
        );
        assert_eq!(
            destination("[2001:db8::1]").unwrap(),
            (None, "2001:db8::1".into())
        );
        for invalid in ["", "bad host", "student@", "student;cmd@host", "a@b@host"] {
            assert!(destination(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn automatic_authentication_never_persists_identity_input() {
        let input = ManualConnectionInput {
            display_name: "Lab".into(),
            destination: "student@lab.example.edu".into(),
            port: 22,
            authentication: ManualAuthentication::Automatic,
            identity_file: Some("C:\\not-used".into()),
        };
        assert_eq!(identity_file(&input).unwrap(), None);
    }

    #[test]
    fn rejects_relative_identity_file() {
        let input = ManualConnectionInput {
            display_name: "Lab".into(),
            destination: "lab.example.edu".into(),
            port: 22,
            authentication: ManualAuthentication::IdentityFile,
            identity_file: Some(".ssh/id_ed25519".into()),
        };
        assert_eq!(identity_file(&input), Err("identityFileInvalid".into()));
    }
}
