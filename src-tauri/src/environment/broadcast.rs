use crate::error::{ProxyEnvError, Result};

#[cfg(windows)]
pub fn environment_changed() -> Result<()> {
    use windows::{
        Win32::Foundation::{LPARAM, WPARAM},
        Win32::UI::WindowsAndMessaging::*,
    };
    let environment = "Environment\0".encode_utf16().collect::<Vec<_>>();
    let mut message_result = 0_usize;
    let send_result = unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(environment.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            2_000,
            Some(&mut message_result),
        )
    };
    if send_result.0 == 0 {
        return Err(ProxyEnvError::BroadcastFailed(
            "SendMessageTimeoutW timed out".into(),
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn environment_changed() -> Result<()> {
    Err(ProxyEnvError::UnsupportedPlatform)
}
