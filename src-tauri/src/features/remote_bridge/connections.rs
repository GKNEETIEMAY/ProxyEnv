use super::{
    BridgeResult, RemoteTarget, RemoteTargetCompatibility, RemoteTargetSource, SshAuthMethod,
};
use crate::services::local_file;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const MAX_STORE_BYTES: u64 = 64 * 1024;
const MAX_CONNECTIONS: usize = 64;
const MAX_DISPLAY_NAME_CHARS: usize = 32;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ManualAuthentication {
    #[default]
    Automatic,
    Password,
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
    #[serde(default)]
    pub authentication: ManualAuthentication,
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
    if value.is_empty()
        || value.chars().count() > MAX_DISPLAY_NAME_CHARS
        || value.chars().any(char::is_control)
    {
        return Err("invalidConnectionName".into());
    }
    Ok(value.to_owned())
}

fn destination_host(value: &str) -> bool {
    if value.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    if value.contains(':')
        || value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return false;
    }
    let value = value.strip_suffix('.').unwrap_or(value);
    !value.is_empty()
        && value.len() <= 253
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        })
}

fn destination(value: &str) -> BridgeResult<(Option<String>, String)> {
    let value = value.trim();
    let (user, host) = if let Some((user, host)) = value.rsplit_once('@') {
        if user.contains('@') || !super::ssh::safe_name(user) {
            return Err("invalidConnectionDestination".into());
        }
        (Some(user.to_owned()), host)
    } else {
        (None, value)
    };
    let bracketed = host.starts_with('[') || host.ends_with(']');
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    if bracketed && host.parse::<std::net::Ipv6Addr>().is_err() {
        return Err("invalidConnectionDestination".into());
    }
    if !destination_host(host) {
        return Err("invalidConnectionDestination".into());
    }
    Ok((user, host.to_owned()))
}

fn identity_file(input: &ManualConnectionInput) -> BridgeResult<Option<String>> {
    if input.authentication != ManualAuthentication::IdentityFile {
        return Ok(None);
    }
    let value = input
        .identity_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("identityFileInvalid")?;
    validate_identity_file(value).map(Some)
}

pub fn validate_identity_file(value: &str) -> BridgeResult<String> {
    if value.chars().any(char::is_control)
        || value.starts_with(r"\\?\")
        || value.starts_with("//?/")
        || value.starts_with(r"\\.\")
        || value.starts_with("//./")
    {
        return Err("identityFileInvalid".into());
    }
    #[cfg(windows)]
    if value.char_indices().any(|(index, character)| {
        matches!(character, '<' | '>' | '"' | '|' | '?' | '*') || character == ':' && index != 1
    }) {
        return Err("identityFileInvalid".into());
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("identityFileInvalid".into());
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| "identityFileInvalid")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("identityFileInvalid".into());
    }
    let header =
        local_file::safe_read_first_line(&path, 1024 * 1024).map_err(|_| "identityFileInvalid")?;
    let header = header.strip_suffix(b"\r").unwrap_or(&header);
    if ![
        b"-----BEGIN OPENSSH PRIVATE KEY-----".as_slice(),
        b"-----BEGIN RSA PRIVATE KEY-----".as_slice(),
        b"-----BEGIN EC PRIVATE KEY-----".as_slice(),
        b"-----BEGIN DSA PRIVATE KEY-----".as_slice(),
        b"-----BEGIN PRIVATE KEY-----".as_slice(),
        b"-----BEGIN ENCRYPTED PRIVATE KEY-----".as_slice(),
    ]
    .contains(&header)
    {
        return Err("identityFileUnsupported".into());
    }
    let canonical = path.canonicalize().map_err(|_| "identityFileInvalid")?;
    #[cfg(windows)]
    let canonical = {
        let value = canonical.to_str().ok_or("identityFileInvalid")?;
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            PathBuf::from(format!(r"\\{unc}"))
        } else if let Some(drive) = value.strip_prefix(r"\\?\") {
            PathBuf::from(drive)
        } else {
            canonical
        }
    };
    Ok(canonical.to_string_lossy().into_owned())
}

fn new_id() -> BridgeResult<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "stateUnavailable")?;
    Ok(format!("manual|{}", hex::encode(bytes)))
}

fn validated(id: String, input: ManualConnectionInput) -> BridgeResult<ManualConnection> {
    if input.port == 0 {
        return Err("invalidConnectionPort".into());
    }
    let display_name = display_name(&input.display_name)?;
    let (user, host) = destination(&input.destination)?;
    Ok(ManualConnection {
        id,
        display_name,
        host,
        user,
        port: input.port,
        authentication: input.authentication,
        identity_file: identity_file(&input)?,
    })
}

pub fn add(input: ManualConnectionInput) -> BridgeResult<ManualConnection> {
    let mut store = load()?;
    if store.connections.len() >= MAX_CONNECTIONS {
        return Err("connectionLimitReached".into());
    }
    let connection = validated(new_id()?, input)?;
    store.connections.push(connection.clone());
    persist(&store)?;
    Ok(connection)
}

pub fn update(id: &str, input: ManualConnectionInput) -> BridgeResult<ManualConnection> {
    let mut store = load()?;
    let index = store
        .connections
        .iter()
        .position(|connection| connection.id == id)
        .ok_or("invalidTarget")?;
    let connection = validated(id.to_owned(), input)?;
    store.connections[index] = connection.clone();
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
        authentication_method: match connection.authentication {
            ManualAuthentication::Automatic => SshAuthMethod::Unknown,
            ManualAuthentication::Password => SshAuthMethod::Password,
            ManualAuthentication::IdentityFile => SshAuthMethod::IdentityFile,
        },
        available: identity_available,
        compatibility: if identity_available {
            RemoteTargetCompatibility::Compatible
        } else {
            RemoteTargetCompatibility::Unsupported
        },
        unavailable_reason: (!identity_available).then(|| "identityFileInvalid".into()),
        can_open_vscode: false,
        can_open_mobaxterm: false,
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
        assert_eq!(
            destination("student@192.168.10.24").unwrap(),
            (Some("student".into()), "192.168.10.24".into())
        );
        for invalid in [
            "",
            "bad host",
            "student@",
            "student;cmd@host",
            "a@b@host",
            "999.168.1.1",
            "192.168.1",
            "192.168.1.1:22",
            "[192.168.1.1]",
            "2001:db8::zz",
            "-lab.example.edu",
            "lab..example.edu",
        ] {
            assert!(destination(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn display_names_use_the_same_unicode_limit_as_the_form() {
        for name in ["Lab", &"中".repeat(32), &"🧪".repeat(32)] {
            assert!(display_name(name).is_ok());
        }
        for name in [
            "",
            "  ",
            "a\u{0007}b",
            "a\u{0085}b",
            &"x".repeat(33),
            &"中".repeat(33),
            &"🧪".repeat(33),
        ] {
            assert_eq!(display_name(name), Err("invalidConnectionName".into()));
        }
        assert_eq!(display_name(" Lab ").unwrap(), "Lab");
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
        let connection = validated("manual|fixed".into(), input).unwrap();
        assert_eq!(connection.id, "manual|fixed");
        assert_eq!(connection.host, "lab.example.edu");
        assert_eq!(connection.user.as_deref(), Some("student"));
    }

    #[test]
    fn legacy_connections_default_to_automatic_authentication() {
        let connection: ManualConnection = serde_json::from_str(
            r#"{"id":"manual|fixed","displayName":"Lab","host":"lab.example.edu","user":"student","port":22,"identityFile":null}"#,
        )
        .unwrap();
        assert_eq!(connection.authentication, ManualAuthentication::Automatic);
    }

    #[test]
    fn identity_picker_checks_headers_and_rejects_public_or_unsupported_files() {
        let path = std::env::temp_dir().join(format!(
            "proxyenv-key-{}",
            new_id().unwrap().replace('|', "-")
        ));
        let value = path.to_str().unwrap();
        for header in [
            "-----BEGIN OPENSSH PRIVATE KEY-----\n",
            "-----BEGIN RSA PRIVATE KEY-----\r\n",
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\n",
        ] {
            // Intentionally not a usable key: this test covers the format header, not login validity.
            fs::write(&path, format!("{header}unread-test-body\n")).unwrap();
            assert!(validate_identity_file(value).is_ok());
        }
        for header in [
            "ssh-ed25519 public-test-data\n",
            "PuTTY-User-Key-File-3: ssh-ed25519\n",
            "not a key\n",
        ] {
            fs::write(&path, header).unwrap();
            assert_eq!(
                validate_identity_file(value),
                Err("identityFileUnsupported".into())
            );
        }
        fs::write(&path, "").unwrap();
        assert_eq!(
            validate_identity_file(value),
            Err("identityFileInvalid".into())
        );
        fs::write(&path, "-----BEGIN OPENSSH PRIVATE KEY-----\n").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(1024 * 1024 + 1)
            .unwrap();
        assert_eq!(
            validate_identity_file(value),
            Err("identityFileInvalid".into())
        );
        fs::remove_file(&path).unwrap();
        assert_eq!(
            validate_identity_file(value),
            Err("identityFileInvalid".into())
        );
        assert_eq!(
            validate_identity_file(std::env::temp_dir().to_str().unwrap()),
            Err("identityFileInvalid".into())
        );
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

    #[test]
    fn rejects_device_and_illegal_identity_paths_before_filesystem_access() {
        for path in [
            r"\\?\C:\Users\example\.ssh\id_ed25519",
            "//?/C:/Users/example/.ssh/id_ed25519",
            r"C:\Users\example\.ssh\id_?.pem",
        ] {
            let input = ManualConnectionInput {
                display_name: "Lab".into(),
                destination: "lab.example.edu".into(),
                port: 22,
                authentication: ManualAuthentication::IdentityFile,
                identity_file: Some(path.into()),
            };
            assert_eq!(identity_file(&input), Err("identityFileInvalid".into()));
        }
    }
}
