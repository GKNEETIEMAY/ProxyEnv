pub(super) mod linux;
pub(super) mod windows;
// M3.1 deliberately has no runtime caller. Keep the internal plan compiled.
#[allow(dead_code)]
#[path = "windows/bridge/mod.rs"]
pub(super) mod windows_bridge;

use super::BridgeResult;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetPlatform {
    Linux,
    Windows,
    Unknown,
    Unsupported,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetPlatformProbe {
    pub target_id: String,
    pub platform: TargetPlatform,
    #[serde(flatten)]
    pub windows_probe: windows::WindowsTargetProbe,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_bridge: Option<WindowsBridgeView>,
}

impl TargetPlatformProbe {
    pub(super) fn new(
        target_id: String,
        platform: TargetPlatform,
        windows_probe: windows::WindowsTargetProbe,
    ) -> Self {
        let windows_bridge = (platform == TargetPlatform::Windows).then_some(WindowsBridgeView {
            transport: WindowsTransportState::Unavailable,
            terminal: WindowsTerminalState::Unavailable,
        });
        Self {
            target_id,
            platform,
            windows_probe,
            windows_bridge,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsBridgeView {
    pub transport: WindowsTransportState,
    pub terminal: WindowsTerminalState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum WindowsTransportState {
    NotPrepared,
    Preparing,
    AwaitingAuthorization,
    Unavailable,
    Closing,
    Disconnected,
    ClosedAbnormally,
    CleanupIncomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum WindowsTerminalState {
    Unprepared,
    AwaitingEnvironment,
    Prepared,
    Unavailable,
    Closed,
}

impl TargetPlatform {
    pub(super) fn from_uname(value: &str) -> Self {
        match value.trim_end_matches(['\r', '\n']) {
            "Linux" => Self::Linux,
            "Darwin" | "FreeBSD" | "OpenBSD" | "NetBSD" | "SunOS" | "AIX" => Self::Unsupported,
            _ => Self::Unknown,
        }
    }

    pub(super) fn require_linux(self) -> BridgeResult<()> {
        match self {
            Self::Linux => Ok(()),
            Self::Windows => Err("windowsBridgeUnavailable".into()),
            Self::Unknown | Self::Unsupported => Err("remoteUnsupported".into()),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum TargetOs {
    Linux,
    Windows,
    Unknown,
}

impl TargetOs {
    // Only this fixed, read-only probe is used for the separate SCP write path.
    // Shell payloads retain their own same-process OS guards.
    pub(super) const PROBE_COMMAND: &str = "uname -s";

    pub(super) fn from_uname(output: &str) -> Self {
        match output.trim_end_matches(['\r', '\n']) {
            "Linux" => Self::Linux,
            "Windows_NT" => Self::Windows,
            _ => Self::Unknown,
        }
    }

    pub(super) fn require_linux(self) -> BridgeResult<()> {
        match self {
            Self::Linux => Ok(()),
            Self::Windows | Self::Unknown => Err("remoteUnsupported".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_probe_preserves_windows_wire_shape_and_rejects_unknown_targets() {
        for (input, expected) in [
            ("Linux\r\n", TargetPlatform::Linux),
            ("Darwin\n", TargetPlatform::Unsupported),
            ("", TargetPlatform::Unknown),
            ("prefix\nLinux\n", TargetPlatform::Unknown),
        ] {
            let platform = TargetPlatform::from_uname(input);
            assert_eq!(platform, expected);
            assert_eq!(
                platform.require_linux().is_ok(),
                expected == TargetPlatform::Linux
            );
        }
        assert_eq!(
            TargetPlatform::Windows.require_linux().unwrap_err(),
            "windowsBridgeUnavailable"
        );
        let snapshot = TargetPlatformProbe::new(
            "candidate".into(),
            TargetPlatform::Windows,
            windows::WindowsTargetProbe::Confirmed {
                shell: windows::WindowsShell::Cmd,
            },
        );
        assert_eq!(
            serde_json::to_value(snapshot).unwrap(),
            serde_json::json!({
                "targetId": "candidate", "platform": "windows", "status": "confirmed", "shell": "cmd",
                "windowsBridge": {"transport": "unavailable", "terminal": "unavailable"}
            })
        );
    }

    #[test]
    fn only_positive_linux_evidence_allows_an_unguarded_transport_write() {
        for value in ["Linux", "Linux\n", "Linux\r\n"] {
            assert_eq!(TargetOs::from_uname(value), TargetOs::Linux);
            assert!(TargetOs::from_uname(value).require_linux().is_ok());
        }
        assert_eq!(TargetOs::from_uname("Windows_NT\r\n"), TargetOs::Windows);
        for value in [
            "Windows_NT",
            "",
            "Darwin",
            "FreeBSD",
            "prefix\nLinux",
            " Linux",
            "Linux\nextra",
        ] {
            assert_eq!(
                TargetOs::from_uname(value).require_linux().unwrap_err(),
                "remoteUnsupported"
            );
        }
    }
}
