use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowsShell {
    Cmd,
    PowerShell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowsProbeReason {
    NoReply,
    UnexpectedReply,
    OutputLimit,
    Timeout,
    SshConfigUnsupported,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum WindowsTargetProbe {
    Confirmed { shell: WindowsShell },
    Unknown { reason: WindowsProbeReason },
}

pub(in crate::features::remote_bridge) fn allows_direct_probe(effective: &str) -> bool {
    // A nested SSH client or external host-key helper does not inherit our
    // read-only flags. CheckHostIP can append to known_hosts on its own.
    !effective.lines().any(|line| {
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        let value = value.trim();
        match key {
            "checkhostip" => !matches!(value, "no" | "false"),
            "proxyjump" | "proxycommand" | "knownhostscommand" => !matches!(value, "" | "none"),
            _ => false,
        }
    })
}

#[cfg(windows)]
pub(in crate::features::remote_bridge) use implementation::*;

#[cfg(windows)]
mod implementation {
    use super::{WindowsProbeReason, WindowsShell, WindowsTargetProbe};
    use crate::features::remote_bridge::{base64_bytes, ssh, BridgeResult};
    use std::{
        io::Read,
        process::{Command, Stdio},
        sync::atomic::{AtomicBool, Ordering},
        thread,
        time::{Duration, Instant},
    };

    pub(in crate::features::remote_bridge) const PROBE_BUDGET: Duration = Duration::from_secs(25);
    const OUTPUT_LIMIT: usize = 4096;

    pub(in crate::features::remote_bridge) const SSH_READ_ONLY_OPTIONS: [&str; 7] = [
        "-oClearAllForwardings=yes",
        "-oUpdateHostKeys=no",
        "-oAddKeysToAgent=no",
        "-oCheckHostIP=no",
        "-oProxyJump=none",
        "-oProxyCommand=none",
        "-oTunnel=no",
    ];

    struct Candidate {
        shell: WindowsShell,
        command: String,
        expected: String,
    }

    impl Candidate {
        fn new(shell: WindowsShell, nonce: [u8; 16]) -> Self {
            let name = match shell {
                WindowsShell::PowerShell => "powershell",
                WindowsShell::Cmd => "cmd",
            };
            let expected = format!("ROUTESPAN_TARGET/1 {} windows {name}", hex::encode(nonce));
            let script = format!(
                "if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {{ [Console]::WriteLine('{expected}') }}"
            );
            let command = match shell {
                WindowsShell::PowerShell => script,
                WindowsShell::Cmd => format!(
                    r#"if "1"=="1" "C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -EncodedCommand {}"#,
                    encode_powershell(&script)
                ),
            };
            Self {
                shell,
                command,
                expected,
            }
        }

        fn classify(&self, reply: Reply) -> BridgeResult<Option<WindowsTargetProbe>> {
            let unknown = |reason| Some(WindowsTargetProbe::Unknown { reason });
            let line = reply.stdout.strip_suffix(b"\n");
            let exact = line.is_some_and(|line| {
                line.strip_suffix(b"\r").unwrap_or(line) == self.expected.as_bytes()
            });
            match reply.exit_code {
                None | Some(255) => Err(ssh::classify_ssh_error(&String::from_utf8_lossy(
                    &reply.stderr,
                ))),
                Some(0) if exact && reply.stderr.is_empty() => {
                    Ok(Some(WindowsTargetProbe::Confirmed { shell: self.shell }))
                }
                Some(0 | 1) if reply.stdout.is_empty() => Ok(None),
                _ => Ok(unknown(WindowsProbeReason::UnexpectedReply)),
            }
        }
    }

    fn encode_powershell(script: &str) -> String {
        let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        base64_bytes(&bytes)
    }

    pub(in crate::features::remote_bridge) fn probe(
        mut run: impl FnMut(&str) -> BridgeResult<Reply>,
    ) -> BridgeResult<WindowsTargetProbe> {
        for shell in [WindowsShell::PowerShell, WindowsShell::Cmd] {
            let mut nonce = [0; 16];
            getrandom::fill(&mut nonce).map_err(|_| "stateUnavailable")?;
            let candidate = Candidate::new(shell, nonce);
            if let Some(detected) = candidate.classify(run(&candidate.command)?)? {
                return Ok(detected);
            }
        }
        Ok(WindowsTargetProbe::Unknown {
            reason: WindowsProbeReason::NoReply,
        })
    }

    pub(in crate::features::remote_bridge) struct Reply {
        exit_code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    }

    pub(in crate::features::remote_bridge) fn capture(
        mut command: Command,
        deadline: Instant,
    ) -> BridgeResult<Reply> {
        if Instant::now() >= deadline {
            return Err("sshTimeout".into());
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut process = ssh::OwnedChild::spawn(&mut command)?;
        let stdout = process.child.stdout.take().ok_or("processFailed")?;
        let stderr = process.child.stderr.take().ok_or("processFailed")?;
        let exceeded = &AtomicBool::new(false);
        thread::scope(|scope| {
            let stdout_reader = scope.spawn(move || read_pipe(stdout, exceeded));
            let stderr_reader = scope.spawn(move || read_pipe(stderr, exceeded));
            let status = loop {
                if exceeded.load(Ordering::Relaxed) {
                    break Err("targetProbeOutputLimit");
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break Err("sshTimeout");
                }
                match process.child.try_wait() {
                    Ok(Some(status)) => break Ok(status),
                    Ok(None) => thread::sleep(remaining.min(Duration::from_millis(15))),
                    Err(_) => break Err("processFailed"),
                }
            };
            // Closing the Windows Job Object also terminates descendants that
            // could otherwise keep either pipe open after the SSH client exits.
            drop(process);
            let stdout = stdout_reader.join().map_err(|_| "processFailed")??;
            let stderr = stderr_reader.join().map_err(|_| "processFailed")??;
            if exceeded.load(Ordering::Relaxed) {
                return Err("targetProbeOutputLimit".into());
            }
            Ok(Reply {
                exit_code: status?.code(),
                stdout,
                stderr,
            })
        })
    }

    fn read_pipe(stream: impl Read, exceeded: &AtomicBool) -> Result<Vec<u8>, &'static str> {
        let mut bytes = Vec::new();
        stream
            .take((OUTPUT_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "processFailed")?;
        if bytes.len() > OUTPUT_LIMIT {
            exceeded.store(true, Ordering::Relaxed);
        }
        Ok(bytes)
    }

    #[cfg(test)]
    mod tests {
        use super::super::allows_direct_probe;
        use super::*;
        use std::os::windows::process::CommandExt;

        fn reply(code: i32, stdout: impl AsRef<[u8]>, stderr: impl AsRef<[u8]>) -> Reply {
            Reply {
                exit_code: Some(code),
                stdout: stdout.as_ref().to_vec(),
                stderr: stderr.as_ref().to_vec(),
            }
        }

        fn cmd(script: &str) -> Command {
            let mut process = Command::new("cmd.exe");
            process.args(["/d", "/c"]).raw_arg(script);
            process
        }

        #[test]
        fn windows_probe_allows_only_one_exact_nonce_bound_ascii_line() {
            let candidate = Candidate::new(WindowsShell::PowerShell, [0xab; 16]);
            let valid = &candidate.expected;
            for ending in ["\n", "\r\n"] {
                assert_eq!(
                    candidate
                        .classify(reply(0, format!("{valid}{ending}"), []))
                        .unwrap(),
                    Some(WindowsTargetProbe::Confirmed {
                        shell: WindowsShell::PowerShell
                    })
                );
            }
            for invalid in [
                valid.clone(),
                format!("{valid}\n\n"),
                format!("{valid}\n{valid}\n"),
                format!("{valid}\r\r\n"),
                format!("banner\n{valid}\n"),
                format!("{valid} \n"),
                format!("\u{feff}{valid}\n"),
                format!("\0{valid}\n"),
                format!(
                    "{}\n",
                    Candidate::new(WindowsShell::Cmd, [0xab; 16]).expected
                ),
                format!(
                    "{}\n",
                    Candidate::new(WindowsShell::PowerShell, [0xba; 16]).expected
                ),
            ] {
                assert_eq!(
                    candidate.classify(reply(0, invalid, [])).unwrap(),
                    Some(WindowsTargetProbe::Unknown {
                        reason: WindowsProbeReason::UnexpectedReply
                    })
                );
            }
            for invalid in [
                reply(1, format!("{valid}\n"), []),
                reply(0, format!("{valid}\n"), "warning"),
            ] {
                assert_eq!(
                    candidate.classify(invalid).unwrap(),
                    Some(WindowsTargetProbe::Unknown {
                        reason: WindowsProbeReason::UnexpectedReply
                    })
                );
            }
        }

        #[test]
        fn windows_probe_falls_back_only_from_empty_stdout_and_compatible_exit_status() {
            for (status, stdout, expected_attempts) in
                [(0, "", 2), (1, "", 2), (2, "", 1), (1, "noise", 1)]
            {
                let mut attempts = 0;
                let result = probe(|_| {
                    attempts += 1;
                    Ok(reply(status, stdout, "syntax"))
                })
                .unwrap();
                assert!(matches!(result, WindowsTargetProbe::Unknown { .. }));
                assert_eq!(attempts, expected_attempts);
            }
            for (error, code) in [
                ("Host key verification failed", "hostKey"),
                ("Permission denied", "sshAuth"),
                ("Connection closed", "sshFailed"),
            ] {
                let mut attempts = 0;
                assert_eq!(
                    probe(|_| {
                        attempts += 1;
                        Ok(reply(255, [], error))
                    })
                    .unwrap_err(),
                    code
                );
                assert_eq!(attempts, 1);
            }
        }

        #[test]
        fn windows_probe_executes_the_templates_in_cmd_and_both_powershell_generations() {
            let deadline = || Instant::now() + Duration::from_secs(12);
            let mut attempts = 0;
            let detected = probe(|script| {
                attempts += 1;
                capture(cmd(script), deadline())
            })
            .unwrap();
            assert_eq!(
                detected,
                WindowsTargetProbe::Confirmed {
                    shell: WindowsShell::Cmd
                }
            );
            assert_eq!(attempts, 2);

            for executable in ["powershell.exe", "pwsh.exe"] {
                // Windows PowerShell is built into supported Windows versions.
                // PowerShell 7 is optional on developer workstations.
                let available = Command::new(executable)
                    .args(["-NoProfile", "-NonInteractive", "-Command", "exit 0"])
                    .output();
                if executable == "pwsh.exe"
                    && available
                        .as_ref()
                        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
                {
                    eprintln!("PowerShell 7 shell fixture unavailable on this workstation");
                    continue;
                }
                assert!(available.unwrap().status.success(), "{executable}");
                let mut attempts = 0;
                let detected = probe(|script| {
                    attempts += 1;
                    let mut process = Command::new(executable);
                    process.args([
                        "-NoLogo",
                        "-NoProfile",
                        "-NonInteractive",
                        "-EncodedCommand",
                        &encode_powershell(script),
                    ]);
                    capture(process, deadline())
                })
                .unwrap();
                assert_eq!(
                    detected,
                    WindowsTargetProbe::Confirmed {
                        shell: WindowsShell::PowerShell
                    },
                    "{executable}"
                );
                assert_eq!(attempts, 1, "{executable}");
            }
        }

        #[test]
        fn windows_probe_caps_both_streams_and_terminates_on_deadline() {
            for flood in [
                "for /L %i in (1,1,900) do @echo 0123456789ABCDEF",
                "for /L %i in (1,1,900) do @echo 0123456789ABCDEF 1>&2",
            ] {
                assert_eq!(
                    capture(cmd(flood), Instant::now() + Duration::from_secs(8))
                        .err()
                        .unwrap(),
                    "targetProbeOutputLimit"
                );
            }
            let mut child = Command::new(std::env::current_exe().unwrap());
            child.args([
                "--exact",
                "features::remote_bridge::ssh::tests::owned_sleep_child",
                "--ignored",
            ]);
            let started = Instant::now();
            assert_eq!(
                capture(child, started + Duration::from_millis(300))
                    .err()
                    .unwrap(),
                "sshTimeout"
            );
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "owned child or readers did not exit promptly"
            );
        }

        #[test]
        fn windows_probe_rejects_configs_that_cannot_keep_ssh_read_only() {
            assert!(allows_direct_probe("checkhostip no\nproxyjump none\n"));
            for configured in [
                "checkhostip yes",
                "checkhostip true",
                "proxyjump bastion",
                "proxycommand example",
                "knownhostscommand helper",
            ] {
                assert!(!allows_direct_probe(configured), "{configured}");
            }
        }

        #[test]
        fn windows_probe_templates_return_no_receipt_in_posix_shells() {
            let Some(shell) = std::env::var_os("PROXYENV_TEST_SHELL") else {
                eprintln!("Git Bash fixture needs PROXYENV_TEST_SHELL");
                return;
            };
            for candidate in [WindowsShell::PowerShell, WindowsShell::Cmd] {
                let candidate = Candidate::new(candidate, [0x5a; 16]);
                let mut process = Command::new(&shell);
                process.args(["--noprofile", "--norc", "-c", &candidate.command]);
                let reply = capture(process, Instant::now() + Duration::from_secs(8)).unwrap();
                assert_eq!(reply.exit_code, Some(2));
                assert!(reply.stdout.is_empty());
                assert_eq!(
                    candidate.classify(reply).unwrap(),
                    Some(WindowsTargetProbe::Unknown {
                        reason: WindowsProbeReason::UnexpectedReply,
                    })
                );
            }
        }
    }
}
