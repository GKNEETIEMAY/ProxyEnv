use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use sysinfo::System;

use super::RunningApplication;

const SYSTEM_PROCESS_NAMES: &[&str] = &[
    "applicationframehost.exe",
    "csrss.exe",
    "dwm.exe",
    "explorer.exe",
    "fontdrvhost.exe",
    "idle",
    "lockapp.exe",
    "lsass.exe",
    "memory compression",
    "registry",
    "searchhost.exe",
    "services.exe",
    "shellexperiencehost.exe",
    "smss.exe",
    "startmenuexperiencehost.exe",
    "svchost.exe",
    "system",
    "system idle process",
    "systemsettings.exe",
    "taskhostw.exe",
    "textinputhost.exe",
    "wininit.exe",
    "winlogon.exe",
];

pub fn enumerate() -> Vec<RunningApplication> {
    let system = System::new_all();
    let selectable_pids = selectable_process_ids(&system);
    let current_pid = std::process::id();
    let applications = system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let pid = pid.as_u32();
            let process_name = process.name().to_string_lossy().trim().to_owned();
            let executable_path = process.exe().map(Path::to_path_buf);
            (pid != current_pid
                && !is_known_system_process(&process_name)
                && is_selectable_process(pid, executable_path.as_deref(), &selectable_pids))
            .then(|| RunningApplication {
                pid,
                display_name: display_name(&process_name, executable_path.as_deref()),
                process_name,
                executable_path,
                // Icon extraction is intentionally deferred; this flag must not
                // claim an icon exists before one has actually been loaded.
                icon_available: false,
            })
        })
        .collect();
    normalize_applications(applications)
}

fn normalize_applications(mut applications: Vec<RunningApplication>) -> Vec<RunningApplication> {
    applications.sort_by(|left, right| {
        left.display_name
            .to_ascii_lowercase()
            .cmp(&right.display_name.to_ascii_lowercase())
            .then_with(|| left.pid.cmp(&right.pid))
    });
    let mut seen = HashSet::new();
    applications.retain(|application| seen.insert(application_key(application)));
    applications
}

fn application_key(application: &RunningApplication) -> String {
    application
        .executable_path
        .as_deref()
        .map(normalized_path)
        .unwrap_or_else(|| application.process_name.to_ascii_lowercase())
}

fn display_name(process_name: &str, executable_path: Option<&Path>) -> String {
    let source = (!process_name.trim().is_empty())
        .then_some(process_name)
        .or_else(|| {
            executable_path
                .and_then(Path::file_stem)
                .and_then(|name| name.to_str())
                .filter(|name| !name.trim().is_empty())
        })
        .unwrap_or("Application");
    let source = source
        .strip_suffix(".exe")
        .or_else(|| source.strip_suffix(".EXE"))
        .unwrap_or(source);
    source.replace(['-', '_'], " ").trim().to_owned()
}

fn is_known_system_process(process_name: &str) -> bool {
    let normalized = process_name.to_ascii_lowercase();
    SYSTEM_PROCESS_NAMES.contains(&normalized.as_str())
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

#[cfg(windows)]
fn is_selectable_process(
    pid: u32,
    executable_path: Option<&Path>,
    visible_pids: &HashSet<u32>,
) -> bool {
    visible_pids.contains(&pid) && !executable_path.is_some_and(is_windows_system_path)
}

#[cfg(windows)]
fn is_windows_system_path(path: &Path) -> bool {
    let path = normalized_path(path);
    let windows_root = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let windows_root = normalized_path(&windows_root);
    path == windows_root || path.starts_with(&format!("{windows_root}\\"))
}

#[cfg(windows)]
fn selectable_process_ids(_: &System) -> HashSet<u32> {
    use windows::{
        core::BOOL,
        Win32::{
            Foundation::{HWND, LPARAM},
            UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible},
        },
    };

    unsafe extern "system" fn collect_visible_window_pid(hwnd: HWND, state: LPARAM) -> BOOL {
        if !unsafe { IsWindowVisible(hwnd).as_bool() } {
            return BOOL(1);
        }
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid != 0 {
            let pids = state.0 as *mut HashSet<u32>;
            if let Some(pids) = unsafe { pids.as_mut() } {
                pids.insert(pid);
            }
        }
        BOOL(1)
    }

    let mut pids = HashSet::new();
    let state = LPARAM((&mut pids as *mut HashSet<u32>) as isize);
    let _ = unsafe { EnumWindows(Some(collect_visible_window_pid), state) };
    pids
}

#[cfg(target_os = "macos")]
fn selectable_process_ids(system: &System) -> HashSet<u32> {
    system
        .processes()
        .keys()
        .map(sysinfo::Pid::as_u32)
        .collect()
}

#[cfg(target_os = "macos")]
fn is_selectable_process(
    pid: u32,
    executable_path: Option<&Path>,
    selectable_pids: &HashSet<u32>,
) -> bool {
    selectable_pids.contains(&pid)
        && executable_path.is_some_and(|path| {
            let path = path.to_string_lossy();
            path.contains(".app/Contents/MacOS/") || is_under_user_home(path.as_ref())
        })
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn selectable_process_ids(system: &System) -> HashSet<u32> {
    system
        .processes()
        .keys()
        .map(sysinfo::Pid::as_u32)
        .collect()
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn is_selectable_process(
    pid: u32,
    executable_path: Option<&Path>,
    selectable_pids: &HashSet<u32>,
) -> bool {
    selectable_pids.contains(&pid)
        && executable_path.is_some_and(|path| is_under_user_home(&path.to_string_lossy()))
}

#[cfg(not(windows))]
fn is_under_user_home(path: &str) -> bool {
    dirs::home_dir().is_some_and(|home| Path::new(path).starts_with(home))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn application(pid: u32, name: &str, path: Option<&str>) -> RunningApplication {
        RunningApplication {
            pid,
            process_name: name.into(),
            display_name: display_name(name, path.map(Path::new)),
            executable_path: path.map(PathBuf::from),
            icon_available: false,
        }
    }

    #[test]
    fn derives_readable_display_names_without_guessing_product_metadata() {
        assert_eq!(display_name("Code.exe", None), "Code");
        assert_eq!(
            display_name("", Some(Path::new(r"C:\Apps\Example-Desktop.exe"))),
            "Example Desktop"
        );
    }

    #[test]
    fn filters_known_windows_shell_and_system_process_names() {
        assert!(is_known_system_process("EXPLORER.EXE"));
        assert!(is_known_system_process("svchost.exe"));
        assert!(!is_known_system_process("Discord.exe"));
    }

    #[test]
    fn sorts_and_deduplicates_multiple_processes_for_the_same_executable() {
        let applications = normalize_applications(vec![
            application(30, "Discord.exe", Some(r"C:\Apps\Discord.exe")),
            application(20, "Discord.exe", Some(r"c:\apps\discord.exe")),
            application(10, "Code.exe", Some(r"C:\Apps\Code.exe")),
        ]);

        assert_eq!(applications.len(), 2);
        assert_eq!(applications[0].display_name, "Code");
        assert_eq!(applications[1].display_name, "Discord");
        assert_eq!(applications[1].pid, 20);
    }

    #[test]
    fn enumeration_excludes_proxyenv_itself_and_returns_unique_applications() {
        let applications = enumerate();
        assert!(applications
            .iter()
            .all(|application| application.pid != std::process::id()));
        let unique_keys = applications
            .iter()
            .map(application_key)
            .collect::<HashSet<_>>();
        assert_eq!(unique_keys.len(), applications.len());
    }
}
