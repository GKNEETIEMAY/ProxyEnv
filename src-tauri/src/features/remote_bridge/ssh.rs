use super::{BridgeResult, Request};
use std::{
    io::{Read, Write},
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

fn aliases_at(path: &std::path::Path) -> BridgeResult<Vec<String>> {
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
pub fn aliases() -> BridgeResult<Vec<String>> {
    let default = dirs::home_dir()
        .ok_or("sshConfigMissing")?
        .join(".ssh/config");
    let mut result = aliases_at(&default)?;
    // An unrelated invalid VS Code settings file must not break OpenSSH hosts.
    if let Some(path) = super::vscode::custom_ssh_config().ok().flatten() {
        if default.canonicalize().ok().as_ref() != Some(&path) {
            result.extend(
                aliases_at(&path)?
                    .into_iter()
                    .map(|alias| format!("vscode:{alias}")),
            );
        }
    }
    Ok(result)
}
pub fn target_parts(target: &str) -> BridgeResult<(&str, Option<std::path::PathBuf>)> {
    if let Some(alias) = target.strip_prefix("vscode:") {
        if !safe_name(alias) {
            return Err("invalidTarget".into());
        }
        Ok((
            alias,
            Some(super::vscode::custom_ssh_config()?.ok_or("vscodeConfigInvalid")?),
        ))
    } else if safe_name(target) {
        Ok((target, None))
    } else {
        Err("invalidTarget".into())
    }
}
fn target_command(target: &str) -> BridgeResult<(Command, &str)> {
    let (alias, config) = target_parts(target)?;
    let mut cmd = command();
    if let Some(path) = config {
        cmd.arg("-F").arg(path);
    }
    Ok((cmd, alias))
}

// Reuse OpenSSH's resolution for IdentityFile, ProxyJump and ssh-agent. Refuse
// preconfigured forwards so a bridge can only open the reviewed endpoints.
pub fn validate_target(alias: &str) -> BridgeResult<()> {
    effective_target(alias).map(|_| ())
}
pub fn fingerprint(alias: &str) -> BridgeResult<String> {
    use sha2::{Digest, Sha256};
    Ok(hex::encode(Sha256::digest(
        effective_target(alias)?.as_bytes(),
    )))
}
fn effective_target(alias: &str) -> BridgeResult<String> {
    if !aliases()?.contains(&alias.to_owned()) {
        return Err("invalidTarget".into());
    }
    let (mut cmd, alias) = target_command(alias)?;
    cmd.args(["-G", alias]);
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
    for hash in [expected, expected_backup] {
        if hash != "absent" && !(hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err("invalidRequest".into());
        }
    }
    let scheme = if request["protocol"] == "socks5" {
        "socks5h"
    } else {
        "http"
    };
    let source = format!("operation='{operation}'\ntool='{tool}'\nport={port}\nports='{}'\nexpected='{expected}'\nexpected_backup='{expected_backup}'\nscheme='{scheme}'\n{}", ports.join(" "), include_str!("remote.sh"));
    let (mut cmd, alias) = target_command(alias)?;
    cmd.args(["-oClearAllForwardings=yes", alias, "sh -s"]);
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
    let (mut cmd, alias) = target_command(&request.alias)?;
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
    cmd.arg(alias)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    OwnedChild::spawn(&mut cmd)
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
        for value in ["vscode:host;id", "vscode:-R", "vscode:a\nb", "vscode:$(id)"] {
            assert!(target_parts(value).is_err());
        }
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
