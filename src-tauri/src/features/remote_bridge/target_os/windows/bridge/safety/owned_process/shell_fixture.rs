use super::{ManagedLaunch, OwnedProcess};
use crate::features::remote_bridge::target_os::windows::WindowsShell;
use std::{env, io, os::windows::ffi::OsStrExt, path::PathBuf};

pub(in super::super) const OUTPUT_DONE: &str = "ROUTESPAN-M33-DONE";
pub(in super::super) const CWD_WITNESS: &str = "routespan-managed-cwd.witness";

#[derive(Clone, Copy)]
pub(in super::super) enum ShellProgram {
    Cmd,
    WindowsPowerShell,
    PowerShell7,
}

impl ShellProgram {
    pub fn kind(self) -> WindowsShell {
        match self {
            Self::Cmd => WindowsShell::Cmd,
            Self::WindowsPowerShell | Self::PowerShell7 => WindowsShell::PowerShell,
        }
    }

    pub fn executable(self) -> Option<PathBuf> {
        let path = match self {
            // cmd parses forward slashes in argv[0] as option delimiters.
            Self::Cmd => PathBuf::from(env::var_os("SystemRoot")?)
                .join("System32")
                .join("cmd.exe"),
            Self::WindowsPowerShell => PathBuf::from(env::var_os("SystemRoot")?)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe"),
            Self::PowerShell7 => {
                let standard = PathBuf::from(env::var_os("ProgramFiles")?)
                    .join("PowerShell")
                    .join("7")
                    .join("pwsh.exe");
                if standard.is_file() {
                    return Some(standard);
                }
                return env::split_paths(&env::var_os("PATH")?)
                    .filter(|directory| directory.is_absolute())
                    .map(|directory| directory.join("pwsh.exe"))
                    .find(|executable| executable.is_file());
            }
        };
        (path.is_absolute() && path.is_file()).then_some(path)
    }
}

impl OwnedProcess {
    pub(in super::super) fn start_shell_fixture(
        program: ShellProgram,
        launch: &ManagedLaunch<'_>,
        cmd_probe_test: &str,
    ) -> io::Result<Self> {
        if launch.shell != program.kind()
            || !launch
                .receipt
                .strip_prefix("ROUTESPAN-MANAGED/1 ")
                .is_some_and(|nonce| {
                    nonce.len() == 32 && nonce.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let executable = program.executable().ok_or(io::ErrorKind::NotFound)?;
        let application: Vec<_> = executable.as_os_str().encode_wide().collect();
        if matches!(program, ShellProgram::Cmd) && application.contains(&u16::from(b'/')) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut command = vec![u16::from(b'"')];
        command.extend(application);
        command.push(u16::from(b'"'));
        if matches!(program, ShellProgram::Cmd) {
            let probe = env::current_exe()?;
            let path: Vec<_> = probe.as_os_str().encode_wide().collect();
            let special = b"\"%!\r\n".map(u16::from);
            if path.iter().any(|unit| special.contains(unit))
                || !cmd_probe_test
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_:".contains(&byte))
            {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            command.extend(" /d /q /s /c \"\"".encode_utf16());
            command.extend(path);
            command.extend(
                format!("\" --exact {cmd_probe_test} --ignored --nocapture\"").encode_utf16(),
            );
        } else {
            let receipt = launch.receipt;
            let arguments = format!(
                " -NoLogo -NoProfile -NonInteractive -Command \"\
                 [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false);\
                 [Console]::WriteLine('{receipt}');\
                 [Console]::WriteLine('ROUTESPAN-SHELL='+$PSVersionTable.PSVersion.ToString());\
                 foreach($name in @('HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','NO_PROXY','ROUTESPAN_M33_UNRELATED'))\
                 {{ $value=[Environment]::GetEnvironmentVariable($name);\
                 if($null -ne $value){{[Console]::WriteLine($name+'='+$value)}} }};\
                 [Console]::WriteLine([Environment]::CurrentDirectory);\
                 [Console]::WriteLine('{OUTPUT_DONE}');[void][Console]::ReadLine();exit 0\""
            );
            command.extend(arguments.encode_utf16());
        }
        command.push(0);
        Self::spawn_suspended(executable.as_os_str(), &mut command, Some(launch))
    }
}
