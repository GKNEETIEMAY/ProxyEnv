use super::{
    connections::{self, ManualAuthentication},
    mobaxterm,
    target_os::{linux, TargetOs},
    BridgeResult, RemoteTarget, RemoteTargetCompatibility, RemoteTargetSource, Request,
    SshAuthMethod,
};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
enum CommandMode {
    Batch,
    Interactive,
    CachedPassword,
    ManagedTerminal,
}

fn command_with_mode(mode: CommandMode) -> Command {
    #[cfg(windows)]
    let mut command = Command::new(
        std::path::PathBuf::from(
            std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
        )
        .join("System32/OpenSSH/ssh.exe"),
    );
    #[cfg(not(windows))]
    let mut command = Command::new("ssh");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command.arg(if matches!(mode, CommandMode::ManagedTerminal) {
        "-tt"
    } else {
        "-T"
    });
    command.args([
        "-oConnectTimeout=8",
        "-oConnectionAttempts=1",
        "-oServerAliveInterval=5",
        "-oServerAliveCountMax=2",
        "-oForwardAgent=no",
        "-oForwardX11=no",
        "-oPermitLocalCommand=no",
        "-oRemoteCommand=none",
        "-oControlMaster=no",
        "-oControlPath=none",
        "-oExitOnForwardFailure=yes",
        "-oForkAfterAuthentication=no",
    ]);
    if matches!(mode, CommandMode::Batch) {
        command.args(["-oStrictHostKeyChecking=yes", "-oBatchMode=yes"]);
    } else if matches!(mode, CommandMode::CachedPassword) {
        command.args([
            "-oStrictHostKeyChecking=yes",
            "-oBatchMode=no",
            "-oPasswordAuthentication=yes",
            "-oKbdInteractiveAuthentication=yes",
            "-oNumberOfPasswordPrompts=1",
            "-oPreferredAuthentications=publickey,keyboard-interactive,password",
        ]);
    } else {
        command.args([
            "-oStrictHostKeyChecking=ask",
            "-oBatchMode=no",
            "-oPasswordAuthentication=yes",
            "-oKbdInteractiveAuthentication=yes",
            "-oNumberOfPasswordPrompts=3",
            "-oPreferredAuthentications=publickey,keyboard-interactive,password",
        ]);
    }
    command
}

#[cfg(test)]
pub fn command() -> Command {
    command_with_mode(CommandMode::Batch)
}

pub fn safe_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}

pub fn safe_host(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && (value.parse::<std::net::IpAddr>().is_ok()
            || (value.as_bytes()[0].is_ascii_alphanumeric()
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')
                })))
}

pub fn aliases_from(text: &str) -> Vec<String> {
    let mut aliases = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut words = line
            .split(|c: char| c.is_whitespace() || c == '=')
            .filter(|s| !s.is_empty());
        if words.next().is_some_and(|s| s.eq_ignore_ascii_case("host")) {
            for alias in words {
                let alias = alias.trim_matches('"');
                if safe_name(alias)
                    && !aliases
                        .iter()
                        .any(|a: &String| a.eq_ignore_ascii_case(alias))
                {
                    aliases.push(alias.to_owned());
                }
            }
        }
    }
    aliases
}

fn config_text_at(path: &Path) -> BridgeResult<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    let metadata = std::fs::metadata(path).map_err(|_| "sshConfigMissing")?;
    if metadata.len() > 1024 * 1024 {
        return Err("sshConfigUnsafe".into());
    }
    std::fs::read_to_string(path).map_err(|_| "sshConfigMissing".into())
}

#[derive(Debug, Clone)]
enum Connection {
    Config {
        alias: String,
        config: Option<PathBuf>,
    },
    Direct {
        host: String,
        user: Option<String>,
        port: u16,
        authentication: ManualAuthentication,
        identity_file: Option<PathBuf>,
    },
}

#[derive(Debug, Clone)]
struct ResolvedTarget {
    public: RemoteTarget,
    connection: Connection,
    config_path: PathBuf,
}

fn path_key(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(path.to_string_lossy().as_bytes()))[..16].to_owned()
}

fn target_id(source: RemoteTargetSource, path: &Path, name: &str) -> String {
    let source = match source {
        RemoteTargetSource::Openssh => "openssh",
        RemoteTargetSource::Vscode => "vscode",
        RemoteTargetSource::Mobaxterm => "mobaxterm",
        RemoteTargetSource::Manual => "manual",
    };
    format!("{source}|{}|{name}", path_key(path))
}

fn display_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(suffix) = path.strip_prefix(&home) {
            return format!("~{}{}", std::path::MAIN_SEPARATOR, suffix.display());
        }
    }
    path.to_string_lossy().into_owned()
}

#[derive(Debug, Default)]
struct EffectiveTargetMetadata {
    host: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<String>,
    authentication_method: SshAuthMethod,
}

fn existing_identity_file(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('"');
    if value.chars().any(char::is_control)
        || value.starts_with(r"\\?\")
        || value.starts_with("//?/")
        || value.starts_with(r"\\.\")
        || value.starts_with("//./")
    {
        return None;
    }
    let home = dirs::home_dir();
    let path = if let Some(suffix) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
        .or_else(|| value.strip_prefix("%d/"))
        .or_else(|| value.strip_prefix("%d\\"))
    {
        home?.join(suffix)
    } else {
        PathBuf::from(value)
    };
    if !path.is_file() {
        return None;
    }
    let canonical = path.canonicalize().unwrap_or(path);
    #[cfg(windows)]
    let canonical = {
        let value = canonical.to_str()?;
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            PathBuf::from(format!(r"\\{unc}"))
        } else if let Some(drive) = value.strip_prefix(r"\\?\") {
            PathBuf::from(drive)
        } else {
            canonical
        }
    };
    Some(canonical.to_string_lossy().into_owned())
}

fn default_identity_file() -> Option<String> {
    let ssh = dirs::home_dir()?.join(".ssh");
    ["id_ed25519", "id_ecdsa", "id_rsa"]
        .into_iter()
        .find_map(|name| existing_identity_file(&ssh.join(name).to_string_lossy()))
}

fn effective_target_metadata(text: &str) -> BridgeResult<EffectiveTargetMetadata> {
    let mut metadata = EffectiveTargetMetadata::default();
    let mut pubkey_authentication = true;
    let mut password_authentication = true;
    let mut keyboard_interactive = true;
    let mut preferred_authentications: Option<Vec<String>> = None;

    for line in text.lines() {
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        let value = value.trim();
        match key {
            "user" if value.eq_ignore_ascii_case("root") => return Err("rootForbidden".into()),
            "user" if !safe_name(value) => return Err("sshConfigUnsafe".into()),
            "user" => metadata.user = Some(value.to_owned()),
            "hostname" if !(safe_name(value) || value.parse::<std::net::IpAddr>().is_ok()) => {
                return Err("sshConfigUnsafe".into());
            }
            "hostname" => metadata.host = Some(value.to_owned()),
            "port" => {
                metadata.port = Some(
                    value
                        .parse::<u16>()
                        .ok()
                        .filter(|port| *port > 0)
                        .ok_or("sshConfigUnsafe")?,
                );
            }
            "identityfile" if metadata.identity_file.is_none() => {
                metadata.identity_file = existing_identity_file(value);
            }
            "pubkeyauthentication" => pubkey_authentication = !value.eq_ignore_ascii_case("no"),
            "passwordauthentication" => password_authentication = !value.eq_ignore_ascii_case("no"),
            "kbdinteractiveauthentication" | "challengeresponseauthentication" => {
                keyboard_interactive = !value.eq_ignore_ascii_case("no");
            }
            "preferredauthentications" => {
                preferred_authentications = Some(
                    value
                        .split(',')
                        .map(|method| method.trim().to_ascii_lowercase())
                        .collect(),
                );
            }
            "localforward" | "remoteforward" | "dynamicforward" => {
                return Err("sshConfigUnsafe".into());
            }
            _ => (),
        }
    }

    metadata.authentication_method = if metadata.identity_file.is_some() {
        SshAuthMethod::IdentityFile
    } else if let Some(preferred) = preferred_authentications {
        if !preferred.iter().any(|method| method == "publickey")
            && preferred.iter().any(|method| method == "password")
        {
            SshAuthMethod::Password
        } else if !preferred.iter().any(|method| method == "publickey")
            && preferred
                .iter()
                .any(|method| method == "keyboard-interactive")
        {
            SshAuthMethod::KeyboardInteractive
        } else {
            SshAuthMethod::Unknown
        }
    } else if !pubkey_authentication && password_authentication {
        SshAuthMethod::Password
    } else if !pubkey_authentication && keyboard_interactive {
        SshAuthMethod::KeyboardInteractive
    } else if std::env::var_os("SSH_AUTH_SOCK").is_some() {
        SshAuthMethod::Agent
    } else {
        SshAuthMethod::Unknown
    };
    Ok(metadata)
}

fn host_pattern_matches(pattern: &str, alias: &str) -> bool {
    let pattern = pattern.as_bytes();
    let alias = alias.as_bytes();
    let mut matched = vec![false; alias.len() + 1];
    matched[0] = true;
    for byte in pattern {
        let mut next = vec![false; alias.len() + 1];
        match byte.to_ascii_lowercase() {
            b'*' => {
                next[0] = matched[0];
                for index in 1..=alias.len() {
                    next[index] = matched[index] || next[index - 1];
                }
            }
            b'?' => {
                next[1..].copy_from_slice(&matched[..alias.len()]);
            }
            expected => {
                for index in 1..=alias.len() {
                    next[index] =
                        matched[index - 1] && alias[index - 1].to_ascii_lowercase() == expected;
                }
            }
        }
        matched = next;
    }
    matched[alias.len()]
}

fn host_block_matches(value: &str, alias: &str) -> bool {
    let mut positive = false;
    for pattern in value.split_whitespace() {
        let (negated, pattern) = pattern
            .strip_prefix('!')
            .map_or((false, pattern), |pattern| (true, pattern));
        if host_pattern_matches(pattern.trim_matches('"'), alias) {
            if negated {
                return false;
            }
            positive = true;
        }
    }
    positive
}

fn config_line(line: &str) -> Option<(&str, &str)> {
    let line = line.split('#').next()?.trim();
    if line.is_empty() {
        return None;
    }
    let separator = line.find(|character: char| character.is_whitespace() || character == '=')?;
    let key = &line[..separator];
    let value = line[separator..]
        .trim_start_matches(|character: char| character.is_whitespace() || character == '=')
        .trim();
    (!key.is_empty() && !value.is_empty()).then_some((key, value))
}

fn config_target_metadata(text: &str, alias: &str) -> BridgeResult<EffectiveTargetMetadata> {
    let mut metadata = EffectiveTargetMetadata {
        host: Some(alias.to_owned()),
        port: Some(22),
        ..EffectiveTargetMetadata::default()
    };
    let mut active = true;
    let mut host_set = false;
    let mut port_set = false;
    let mut pubkey_authentication = None;
    let mut password_authentication = None;
    let mut keyboard_interactive = None;
    let mut preferred_authentications: Option<Vec<String>> = None;
    let mut identity_configured = false;

    for line in text.lines() {
        let Some((key, value)) = config_line(line) else {
            continue;
        };
        if key.eq_ignore_ascii_case("host") {
            active = host_block_matches(value, alias);
            continue;
        }
        if !active {
            continue;
        }
        if key.eq_ignore_ascii_case("hostname") && !host_set {
            let value = value.trim_matches('"');
            if !safe_host(value) {
                return Err("sshConfigUnsafe".into());
            }
            metadata.host = Some(value.to_owned());
            host_set = true;
        } else if key.eq_ignore_ascii_case("user") && metadata.user.is_none() {
            let value = value.trim_matches('"');
            if value.eq_ignore_ascii_case("root") {
                return Err("rootForbidden".into());
            }
            if !safe_name(value) {
                return Err("sshConfigUnsafe".into());
            }
            metadata.user = Some(value.to_owned());
        } else if key.eq_ignore_ascii_case("port") && !port_set {
            metadata.port = Some(
                value
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port > 0)
                    .ok_or("sshConfigUnsafe")?,
            );
            port_set = true;
        } else if key.eq_ignore_ascii_case("identityfile") && metadata.identity_file.is_none() {
            identity_configured = true;
            metadata.identity_file = existing_identity_file(value);
        } else if key.eq_ignore_ascii_case("pubkeyauthentication")
            && pubkey_authentication.is_none()
        {
            pubkey_authentication = Some(!value.eq_ignore_ascii_case("no"));
        } else if key.eq_ignore_ascii_case("passwordauthentication")
            && password_authentication.is_none()
        {
            password_authentication = Some(!value.eq_ignore_ascii_case("no"));
        } else if (key.eq_ignore_ascii_case("kbdinteractiveauthentication")
            || key.eq_ignore_ascii_case("challengeresponseauthentication"))
            && keyboard_interactive.is_none()
        {
            keyboard_interactive = Some(!value.eq_ignore_ascii_case("no"));
        } else if key.eq_ignore_ascii_case("preferredauthentications")
            && preferred_authentications.is_none()
        {
            preferred_authentications = Some(
                value
                    .split(',')
                    .map(|method| method.trim().to_ascii_lowercase())
                    .collect(),
            );
        }
    }

    if !identity_configured && pubkey_authentication != Some(false) {
        metadata.identity_file = default_identity_file();
    }

    metadata.authentication_method = if metadata.identity_file.is_some() {
        SshAuthMethod::IdentityFile
    } else if let Some(preferred) = preferred_authentications {
        if !preferred.iter().any(|method| method == "publickey")
            && preferred.iter().any(|method| method == "password")
        {
            SshAuthMethod::Password
        } else if !preferred.iter().any(|method| method == "publickey")
            && preferred
                .iter()
                .any(|method| method == "keyboard-interactive")
        {
            SshAuthMethod::KeyboardInteractive
        } else {
            SshAuthMethod::Unknown
        }
    } else if pubkey_authentication == Some(false) && password_authentication != Some(false) {
        SshAuthMethod::Password
    } else if pubkey_authentication == Some(false) && keyboard_interactive != Some(false) {
        SshAuthMethod::KeyboardInteractive
    } else if std::env::var_os("SSH_AUTH_SOCK").is_some() {
        SshAuthMethod::Agent
    } else {
        SshAuthMethod::Unknown
    };
    Ok(metadata)
}

fn config_targets(
    path: &Path,
    source: RemoteTargetSource,
    source_label: &str,
    can_open_vscode: bool,
    can_open_mobaxterm: bool,
) -> BridgeResult<Vec<ResolvedTarget>> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let config_text = config_text_at(path)?;
    Ok(aliases_from(&config_text)
        .into_iter()
        .map(|alias| {
            let config = (source == RemoteTargetSource::Vscode).then_some(canonical.clone());
            let metadata = config_target_metadata(&config_text, &alias).unwrap_or_else(|_| {
                EffectiveTargetMetadata {
                    host: Some(alias.clone()),
                    port: Some(22),
                    ..EffectiveTargetMetadata::default()
                }
            });
            ResolvedTarget {
                public: RemoteTarget {
                    id: target_id(source, &canonical, &alias),
                    display_name: alias.clone(),
                    source,
                    source_label: source_label.into(),
                    config_path: display_path(&canonical),
                    ssh_alias: Some(alias.clone()),
                    host: metadata.host,
                    user: metadata.user,
                    port: metadata.port,
                    identity_file: metadata.identity_file,
                    authentication_method: metadata.authentication_method,
                    available: true,
                    compatibility: RemoteTargetCompatibility::Compatible,
                    unavailable_reason: None,
                    can_open_vscode,
                    can_open_mobaxterm,
                },
                connection: Connection::Config { alias, config },
                config_path: canonical.clone(),
            }
        })
        .collect())
}

fn discovered() -> BridgeResult<Vec<ResolvedTarget>> {
    let can_open_mobaxterm = mobaxterm::available();
    let default = dirs::home_dir()
        .ok_or("sshConfigMissing")?
        .join(".ssh/config");
    let vscode_setting = super::vscode::custom_ssh_config();
    let vscode_config = vscode_setting.as_ref().ok().and_then(|path| path.clone());
    let default_canonical = default.canonicalize().ok();
    let vscode_uses_default = vscode_setting.is_ok()
        && (vscode_config.is_none()
            || (default_canonical.is_some()
                && vscode_config.as_ref() == default_canonical.as_ref()));
    let mut result = config_targets(
        &default,
        RemoteTargetSource::Openssh,
        "OpenSSH",
        vscode_uses_default,
        can_open_mobaxterm,
    )?;
    if let Some(path) = vscode_config {
        if default_canonical.as_ref() != Some(&path) {
            result.extend(config_targets(
                &path,
                RemoteTargetSource::Vscode,
                "VS Code Remote",
                true,
                can_open_mobaxterm,
            )?);
        }
    }
    for path in mobaxterm::config_paths() {
        let Ok(sessions) = mobaxterm::sessions(&path) else {
            // MobaXterm is an optional source. A malformed or unreadable INI
            // must not hide otherwise valid OpenSSH or VS Code targets.
            continue;
        };
        for session in sessions {
            let compatible = session.compatible;
            let public = RemoteTarget {
                id: target_id(RemoteTargetSource::Mobaxterm, &path, &session.name),
                display_name: session.name.clone(),
                source: RemoteTargetSource::Mobaxterm,
                source_label: "MobaXterm".into(),
                config_path: display_path(&path),
                ssh_alias: None,
                host: session.host.clone(),
                user: session.user.clone(),
                port: session.port,
                identity_file: None,
                authentication_method: SshAuthMethod::Unknown,
                available: compatible,
                compatibility: if compatible {
                    RemoteTargetCompatibility::Compatible
                } else {
                    RemoteTargetCompatibility::Unsupported
                },
                unavailable_reason: (!compatible).then(|| "mobaSessionUnsupported".into()),
                can_open_vscode: false,
                can_open_mobaxterm,
            };
            if let (Some(host), Some(user), Some(port)) = (session.host, session.user, session.port)
            {
                result.push(ResolvedTarget {
                    public,
                    connection: Connection::Direct {
                        host,
                        user: Some(user),
                        port,
                        authentication: ManualAuthentication::Automatic,
                        identity_file: None,
                    },
                    config_path: path.clone(),
                });
            } else {
                result.push(ResolvedTarget {
                    public,
                    connection: Connection::Direct {
                        host: String::new(),
                        user: None,
                        port: 0,
                        authentication: ManualAuthentication::Automatic,
                        identity_file: None,
                    },
                    config_path: path.clone(),
                });
            }
        }
    }
    for connection in connections::all()? {
        let mut public = connections::target(&connection);
        public.can_open_mobaxterm = can_open_mobaxterm;
        // Remote - SSH accepts a structured direct authority. A non-default
        // identity or a different config cannot be silently dropped at launch.
        if let Some(alias) = result.iter().find(|candidate| {
            candidate.public.can_open_vscode
                && candidate.public.ssh_alias.is_some()
                && candidate.public.host == public.host
                && candidate.public.user == public.user
                && candidate.public.port == public.port
                && candidate.public.identity_file == public.identity_file
        }) {
            public.ssh_alias = alias.public.ssh_alias.clone();
            public.can_open_vscode = public.available;
        } else {
            public.can_open_vscode =
                super::vscode::supports_direct_target(&public, vscode_uses_default);
        }
        result.push(ResolvedTarget {
            public,
            connection: Connection::Direct {
                host: connection.host,
                user: connection.user,
                port: connection.port,
                authentication: connection.authentication,
                identity_file: connection.identity_file.map(PathBuf::from),
            },
            config_path: PathBuf::new(),
        });
    }
    Ok(result)
}

pub fn targets() -> BridgeResult<Vec<RemoteTarget>> {
    Ok(discovered()?
        .into_iter()
        .map(|target| target.public)
        .collect())
}

pub fn target(id: &str) -> BridgeResult<RemoteTarget> {
    discovered()?
        .into_iter()
        .find(|target| target.public.id == id)
        .map(|target| target.public)
        .ok_or_else(|| "invalidTarget".into())
}

pub fn target_config_path(id: &str) -> BridgeResult<PathBuf> {
    let path = resolve(id)?.config_path;
    if !path.is_absolute() || !path.is_file() {
        return Err("sshConfigMissing".into());
    }
    Ok(path)
}

fn resolve(id: &str) -> BridgeResult<ResolvedTarget> {
    discovered()?
        .into_iter()
        .find(|target| target.public.id == id)
        .ok_or_else(|| "invalidTarget".into())
}

fn target_command_with_mode(id: &str, mode: CommandMode) -> BridgeResult<(Command, String)> {
    resolved_target_command(&resolve(id)?, mode)
}

fn resolved_target_command(
    target: &ResolvedTarget,
    mode: CommandMode,
) -> BridgeResult<(Command, String)> {
    if !target.public.available
        || target.public.compatibility != RemoteTargetCompatibility::Compatible
    {
        return Err(target
            .public
            .unavailable_reason
            .clone()
            .unwrap_or_else(|| "targetUnsupported".into()));
    }
    let mut cmd = command_with_mode(mode);
    let destination = match &target.connection {
        Connection::Config { alias, config } => {
            if let Some(path) = config {
                cmd.arg("-F").arg(path);
            }
            alias.clone()
        }
        Connection::Direct {
            host,
            user,
            port,
            authentication,
            identity_file,
        } => {
            if !safe_host(host)
                || user.as_deref().is_some_and(|user| !safe_name(user))
                || *port == 0
            {
                return Err("mobaSessionUnsupported".into());
            }
            cmd.arg("-p").arg(port.to_string());
            if let Some(user) = user {
                cmd.arg("-l").arg(user);
            }
            if *authentication == ManualAuthentication::Password {
                cmd.args([
                    "-oPubkeyAuthentication=no",
                    "-oPreferredAuthentications=keyboard-interactive,password",
                ]);
            }
            if let Some(identity_file) = identity_file {
                let metadata =
                    std::fs::symlink_metadata(identity_file).map_err(|_| "identityFileInvalid")?;
                if !identity_file.is_absolute()
                    || !metadata.is_file()
                    || metadata.file_type().is_symlink()
                {
                    return Err("identityFileInvalid".into());
                }
                cmd.arg("-oIdentitiesOnly=yes").arg("-i").arg(identity_file);
            }
            host.clone()
        }
    };
    Ok((cmd, destination))
}

pub fn launch_mobaxterm_target(id: &str) -> BridgeResult<()> {
    let target = resolve(id)?;
    if !target.public.available
        || target.public.compatibility != RemoteTargetCompatibility::Compatible
    {
        return Err("mobaSessionUnsupported".into());
    }
    if target.public.source == RemoteTargetSource::Mobaxterm {
        mobaxterm::launch(&target.config_path, &target.public.display_name)
    } else {
        mobaxterm::launch_application()
    }
}

fn target_command_with_batch_mode(id: &str, batch_mode: bool) -> BridgeResult<(Command, String)> {
    target_command_with_mode(
        id,
        if batch_mode {
            CommandMode::Batch
        } else {
            CommandMode::Interactive
        },
    )
}

#[cfg(windows)]
fn remote_resolved_target_command(
    target: &ResolvedTarget,
    fingerprint: &str,
) -> BridgeResult<(Command, String)> {
    let Some((password, entropy)) = super::credential_cache::terminal_payload(fingerprint)? else {
        return resolved_target_command(target, CommandMode::Batch);
    };
    let (mut command, destination) = resolved_target_command(target, CommandMode::CachedPassword)?;
    let askpass = std::env::current_exe().map_err(|_| "processFailed")?;
    command
        .env("PROXYENV_SSH_ASKPASS", "1")
        .env("PROXYENV_SSH_PASSWORD", password)
        .env("PROXYENV_SSH_PASSWORD_ENTROPY", entropy)
        .env("SSH_ASKPASS", askpass)
        .env("SSH_ASKPASS_REQUIRE", "force")
        .env("DISPLAY", "proxyenv:0");
    Ok((command, destination))
}

#[cfg(not(windows))]
fn remote_resolved_target_command(
    target: &ResolvedTarget,
    _fingerprint: &str,
) -> BridgeResult<(Command, String)> {
    resolved_target_command(target, CommandMode::Batch)
}

pub(super) fn interactive_target_command(id: &str) -> BridgeResult<(Command, String)> {
    target_command_with_batch_mode(id, false)
}

#[cfg(windows)]
fn powershell_terminal_command(
    ssh_command: &Command,
    credential: Option<(String, String)>,
) -> BridgeResult<Command> {
    let launch = serde_json::json!({
        "program": ssh_command.get_program().to_string_lossy(),
        "arguments": ssh_command
            .get_args()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>(),
    });
    let payload = serde_json::to_string(&launch).map_err(|_| "stateUnavailable")?;
    let powershell = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
    )
    .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut terminal = Command::new(powershell);
    terminal
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NoExit",
            "-Command",
            "$ErrorActionPreference='Stop'; $launch=ConvertFrom-Json -InputObject $env:PROXYENV_SSH_LAUNCH; Remove-Item Env:PROXYENV_SSH_LAUNCH -ErrorAction SilentlyContinue; $Host.UI.RawUI.WindowTitle='RouteSpan Proxy Terminal'; [string[]]$sshArguments=$launch.arguments; & ([string]$launch.program) @sshArguments; Remove-Item Env:PROXYENV_SSH_PASSWORD,Env:PROXYENV_SSH_PASSWORD_ENTROPY,Env:PROXYENV_SSH_ASKPASS,Env:SSH_ASKPASS,Env:SSH_ASKPASS_REQUIRE,Env:DISPLAY -ErrorAction SilentlyContinue; if ($LASTEXITCODE -ne 0) { Write-Host ''; Write-Host ('OpenSSH exited with code {0}.' -f $LASTEXITCODE) -ForegroundColor Red }",
        ])
        .env("PROXYENV_SSH_LAUNCH", payload);
    if let Some((password, entropy)) = credential {
        let askpass = std::env::current_exe().map_err(|_| "processFailed")?;
        terminal
            .env("PROXYENV_SSH_ASKPASS", "1")
            .env("PROXYENV_SSH_PASSWORD", password)
            .env("PROXYENV_SSH_PASSWORD_ENTROPY", entropy)
            .env("SSH_ASKPASS", askpass)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("DISPLAY", "proxyenv:0");
    } else {
        for name in [
            "PROXYENV_SSH_ASKPASS",
            "PROXYENV_SSH_PASSWORD",
            "PROXYENV_SSH_PASSWORD_ENTROPY",
            "SSH_ASKPASS",
            "SSH_ASKPASS_REQUIRE",
            "DISPLAY",
        ] {
            terminal.env_remove(name);
        }
    }
    Ok(terminal)
}

#[cfg(windows)]
fn quote_windows_argument(argument: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    let argument = argument.encode_wide().collect::<Vec<_>>();
    let must_quote = argument.is_empty()
        || argument
            .iter()
            .any(|character| matches!(*character, 0x20 | 0x09 | 0x22));
    if !must_quote {
        return argument;
    }
    let mut quoted = vec![b'"' as u16];
    let mut backslashes = 0usize;
    for character in argument {
        if character == b'\\' as u16 {
            backslashes += 1;
        } else if character == b'"' as u16 {
            quoted.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2 + 1));
            quoted.push(character);
            backslashes = 0;
        } else {
            quoted.extend(std::iter::repeat_n(b'\\' as u16, backslashes));
            quoted.push(character);
            backslashes = 0;
        }
    }
    quoted.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2));
    quoted.push(b'"' as u16);
    quoted
}

#[cfg(windows)]
fn command_line(command: &Command) -> Vec<u16> {
    let mut line = quote_windows_argument(command.get_program());
    for argument in command.get_args() {
        line.push(b' ' as u16);
        line.extend(quote_windows_argument(argument));
    }
    line.push(0);
    line
}

#[cfg(windows)]
fn environment_block(command: &Command) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    let mut environment = std::env::vars_os().collect::<Vec<_>>();
    for (name, value) in command.get_envs() {
        environment.retain(|(existing, _)| {
            !existing
                .to_string_lossy()
                .eq_ignore_ascii_case(&name.to_string_lossy())
        });
        if let Some(value) = value {
            environment.push((name.to_os_string(), value.to_os_string()));
        }
    }
    environment.sort_by(|left, right| {
        left.0
            .to_string_lossy()
            .to_lowercase()
            .cmp(&right.0.to_string_lossy().to_lowercase())
    });
    let mut block = Vec::new();
    for (name, value) in environment {
        block.extend(name.encode_wide());
        block.push(b'=' as u16);
        block.extend(value.encode_wide());
        block.push(0);
    }
    block.push(0);
    block
}

#[cfg(windows)]
fn spawn_new_console(command: &Command) -> BridgeResult<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::{PCWSTR, PWSTR},
        Win32::{
            Foundation::CloseHandle,
            System::Threading::{
                CreateProcessW, CREATE_NEW_CONSOLE, CREATE_UNICODE_ENVIRONMENT,
                PROCESS_INFORMATION, STARTUPINFOW,
            },
        },
    };

    let mut program = command.get_program().encode_wide().collect::<Vec<_>>();
    program.push(0);
    let mut command_line = command_line(command);
    let environment = environment_block(command);
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..STARTUPINFOW::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR(program.as_ptr()),
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NEW_CONSOLE | CREATE_UNICODE_ENVIRONMENT,
            Some(environment.as_ptr().cast()),
            PCWSTR::null(),
            &startup,
            &mut process,
        )
        .map_err(|_| "processFailed")?;
        let _ = CloseHandle(process.hThread);
        let _ = CloseHandle(process.hProcess);
    }
    Ok(())
}

pub fn launch_managed_terminal(
    target_id: &str,
    session_id: &str,
    fingerprint: &str,
) -> BridgeResult<()> {
    launch_terminal(
        target_id,
        fingerprint,
        Some(linux::managed_terminal_remote_command(session_id)?),
    )
}

pub fn launch_manual_terminal(target_id: &str, fingerprint: &str) -> BridgeResult<()> {
    launch_terminal(target_id, fingerprint, None)
}

fn launch_terminal(
    target_id: &str,
    fingerprint: &str,
    remote_command: Option<String>,
) -> BridgeResult<()> {
    #[cfg(windows)]
    {
        let (mut command, destination) =
            target_command_with_mode(target_id, CommandMode::ManagedTerminal)?;
        command.arg("-oClearAllForwardings=yes").arg(destination);
        if let Some(remote_command) = remote_command {
            command.arg(remote_command);
        }
        let credential = super::credential_cache::terminal_payload(fingerprint)?;
        let terminal = powershell_terminal_command(&command, credential)?;
        spawn_new_console(&terminal)
    }
    #[cfg(not(windows))]
    {
        let _ = (target_id, fingerprint, remote_command);
        Err("processFailed".into())
    }
}

// Reuse OpenSSH's resolution for IdentityFile, ProxyJump and ssh-agent. Refuse
// preconfigured forwards so a bridge can only open the reviewed endpoints.
pub fn validate_target(target_id: &str) -> BridgeResult<()> {
    effective_target(target_id).map(|_| ())
}
pub fn fingerprint(target_id: &str) -> BridgeResult<String> {
    Ok(config_fingerprint(target_id, &effective_target(target_id)?))
}

fn config_fingerprint(target_id: &str, effective: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(format!("{target_id}\n{effective}")))
}

pub fn non_interactive_auth_method(target_id: &str) -> SshAuthMethod {
    let Ok(effective) = effective_target(target_id) else {
        return SshAuthMethod::Unknown;
    };
    effective_target_metadata(&effective)
        .map(|metadata| metadata.authentication_method)
        .unwrap_or_default()
}
fn effective_target(target_id: &str) -> BridgeResult<String> {
    effective_resolved_target(&resolve(target_id)?)
}

fn effective_resolved_target(target: &ResolvedTarget) -> BridgeResult<String> {
    effective_resolved_target_before(target, Instant::now() + Duration::from_secs(12))
}

fn effective_resolved_target_before(
    target: &ResolvedTarget,
    deadline: Instant,
) -> BridgeResult<String> {
    let (mut cmd, destination) = resolved_target_command(target, CommandMode::Batch)?;
    cmd.args(["-G", &destination]);
    let text = output_before(cmd, None, deadline)?;
    effective_target_metadata(&text)?;
    Ok(text)
}

pub(super) struct BoundTarget {
    target: ResolvedTarget,
    fingerprint: String,
    host: String,
    user: String,
    port: u16,
}

// The production constructor belongs to the SSH host-verification boundary.
// M3.1 does not yet have an authenticated transport that can issue this value.
pub(super) struct VerifiedHostIdentity<'a> {
    binding: &'a BoundTarget,
    generation: u64,
    key_sha256: [u8; 32],
}

impl<'a> VerifiedHostIdentity<'a> {
    pub(super) fn key_for(&self, target: &BoundTarget, generation: u64) -> BridgeResult<[u8; 32]> {
        if self.binding.fingerprint != target.fingerprint || self.generation != generation {
            return Err("hostKey".into());
        }
        Ok(self.key_sha256)
    }

    #[cfg(test)]
    pub(super) fn fixture(binding: &'a BoundTarget, generation: u64, key_sha256: [u8; 32]) -> Self {
        Self {
            binding,
            generation,
            key_sha256,
        }
    }
}

impl BoundTarget {
    pub(super) fn capture_connected(id: &str, fingerprint: &str) -> BridgeResult<Self> {
        let target = Self::from_resolved(resolve(id)?)?;
        target.check_fingerprint(fingerprint)?;
        Ok(target)
    }

    pub(super) fn checked_config_fingerprint(&self, effective: &str) -> BridgeResult<&str> {
        self.check_fingerprint(&config_fingerprint(&self.target.public.id, effective))?;
        Ok(&self.fingerprint)
    }

    pub(super) fn endpoint_identity(&self) -> (&str, &str, u16) {
        (&self.host, &self.user, self.port)
    }

    #[cfg(test)]
    pub(super) fn fixture(id: &str, effective: &str) -> BridgeResult<Self> {
        let test_connection = connections::ManualConnection {
            id: id.to_owned(),
            display_name: "Windows plan fixture".into(),
            host: "fixture.example".into(),
            user: None,
            port: 22,
            authentication: ManualAuthentication::Automatic,
            identity_file: None,
        };
        Self::from_effective(
            ResolvedTarget {
                public: connections::target(&test_connection),
                connection: Connection::Config {
                    alias: id.to_owned(),
                    config: None,
                },
                config_path: PathBuf::new(),
            },
            effective,
        )
    }

    fn from_resolved(target: ResolvedTarget) -> BridgeResult<Self> {
        let effective = effective_resolved_target(&target)?;
        Self::from_effective(target, &effective)
    }

    fn from_effective(target: ResolvedTarget, effective: &str) -> BridgeResult<Self> {
        let metadata = effective_target_metadata(effective)?;
        Ok(Self {
            fingerprint: config_fingerprint(&target.public.id, effective),
            target,
            host: metadata.host.ok_or("sshConfigUnsafe")?,
            user: metadata.user.ok_or("sshConfigUnsafe")?,
            port: metadata.port.ok_or("sshConfigUnsafe")?,
        })
    }

    fn check_resolved(&self, current: &ResolvedTarget, deadline: Instant) -> BridgeResult<()> {
        if current.public.id != self.target.public.id {
            return Err("sshConfigChanged".into());
        }
        self.checked_config_fingerprint(&effective_resolved_target_before(current, deadline)?)
            .map(|_| ())
    }

    fn check_fingerprint(&self, fingerprint: &str) -> BridgeResult<()> {
        if self.fingerprint != fingerprint {
            return Err("sshConfigChanged".into());
        }
        Ok(())
    }

    fn check_current(&self) -> BridgeResult<()> {
        self.check_current_before(Instant::now() + Duration::from_secs(12))
    }

    fn check_current_before(&self, deadline: Instant) -> BridgeResult<()> {
        let current = resolve(&self.target.public.id).map_err(|code| {
            if code == "invalidTarget" {
                "sshConfigChanged".into()
            } else {
                code
            }
        })?;
        self.check_resolved(&current, deadline)
    }

    fn pin_endpoint(&self, command: &mut Command) {
        if matches!(&self.target.connection, Connection::Config { .. }) {
            // Keep the alias for its SSH options, but do not let a late edit
            // of HostName, User, or Port redirect a command after verification.
            let (host, user, port) = self.endpoint_identity();
            command.args([
                format!("-oHostName={host}"),
                format!("-oUser={user}"),
                format!("-oPort={port}"),
            ]);
        }
    }

    fn remote_command(&self) -> BridgeResult<(Command, String)> {
        self.check_current()?;
        self.build_remote_command()
    }

    fn build_remote_command(&self) -> BridgeResult<(Command, String)> {
        let (mut command, destination) =
            remote_resolved_target_command(&self.target, &self.fingerprint)?;
        self.pin_endpoint(&mut command);
        Ok((command, destination))
    }
}

#[cfg(windows)]
pub(super) fn probe_target_platform(id: String) -> BridgeResult<super::TargetPlatformProbe> {
    use super::target_os::windows::{self, WindowsProbeReason, WindowsTargetProbe};
    use super::{TargetPlatform, TargetPlatformProbe};

    let deadline = Instant::now() + windows::PROBE_BUDGET;
    let detect = || -> BridgeResult<(TargetPlatform, WindowsTargetProbe)> {
        let target = resolve(&id)?;
        let effective = effective_resolved_target_before(&target, deadline)?;
        if !windows::allows_direct_probe(&effective) {
            return Ok((
                TargetPlatform::Unknown,
                WindowsTargetProbe::Unknown {
                    reason: WindowsProbeReason::SshConfigUnsupported,
                },
            ));
        }
        let target = BoundTarget::from_effective(target, &effective)?;
        let run = |remote_command: &str| {
            target.check_current_before(deadline)?;
            let (mut command, destination) = target.build_remote_command()?;
            command
                .args(windows::SSH_READ_ONLY_OPTIONS)
                .arg(destination)
                .arg(remote_command);
            windows::capture(command, deadline)
        };
        let platform = run(TargetOs::PROBE_COMMAND)?.uname()?;
        if platform != TargetPlatform::Unknown {
            target.check_current_before(deadline)?;
            return Ok((
                platform,
                WindowsTargetProbe::Unknown {
                    reason: WindowsProbeReason::NoReply,
                },
            ));
        }
        let result = windows::probe(run)?;
        let platform = if matches!(result, WindowsTargetProbe::Confirmed { .. }) {
            target.check_current_before(deadline)?;
            TargetPlatform::Windows
        } else {
            TargetPlatform::Unknown
        };
        if Instant::now() >= deadline {
            return Err("sshTimeout".into());
        }
        Ok((platform, result))
    };
    let (platform, windows_probe) = detect().or_else(|code| match code.as_str() {
        "sshTimeout" => Ok((
            TargetPlatform::Unknown,
            WindowsTargetProbe::Unknown {
                reason: WindowsProbeReason::Timeout,
            },
        )),
        "targetProbeOutputLimit" => Ok((
            TargetPlatform::Unknown,
            WindowsTargetProbe::Unknown {
                reason: WindowsProbeReason::OutputLimit,
            },
        )),
        _ => Err(code),
    })?;
    Ok(TargetPlatformProbe::new(id, platform, windows_probe))
}

pub struct OwnedChild {
    pub child: Child,
    #[cfg(windows)]
    job: windows::Win32::Foundation::HANDLE,
}

pub trait ManagedSsh: Send {
    fn is_running(&mut self) -> BridgeResult<bool>;
}

impl ManagedSsh for OwnedChild {
    fn is_running(&mut self) -> BridgeResult<bool> {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|_| "processFailed".into())
    }
}
// HANDLE is only owned and closed here; all child access is serialized.
#[cfg(windows)]
unsafe impl Send for OwnedChild {}
impl OwnedChild {
    pub fn spawn(cmd: &mut Command) -> BridgeResult<Self> {
        let child = cmd.spawn().map_err(|_| "sshMissing")?;
        #[cfg(windows)]
        unsafe {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::{
                Foundation::{CloseHandle, HANDLE},
                System::JobObjects::*,
            };
            let mut child = child;
            let job = match CreateJobObjectW(None, None) {
                Ok(job) => job,
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("processFailed".into());
                }
            };
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            )
            .is_err()
                || AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())).is_err()
            {
                let _ = child.kill();
                let _ = child.wait();
                let _ = CloseHandle(job);
                return Err("processFailed".into());
            }
            Ok(Self { child, job })
        }
        #[cfg(not(windows))]
        Ok(Self { child })
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        #[cfg(windows)]
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

pub fn output(
    cmd: Command,
    input: Option<Zeroizing<String>>,
    seconds: u64,
) -> BridgeResult<String> {
    output_before(cmd, input, Instant::now() + Duration::from_secs(seconds))
}

fn output_before(
    mut cmd: Command,
    input: Option<Zeroizing<String>>,
    deadline: Instant,
) -> BridgeResult<String> {
    if Instant::now() >= deadline {
        return Err("sshTimeout".into());
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut process = OwnedChild::spawn(&mut cmd)?;
    let stdout = process.child.stdout.take().ok_or("processFailed")?;
    let stderr = process.child.stderr.take().ok_or("processFailed")?;
    let read = |mut stream: Box<dyn Read + Send>| {
        let mut data = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(n) = stream.read(&mut chunk) {
            if n == 0 {
                break;
            }
            if data.len() < 65536 {
                data.extend_from_slice(&chunk[..n.min(65536 - data.len())]);
            }
        }
        data
    };
    let out = thread::spawn(move || read(Box::new(stdout)));
    let err = thread::spawn(move || read(Box::new(stderr)));
    let mut stdin = process.child.stdin.take().ok_or("processFailed")?;
    let writer = thread::spawn(move || {
        if let Some(input) = input {
            let _ = stdin.write_all(input.as_bytes());
        }
    });
    let status = loop {
        if let Some(status) = process.child.try_wait().map_err(|_| "processFailed")? {
            break status;
        }
        if Instant::now() >= deadline {
            return Err("sshTimeout".into());
        }
        thread::sleep(Duration::from_millis(40));
    };
    // Also close the job before joining pipes: a ProxyJump descendant must not
    // keep output readers alive after the parent SSH client exits.
    drop(process);
    let _ = writer.join();
    let output = out.join().map_err(|_| "processFailed")?;
    let error = err.join().map_err(|_| "processFailed")?;
    if output.len() >= 65536 {
        return Err("sshConfigUnsafe".into());
    }
    if !status.success() {
        return Err(classify_ssh_error(&String::from_utf8_lossy(&error)));
    }
    String::from_utf8(output).map_err(|_| "remoteFailed".into())
}

pub(super) fn classify_ssh_error(transcript: &str) -> String {
    let text = transcript.to_ascii_lowercase();
    if text.contains("remote host identification has changed")
        || text.contains("offending") && text.contains("known_hosts")
    {
        "hostKeyChanged"
    } else if text.contains("host key") {
        "hostKey"
    } else if text.contains("forward") {
        "forwardDenied"
    } else if text.contains("permission denied")
        || text.contains("authentication failed")
        || text.contains("password")
    {
        "sshAuth"
    } else {
        "sshFailed"
    }
    .into()
}

pub fn remote(alias: &str, mut request: serde_json::Value) -> BridgeResult<serde_json::Value> {
    // Runtime evidence can only come from our checked SSH extension probe.
    // Preview, apply and each background profile update re-probe independently.
    if let Some(object) = request.as_object_mut() {
        object.remove("extensionVersion");
    }
    let result = remote_execute(alias, &request);
    if matches!(request["operation"].as_str(), Some("preview" | "apply"))
        && result
            .as_ref()
            .err()
            .is_some_and(|error| matches!(error.as_str(), "cliMissing" | "cliUnsupported"))
    {
        let tool =
            super::tool_adapter::by_name(request["tool"].as_str().ok_or("invalidRequest")?)?.id();
        let inspection = super::extension::discover(alias)?;
        request["extensionVersion"] =
            serde_json::json!(super::extension::profile_version(&inspection, tool)?);
        return remote_execute(alias, &request);
    }
    result
}

fn remote_execute(alias: &str, request: &serde_json::Value) -> BridgeResult<serde_json::Value> {
    let (source, operation) = linux::remote_payload(request)?;
    let timeout_seconds = if operation == "tool-verify" { 90 } else { 25 };
    linux::parse_remote_output(
        operation,
        &run_linux_script(alias, source, timeout_seconds)?,
    )
}

fn run_linux_script(alias: &str, source: Zeroizing<String>, seconds: u64) -> BridgeResult<String> {
    let target = BoundTarget::from_resolved(resolve(alias)?)?;
    require_linux(&target)?;
    execute_linux_script(target.remote_command()?, source, seconds)
}

fn require_linux(target: &BoundTarget) -> BridgeResult<()> {
    let (mut command, destination) = target.remote_command()?;
    command
        .args([
            "-oClearAllForwardings=yes",
            "-oUpdateHostKeys=no",
            "-oAddKeysToAgent=no",
        ])
        .arg(destination)
        .arg(TargetOs::PROBE_COMMAND);
    #[cfg(windows)]
    let platform =
        super::target_os::windows::capture(command, Instant::now() + Duration::from_secs(25))?
            .uname()?;
    #[cfg(not(windows))]
    let platform = super::TargetPlatform::from_uname(&output(command, None, 25)?);
    platform.require_linux()
}

pub(super) fn require_linux_target(id: &str, fingerprint: &str) -> BridgeResult<()> {
    require_linux(&BoundTarget::capture_connected(id, fingerprint)?)
}

fn execute_linux_script(
    (mut cmd, destination): (Command, String),
    source: Zeroizing<String>,
    seconds: u64,
) -> BridgeResult<String> {
    cmd.arg("-oClearAllForwardings=yes")
        .arg(destination)
        .arg(linux::SCRIPT_COMMAND);
    output(cmd, Some(source), seconds)
}

pub(super) fn skill_remote(
    target: &BoundTarget,
    request: &serde_json::Value,
) -> BridgeResult<serde_json::Value> {
    if !matches!(request["operation"].as_str(), Some("status" | "remove")) {
        return Err("invalidRequest".into());
    }
    execute_bound_skill(target, request)
}

fn execute_bound_skill(
    target: &BoundTarget,
    request: &serde_json::Value,
) -> BridgeResult<serde_json::Value> {
    let source = linux::skill_payload(request)?;
    require_linux(target)?;
    linux::parse_skill_output(&execute_linux_script(target.remote_command()?, source, 30)?)
}

fn scp_target_command(target: &BoundTarget) -> BridgeResult<(Command, String)> {
    target.check_current()?;
    #[cfg(windows)]
    let mut command = Command::new(
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
            .join("System32/OpenSSH/scp.exe"),
    );
    #[cfg(not(windows))]
    let mut command = Command::new("scp");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command.args([
        "-q",
        "-O",
        "-oConnectTimeout=8",
        "-oConnectionAttempts=1",
        "-oForwardAgent=no",
        "-oForwardX11=no",
        "-oPermitLocalCommand=no",
        "-oControlMaster=no",
        "-oControlPath=none",
        "-oClearAllForwardings=yes",
    ]);
    target.pin_endpoint(&mut command);
    let destination = match &target.target.connection {
        Connection::Config { alias, config } => {
            if let Some(path) = config {
                command.arg("-F").arg(path);
            }
            alias.clone()
        }
        Connection::Direct {
            host,
            user,
            port,
            authentication,
            identity_file,
        } => {
            if !safe_host(host)
                || user.as_deref().is_some_and(|user| !safe_name(user))
                || *port == 0
            {
                return Err("targetUnsupported".into());
            }
            command.arg("-P").arg(port.to_string());
            if *authentication == ManualAuthentication::Password {
                command.args([
                    "-oPubkeyAuthentication=no",
                    "-oPreferredAuthentications=keyboard-interactive,password",
                ]);
            }
            if let Some(identity_file) = identity_file {
                let metadata =
                    fs::symlink_metadata(identity_file).map_err(|_| "identityFileInvalid")?;
                if !identity_file.is_absolute()
                    || !metadata.is_file()
                    || metadata.file_type().is_symlink()
                {
                    return Err("identityFileInvalid".into());
                }
                command
                    .arg("-oIdentitiesOnly=yes")
                    .arg("-i")
                    .arg(identity_file);
            }
            let host = if host.contains(':') {
                format!("[{host}]")
            } else {
                host.clone()
            };
            user.as_ref()
                .map_or(host.clone(), |user| format!("{user}@{host}"))
        }
    };
    #[cfg(windows)]
    {
        if let Some((password, entropy)) =
            super::credential_cache::terminal_payload(&target.fingerprint)?
        {
            let askpass = std::env::current_exe().map_err(|_| "processFailed")?;
            command
                .args([
                    "-oStrictHostKeyChecking=yes",
                    "-oBatchMode=no",
                    "-oPasswordAuthentication=yes",
                    "-oKbdInteractiveAuthentication=yes",
                    "-oNumberOfPasswordPrompts=1",
                ])
                .env("PROXYENV_SSH_ASKPASS", "1")
                .env("PROXYENV_SSH_PASSWORD", password)
                .env("PROXYENV_SSH_PASSWORD_ENTROPY", entropy)
                .env("SSH_ASKPASS", askpass)
                .env("SSH_ASKPASS_REQUIRE", "force")
                .env("DISPLAY", "proxyenv:0");
        } else {
            command.args(["-oStrictHostKeyChecking=yes", "-oBatchMode=yes"]);
        }
    }
    #[cfg(not(windows))]
    command.args(["-oStrictHostKeyChecking=yes", "-oBatchMode=yes"]);
    Ok((command, destination))
}

pub(super) struct LinuxSkillStage<'a> {
    target: &'a BoundTarget,
    request: serde_json::Value,
}

pub(super) fn prepare_skill_upload(
    target: &BoundTarget,
    request: serde_json::Value,
) -> BridgeResult<LinuxSkillStage<'_>> {
    if request["operation"] != "prepare" {
        return Err("invalidRequest".into());
    }
    let source = linux::skill_payload(&request)?;
    let (mut command, destination) = target.remote_command()?;
    command
        .arg("-oClearAllForwardings=yes")
        .arg(destination)
        .arg(TargetOs::PROBE_COMMAND);
    let evidence = output(command, None, 25)?;
    TargetOs::from_uname(&evidence).require_linux()?;
    let prepared =
        linux::parse_skill_output(&execute_linux_script(target.remote_command()?, source, 30)?)?;
    if prepared["prepared"] != true {
        return Err("skillVerifyFailed".into());
    }
    Ok(LinuxSkillStage { target, request })
}

pub(super) enum SkillStageFinish {
    Apply,
    Cleanup,
}

pub(super) fn finish_skill_upload(
    stage: &LinuxSkillStage<'_>,
    finish: SkillStageFinish,
) -> BridgeResult<serde_json::Value> {
    let mut request = stage.request.clone();
    request["operation"] = match finish {
        SkillStageFinish::Apply => "apply",
        SkillStageFinish::Cleanup => "cleanup",
    }
    .into();
    execute_bound_skill(stage.target, &request)
}

pub(super) fn skill_upload(
    stage: &LinuxSkillStage<'_>,
    local_path: &Path,
    relative_path: &str,
) -> BridgeResult<()> {
    if !local_path.is_absolute() {
        return Err("invalidRequest".into());
    }
    let field = |name: &str| stage.request[name].as_str().ok_or("invalidRequest");
    let path = linux::skill_upload_path(
        field("tool")?,
        field("skillName")?,
        field("skillHash")?,
        relative_path,
    )?;
    let metadata = fs::symlink_metadata(local_path).map_err(|_| "localSkillChanged")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("localSkillUnsafe".into());
    }
    let (mut command, destination) = scp_target_command(stage.target)?;
    let remote = format!("{destination}:{path}");
    command
        .arg("--")
        .arg(scp_local_path(local_path)?)
        .arg(remote);
    output(command, None, 30).map(|_| ())
}

#[cfg(windows)]
fn scp_local_path(path: &Path) -> BridgeResult<PathBuf> {
    let path = path.to_str().ok_or("localSkillUnsafe")?;
    // Rust canonicalization returns \\?\C:\..., which Windows scp treats as
    // a remote host because it sees the drive colon after an unrecognized prefix.
    let ordinary = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(drive) = path.strip_prefix(r"\\?\") {
        if drive.len() < 3
            || !drive.as_bytes()[0].is_ascii_alphabetic()
            || drive.as_bytes()[1] != b':'
            || drive.as_bytes()[2] != b'\\'
        {
            return Err("localSkillUnsafe".into());
        }
        drive.to_owned()
    } else {
        path.to_owned()
    };
    let ordinary = PathBuf::from(ordinary);
    if !ordinary.is_absolute() {
        return Err("localSkillUnsafe".into());
    }
    Ok(ordinary)
}

#[cfg(not(windows))]
fn scp_local_path(path: &Path) -> BridgeResult<PathBuf> {
    Ok(path.to_path_buf())
}

pub fn tunnel(request: &Request, endpoints: &[(u16, String, u16)]) -> BridgeResult<OwnedChild> {
    // The preflight and the persistent reverse-forward process must use the
    // same session authentication source. Otherwise a password-only target
    // can pass preflight through the protected bridge cache, then fail here
    // when the tunnel silently falls back to BatchMode=yes.
    let target = BoundTarget::from_resolved(resolve(&request.target_id)?)?;
    require_linux(&target)?;
    let (mut cmd, destination) = target.remote_command()?;
    cmd.args(["-N", "-oClearAllForwardings=no"]);
    for (remote, host, local) in endpoints {
        let host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.clone()
        };
        cmd.arg("-R")
            .arg(reverse_forward_spec(*remote, &host, *local));
    }
    cmd.arg(destination)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    OwnedChild::spawn(&mut cmd)
}

pub(super) fn reverse_forward_spec(remote: u16, host: &str, local: u16) -> String {
    format!("127.0.0.1:{remote}:{host}:{local}")
}

pub(super) fn extension_remote(
    alias: &str,
    request: &serde_json::Value,
) -> BridgeResult<serde_json::Value> {
    let source = linux::extension_payload(request);
    linux::parse_extension_output(&run_linux_script(alias, source, 30)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use linux::{
        managed_terminal_remote_command, parse_remote_output, remote_payload, skill_payload,
    };

    #[test]
    fn new_host_key_can_request_confirmation_but_changed_keys_remain_blocked() {
        assert_eq!(
            classify_ssh_error("No ED25519 host key is known; Host key verification failed."),
            "hostKey"
        );
        assert_eq!(
            classify_ssh_error(
                "WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED! Host key verification failed."
            ),
            "hostKeyChanged"
        );
        assert_eq!(
            classify_ssh_error("Offending ED25519 key in C:/test/known_hosts:1"),
            "hostKeyChanged"
        );
        assert_eq!(
            classify_ssh_error("Permission denied (publickey,password)."),
            "sshAuth"
        );
        assert_eq!(
            classify_ssh_error("remote port forwarding failed for listen port"),
            "forwardDenied"
        );
        assert_eq!(classify_ssh_error("Connection refused"), "sshFailed");
    }
    #[cfg(windows)]
    #[test]
    fn skill_upload_uses_a_windows_path_scp_recognizes_as_local() {
        assert_eq!(
            scp_local_path(Path::new(
                r"\\?\C:\Users\demo\.cc-switch\skills\example\SKILL.md"
            ))
            .unwrap(),
            PathBuf::from(r"C:\Users\demo\.cc-switch\skills\example\SKILL.md")
        );
        assert_eq!(
            scp_local_path(Path::new(r"\\?\UNC\server\share\SKILL.md")).unwrap(),
            PathBuf::from(r"\\server\share\SKILL.md")
        );
        assert!(scp_local_path(Path::new(r"\\?\GLOBALROOT\Device\example")).is_err());
    }

    #[test]
    fn skills_stage_rejects_alias_retargeting_and_requires_a_fresh_binding_for_the_new_host() {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let alias = format!("routespan-stage-{}", hex::encode(nonce));
        let path = std::env::temp_dir().join(&alias);
        let fixture = |host: &str, user: &str, port: u16| {
            fs::write(
                &path,
                format!("Host {alias}\n  HostName {host}\n  User {user}\n  Port {port}\n"),
            )
            .unwrap();
            let mut target =
                config_targets(&path, RemoteTargetSource::Vscode, "test", false, false)
                    .unwrap()
                    .pop()
                    .unwrap();
            target.connection = Connection::Config {
                alias: alias.clone(),
                config: Some(path.clone()),
            };
            target
        };

        let first = fixture("first.example", "alice", 2201);
        let binding = BoundTarget::from_resolved(first.clone()).unwrap();
        let stage = LinuxSkillStage {
            target: &binding,
            request: serde_json::json!({
                "operation":"prepare", "tool":"codex", "skillName":"valid-skill",
                "skillHash":"a".repeat(64), "fileCount":1, "totalSize":8,
                "manifestBase64":"YWJjCg==", "directoriesBase64":""
            }),
        };
        let deadline = || Instant::now() + Duration::from_secs(12);
        stage.target.check_resolved(&first, deadline()).unwrap();
        let second = fixture("second.example", "bob", 2202);
        assert_eq!(first.public.id, second.public.id);
        assert_eq!(
            stage
                .target
                .check_resolved(&second, deadline())
                .unwrap_err(),
            "sshConfigChanged"
        );

        // With the alias already retargeted, read back the launch options of
        // the original command. Host, login and port must still point to A.
        let (mut command, destination) =
            resolved_target_command(&stage.target.target, CommandMode::Batch).unwrap();
        stage.target.pin_endpoint(&mut command);
        command.args(["-G", &destination]);
        let effective = effective_target_metadata(&output(command, None, 8).unwrap()).unwrap();
        assert_eq!(effective.host.as_deref(), Some("first.example"));
        assert_eq!(effective.user.as_deref(), Some("alice"));
        assert_eq!(effective.port, Some(2201));

        // The temporary alias is absent from application discovery. Apply and
        // error-path cleanup both fail without launching a remote command.
        for finish in [SkillStageFinish::Apply, SkillStageFinish::Cleanup] {
            assert_eq!(
                finish_skill_upload(&stage, finish).unwrap_err(),
                "sshConfigChanged"
            );
        }
        let retry = BoundTarget::from_resolved(second.clone()).unwrap();
        assert_eq!(
            retry.check_fingerprint(&binding.fingerprint).unwrap_err(),
            "sshConfigChanged"
        );
        retry.check_resolved(&second, deadline()).unwrap();
        let restored = fixture("first.example", "alice", 2201);
        stage.target.check_resolved(&restored, deadline()).unwrap();
        assert_eq!(
            retry.check_resolved(&restored, deadline()).unwrap_err(),
            "sshConfigChanged"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn aliases_reject_patterns_options_and_shell_syntax() {
        assert_eq!(
            aliases_from(
                "Host gpu dev-vm\nHost * !bad x;id x|id $(id) -oFoo\nHost=gpu\nHost \"lab\""
            ),
            vec!["gpu", "dev-vm", "lab"]
        );
        for value in ["a\nb", "a&&b", "a`id`", "a@b", "a b", "-R", "a/b"] {
            assert!(!safe_name(value));
        }
        assert!(!safe_host("bad host"));
    }
    #[test]
    fn general_proxy_operations_do_not_require_ai_session_material() {
        assert!(remote_payload(&serde_json::json!({
            "operation": "test",
            "port": 17897,
            "protocol": "http"
        }))
        .is_ok());
        assert!(remote_payload(&serde_json::json!({
            "operation": "test",
            "port": 17897,
            "protocol": "http",
            "sessionToken": "not-a-token"
        }))
        .is_err());
        let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert!(remote_payload(&serde_json::json!({
            "operation": "test",
            "port": 17897,
            "protocol": "http",
            "sessionToken": token
        }))
        .is_ok());
        assert!(remote_payload(&serde_json::json!({
            "operation": "session-env-apply",
            "port": 17897,
            "protocol": "mixed",
            "sessionId": "0123456789abcdef0123456789abcdef"
        }))
        .is_ok());
        assert_eq!(
            remote_payload(&serde_json::json!({
                "operation": "status",
                "tool": "codex",
                "port": 25721
            }))
            .unwrap_err(),
            "invalidRequest"
        );
    }
    #[test]
    fn read_only_probes_do_not_upload_configuration_helpers() {
        let (source, _) =
            remote_payload(&serde_json::json!({"operation":"tool-discover"})).unwrap();
        assert!(source.contains("discover_version codex"));
        assert!(source.contains("discover_version claude"));
        assert!(!source.contains("claude_json"));
        assert!(!source.contains("profile_decode"));
        assert!(!source.contains("# PROXYENV_CONFIG_DEPENDENCIES"));
        assert!(source.len() < 12000);
        for text in ["{", "{\"HOME\":\"private\"}", &" ".repeat(4097)] {
            assert!(parse_remote_output("tool-discover", text).is_err());
        }
        for operation in ["check", "verify", "internet", "test"] {
            let (source, _) = remote_payload(&serde_json::json!({
                "operation": operation, "ports": [17897]
            }))
            .unwrap();
            assert!(
                source.len() < 4096,
                "{operation} should remain a compact probe"
            );
            assert!(source.contains("check_ports()"));
            assert!(source.contains("fail rootForbidden"));
            assert!(!source.contains("claude_json"));
            assert!(!source.contains("umask 077"));
        }
        let (source, _) = remote_payload(&serde_json::json!({
            "operation": "session-env-apply",
            "sessionId": "0123456789abcdef0123456789abcdef"
        }))
        .unwrap();
        assert!(source.contains("umask 077"));
        assert!(source.contains("claude_json"));
    }
    #[test]
    fn extension_profile_payload_accepts_only_compatible_normalized_versions() {
        let valid = serde_json::json!({"operation":"preview","tool":"claude","port":25721,"sessionToken":"a".repeat(64),"profileHash":"b".repeat(64),"profileSettingsBase64":"e30=","extensionVersion":"2.1.227"});
        assert!(remote_payload(&valid)
            .unwrap()
            .0
            .contains("extension_version='2.1.227'"));
        for version in [
            "2.1.226",
            "3.0.0",
            "2.1.227;id",
            "2.1.227\nsecret",
            "/home/private",
            "2.1.227-alpha",
        ] {
            let mut invalid = valid.clone();
            invalid["extensionVersion"] = serde_json::json!(version);
            assert!(remote_payload(&invalid).is_err());
        }
        let mut invalid = valid;
        invalid["operation"] = serde_json::json!("tool-verify");
        assert!(
            remote_payload(&invalid).is_err(),
            "Plugin proof cannot invent an executable CLI"
        );
    }
    #[test]
    fn openssh_parameters_keep_security_overrides_and_have_no_shell() {
        let command = command();
        let args: Vec<_> = command
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        for required in [
            "-oBatchMode=yes",
            "-oStrictHostKeyChecking=yes",
            "-oForwardAgent=no",
            "-oPermitLocalCommand=no",
            "-oExitOnForwardFailure=yes",
            "-oControlPath=none",
        ] {
            assert!(args.iter().any(|arg| arg == required));
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_probe_preserves_strict_host_keys_without_ssh_writes_or_forwarding() {
        let mut command = command_with_mode(CommandMode::Batch);
        command
            .args(super::super::target_os::windows::SSH_READ_ONLY_OPTIONS)
            .args(["-F", "none", "-G", "localhost"]);
        let effective = output(command, None, 8).unwrap();
        for (key, values) in [
            ("stricthostkeychecking", ["true", "yes"]),
            ("updatehostkeys", ["false", "no"]),
            ("checkhostip", ["false", "no"]),
            ("addkeystoagent", ["false", "no"]),
            ("clearallforwardings", ["true", "yes"]),
            ("tunnel", ["false", "no"]),
        ] {
            assert!(
                effective.lines().any(|line| {
                    let (option, value) = line.split_once(' ').unwrap_or((line, ""));
                    option == key && values.contains(&value)
                }),
                "{key}"
            );
        }
    }
    #[test]
    fn interactive_openssh_parameters_enable_prompts_without_weakening_hardening() {
        let command = command_with_mode(CommandMode::Interactive);
        let args: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        for required in [
            "-oBatchMode=no",
            "-oPasswordAuthentication=yes",
            "-oKbdInteractiveAuthentication=yes",
            "-oStrictHostKeyChecking=ask",
            "-oForwardAgent=no",
            "-oForwardX11=no",
            "-oPermitLocalCommand=no",
            "-oExitOnForwardFailure=yes",
            "-oControlPath=none",
        ] {
            assert!(args.iter().any(|argument| argument == required));
        }
        assert!(!args.iter().any(|argument| argument == "-oBatchMode=yes"));
        assert!(!args
            .iter()
            .any(|argument| argument == "-oStrictHostKeyChecking=no"));
    }
    #[test]
    fn cached_password_mode_keeps_host_key_strict_and_limits_password_prompts() {
        let command = command_with_mode(CommandMode::CachedPassword);
        let args: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        for required in [
            "-oStrictHostKeyChecking=yes",
            "-oBatchMode=no",
            "-oPasswordAuthentication=yes",
            "-oKbdInteractiveAuthentication=yes",
            "-oNumberOfPasswordPrompts=1",
            "-oPreferredAuthentications=publickey,keyboard-interactive,password",
        ] {
            assert!(args.iter().any(|argument| argument == required));
        }
        assert!(!args
            .iter()
            .any(|argument| argument == "-oStrictHostKeyChecking=ask"));
    }
    #[test]
    fn tool_verification_output_accepts_only_public_status_values() {
        for verification in [
            "verified",
            "authenticationRequired",
            "routeUnavailable",
            "timedOut",
            "failed",
        ] {
            let text = format!(r#"{{"verification":"{verification}"}}"#);
            let value = parse_remote_output("tool-verify", &text).unwrap();
            assert_eq!(value["verification"], verification);
        }
        for invalid in [
            r#"{"verification":"verifyPending"}"#,
            r#"{"verification":"secret remote output"}"#,
            r#"{"message":"PROXYENV_VERIFY_OK"}"#,
        ] {
            assert!(parse_remote_output("tool-verify", invalid).is_err());
        }
        let sanitized = parse_remote_output(
            "tool-verify",
            r#"{"verification":"verified","remoteOutput":"secret"}"#,
        )
        .unwrap();
        assert_eq!(sanitized, serde_json::json!({"verification":"verified"}));
    }
    #[test]
    fn managed_terminal_is_interactive_and_does_not_modify_shell_startup_files() {
        let command = command_with_mode(CommandMode::ManagedTerminal);
        let args: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        for required in [
            "-tt",
            "-oBatchMode=no",
            "-oStrictHostKeyChecking=ask",
            "-oForwardAgent=no",
            "-oPermitLocalCommand=no",
        ] {
            assert!(args.iter().any(|argument| argument == required));
        }
        assert!(!args.iter().any(|argument| argument == "-T"));

        let session_id = "0123456789abcdef0123456789abcdef";
        let script = managed_terminal_remote_command(session_id).unwrap();
        for expected in [
            ".proxyenv/sessions/0123456789abcdef0123456789abcdef/env.sh",
            ". \"$proxyenv_session\"",
            "exec \"${SHELL:-/bin/sh}\" -i",
        ] {
            assert!(script.contains(expected));
        }
        for forbidden in [
            ".bashrc",
            ".profile",
            "/etc/environment",
            "sudo",
            "proxyenv:",
        ] {
            assert!(!script.contains(forbidden));
        }
    }
    #[cfg(windows)]
    #[test]
    fn managed_terminal_uses_powershell_and_passes_ssh_arguments_as_data() {
        let mut ssh = command_with_mode(CommandMode::ManagedTerminal);
        ssh.args(["example", "printf 'safe'"]);
        let terminal = powershell_terminal_command(
            &ssh,
            Some(("encrypted-value".into(), "target-entropy".into())),
        )
        .unwrap();
        assert!(terminal
            .get_program()
            .to_string_lossy()
            .ends_with("powershell.exe"));
        let arguments = terminal
            .get_args()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>();
        for required in ["-NoProfile", "-NoExit", "-Command"] {
            assert!(arguments.iter().any(|argument| argument == required));
        }
        let payload = terminal
            .get_envs()
            .find_map(|(name, value)| {
                if name == "PROXYENV_SSH_LAUNCH" {
                    value.map(|value| value.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .unwrap();
        let launch: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(
            launch["arguments"].as_array().unwrap().last().unwrap(),
            "printf 'safe'"
        );
        let environment = terminal
            .get_envs()
            .filter_map(|(name, value)| {
                value.map(|value| {
                    (
                        name.to_string_lossy().into_owned(),
                        value.to_string_lossy().into_owned(),
                    )
                })
            })
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(environment["SSH_ASKPASS_REQUIRE"], "force");
        assert_eq!(environment["PROXYENV_SSH_PASSWORD"], "encrypted-value");
        assert!(!environment.values().any(|value| value == "plain-password"));
    }
    #[cfg(windows)]
    #[test]
    fn detached_console_arguments_follow_windows_quoting_rules() {
        let quote =
            |value: &str| String::from_utf16(&quote_windows_argument(value.as_ref())).unwrap();
        assert_eq!(quote("plain"), "plain");
        assert_eq!(quote("two words"), "\"two words\"");
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote("C:\\Program Files\\"), "\"C:\\Program Files\\\\\"");
    }
    #[test]
    fn reverse_forward_keeps_remote_consumer_and_local_upstream_ports_separate() {
        for (remote, local) in [(7897, 10809), (10809, 10809)] {
            assert_eq!(
                reverse_forward_spec(remote, "127.0.0.1", local),
                format!("127.0.0.1:{remote}:127.0.0.1:{local}")
            );
        }
    }
    #[test]
    fn target_ids_separate_sources_paths_and_aliases_without_exposing_paths() {
        let first = Path::new("C:/Users/example/.ssh/config");
        let second = Path::new("D:/SSH/config");
        let open_ssh = target_id(RemoteTargetSource::Openssh, first, "dev");
        let vscode = target_id(RemoteTargetSource::Vscode, first, "dev");
        let other_path = target_id(RemoteTargetSource::Openssh, second, "dev");
        let other_alias = target_id(RemoteTargetSource::Openssh, first, "prod");

        assert_ne!(open_ssh, vscode);
        assert_ne!(open_ssh, other_path);
        assert_ne!(open_ssh, other_alias);
        assert!(!open_ssh.contains("Users"));
        assert!(!other_path.contains("D:/SSH"));
    }

    #[test]
    fn effective_target_metadata_exposes_endpoint_and_explicit_password_authentication() {
        let metadata = effective_target_metadata(
            "user student\nhostname lab.example.edu\nport 2222\n\
             pubkeyauthentication no\npasswordauthentication yes\n\
             kbdinteractiveauthentication yes\n\
             preferredauthentications keyboard-interactive,password\n",
        )
        .unwrap();
        assert_eq!(metadata.user.as_deref(), Some("student"));
        assert_eq!(metadata.host.as_deref(), Some("lab.example.edu"));
        assert_eq!(metadata.port, Some(2222));
        assert_eq!(metadata.authentication_method, SshAuthMethod::Password);
    }

    #[test]
    fn effective_target_metadata_rejects_forwarding_from_imported_config() {
        assert_eq!(
            effective_target_metadata(
                "user student\nhostname lab.example.edu\nport 22\nlocalforward 8080 localhost:80\n"
            )
            .unwrap_err(),
            "sshConfigUnsafe"
        );
    }

    #[test]
    fn config_target_metadata_reads_endpoint_and_auth_without_executing_ssh() {
        let metadata = config_target_metadata(
            "Host lab\n  HostName lab.example.edu\n  User student\n  Port 2222\n  PubkeyAuthentication no\n  PreferredAuthentications keyboard-interactive,password\nHost *\n  Port 22\n",
            "lab",
        )
        .unwrap();
        assert_eq!(metadata.user.as_deref(), Some("student"));
        assert_eq!(metadata.host.as_deref(), Some("lab.example.edu"));
        assert_eq!(metadata.port, Some(2222));
        assert_eq!(metadata.authentication_method, SshAuthMethod::Password);
    }

    #[test]
    fn config_target_metadata_applies_safe_wildcard_defaults() {
        let metadata = config_target_metadata(
            "Host lab\n  HostName 10.0.0.8\nHost *\n  User student\n  Port 2200\n",
            "lab",
        )
        .unwrap();
        assert_eq!(metadata.host.as_deref(), Some("10.0.0.8"));
        assert_eq!(metadata.user.as_deref(), Some("student"));
        assert_eq!(metadata.port, Some(2200));
    }
    #[test]
    fn skill_projection_payload_accepts_only_bounded_allowlisted_metadata() {
        let valid = serde_json::json!({
            "operation":"prepare",
            "tool":"codex",
            "skillName":"safe-skill",
            "skillHash":"a".repeat(64),
            "fileCount":1,
            "totalSize":8,
            "manifestBase64":"YWJjCg==",
            "directoriesBase64":""
        });
        let payload = skill_payload(&valid).unwrap();
        assert!(!payload.contains('\r'), "Linux shell payload must use LF");
        assert!(payload.contains("skill_name='safe-skill'"));
        assert!(payload.contains("skills_staging=\"$staging_root/skills\""));
        for (field, value) in [
            ("skillName", "../escape"),
            ("skillName", ".proxyenv-owner"),
            ("tool", "other"),
            ("operation", "execute"),
        ] {
            let mut invalid = valid.clone();
            invalid[field] = serde_json::json!(value);
            assert!(skill_payload(&invalid).is_err(), "accepted {field}={value}");
        }
        let mut oversized = valid;
        oversized["totalSize"] = serde_json::json!(8 * 1024 * 1024 + 1);
        assert!(skill_payload(&oversized).is_err());
    }
    #[test]
    fn skill_projection_script_uses_staging_ownership_and_atomic_activation() {
        let script = linux::SKILL_SCRIPT;
        for required in [
            "skills_staging=\"$staging_root/skills\"",
            ".proxyenv-owner",
            ".proxyenv-manifest",
            "sha256sum",
            "mv \"$stage\" \"$destination\"",
            "owner_matches \"$destination\"",
        ] {
            assert!(script.contains(required), "missing {required}");
        }
        for forbidden in ["scp -r", ".bashrc", ".profile", "sudo "] {
            assert!(!script.contains(forbidden), "found {forbidden}");
        }
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "executed as an owned subprocess by owned_process_ends_on_drop"]
    fn owned_sleep_child() {
        std::thread::sleep(Duration::from_secs(30));
    }
    #[cfg(windows)]
    #[test]
    fn owned_process_ends_on_drop() {
        use windows::Win32::{
            Foundation::{CloseHandle, WAIT_OBJECT_0},
            System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
        };
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "features::remote_bridge::ssh::tests::owned_sleep_child",
                "--ignored",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut owned = OwnedChild::spawn(&mut command).unwrap();
        assert!(owned.child.try_wait().unwrap().is_none());
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, owned.child.id()).unwrap() };
        drop(owned);
        unsafe {
            assert_eq!(WaitForSingleObject(handle, 2000), WAIT_OBJECT_0);
            let _ = CloseHandle(handle);
        }
    }
}
