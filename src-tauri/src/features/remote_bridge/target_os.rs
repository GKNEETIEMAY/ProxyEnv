pub(super) mod linux;
pub(super) mod windows;

use super::BridgeResult;

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
