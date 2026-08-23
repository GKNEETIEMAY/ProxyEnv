use super::{TunEvidence, TunEvidenceKind, TunObservation, TunObservationState};

const TUNNEL_INTERFACE_TYPE: u32 = 131;

#[derive(Debug, Clone, Eq, PartialEq)]
struct AdapterSnapshot {
    name: String,
    description: String,
    interface_type: u32,
    operational: bool,
}

pub fn observe() -> TunObservation {
    classify(enumerate_adapters())
}

fn classify(adapters: Result<Vec<AdapterSnapshot>, String>) -> TunObservation {
    let adapters = match adapters {
        Ok(adapters) => adapters,
        Err(error) => {
            return TunObservation {
                state: TunObservationState::Unknown,
                interface_name: None,
                description: None,
                evidence: vec![TunEvidence {
                    kind: TunEvidenceKind::EnumerationUnavailable,
                    interface_name: None,
                    detail: error,
                }],
            };
        }
    };

    let candidates = adapters
        .into_iter()
        .filter_map(|adapter| {
            let combined = format!("{} {}", adapter.name, adapter.description).to_lowercase();
            let name_signal = looks_like_virtual_tunnel(&combined);
            let type_signal = adapter.interface_type == TUNNEL_INTERFACE_TYPE;
            (name_signal || type_signal).then_some((adapter, name_signal, type_signal))
        })
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        return TunObservation {
            state: TunObservationState::NotDetected,
            interface_name: None,
            description: None,
            evidence: Vec::new(),
        };
    }

    let primary_index = candidates
        .iter()
        .position(|(adapter, name_signal, type_signal)| {
            adapter.operational && *name_signal && *type_signal
        })
        .or_else(|| {
            candidates
                .iter()
                .position(|(adapter, _, _)| adapter.operational)
        })
        .unwrap_or(0);
    let primary = &candidates[primary_index].0;
    let detected = candidates
        .iter()
        .any(|(adapter, name_signal, type_signal)| {
            adapter.operational && *name_signal && *type_signal
        });
    let mut evidence = Vec::new();

    for (adapter, name_signal, type_signal) in &candidates {
        if *type_signal {
            evidence.push(TunEvidence {
                kind: TunEvidenceKind::TunnelInterfaceType,
                interface_name: Some(adapter.name.clone()),
                detail: "the operating system reports a tunnel interface type".into(),
            });
        }
        if *name_signal {
            evidence.push(TunEvidence {
                kind: TunEvidenceKind::VirtualAdapterName,
                interface_name: Some(adapter.name.clone()),
                detail: "the interface name or description resembles a virtual tunnel adapter"
                    .into(),
            });
        }
        if adapter.operational {
            evidence.push(TunEvidence {
                kind: TunEvidenceKind::InterfaceOperational,
                interface_name: Some(adapter.name.clone()),
                detail: "the interface is currently operational".into(),
            });
        }
    }

    TunObservation {
        state: if detected {
            TunObservationState::Detected
        } else {
            TunObservationState::Possible
        },
        interface_name: non_empty(&primary.name),
        description: non_empty(&primary.description),
        evidence,
    }
}

fn non_empty(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_owned())
}

fn looks_like_virtual_tunnel(value: &str) -> bool {
    [
        "wintun",
        "wireguard",
        "tap-windows",
        "tailscale tunnel",
        "zerotier virtual",
        "clash tun",
        "mihomo tun",
        "sing-box tun",
        "singbox tun",
    ]
    .iter()
    .any(|token| value.contains(token))
}

#[cfg(windows)]
fn enumerate_adapters() -> Result<Vec<AdapterSnapshot>, String> {
    use std::{mem::size_of, slice};
    use windows::{
        core::PWSTR,
        Win32::{
            Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR},
            NetworkManagement::{
                IpHelper::{
                    GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
                    GAA_FLAG_SKIP_MULTICAST, GAA_FLAG_SKIP_UNICAST, GET_ADAPTERS_ADDRESSES_FLAGS,
                    IP_ADAPTER_ADDRESSES_LH,
                },
                Ndis::IfOperStatusUp,
            },
            Networking::WinSock::AF_UNSPEC,
        },
    };

    unsafe fn wide_string(value: PWSTR) -> String {
        if value.is_null() {
            return String::new();
        }

        let mut length = 0usize;
        while length < 32_768 && unsafe { *value.0.add(length) } != 0 {
            length += 1;
        }
        String::from_utf16_lossy(unsafe { slice::from_raw_parts(value.0, length) })
    }

    let flags = GET_ADAPTERS_ADDRESSES_FLAGS(
        GAA_FLAG_SKIP_UNICAST.0
            | GAA_FLAG_SKIP_ANYCAST.0
            | GAA_FLAG_SKIP_MULTICAST.0
            | GAA_FLAG_SKIP_DNS_SERVER.0,
    );
    let mut required_size = 0u32;
    let initial =
        unsafe { GetAdaptersAddresses(AF_UNSPEC.0 as u32, flags, None, None, &mut required_size) };

    if initial == NO_ERROR.0 && required_size == 0 {
        return Ok(Vec::new());
    }
    if initial != ERROR_BUFFER_OVERFLOW.0 {
        return Err(format!(
            "adapter enumeration failed with Windows error {initial}"
        ));
    }

    for _ in 0..2 {
        let unit_count = (required_size as usize).div_ceil(size_of::<usize>());
        let mut buffer = vec![0usize; unit_count];
        let adapter_ptr = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        let result = unsafe {
            GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                flags,
                None,
                Some(adapter_ptr),
                &mut required_size,
            )
        };

        if result == ERROR_BUFFER_OVERFLOW.0 {
            continue;
        }
        if result != NO_ERROR.0 {
            return Err(format!(
                "adapter enumeration failed with Windows error {result}"
            ));
        }

        let mut adapters = Vec::new();
        let mut current = adapter_ptr;
        while !current.is_null() {
            let adapter = unsafe { &*current };
            adapters.push(AdapterSnapshot {
                name: unsafe { wide_string(adapter.FriendlyName) },
                description: unsafe { wide_string(adapter.Description) },
                interface_type: adapter.IfType,
                operational: adapter.OperStatus == IfOperStatusUp,
            });
            current = adapter.Next;
        }
        return Ok(adapters);
    }

    Err("adapter list changed repeatedly while it was being observed".into())
}

#[cfg(not(windows))]
fn enumerate_adapters() -> Result<Vec<AdapterSnapshot>, String> {
    Err("TUN observation is not available on this operating system yet".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(
        name: &str,
        description: &str,
        interface_type: u32,
        operational: bool,
    ) -> AdapterSnapshot {
        AdapterSnapshot {
            name: name.into(),
            description: description.into(),
            interface_type,
            operational,
        }
    }

    #[test]
    fn reports_not_detected_without_virtual_interfaces() {
        let observation = classify(Ok(vec![adapter("Ethernet", "Intel Ethernet", 6, true)]));

        assert_eq!(observation.state, TunObservationState::NotDetected);
        assert!(observation.evidence.is_empty());
    }

    #[test]
    fn reports_possible_for_a_single_name_signal() {
        let observation = classify(Ok(vec![adapter("Wintun Userspace Tunnel", "", 6, true)]));

        assert_eq!(observation.state, TunObservationState::Possible);
        assert_eq!(
            observation.interface_name.as_deref(),
            Some("Wintun Userspace Tunnel")
        );
    }

    #[test]
    fn reports_detected_only_when_independent_signals_agree() {
        let observation = classify(Ok(vec![adapter(
            "Clash TUN",
            "Wintun virtual adapter",
            TUNNEL_INTERFACE_TYPE,
            true,
        )]));

        assert_eq!(observation.state, TunObservationState::Detected);
        assert!(observation.evidence.len() >= 3);
    }

    #[test]
    fn keeps_ambiguous_virtual_interfaces_as_possible() {
        let observation = classify(Ok(vec![
            adapter("WireGuard", "Virtual interface", 6, false),
            adapter(
                "Unknown tunnel",
                "Generic adapter",
                TUNNEL_INTERFACE_TYPE,
                true,
            ),
        ]));

        assert_eq!(observation.state, TunObservationState::Possible);
    }

    #[test]
    fn reports_unknown_when_enumeration_fails() {
        let observation = classify(Err("access denied".into()));

        assert_eq!(observation.state, TunObservationState::Unknown);
        assert_eq!(
            observation.evidence[0].kind,
            TunEvidenceKind::EnumerationUnavailable
        );
    }
}
