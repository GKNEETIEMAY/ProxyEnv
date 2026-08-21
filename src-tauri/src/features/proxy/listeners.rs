use std::net::IpAddr;

use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};

use crate::error::{ProxyEnvError, Result};

#[derive(Debug, Clone)]
pub struct TcpListener {
    pub host: String,
    pub port: u16,
    pub pids: Vec<u32>,
}

fn is_local(address: IpAddr) -> bool {
    address.is_loopback() || address.is_unspecified()
}

pub fn enumerate() -> Result<Vec<TcpListener>> {
    let address_family = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let sockets = get_sockets_info(address_family, ProtocolFlags::TCP)
        .map_err(|error| ProxyEnvError::Detection(error.to_string()))?;
    Ok(sockets
        .into_iter()
        .filter_map(|socket| match socket.protocol_socket_info {
            ProtocolSocketInfo::Tcp(tcp)
                if tcp.state == TcpState::Listen && is_local(tcp.local_addr) =>
            {
                Some(TcpListener {
                    host: match tcp.local_addr {
                        IpAddr::V4(address) if address.is_unspecified() => "127.0.0.1".into(),
                        IpAddr::V6(address) if address.is_unspecified() => "::1".into(),
                        address => address.to_string(),
                    },
                    port: tcp.local_port,
                    pids: socket.associated_pids,
                })
            }
            _ => None,
        })
        .collect())
}
