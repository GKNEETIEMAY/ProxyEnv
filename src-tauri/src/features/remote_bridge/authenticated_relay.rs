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
pub(super) const SESSION_HEADER: &str = "x-proxyenv-session";

#[derive(Clone, Copy)]
pub(super) enum RelayMode {
    General(ProxyProtocol),
    AiHttp,
}

// Only the active AI relay can issue this route. It is not an arbitrary
// loopback proxy: the destination is fixed and AI authentication still runs.
#[derive(Clone)]
pub(super) struct AiRoute {
    remote_port: u16,
    upstream: ProxyEndpoint,
    session: Arc<Session>,
}

impl AiRoute {
    fn request_line(&self, line: &str) -> Option<String> {
        let mut parts = line.split(' ');
        let method = parts.next()?;
        let target = parts.next()?;
        let version = parts.next()?;
        if parts.next().is_some()
            || !matches!(method, "GET" | "HEAD" | "POST")
            || !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        {
            return None;
        }
        for host in ["127.0.0.1", "localhost", "[::1]"] {
            let prefix = format!("http://{host}:{}/", self.remote_port);
            if let Some(path) = target.strip_prefix(&prefix) {
                return Some(format!("{method} /{path} {version}"));
            }
        }
        None
    }
}

struct Session {
    secret: Zeroizing<[u8; 32]>,
    revoked: AtomicBool,
    next_connection: AtomicU64,
    connections: Mutex<HashMap<u64, (TcpStream, Option<TcpStream>)>>,
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

    fn track(
        &self,
        downstream: &TcpStream,
        upstream: Option<&TcpStream>,
    ) -> Result<u64, &'static str> {
        let id = self.next_connection.fetch_add(1, Ordering::Relaxed);
        let pair = (
            downstream.try_clone().map_err(|_| "relayUnavailable")?,
            upstream
                .map(|upstream| upstream.try_clone().map_err(|_| "relayUnavailable"))
                .transpose()?,
        );
        let mut connections = self.connections.lock().map_err(|_| "relayUnavailable")?;
        if self.revoked.load(Ordering::Acquire) {
            return Err("relayUnauthorized");
        }
        connections.insert(id, pair);
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
                if let Some(upstream) = upstream {
                    let _ = upstream.shutdown(Shutdown::Both);
                }
            }
        }
    }
}

pub(crate) struct AuthenticatedRelay {
    port: u16,
    upstream: ProxyEndpoint,
    session_id: String,
    session: Arc<Session>,
    worker: Option<JoinHandle<()>>,
}

impl AuthenticatedRelay {
    pub(super) fn start(upstream: ProxyEndpoint, mode: RelayMode) -> BridgeResult<Self> {
        Self::start_with_ai_route(upstream, mode, None)
    }

    pub(super) fn start_with_ai_route(
        upstream: ProxyEndpoint,
        mode: RelayMode,
        ai_route: Option<AiRoute>,
    ) -> BridgeResult<Self> {
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
        let route_upstream = upstream.clone();
        let worker = thread::Builder::new()
            .name("proxyenv-authenticated-relay".into())
            .spawn(move || {
                while !worker_session.revoked.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((downstream, _)) => {
                            let endpoint = upstream.clone();
                            let connection_session = Arc::clone(&worker_session);
                            let route = ai_route.clone();
                            let _ = thread::Builder::new()
                                .name("proxyenv-authenticated-connection".into())
                                .spawn(move || {
                                    let _ = handle(
                                        downstream,
                                        endpoint,
                                        mode,
                                        connection_session,
                                        route,
                                    );
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
            upstream: route_upstream,
            session_id: hex::encode(session_id),
            session,
            worker: Some(worker),
        })
    }

    pub(super) fn port(&self) -> u16 {
        self.port
    }

    pub(super) fn ai_route(&self, remote_port: u16) -> AiRoute {
        AiRoute {
            remote_port,
            upstream: self.upstream.clone(),
            session: Arc::clone(&self.session),
        }
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
    ai_route: Option<AiRoute>,
) -> Result<(), &'static str> {
    downstream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|_| "relayUnavailable")?;
    downstream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|_| "relayUnavailable")?;
    match mode {
        RelayMode::AiHttp => {
            let id = session.track(&downstream, None)?;
            let result = super::codex_relay::handle_authenticated(downstream, upstream, |token| {
                session.accepts(token)
            });
            session.untrack(id);
            result
        }
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
                handle_http(downstream, upstream, session, ai_route)
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
    ai_route: Option<AiRoute>,
) -> Result<(), &'static str> {
    let received = read_head(&mut downstream)?;
    let header_end = received
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or("invalidRequest")?
        + 4;
    let head = std::str::from_utf8(&received[..header_end]).map_err(|_| "invalidRequest")?;
    let mut output = Zeroizing::new(String::new());
    let mut lines = head[..head.len() - 4].split("\r\n");
    let request_line = lines.next().ok_or("invalidRequest")?;
    if request_line.contains(['\r', '\n']) {
        return Err("invalidRequest");
    }
    let ai_request_line = ai_route
        .as_ref()
        .and_then(|route| route.request_line(request_line));
    output.push_str(ai_request_line.as_deref().unwrap_or(request_line));
    output.push_str("\r\n");
    for line in lines {
        if line.starts_with([' ', '\t']) {
            return Err("invalidRequest");
        }
        let (name, _) = line.split_once(':').ok_or("invalidRequest")?;
        if name.trim().eq_ignore_ascii_case("proxy-authorization") {
            // General proxy access is intentionally credential-free. Never
            // forward stale or user-supplied proxy credentials upstream.
            continue;
        }
        // Never leak the AI session token to a general proxy/upstream site.
        if ai_request_line.is_none() && name.trim().eq_ignore_ascii_case(SESSION_HEADER) {
            continue;
        }
        output.push_str(line);
        output.push_str("\r\n");
    }
    if session.revoked.load(Ordering::Acquire) {
        return Err("relayUnauthorized");
    }
    output.push_str("\r\n");
    if ai_request_line.is_some() {
        let route = ai_route.as_ref().ok_or("relayUnavailable")?;
        if route.session.revoked.load(Ordering::Acquire) {
            downstream
                .write_all(
                    b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .map_err(|_| "relayUnavailable")?;
            return Err("relayUnavailable");
        }
        // Reuse the AI parser/authenticator, not a second connection to an
        // ephemeral listener whose port could be reused after revocation.
        let mut buffered = Zeroizing::new(output.as_bytes().to_vec());
        buffered.extend_from_slice(&received[header_end..]);
        let id = session.track(&downstream, None)?;
        let result = (|| {
            let ai_id = route.session.track(&downstream, None)?;
            let result = super::codex_relay::handle_authenticated_buffered(
                downstream,
                route.upstream.clone(),
                |token| route.session.accepts(token) && !session.revoked.load(Ordering::Acquire),
                buffered,
            );
            route.session.untrack(ai_id);
            result
        })();
        session.untrack(id);
        return result;
    }
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
    let id = session.track(&downstream, Some(&upstream_stream))?;
    let result = tunnel(downstream, upstream_stream);
    session.untrack(id);
    result
}

fn handle_socks5(
    mut downstream: TcpStream,
    upstream: ProxyEndpoint,
    session: Arc<Session>,
) -> Result<(), &'static str> {
    let methods = read_socks_frame(&mut downstream, 2, |head| usize::from(head[1]))?;
    if methods[0] != 5 || !methods[2..].contains(&0) {
        downstream
            .write_all(&[5, 0xff])
            .map_err(|_| "relayUnavailable")?;
        return Err("invalidRequest");
    }
    downstream
        .write_all(&[5, 0])
        .map_err(|_| "relayUnavailable")?;
    if session.revoked.load(Ordering::Acquire) {
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
    let id = session.track(&downstream, Some(&upstream_stream))?;
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
    let upload = thread::spawn(move || {
        let result = std::io::copy(&mut left_reader, &mut right_writer);
        // EOF closes only this direction; the peer can still send a response.
        let _ = right_writer.shutdown(Shutdown::Write);
        if result.is_err() {
            let _ = left_reader.shutdown(Shutdown::Both);
            let _ = right_writer.shutdown(Shutdown::Both);
        }
        result
    });
    let download = std::io::copy(&mut right, &mut left).map_err(|_| "relayUnavailable");
    let _ = left.shutdown(Shutdown::Write);
    if download.is_err() {
        let _ = left.shutdown(Shutdown::Both);
        let _ = right.shutdown(Shutdown::Both);
    }
    let upload = upload.join().map_err(|_| "relayUnavailable")?;
    let _ = left.shutdown(Shutdown::Both);
    let _ = right.shutdown(Shutdown::Both);
    download?;
    upload.map(|_| ()).map_err(|_| "relayUnavailable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn tunnel_pair() -> (TcpStream, TcpStream, JoinHandle<Result<(), &'static str>>) {
        let left_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let right_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let client = TcpStream::connect(left_listener.local_addr().unwrap()).unwrap();
        let (left, _) = left_listener.accept().unwrap();
        let right = TcpStream::connect(right_listener.local_addr().unwrap()).unwrap();
        let (server, _) = right_listener.accept().unwrap();
        for stream in [&client, &server] {
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(3)))
                .unwrap();
        }
        (client, server, thread::spawn(move || tunnel(left, right)))
    }

    #[test]
    fn proxy_tunnel_propagates_upload_eof_without_losing_the_response() {
        let (mut client, mut server, relay) = tunnel_pair();
        client.write_all(b"request").unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let mut request = Vec::new();
        server.read_to_end(&mut request).unwrap();
        assert_eq!(request, b"request");
        server.write_all(b"response").unwrap();
        server.shutdown(Shutdown::Write).unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).unwrap();
        assert_eq!(response, b"response");
        assert!(relay.join().unwrap().is_ok());
    }

    #[test]
    fn proxy_tunnel_preserves_upload_after_the_response_eof() {
        let (mut client, mut server, relay) = tunnel_pair();
        server.write_all(b"response").unwrap();
        server.shutdown(Shutdown::Write).unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).unwrap();
        assert_eq!(response, b"response");
        client.write_all(b"remaining upload").unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let mut request = Vec::new();
        server.read_to_end(&mut request).unwrap();
        assert_eq!(request, b"remaining upload");
        assert!(relay.join().unwrap().is_ok());
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

    fn exchange(port: u16, request: &str) -> String {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    fn routed_proxy(
        ai: &AuthenticatedRelay,
        remote_port: u16,
    ) -> (AuthenticatedRelay, TcpListener) {
        // Keeping this socket unaccepted lets tests prove the general upstream
        // is never used for AI requests (even after the AI route is revoked).
        let unused = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        unused.set_nonblocking(true).unwrap();
        let proxy = AuthenticatedRelay::start_with_ai_route(
            ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: unused.local_addr().unwrap().port(),
                protocol: ProxyProtocol::Http,
            },
            RelayMode::General(ProxyProtocol::Http),
            Some(ai.ai_route(remote_port)),
        )
        .unwrap();
        (proxy, unused)
    }

    #[test]
    fn ai_route_matches_only_explicit_active_loopback_authority() {
        let (endpoint, _receiver) = upstream();
        let ai = AuthenticatedRelay::start(endpoint, RelayMode::AiHttp).unwrap();
        let route = ai.ai_route(15721);
        for host in ["127.0.0.1", "localhost", "[::1]"] {
            assert_eq!(
                route.request_line(&format!("POST http://{host}:15721/v1/responses HTTP/1.1")),
                Some("POST /v1/responses HTTP/1.1".into())
            );
        }
        for line in [
            "POST http://127.0.0.1:15722/v1/responses HTTP/1.1",
            "POST http://127.0.0.1:157210/v1/responses HTTP/1.1",
            "POST http://127.0.0.1:15721@evil.example/v1/responses HTTP/1.1",
            "POST http://user@127.0.0.1:15721/v1/responses HTTP/1.1",
            "POST http://127.0.0.1.evil.example:15721/v1/responses HTTP/1.1",
            "POST https://127.0.0.1:15721/v1/responses HTTP/1.1",
            "POST /v1/responses HTTP/1.1",
            "CONNECT 127.0.0.1:15721 HTTP/1.1",
            "POST http://127.0.0.1:15721/v1/responses HTTP/1.1 extra",
        ] {
            assert!(route.request_line(line).is_none(), "{line}");
        }
    }

    #[test]
    fn misrouted_ai_requires_ai_token_not_proxy_credentials() {
        let (endpoint, receiver) = upstream();
        let ai = AuthenticatedRelay::start(endpoint, RelayMode::AiHttp).unwrap();
        let (proxy, unused) = routed_proxy(&ai, 15721);
        for headers in [
            String::new(),
            "Proxy-Authorization: Basic ignored\r\n".into(),
            format!("X-ProxyEnv-Session: {}\r\n", proxy.token().as_str()),
        ] {
            let response = exchange(proxy.port(), &format!(
                "POST http://127.0.0.1:15721/v1/responses HTTP/1.1\r\n{headers}Content-Length: 0\r\n\r\n"));
            assert!(response.starts_with("HTTP/1.1 401"), "{response}");
        }
        let response = exchange(proxy.port(), &format!(
            "POST http://127.0.0.1:15721/v1/responses HTTP/1.1\r\nX-ProxyEnv-Session: {}\r\nContent-Length: 0\r\n\r\n", ai.token().as_str()));
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        let request =
            String::from_utf8(receiver.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
        assert!(request.starts_with("POST /v1/responses HTTP/1.1"));
        assert!(!request.to_ascii_lowercase().contains(SESSION_HEADER));
        assert!(unused.accept().is_err());
    }

    #[test]
    fn revoked_ai_route_fails_closed_even_when_general_proxy_remains_available() {
        let (endpoint, _receiver) = upstream();
        let ai = AuthenticatedRelay::start(endpoint, RelayMode::AiHttp).unwrap();
        let (proxy, unused) = routed_proxy(&ai, 15721);
        let token = ai.token();
        drop(ai);
        let response = exchange(proxy.port(), &format!(
            "POST http://127.0.0.1:15721/v1/responses HTTP/1.1\r\nX-ProxyEnv-Session: {}\r\nContent-Length: 0\r\n\r\n", token.as_str()));
        assert!(response.starts_with("HTTP/1.1 502"), "{response}");
        assert!(unused.accept().is_err());
    }

    #[test]
    fn general_proxy_strips_ai_credentials_on_unrelated_requests() {
        let (endpoint, receiver) = upstream();
        let proxy =
            AuthenticatedRelay::start(endpoint, RelayMode::General(ProxyProtocol::Http)).unwrap();
        let response = exchange(
            proxy.port(),
            "GET http://example.com/ HTTP/1.1\r\nProxy-Authorization: Basic ignored\r\nX-ProxyEnv-Session: private-ai-token\r\n\r\n",
        );
        assert!(response.starts_with("HTTP/1.1 200"));
        let request =
            String::from_utf8(receiver.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
        assert!(!request.to_ascii_lowercase().contains(SESSION_HEADER));
        assert!(!request.contains("private-ai-token"));
        assert!(!request.to_ascii_lowercase().contains("proxy-authorization"));
    }

    #[test]
    fn codex_request_through_http_proxy_preserves_body_and_streams_before_completion() {
        use std::io::{BufRead, BufReader};
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let endpoint = ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: listener.local_addr().unwrap().port(),
            protocol: ProxyProtocol::Http,
        };
        let (continue_tx, continue_rx) = mpsc::channel();
        let body = r#"{"model":"unchanged","stream":true,"tools":[{"type":"function","name":"exec_command"}]}"#;
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            loop {
                let mut line = String::new();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                head.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            let mut actual = vec![0; body.len()];
            reader.read_exact(&mut actual).unwrap();
            assert_eq!(actual, body.as_bytes());
            assert!(!head.to_ascii_lowercase().contains(SESSION_HEADER));
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: first\n\n").unwrap();
            // The second event cannot be sent until the client has received the
            // first: this catches accidental buffering of the entire response.
            continue_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            stream.write_all(b"data: {\"type\":\"response.function_call_arguments.done\",\"arguments\":\"{}\"}\n\n").unwrap();
        });
        let ai = AuthenticatedRelay::start(endpoint, RelayMode::AiHttp).unwrap();
        let (proxy, unused) = routed_proxy(&ai, 15721);
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}", proxy.port())).unwrap())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let response = client
            .post("http://127.0.0.1:15721/v1/responses")
            .header(SESSION_HEADER, ai.token().as_str())
            .body(body)
            .send()
            .unwrap();
        assert_eq!(response.status(), 200);
        let mut reader = BufReader::new(response);
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        assert_eq!(first, "data: first\n");
        continue_tx.send(()).unwrap();
        let mut tail = String::new();
        reader.read_to_string(&mut tail).unwrap();
        assert!(tail.contains("response.function_call_arguments.done"));
        worker.join().unwrap();
        assert!(unused.accept().is_err());
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
    fn ai_relay_repeated_unauthorized_requests_receive_http_responses() {
        let unused_upstream = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let relay = AuthenticatedRelay::start(
            ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: unused_upstream.local_addr().unwrap().port(),
                protocol: ProxyProtocol::Http,
            },
            RelayMode::AiHttp,
        )
        .unwrap();
        for _ in 0..20 {
            let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, relay.port())).unwrap();
            client
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap();
            assert!(response.starts_with("HTTP/1.1 401"), "{response}");
        }
    }

    #[test]
    fn ai_relay_reports_upstream_failure_instead_of_closing_silently() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
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
        let request = format!(
            "POST /v1/messages HTTP/1.1\r\nX-ProxyEnv-Session: {}\r\nContent-Length: 2\r\n\r\n{{}}",
            relay.token().as_str()
        );
        client.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 502"), "{response}");
    }

    #[test]
    fn general_http_relay_accepts_requests_without_credentials() {
        let (endpoint, receiver) = upstream();
        let relay =
            AuthenticatedRelay::start(endpoint, RelayMode::General(ProxyProtocol::Http)).unwrap();
        let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, relay.port())).unwrap();
        client
            .write_all(b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        let upstream_request = String::from_utf8(receiver.recv().unwrap()).unwrap();
        assert!(upstream_request.starts_with("CONNECT example.com:443 HTTP/1.1"));
    }

    #[test]
    fn configured_proxy_url_needs_no_username_or_password() {
        let (endpoint, receiver) = upstream();
        let relay =
            AuthenticatedRelay::start(endpoint, RelayMode::General(ProxyProtocol::Http)).unwrap();
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}", relay.port())).unwrap())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let response = client.get("http://example.test/").send().unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let request =
            String::from_utf8(receiver.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
        assert!(!request.to_ascii_lowercase().contains("proxy-authorization"));
    }

    #[test]
    fn general_socks_relay_negotiates_no_authentication() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let relay = AuthenticatedRelay::start(
            ProxyEndpoint {
                host: "127.0.0.1".into(),
                port,
                protocol: ProxyProtocol::Socks5,
            },
            RelayMode::General(ProxyProtocol::Socks5),
        )
        .unwrap();
        let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, relay.port())).unwrap();
        client.write_all(&[5, 1, 0]).unwrap();
        let mut method = [0u8; 2];
        client.read_exact(&mut method).unwrap();
        assert_eq!(method, [5, 0]);
        drop(client);
        drop(listener);
    }

    #[test]
    fn ai_relay_forwards_http_response_after_authentication() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let upstream = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut received = Vec::new();
            let header_end = loop {
                let mut chunk = [0u8; 1024];
                let count = stream.read(&mut chunk).unwrap();
                assert!(count > 0);
                received.extend_from_slice(&chunk[..count]);
                if let Some(index) = received.windows(4).position(|part| part == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            while received.len() < header_end + 11 {
                let mut chunk = [0u8; 11];
                let count = stream.read(&mut chunk).unwrap();
                assert!(count > 0);
                received.extend_from_slice(&chunk[..count]);
            }
            let headers = String::from_utf8_lossy(&received[..header_end]).to_ascii_lowercase();
            assert!(headers.starts_with("post /v1/responses http/1.1"));
            assert!(!headers.contains(SESSION_HEADER));
            assert_eq!(&received[header_end..header_end + 11], b"{\"ok\":true}");
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
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with(b"\r\n\r\ndata:next"));
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
