use std::collections::{HashMap, HashSet};

use crate::error::Result;

use super::{
    listeners::{self, TcpListener},
    models::{Confidence, DetectionSource, ProxyCandidate, ProxyProtocol},
    probe, processes, system_proxy,
};

struct ClientRule {
    name: &'static str,
    icon: &'static str,
    frontends: &'static [&'static str],
    exclusive: &'static [&'static str],
    cores: &'static [&'static str],
}

const CLIENTS: &[ClientRule] = &[
    ClientRule {
        name: "Clash Verge Rev",
        icon: "clash-verge-rev",
        frontends: &["clash-verge.exe"],
        exclusive: &[
            "clash-verge-service.exe",
            "verge-mihomo.exe",
            "verge-mihomo-alpha.exe",
        ],
        cores: &["mihomo.exe"],
    },
    ClientRule {
        name: "v2rayN",
        icon: "v2rayn",
        frontends: &["v2rayn.exe"],
        exclusive: &[],
        cores: &["xray.exe", "sing-box.exe", "mihomo.exe", "v2ray.exe"],
    },
    ClientRule {
        name: "FlClash",
        icon: "flclash",
        frontends: &["flclash.exe"],
        exclusive: &[],
        cores: &["mihomo.exe"],
    },
    ClientRule {
        name: "Hiddify",
        icon: "hiddify",
        frontends: &["hiddify.exe", "hiddifynext.exe"],
        exclusive: &[],
        cores: &["sing-box.exe"],
    },
    ClientRule {
        name: "Clash Nyanpasu",
        icon: "clash-nyanpasu",
        frontends: &["clash-nyanpasu.exe"],
        exclusive: &["clash-rs.exe"],
        cores: &["mihomo.exe"],
    },
];

fn identify(
    process_name: Option<&str>,
    running_names: &HashSet<&str>,
) -> Option<&'static ClientRule> {
    let process_name = process_name?;
    if let Some(rule) = CLIENTS.iter().find(|rule| {
        rule.frontends.contains(&process_name) || rule.exclusive.contains(&process_name)
    }) {
        return Some(rule);
    }
    let matches = CLIENTS
        .iter()
        .filter(|rule| {
            rule.cores.contains(&process_name)
                && rule
                    .frontends
                    .iter()
                    .any(|name| running_names.contains(name))
        })
        .collect::<Vec<_>>();
    if matches.len() == 1 {
        Some(matches[0])
    } else {
        None
    }
}

fn owner<'a>(
    listener: &TcpListener,
    processes: &'a HashMap<u32, processes::RunningProcess>,
) -> Option<&'a processes::RunningProcess> {
    listener.pids.iter().find_map(|pid| processes.get(pid))
}

fn is_frontend_only(process_name: &str) -> bool {
    CLIENTS
        .iter()
        .any(|rule| rule.frontends.contains(&process_name))
        && !CLIENTS.iter().any(|rule| {
            rule.exclusive.contains(&process_name) || rule.cores.contains(&process_name)
        })
}

pub fn detect() -> Result<Vec<ProxyCandidate>> {
    let processes = processes::enumerate();
    let running_names = processes
        .values()
        .map(|process| process.name.as_str())
        .collect::<HashSet<_>>();
    let listeners = listeners::enumerate()?;
    let system_endpoint = system_proxy::read();
    let mut candidates = Vec::new();

    for listener in &listeners {
        let is_system = system_endpoint
            .as_ref()
            .is_some_and(|endpoint| endpoint.port == listener.port);
        let process = owner(listener, &processes);
        let client = identify(process.map(|process| process.name.as_str()), &running_names);
        if !is_system && process.is_some_and(|process| is_frontend_only(process.name.as_str())) {
            continue;
        }
        if !is_system && client.is_none() {
            continue;
        }
        let protocol = probe::protocol(&listener.host, listener.port);
        if !is_system && matches!(protocol, ProxyProtocol::Unknown) {
            continue;
        }
        let mut source = Vec::new();
        if is_system {
            source.push(DetectionSource::WindowsSystemProxy);
        }
        if process.is_some() {
            source.push(DetectionSource::ProcessListener);
        }
        if !matches!(protocol, ProxyProtocol::Unknown) {
            source.push(DetectionSource::ProtocolProbe);
        }
        let confidence = match (is_system, client.is_some()) {
            (true, true) => Confidence::VeryHigh,
            (true, false) | (false, true) => Confidence::High,
            _ => Confidence::Medium,
        };
        candidates.push(ProxyCandidate {
            id: format!("{}:{}", listener.host, listener.port),
            client_name: Some(client.map_or("其他代理", |client| client.name).into()),
            icon_key: Some(client.map_or("generic-proxy", |client| client.icon).into()),
            process_name: process.map(|process| process.name.clone()),
            pid: process.map(|process| process.pid),
            host: listener.host.clone(),
            port: listener.port,
            protocol,
            source,
            confidence,
            listening: true,
        });
    }

    if let Some(endpoint) = system_endpoint {
        if !candidates
            .iter()
            .any(|candidate| candidate.port == endpoint.port)
        {
            let running_clients = CLIENTS
                .iter()
                .filter(|rule| {
                    rule.frontends
                        .iter()
                        .any(|name| running_names.contains(name))
                })
                .collect::<Vec<_>>();
            let client = if running_clients.len() == 1 {
                Some(running_clients[0])
            } else {
                None
            };
            let listening = probe::listening(&endpoint.host, endpoint.port);
            candidates.push(ProxyCandidate {
                id: format!("{}:{}", endpoint.host, endpoint.port),
                client_name: Some(client.map_or("其他代理", |client| client.name).into()),
                icon_key: Some(client.map_or("generic-proxy", |client| client.icon).into()),
                process_name: None,
                pid: None,
                host: endpoint.host,
                port: endpoint.port,
                protocol: ProxyProtocol::Unknown,
                source: vec![DetectionSource::WindowsSystemProxy],
                confidence: if client.is_some() {
                    Confidence::VeryHigh
                } else {
                    Confidence::High
                },
                listening,
            });
        }
    }

    candidates.sort_by_key(candidate_priority);
    Ok(candidates)
}

fn candidate_priority(candidate: &ProxyCandidate) -> (u8, u8, u8, u8) {
    let listening = u8::from(!candidate.listening);
    let system_proxy = u8::from(
        !candidate
            .source
            .iter()
            .any(|source| matches!(source, DetectionSource::WindowsSystemProxy)),
    );
    let protocol = match candidate.protocol {
        ProxyProtocol::Mixed => 0,
        ProxyProtocol::Http => 1,
        ProxyProtocol::Socks5 => 2,
        ProxyProtocol::Unknown => 3,
    };
    let confidence = match candidate.confidence {
        Confidence::VeryHigh => 0,
        Confidence::High => 1,
        Confidence::Medium => 2,
        Confidence::Low => 3,
    };
    (listening, system_proxy, confidence, protocol)
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    #[ignore = "requires a running local proxy client"]
    fn detects_running_local_proxy() {
        let candidates = detect().expect("proxy detection should complete");
        eprintln!("{candidates:#?}");
        assert!(
            candidates.iter().any(|candidate| candidate.listening),
            "expected at least one listening local proxy candidate"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(port: u16, protocol: ProxyProtocol, listening: bool) -> ProxyCandidate {
        ProxyCandidate {
            id: format!("127.0.0.1:{port}"),
            client_name: Some("test".into()),
            icon_key: Some("generic-proxy".into()),
            process_name: None,
            pid: None,
            host: "127.0.0.1".into(),
            port,
            protocol,
            source: vec![],
            confidence: Confidence::High,
            listening,
        }
    }

    #[test]
    fn active_http_proxy_beats_stale_system_endpoint() {
        let active = candidate(10809, ProxyProtocol::Http, true);
        let stale = candidate(7897, ProxyProtocol::Unknown, false);
        assert!(candidate_priority(&active) < candidate_priority(&stale));
    }

    #[test]
    fn http_proxy_beats_socks_when_both_are_active() {
        let http = candidate(10809, ProxyProtocol::Http, true);
        let socks = candidate(10808, ProxyProtocol::Socks5, true);
        assert!(candidate_priority(&http) < candidate_priority(&socks));
    }

    #[test]
    fn current_windows_proxy_beats_another_active_client() {
        let mut current = candidate(10809, ProxyProtocol::Http, true);
        current.source.push(DetectionSource::WindowsSystemProxy);
        current.confidence = Confidence::VeryHigh;
        let old_client = candidate(7897, ProxyProtocol::Mixed, true);
        assert!(candidate_priority(&current) < candidate_priority(&old_client));
    }
}
