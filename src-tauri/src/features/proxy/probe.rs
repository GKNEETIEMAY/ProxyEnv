use std::{
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream},
    time::Duration,
};

use super::models::ProxyProtocol;

const TIMEOUT: Duration = Duration::from_millis(450);

fn address(host: &str, port: u16) -> Option<SocketAddr> {
    Some(SocketAddr::new(host.parse::<IpAddr>().ok()?, port))
}

pub fn listening(host: &str, port: u16) -> bool {
    address(host, port)
        .and_then(|address| TcpStream::connect_timeout(&address, TIMEOUT).ok())
        .is_some()
}

fn socks5(host: &str, port: u16) -> bool {
    let Some(address) = address(host, port) else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&address, TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    if stream.write_all(&[0x05, 0x01, 0x00]).is_err() {
        return false;
    }
    let mut response = [0_u8; 2];
    stream.read_exact(&mut response).is_ok() && response == [0x05, 0x00]
}

fn http(host: &str, port: u16) -> bool {
    let Some(address) = address(host, port) else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&address, TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    let request = b"CONNECT 127.0.0.1:9 HTTP/1.1\r\nHost: 127.0.0.1:9\r\n\r\n";
    if stream.write_all(request).is_err() {
        return false;
    }
    let mut response = [0_u8; 12];
    let Ok(length) = stream.read(&mut response) else {
        return false;
    };
    response[..length].starts_with(b"HTTP/")
}

pub fn protocol(host: &str, port: u16) -> ProxyProtocol {
    match (http(host, port), socks5(host, port)) {
        (true, true) => ProxyProtocol::Mixed,
        (true, false) => ProxyProtocol::Http,
        (false, true) => ProxyProtocol::Socks5,
        (false, false) => ProxyProtocol::Unknown,
    }
}
