pub(crate) mod connectivity;
mod detector;
mod listeners;
mod models;
pub(crate) mod plan;
mod probe;
mod processes;
mod service;
mod system_proxy;

pub use models::{
    ProxyCandidate, ProxyEndpoint, ProxyEndpointInspection, ProxyEnvironmentState,
    ProxyEnvironmentStatus, ProxyProtocol, ProxyVariable,
};
pub use service::ProxyEnvironmentService;

use crate::error::Result;

pub fn detect() -> Result<Vec<ProxyCandidate>> {
    detector::detect()
}

pub fn inspect_endpoint(endpoint: &ProxyEndpoint) -> ProxyEndpointInspection {
    let listening = probe::listening(endpoint.host.trim(), endpoint.port);
    let detected_protocol = if listening {
        probe::protocol(endpoint.host.trim(), endpoint.port)
    } else {
        ProxyProtocol::Unknown
    };
    let protocol_matches = protocol_matches(endpoint.protocol, detected_protocol);
    ProxyEndpointInspection {
        listening,
        detected_protocol,
        protocol_matches,
    }
}

fn protocol_matches(selected: ProxyProtocol, detected: ProxyProtocol) -> bool {
    matches!(
        (selected, detected),
        (
            ProxyProtocol::Http,
            ProxyProtocol::Http | ProxyProtocol::Mixed
        ) | (
            ProxyProtocol::Socks5,
            ProxyProtocol::Socks5 | ProxyProtocol::Mixed
        ) | (ProxyProtocol::Mixed, ProxyProtocol::Mixed)
    )
}

#[cfg(test)]
mod tests {
    use super::{protocol_matches, ProxyProtocol};

    #[test]
    fn compares_selected_and_detected_proxy_protocols() {
        assert!(protocol_matches(ProxyProtocol::Http, ProxyProtocol::Http));
        assert!(protocol_matches(ProxyProtocol::Http, ProxyProtocol::Mixed));
        assert!(protocol_matches(
            ProxyProtocol::Socks5,
            ProxyProtocol::Mixed
        ));
        assert!(protocol_matches(ProxyProtocol::Mixed, ProxyProtocol::Mixed));
        assert!(!protocol_matches(ProxyProtocol::Mixed, ProxyProtocol::Http));
        assert!(!protocol_matches(
            ProxyProtocol::Http,
            ProxyProtocol::Socks5
        ));
        assert!(!protocol_matches(
            ProxyProtocol::Http,
            ProxyProtocol::Unknown
        ));
    }
}
