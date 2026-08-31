use std::{path::Path, thread, time::Duration};

use sysinfo::{Pid, System};

use crate::error::{ProxyEnvError, Result};

use super::{launcher, LaunchApplicationResult, LaunchEnvironmentMode, ManagedApplication};

const GRACEFUL_CLOSE_WAIT: Duration = Duration::from_millis(1_500);
const FORCE_CLOSE_WAIT: Duration = Duration::from_millis(3_000);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(75);

pub fn restart_without_proxy(
    application: &ManagedApplication,
    pid: u32,
) -> Result<LaunchApplicationResult> {
    let executable = launcher::validate_executable(&application.executable_path)?;
    close_authorized_process(pid, &executable)?;
    launcher::launch(
        application,
        LaunchEnvironmentMode::ClearProxyVariables,
        None,
    )
}

fn close_authorized_process(pid: u32, expected_executable: &Path) -> Result<()> {
    if pid == 0 || pid == std::process::id() {
        return Err(ProxyEnvError::ApplicationRestart(
            "the selected process cannot be closed".into(),
        ));
    }

    let system = System::new_all();
    let process = system.process(Pid::from_u32(pid)).ok_or_else(|| {
        ProxyEnvError::ApplicationRestart("the selected process is no longer running".into())
    })?;
    let actual_executable = process.exe().ok_or_else(|| {
        ProxyEnvError::ApplicationRestart(
            "the selected process executable could not be verified".into(),
        )
    })?;

    if !same_executable(expected_executable, actual_executable)? {
        return Err(ProxyEnvError::ApplicationRestart(
            "the selected process no longer matches the authorized application".into(),
        ));
    }

    #[cfg(windows)]
    if post_close_messages(pid) && wait_until_stopped(pid, GRACEFUL_CLOSE_WAIT) {
        return Ok(());
    }

    let system = System::new_all();
    let Some(process) = system.process(Pid::from_u32(pid)) else {
        return Ok(());
    };
    let actual_executable = process.exe().ok_or_else(|| {
        ProxyEnvError::ApplicationRestart(
            "the selected process executable could not be reverified".into(),
        )
    })?;
    if !same_executable(expected_executable, actual_executable)? {
        return Err(ProxyEnvError::ApplicationRestart(
            "the selected process changed before it could be closed".into(),
        ));
    }
    if !process.kill() {
        return Err(ProxyEnvError::ApplicationRestart(
            "the selected process refused to close".into(),
        ));
    }
    if !wait_until_stopped(pid, FORCE_CLOSE_WAIT) {
        return Err(ProxyEnvError::ApplicationRestart(
            "the selected process did not stop in time".into(),
        ));
    }
    Ok(())
}

fn same_executable(expected: &Path, actual: &Path) -> Result<bool> {
    let expected = expected
        .canonicalize()
        .map_err(|error| ProxyEnvError::ApplicationRestart(error.to_string()))?;
    let actual = actual
        .canonicalize()
        .map_err(|error| ProxyEnvError::ApplicationRestart(error.to_string()))?;
    #[cfg(windows)]
    {
        Ok(normalized_windows_path(&expected) == normalized_windows_path(&actual))
    }
    #[cfg(not(windows))]
    {
        Ok(expected == actual)
    }
}

#[cfg(windows)]
fn normalized_windows_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_ascii_lowercase()
}

fn wait_until_stopped(pid: u32, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if System::new_all().process(Pid::from_u32(pid)).is_none() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

#[cfg(windows)]
fn post_close_messages(pid: u32) -> bool {
    use windows::{
        core::BOOL,
        Win32::{
            Foundation::{HWND, LPARAM, WPARAM},
            UI::WindowsAndMessaging::{
                EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
            },
        },
    };

    struct CloseWindowsState {
        pid: u32,
        found: bool,
    }

    unsafe extern "system" fn close_window(hwnd: HWND, state: LPARAM) -> BOOL {
        let Some(state) = (state.0 as *mut CloseWindowsState).as_mut() else {
            return BOOL(0);
        };
        let mut window_pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        if window_pid == state.pid {
            state.found = true;
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        BOOL(1)
    }

    let mut state = CloseWindowsState { pid, found: false };
    let parameter = LPARAM((&mut state as *mut CloseWindowsState) as isize);
    let _ = unsafe { EnumWindows(Some(close_window), parameter) };
    state.found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_canonical_executable_paths() {
        let current = std::env::current_exe().unwrap();
        assert!(same_executable(&current, &current).unwrap());
    }

    #[test]
    fn refuses_to_close_proxyenv_itself() {
        let current = std::env::current_exe().unwrap();
        let error = close_authorized_process(std::process::id(), &current).unwrap_err();
        assert!(error
            .to_string()
            .starts_with("failed to restart application"));
    }
}
