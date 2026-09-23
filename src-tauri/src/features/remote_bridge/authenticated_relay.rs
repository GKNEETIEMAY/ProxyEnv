use super::BridgeResult;
use crate::features::proxy::{ProxyEndpoint, ProxyProtocol};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use zeroize::Zeroizing;

const MAX_HEADER_BYTES: usize = 64 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(30);
const USERNAME: &str = "proxyenv";
const SESSION_HEADER: &str = "x-proxyenv-session";

#[derive(Clone, Copy)]
pub(super) enum RelayMode {
    General(ProxyProtocol),
    AiHttp,
}

struct Session {
    secret: Zeroizing<[u8; 32]>,
    revoked: AtomicBool,
    next_connection: AtomicU64,
    connections: Mutex<HashMap<u64, (TcpStream, TcpStream)>>,
}

impl Session {
    fn new() -> BridgeResult<Self> {
        let mut secret = Zeroizing::new([0u8; 32]);
        getrandom::fill(secret.as_mut()).map_err(|_| "stateUnavailable")?;
        Ok(Self {
            secret,
            revoked: AtomicBool::new(false),
            next_connection: AtomicU64::new(1),
            connections: Mutex::new(HashMap::new()),
        })
    }

    fn token(&self) -> Zeroizing<String> {
        Zeroizing::new(hex::encode(self.secret.as_ref()))
    }

    fn accepts(&self, candidate: &[u8]) -> bool {
        if self.revoked.load(Ordering::Acquire) || candidate.len() != 64 {
            return false;
        }
        let expected = self.token();
        expected
            .as_bytes()
            .iter()
            .zip(candidate)
            .fold(0u8, |difference, (left, right)| difference | (left ^ right))
            == 0
    }

    fn track(&self, downstream: &TcpStream, upstream: &TcpStream) -> Result<u64, &'static str> {
        let id = self.next_connection.fetch_add(1, Ordering::Relaxed);
        let pair = (
            downstream.try_clone().map_err(|_| "relayUnavailable")?,
            upstream.try_clone().map_err(|_| "relayUnavailable")?,
        );
        self.connections
            .lock()
            .map_err(|_| "relayUnavailable")?
            .insert(id, pair);
        Ok(id)
    }

    fn untrack(&self, id: u64) {
        if let Ok(mut connections) = self.connections.lock() {
            connections.remove(&id);
        }
    }

    fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
        if let Ok(mut connections) = self.connections.lock() {
            for (_, (downstream, upstream)) in connections.drain() {
                let _ = downstream.shutdown(Shutdown::Both);
                let _ = upstream.shutdown(Shutdown::Both);
            }
        }
    }
}

pub(crate) struct AuthenticatedRelay {
    port: u16,
    session_id: String,
    session: Arc<Session>,
    worker: Option<JoinHandle<()>>,
}

impl AuthenticatedRelay {
    pub(super) fn start(upstream: ProxyEndpoint, mode: RelayMode) -> BridgeResult<Self> {
        let upstream_ip = upstream
            .host
            .parse::<IpAddr>()
            .map_err(|_| "relayUnavailable")?;
        if !upstream_ip.is_loopback() || upstream.protocol == ProxyProtocol::Unknown {
            return Err("relayUnavailable".into());
        }
        let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .map_err(|_| "relayUnavailable")?;
        let port = listener
            .local_addr()
            .map_err(|_| "relayUnavailable")?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|_| "relayUnavailable")?;
        let session = Arc::new(Session::new()?);
        let mut session_id = [0u8; 16];
        getrandom::fill(&mut session_id).map_err(|_| "stateUnavailable")?;
        let worker_session = Arc::clone(&session);
        let worker = thread::Builder::new()
            .name("proxyenv-authenticated-relay".into())
            .spawn(move || {
                while !worker_session.revoked.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((downstream, _)) => {
                            let endpoint = upstream.clone();
                            let connection_session = Arc::clone(&worker_session);
                            let _ = thread::Builder::new()
                                .name("proxyenv-authenticated-connection".into())
                                .spawn(move || {
                                    let _ = handle(downstream, endpoint, mode, connection_session);
                                });
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|_| "relayUnavailable")?;
        Ok(Self {
            port,
            session_id: hex::encode(session_id),
            session,
            worker: Some(worker),
        })
    }

    pub(super) fn port(&self) -> u16 {
        self.port
    }

    pub(super) fn token(&self) -> Zeroizing<String> {
        self.session.token()
    }

    pub(super) fn session_id(&self) -> &str {
        &self.session_id
    }

    pub(super) fn is_running(&self) -> bool {
        !self.session.revoked.load(Ordering::Acquire)
            && self
                .worker
                .as_ref()
                .is_some_and(|worker| !worker.is_finished())
    }
}

impl Drop for AuthenticatedRelay {
    fn drop(&mut self) {
        self.session.revoke();
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn handle(
    downstream: TcpStream,
    upstream: ProxyEndpoint,
    mode: RelayMode,
    session: Arc<Session>,
) -> Result<(), &'static str> {
    downstream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|_| "relayUnavailable")?;
    downstream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|_| "relayUnavailable")?;
    match mode {
        RelayMode::AiHttp => handle_http(downstream, upstream, session, SESSION_HEADER),
        RelayMode::General(protocol) => {
            let mut first = [0u8; 1];
            let count = downstream.peek(&mut first).map_err(|_| "invalidRequest")?;
            if count == 1 && first[0] == 5 {
                if matches!(protocol, ProxyProtocol::Socks5 | ProxyProtocol::Mixed) {
                    handle_socks5(downstream, upstream, session)
                } else {
                    Err("invalidRequest")
                }
            } else if matches!(protocol, ProxyProtocol::Http | ProxyProtocol::Mixed) {
                handle_http(downstream, upstream, session, "proxy-authorization")
            } else {
                Err("invalidRequest")
            }
        }
    }
}

fn read_head(stream: &mut TcpStream) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    let mut received = Zeroizing::new(Vec::new());
    loop {
        if received.len() >= MAX_HEADER_BYTES {
            return Err("invalidRequest");
        }
        let mut chunk = [0u8; 2048];
        let count = stream.read(&mut chunk).map_err(|_| "invalidRequest")?;
        if count == 0 {
            return Err("invalidRequest");
        }
        received.extend_from_slice(&chunk[..count]);
        if received.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(received);
        }
    }
}

fn handle_http(
    mut downstream: TcpStream,
    upstream: ProxyEndpoint,
    session: Arc<Session>,
    credential_header: &str,
) -> Result<(), &'static str> {
    let received = read_head(&mut downstream)?;
    let header_end = received
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or("invalidRequest")?
        + 4;
    let head = std::str::from_utf8(&received[..header_end]).map_err(|_| "invalidRequest")?;
    let mut output = String::new();
    let mut lines = head[..head.len() - 4].split("\r\n");
    let request_line = lines.next().ok_or("invalidRequest")?;
    if request_line.contains(['\r', '\n']) {
        return Err("invalidRequest");
    }
    output.push_str(request_line);
    output.push_str("\r\n");
    let mut authenticated = false;
    for line in lines {
        if line.starts_with([' ', '\t']) {
            return Err("invalidRequest");
        }
        let (name, value) = line.split_once(':').ok_or("invalidRequest")?;
        if name.trim().eq_ignore_ascii_case(credential_header) {
            if authenticated {
                return Err("invalidRequest");
            }
            authenticated = if credential_header == "proxy-authorization" {
                valid_basic(value.trim(), &session)
            } else {
                session.accepts(value.trim().as_bytes())
            };
            continue;
        }
        output.push_str(line);
        output.push_str("\r\n");
    }
    if !authenticated {
        if credential_header == "proxy-authorization" {
            let _ = downstream.write_all(
                b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"ProxyEnv\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        } else {
            let _ = downstream.write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
        return Err("relayUnauthorized");
    }
    if session.revoked.load(Ordering::Acquire) {
        return Err("relayUnauthorized");
    }
    output.push_str("\r\n");
    let mut upstream_stream =
        TcpStream::connect((&*upstream.host, upstream.port)).map_err(|_| "relayUnavailable")?;
    upstream_stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|_| "relayUnavailable")?;
    upstream_stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|_| "relayUnavailable")?;
    upstream_stream
        .write_all(output.as_bytes())
        .and_then(|_| upstream_stream.write_all(&received[header_end..]))
        .map_err(|_| "relayUnavailable")?;
    let id = session.track(&downstream, &upstream_stream)?;
    let result = tunnel(downstream, upstream_stream);
    session.untrack(id);
    result
}

fn valid_basic(value: &str, session: &Session) -> bool {
    let Some((scheme, encoded)) = value.split_once(' ') else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("basic") || encoded.contains(char::is_whitespace) {
        return false;
    }
    let Some(decoded) = decode_base64(encoded.as_bytes()) else {
        return false;
    };
    let decoded = Zeroizing::new(decoded);
    let prefix = format!("{USERNAME}:");
    decoded.starts_with(prefix.as_bytes()) && session.accepts(&decoded[prefix.len()..])
}

fn decode_base64(input: &[u8]) -> Option<Vec<u8>> {
    if input.is_empty() || !input.len().is_multiple_of(4) {
        return None;
    }
    let mut output = Vec::with_capacity(input.len() / 4 * 3);
    for chunk in input.chunks_exact(4) {
        let mut values = [0u8; 4];
        let mut padding = 0;
        for (index, byte) in chunk.iter().copied().enumerate() {
            values[index] = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' if index >= 2 => {
                    padding += 1;
                    0
                }
                _ => return None,
            };
        }
        if padding > 2 || (padding > 0 && chunk[3] != b'=') {
            return None;
        }
        output.push((values[0] << 2) | (values[1] >> 4));
        if padding < 2 {
            output.push((values[1] << 4) | (values[2] >> 2));
        }
        if padding == 0 {
            output.push((values[2] << 6) | values[3]);
        }
    }
    Some(output)
}

fn handle_socks5(
    mut downstream: TcpStream,
    upstream: ProxyEndpoint,
    session: Arc<Session>,
) -> Result<(), &'static str> {
    let methods = read_socks_frame(&mut downstream, 2, |head| usize::from(head[1]))?;
    if methods[0] != 5 || !methods[2..].contains(&2) {
        downstream
            .write_all(&[5, 0xff])
            .map_err(|_| "relayUnavailable")?;
        return Err("relayUnauthorized");
    }
    downstream
        .write_all(&[5, 2])
        .map_err(|_| "relayUnavailable")?;
    let username = read_socks_frame(&mut downstream, 2, |head| usize::from(head[1]))?;
    if username[0] != 1 {
        return Err("invalidRequest");
    }
    let mut password_head = [0u8; 1];
    downstream
        .read_exact(&mut password_head)
        .map_err(|_| "invalidRequest")?;
    let mut password = Zeroizing::new(vec![0u8; usize::from(password_head[0])]);
    downstream
        .read_exact(password.as_mut_slice())
        .map_err(|_| "invalidRequest")?;
    let valid = username[2..] == *USERNAME.as_bytes() && session.accepts(password.as_slice());
    downstream
        .write_all(&[1, if valid { 0 } else { 1 }])
        .map_err(|_| "relayUnavailable")?;
    if !valid || session.revoked.load(Ordering::Acquire) {
        return Err("relayUnauthorized");
    }
    let request = read_socks_request(&mut downstream)?;
    let mut upstream_stream =
        TcpStream::connect((&*upstream.host, upstream.port)).map_err(|_| "relayUnavailable")?;
    upstream_stream
        .write_all(&[5, 1, 0])
        .map_err(|_| "relayUnavailable")?;
    let mut method = [0u8; 2];
    upstream_stream
        .read_exact(&mut method)
        .map_err(|_| "relayUnavailable")?;
    if method != [5, 0] {
        return Err("relayUnavailable");
    }
    upstream_stream
        .write_all(&request)
        .map_err(|_| "relayUnavailable")?;
    let response = read_socks_request(&mut upstream_stream)?;
    downstream
        .write_all(&response)
        .map_err(|_| "relayUnavailable")?;
    if response.get(1) != Some(&0) {
        return Err("relayUnavailable");
    }
    let id = session.track(&downstream, &upstream_stream)?;
    let result = tunnel(downstream, upstream_stream);
    session.untrack(id);
    result
}

fn read_socks_frame<F>(
    stream: &mut TcpStream,
    head_len: usize,
    payload_len: F,
) -> Result<Vec<u8>, &'static str>
where
    F: Fn(&[u8]) -> usize,
{
    let mut frame = vec![0u8; head_len];
    stream
        .read_exact(&mut frame)
        .map_err(|_| "invalidRequest")?;
    let length = payload_len(&frame);
    if length == 0 || length > 255 {
        return Err("invalidRequest");
    }
    frame.resize(head_len + length, 0);
    stream
        .read_exact(&mut frame[head_len..])
        .map_err(|_| "invalidRequest")?;
    Ok(frame)
}

fn read_socks_request(stream: &mut TcpStream) -> Result<Vec<u8>, &'static str> {
    let mut head = [0u8; 4];
    stream.read_exact(&mut head).map_err(|_| "invalidRequest")?;
    if head[0] != 5 {
        return Err("invalidRequest");
    }
    let address_len = match head[3] {
        1 => 4,
        4 => 16,
        3 => {
            let mut length = [0u8; 1];
            stream
                .read_exact(&mut length)
                .map_err(|_| "invalidRequest")?;
            let mut request = head.to_vec();
            request.push(length[0]);
            let mut tail = vec![0u8; usize::from(length[0]) + 2];
            stream.read_exact(&mut tail).map_err(|_| "invalidRequest")?;
            request.extend_from_slice(&tail);
            return Ok(request);
        }
        _ => return Err("invalidRequest"),
    };
    let mut request = head.to_vec();
    let mut tail = vec![0u8; address_len + 2];
    stream.read_exact(&mut tail).map_err(|_| "invalidRequest")?;
    request.extend_from_slice(&tail);
    Ok(request)
}

fn tunnel(mut left: TcpStream, mut right: TcpStream) -> Result<(), &'static str> {
    left.set_read_timeout(None)
        .map_err(|_| "relayUnavailable")?;
    right
        .set_read_timeout(None)
        .map_err(|_| "relayUnavailable")?;
    let mut left_reader = left.try_clone().map_err(|_| "relayUnavailable")?;
    let mut right_writer = right.try_clone().map_err(|_| "relayUnavailable")?;
    let upload = thread::spawn(move || std::io::copy(&mut left_reader, &mut right_writer));
    let download = std::io::copy(&mut right, &mut left).map_err(|_| "relayUnavailable");
    let _ = left.shutdown(Shutdown::Both);
    let _ = right.shutdown(Shutdown::Both);
    let upload = upload.join().map_err(|_| "relayUnavailable")?;
    download?;
    upload.map(|_| ()).map_err(|_| "relayUnavailable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn basic(token: &str) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let input = format!("{USERNAME}:{token}");
        let bytes = input.as_bytes();
        let mut output = String::new();
        for chunk in bytes.chunks(3) {
            let a = chunk[0];
            let b = *chunk.get(1).unwrap_or(&0);
            let c = *chunk.get(2).unwrap_or(&0);
            output.push(TABLE[(a >> 2) as usize] as char);
            output.push(TABLE[((a & 3) << 4 | b >> 4) as usize] as char);
            output.push(if chunk.len() > 1 {
                TABLE[((b & 15) << 2 | c >> 6) as usize] as char
            } else {
                '='
            });
            output.push(if chunk.len() > 2 {
                TABLE[(c & 63) as usize] as char
            } else {
                '='
            });
        }
        output
    }

    fn upstream() -> (ProxyEndpoint, mpsc::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let count = stream.read(&mut request).unwrap();
            sender.send(request[..count].to_vec()).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .unwrap();
        });
        (
            ProxyEndpoint {
                host: "127.0.0.1".into(),
                port,
                protocol: ProxyProtocol::Http,
            },
            receiver,
        )
    }

    #[test]
    fn ai_relay_rejects_missing_token_and_strips_valid_token() {
        let (endpoint, receiver) = upstream();
        let relay = AuthenticatedRelay::start(endpoint, RelayMode::AiHttp).unwrap();
        let mut denied = TcpStream::connect((Ipv4Addr::LOCALHOST, relay.port())).unwrap();
        denied
            .write_all(b"POST /v1/responses HTTP/1.1\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        denied.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 401"));

        let mut allowed = TcpStream::connect((Ipv4Addr::LOCALHOST, relay.port())).unwrap();
        let request = format!(
            "POST /v1/responses HTTP/1.1\r\nX-ProxyEnv-Session: {}\r\nContent-Length: 0\r\n\r\n",
            relay.token().as_str()
        );
        allowed.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        allowed.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        let upstream_request = String::from_utf8(receiver.recv().unwrap()).unwrap();
        assert!(!upstream_request
            .to_ascii_lowercase()
            .contains(SESSION_HEADER));
    }

    #[test]
    fn general_http_relay_requires_session_credentials() {
        let (endpoint, receiver) = upstream();
        let relay =
            AuthenticatedRelay::start(endpoint, RelayMode::General(ProxyProtocol::Http)).unwrap();
        let mut denied = TcpStream::connect((Ipv4Addr::LOCALHOST, relay.port())).unwrap();
        denied
            .write_all(b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        denied.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 407"));

        let mut allowed = TcpStream::connect((Ipv4Addr::LOCALHOST, relay.port())).unwrap();
        let request = format!(
            "CONNECT example.com:443 HTTP/1.1\r\nProxy-Authorization: Basic {}\r\n\r\n",
            basic(relay.token().as_str())
        );
        allowed.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        allowed.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        let upstream_request = String::from_utf8(receiver.recv().unwrap()).unwrap();
        assert!(!upstream_request
            .to_ascii_lowercase()
            .contains("proxy-authorization"));
    }

    #[test]
    fn ai_relay_preserves_streaming_http_bytes_after_authentication() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let expected = b"POST /v1/responses HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"ok\":true}".to_vec();
        let expected_upstream = expected.clone();
        let upstream = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut received = vec![0u8; expected_upstream.len()];
            stream.read_exact(&mut received).unwrap();
            assert_eq!(received, expected_upstream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\ndata:\r\n")
                .unwrap();
            stream.write_all(b"4\r\nnext\r\n0\r\n\r\n").unwrap();
        });
        let relay = AuthenticatedRelay::start(
            ProxyEndpoint {
                host: "127.0.0.1".into(),
                port,
                protocol: ProxyProtocol::Http,
            },
            RelayMode::AiHttp,
        )
        .unwrap();
        let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, relay.port())).unwrap();
        let head = format!(
            "POST /v1/responses HTTP/1.1\r\nX-ProxyEnv-Session: {}\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n",
            relay.token().as_str()
        );
        client.write_all(head.as_bytes()).unwrap();
        client.write_all(b"{\"ok\":true}").unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).unwrap();
        assert_eq!(
            response,
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\ndata:\r\n4\r\nnext\r\n0\r\n\r\n"
        );
        upstream.join().unwrap();
    }

    #[test]
    fn session_tokens_rotate_and_drop_revokes_the_listener() {
        let (first_endpoint, _first_receiver) = upstream();
        let first =
            AuthenticatedRelay::start(first_endpoint, RelayMode::General(ProxyProtocol::Http))
                .unwrap();
        let first_token = first.token();
        let first_session_id = first.session_id().to_owned();
        let first_port = first.port();
        drop(first);
        assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, first_port)).is_err());

        let (second_endpoint, _second_receiver) = upstream();
        let second =
            AuthenticatedRelay::start(second_endpoint, RelayMode::General(ProxyProtocol::Http))
                .unwrap();
        assert_ne!(first_token.as_str(), second.token().as_str());
        assert_ne!(first_session_id, second.session_id());
    }
}
