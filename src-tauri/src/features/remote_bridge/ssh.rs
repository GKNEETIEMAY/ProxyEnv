use super::{
    mobaxterm, BridgeResult, RemoteTarget, RemoteTargetCompatibility, RemoteTargetSource, Request,
};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn command() -> Command {
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
        command.creation_flags(0x08000000);
    }
    command.args([
        "-T",
        "-oBatchMode=yes",
        "-oStrictHostKeyChecking=yes",
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
    command
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

fn aliases_at(path: &Path) -> BridgeResult<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let metadata = std::fs::metadata(path).map_err(|_| "sshConfigMissing")?;
    if metadata.len() > 1024 * 1024 {
        return Err("sshConfigUnsafe".into());
    }
    Ok(aliases_from(
        &std::fs::read_to_string(path).map_err(|_| "sshConfigMissing")?,
    ))
}
#[derive(Debug, Clone)]
enum Connection {
    Config {
        alias: String,
        config: Option<PathBuf>,
    },
    Direct {
        host: String,
        user: String,
        port: u16,
    },
}

#[derive(Debug, Clone)]
struct ResolvedTarget {
    public: RemoteTarget,
    connection: Connection,
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

fn config_targets(
    path: &Path,
    source: RemoteTargetSource,
    source_label: &str,
    can_open_vscode: bool,
) -> BridgeResult<Vec<ResolvedTarget>> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    Ok(aliases_at(path)?
        .into_iter()
        .map(|alias| ResolvedTarget {
            public: RemoteTarget {
                id: target_id(source, &canonical, &alias),
                display_name: alias.clone(),
                source,
                source_label: source_label.into(),
                config_path: display_path(&canonical),
                ssh_alias: Some(alias.clone()),
                host: None,
                user: None,
                port: None,
                identity_file: None,
                available: true,
                compatibility: RemoteTargetCompatibility::Compatible,
                unavailable_reason: None,
                can_open_vscode,
            },
            connection: Connection::Config {
                alias,
                config: (source == RemoteTargetSource::Vscode).then_some(canonical.clone()),
            },
        })
        .collect())
}

fn discovered() -> BridgeResult<Vec<ResolvedTarget>> {
    let default = dirs::home_dir()
        .ok_or("sshConfigMissing")?
        .join(".ssh/config");
    let vscode_config = super::vscode::custom_ssh_config().ok().flatten();
    let default_canonical = default.canonicalize().ok();
    let vscode_uses_default = vscode_config.is_none()
        || (default_canonical.is_some() && vscode_config.as_ref() == default_canonical.as_ref());
    let mut result = config_targets(
        &default,
        RemoteTargetSource::Openssh,
        "OpenSSH",
        vscode_uses_default,
    )?;
    if let Some(path) = vscode_config {
        if default_canonical.as_ref() != Some(&path) {
            result.extend(config_targets(
                &path,
                RemoteTargetSource::Vscode,
                "VS Code Remote",
                true,
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
                available: compatible,
                compatibility: if compatible {
                    RemoteTargetCompatibility::Compatible
                } else {
                    RemoteTargetCompatibility::Unsupported
                },
                unavailable_reason: (!compatible).then(|| "mobaSessionUnsupported".into()),
                can_open_vscode: false,
            };
            if let (Some(host), Some(user), Some(port)) = (session.host, session.user, session.port)
            {
                result.push(ResolvedTarget {
                    public,
                    connection: Connection::Direct { host, user, port },
                });
            } else {
                result.push(ResolvedTarget {
                    public,
                    connection: Connection::Direct {
                        host: String::new(),
                        user: String::new(),
                        port: 0,
                    },
                });
            }
        }
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

fn resolve(id: &str) -> BridgeResult<ResolvedTarget> {
    discovered()?
        .into_iter()
        .find(|target| target.public.id == id)
        .ok_or_else(|| "invalidTarget".into())
}

fn target_command(id: &str) -> BridgeResult<(Command, String)> {
    let target = resolve(id)?;
    if !target.public.available
        || target.public.compatibility != RemoteTargetCompatibility::Compatible
    {
        return Err(target
            .public
            .unavailable_reason
            .unwrap_or_else(|| "targetUnsupported".into()));
    }
    let mut cmd = command();
    let destination = match target.connection {
        Connection::Config { alias, config } => {
            if let Some(path) = config {
                cmd.arg("-F").arg(path);
            }
            alias
        }
        Connection::Direct { host, user, port } => {
            if !safe_host(&host) || !safe_name(&user) || port == 0 {
                return Err("mobaSessionUnsupported".into());
            }
            cmd.arg("-p").arg(port.to_string()).arg("-l").arg(user);
            host
        }
    };
    Ok((cmd, destination))
}

// Reuse OpenSSH's resolution for IdentityFile, ProxyJump and ssh-agent. Refuse
// preconfigured forwards so a bridge can only open the reviewed endpoints.
pub fn validate_target(target_id: &str) -> BridgeResult<()> {
    effective_target(target_id).map(|_| ())
}
pub fn fingerprint(target_id: &str) -> BridgeResult<String> {
    use sha2::{Digest, Sha256};
    Ok(hex::encode(Sha256::digest(format!(
        "{target_id}\n{}",
        effective_target(target_id)?
    ))))
}
fn effective_target(target_id: &str) -> BridgeResult<String> {
    let (mut cmd, destination) = target_command(target_id)?;
    cmd.args(["-G", &destination]);
    let text = output(cmd, None, 12)?;
    for line in text.lines() {
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        match key {
            "user" if value.eq_ignore_ascii_case("root") => return Err("rootForbidden".into()),
            "user" if !safe_name(value) => return Err("sshConfigUnsafe".into()),
            "hostname" if !(safe_name(value) || value.parse::<std::net::IpAddr>().is_ok()) => {
                return Err("sshConfigUnsafe".into())
            }
            "port" if value.parse::<u16>().ok().filter(|p| *p > 0).is_none() => {
                return Err("sshConfigUnsafe".into())
            }
            "localforward" | "remoteforward" | "dynamicforward" => {
                return Err("sshConfigUnsafe".into())
            }
            _ => (),
        }
    }
    Ok(text)
}

pub struct OwnedChild {
    pub child: Child,
    #[cfg(windows)]
    job: windows::Win32::Foundation::HANDLE,
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

pub fn output(mut cmd: Command, input: Option<String>, seconds: u64) -> BridgeResult<String> {
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
    let deadline = Instant::now() + Duration::from_secs(seconds);
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
        let error = String::from_utf8_lossy(&error).to_lowercase();
        return Err(if error.contains("host key") {
            "hostKey"
        } else if error.contains("permission denied") {
            "sshAuth"
        } else if error.contains("forward") {
            "forwardDenied"
        } else {
            "sshFailed"
        }
        .into());
    }
    String::from_utf8(output).map_err(|_| "remoteFailed".into())
}

pub fn remote(alias: &str, request: serde_json::Value) -> BridgeResult<serde_json::Value> {
    let operation = request["operation"].as_str().ok_or("invalidRequest")?;
    if ![
        "check",
        "verify",
        "internet",
        "test",
        "preview",
        "apply",
        "restore",
        "restore-preview",
    ]
    .contains(&operation)
    {
        return Err("invalidRequest".into());
    }
    let tool = request["tool"].as_str().unwrap_or("codex");
    if !["codex", "claude"].contains(&tool) {
        return Err("invalidRequest".into());
    }
    let port = request["port"].as_u64().unwrap_or(25721);
    if !(1024..=65535).contains(&port) {
        return Err("invalidPort".into());
    }
    let mut ports = Vec::new();
    if let Some(values) = request["ports"].as_array() {
        for value in values {
            let value = value.as_u64().ok_or("invalidPort")?;
            if !(1024..=65535).contains(&value) {
                return Err("invalidPort".into());
            }
            ports.push(value.to_string());
        }
    }
    if operation == "test" {
        ports = vec![port.to_string()];
    }
    let expected = request["expectedHash"].as_str().unwrap_or("absent");
    let expected_backup = request["backupHash"].as_str().unwrap_or("absent");
    let expected_state = request["stateHash"].as_str().unwrap_or("absent");
    for hash in [expected, expected_backup, expected_state] {
        if hash != "absent" && !(hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err("invalidRequest".into());
        }
    }
    let scheme = if request["protocol"] == "socks5" {
        "socks5h"
    } else {
        "http"
    };
    let source = format!("operation='{operation}'\ntool='{tool}'\nport={port}\nports='{}'\nexpected='{expected}'\nexpected_backup='{expected_backup}'\nexpected_state='{expected_state}'\nscheme='{scheme}'\n{}", ports.join(" "), include_str!("remote.sh"));
    let (mut cmd, destination) = target_command(alias)?;
    cmd.arg("-oClearAllForwardings=yes")
        .arg(destination)
        .arg("sh -s");
    let text = output(cmd, Some(source), 25)?;
    let value: serde_json::Value =
        serde_json::from_str(text.trim()).map_err(|_| "remoteUnsupported")?;
    if let Some(code) = value.get("error").and_then(|v| v.as_str()) {
        // This allowlist also prevents a malicious SSH server leaking arbitrary text.
        const CODES: &[&str] = &[
            "invalidPort",
            "remoteUnsupported",
            "unsafeBinding",
            "portInUse",
            "unsafePath",
            "configConflict",
            "cliUnsupported",
            "customHome",
            "invalidRequest",
            "noBackup",
            "verifyFailed",
            "rollbackConflict",
            "rollbackFailed",
            "writeRolledBack",
            "networkFailed",
            "dependencyMissing",
            "remoteFailed",
            "rootForbidden",
        ];
        return Err(if CODES.contains(&code) {
            code
        } else {
            "remoteFailed"
        }
        .into());
    }
    let valid = match operation {
        "check" | "verify" => value["verified"] == true,
        "test" => value["tested"] == true,
        "apply" => value["configured"] == true,
        "restore" => value["configured"] == false,
        _ => value.is_object(),
    };
    if !valid {
        return Err("remoteFailed".into());
    }
    Ok(value)
}

pub fn tunnel(request: &Request, endpoints: &[(u16, String, u16)]) -> BridgeResult<OwnedChild> {
    let (mut cmd, destination) = target_command(&request.target_id)?;
    cmd.args(["-N", "-oClearAllForwardings=no"]);
    for (remote, host, local) in endpoints {
        let host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.clone()
        };
        cmd.arg("-R")
            .arg(format!("127.0.0.1:{remote}:{host}:{local}"));
    }
    cmd.arg(destination)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    OwnedChild::spawn(&mut cmd)
}

pub(super) fn extension_remote(
    alias: &str,
    request: &serde_json::Value,
) -> BridgeResult<serde_json::Value> {
    // JSON is data inside a quoted heredoc. No user-provided command or path is executed.
    let source = format!("{}\n\"$bridge_node\" - <<'PROXYENV_EXTENSION_JS'\nglobalThis.bridgeExtensionRequest = {};\n{}\nPROXYENV_EXTENSION_JS\n",
        include_str!("extension-launch.sh"), request, include_str!("extension-helper.cjs"));
    let (mut cmd, destination) = target_command(alias)?;
    cmd.arg("-oClearAllForwardings=yes")
        .arg(destination)
        .arg("sh -s");
    let raw = output(cmd, Some(source), 30)?;
    let value: serde_json::Value = serde_json::from_str(raw.trim()).map_err(|_| "remoteFailed")?;
    if let Some(error) = value["error"].as_str() {
        return Err(match error {
            "configConflict"
            | "unsafePath"
            | "noBackup"
            | "verifyFailed"
            | "rollbackConflict"
            | "rollbackFailed"
            | "writeRolledBack"
            | "extensionMissing"
            | "extensionUnsupported"
            | "extensionContextChanged"
            | "customHome"
            | "remoteUnsupported" => error,
            _ => "remoteFailed",
        }
        .into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
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
