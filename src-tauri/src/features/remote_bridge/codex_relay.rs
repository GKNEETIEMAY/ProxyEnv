use super::BridgeResult;
use crate::features::proxy::ProxyEndpoint;
use reqwest::{
    blocking::Client,
    header::{HeaderName, HeaderValue},
    Url,
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHUNK_FRAMING_BYTES: usize = 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(30);

pub struct CodexRelay {
    port: u16,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl CodexRelay {
    pub fn start(upstream: ProxyEndpoint) -> BridgeResult<Self> {
        Self::start_inner(upstream)
    }

    fn start_inner(upstream: ProxyEndpoint) -> BridgeResult<Self> {
        let upstream_ip = upstream
            .host
            .parse::<IpAddr>()
            .map_err(|_| "ccUnavailable")?;
        if !upstream_ip.is_loopback() {
            return Err("ccUnavailable".into());
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
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("proxyenv-codex-relay".into())
            .spawn(move || {
                while !worker_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let endpoint = upstream.clone();
                            let _ = thread::Builder::new()
                                .name("proxyenv-codex-request".into())
                                .spawn(move || handle(stream, endpoint));
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
            stop,
            worker: Some(worker),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn is_running(&self) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
    }
}

impl Drop for CodexRelay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn handle(mut downstream: TcpStream, upstream: ProxyEndpoint) {
    let _ = downstream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = downstream.set_write_timeout(Some(IO_TIMEOUT));
    let result = downstream
        .set_nonblocking(false)
        .map_err(|_| "relayUnavailable")
        .and_then(|_| read_request(&mut downstream))
        .and_then(|request| forward(&mut downstream, upstream, request));
    if let Err(code) = result {
        let (status, message) = match code {
            "relayUnavailable" => (502, "the local CC Switch route is unavailable"),
            _ => (400, "the request could not be relayed safely"),
        };
        let body =
            json!({"error":{"type":"proxyenv_error","code":code,"message":message}}).to_string();
        let response = format!(
            "HTTP/1.1 {status} ProxyEnv Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = downstream.write_all(response.as_bytes());
        let _ = downstream.flush();
    }
    let _ = downstream.shutdown(Shutdown::Write);
}

fn read_request(stream: &mut TcpStream) -> Result<Request, &'static str> {
    let mut received = Vec::new();
    let header_end = loop {
        if received.len() >= MAX_HEADER_BYTES {
            return Err("invalidRequest");
        }
        let mut chunk = [0u8; 4096];
        let count = stream.read(&mut chunk).map_err(|_| "invalidRequest")?;
        if count == 0 {
            return Err("invalidRequest");
        }
        received.extend_from_slice(&chunk[..count]);
        if let Some(index) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let head = std::str::from_utf8(&received[..header_end]).map_err(|_| "invalidRequest")?;
    let mut lines = head[..head.len() - 4].split("\r\n");
    let mut request_line = lines.next().ok_or("invalidRequest")?.split_whitespace();
    let method = request_line.next().ok_or("invalidRequest")?;
    let target = request_line.next().ok_or("invalidRequest")?;
    let version = request_line.next().ok_or("invalidRequest")?;
    if request_line.next().is_some()
        || method != "POST"
        || !matches!(version, "HTTP/1.0" | "HTTP/1.1")
    {
        return Err("invalidRequest");
    }
    let path = normalize_request_target(target)?;
    let mut headers = Vec::new();
    let mut content_length = None;
    let mut chunked = false;
    let mut expects_continue = false;
    for line in lines {
        if line.starts_with([' ', '\t']) {
            return Err("invalidRequest");
        }
        let (name, value) = line.split_once(':').ok_or("invalidRequest")?;
        let name = name.trim();
        let value = value.trim();
        if name.is_empty() || value.contains(['\r', '\n']) {
            return Err("invalidRequest");
        }
        if name.eq_ignore_ascii_case("content-length") {
            let parsed = value.parse::<usize>().map_err(|_| "invalidRequest")?;
            if content_length.replace(parsed).is_some() {
                return Err("invalidRequest");
            }
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            if value.eq_ignore_ascii_case("chunked") && !chunked {
                chunked = true;
            } else if !value.eq_ignore_ascii_case("identity") {
                return Err("invalidRequest");
            }
        }
        if name.eq_ignore_ascii_case("expect") {
            if value.eq_ignore_ascii_case("100-continue") {
                expects_continue = true;
            } else {
                return Err("invalidRequest");
            }
        }
        headers.push((name.to_owned(), value.to_owned()));
    }
    if chunked && content_length.is_some() {
        return Err("invalidRequest");
    }
    if expects_continue {
        stream
            .write_all(b"HTTP/1.1 100 Continue\r\n\r\n")
            .and_then(|_| stream.flush())
            .map_err(|_| "invalidRequest")?;
    }
    let body = if chunked {
        decode_chunked(stream, received[header_end..].to_vec())?
    } else {
        let content_length = content_length.ok_or("invalidRequest")?;
        if content_length > MAX_REQUEST_BYTES {
            return Err("invalidRequest");
        }
        let mut body = received[header_end..].to_vec();
        if body.len() > content_length {
            body.truncate(content_length);
        }
        while body.len() < content_length {
            let remaining = content_length - body.len();
            let mut chunk = [0u8; 8192];
            let capacity = remaining.min(chunk.len());
            let count = stream
                .read(&mut chunk[..capacity])
                .map_err(|_| "invalidRequest")?;
            if count == 0 {
                return Err("invalidRequest");
            }
            body.extend_from_slice(&chunk[..count]);
        }
        body
    };
    Ok(Request {
        method: method.to_owned(),
        path,
        headers,
        body,
    })
}

fn normalize_request_target(target: &str) -> Result<String, &'static str> {
    if target.starts_with('/') && !target.starts_with("//") {
        return Ok(target.to_owned());
    }
    let url = Url::parse(target).map_err(|_| "invalidRequest")?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("invalidRequest");
    }
    let loopback = match url.host_str() {
        Some(host) if host.eq_ignore_ascii_case("localhost") => true,
        Some(host) => host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()),
        None => false,
    };
    if !loopback {
        return Err("invalidRequest");
    }
    let mut path = url.path().to_owned();
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    if path.starts_with("//") {
        return Err("invalidRequest");
    }
    Ok(path)
}

fn decode_chunked(stream: &mut TcpStream, mut encoded: Vec<u8>) -> Result<Vec<u8>, &'static str> {
    let mut cursor = 0usize;
    let mut body = Vec::new();
    loop {
        let line_end = loop {
            if let Some(offset) = encoded[cursor..]
                .windows(2)
                .position(|window| window == b"\r\n")
            {
                break cursor + offset;
            }
            read_chunk_bytes(stream, &mut encoded, body.len(), cursor)?;
        };
        let size_text = std::str::from_utf8(&encoded[cursor..line_end])
            .map_err(|_| "invalidRequest")?
            .split(';')
            .next()
            .ok_or("invalidRequest")?
            .trim();
        if size_text.is_empty() || size_text.len() > 16 {
            return Err("invalidRequest");
        }
        let size = usize::from_str_radix(size_text, 16).map_err(|_| "invalidRequest")?;
        cursor = line_end + 2;
        if size == 0 {
            loop {
                let trailer_end = loop {
                    if let Some(offset) = encoded[cursor..]
                        .windows(2)
                        .position(|window| window == b"\r\n")
                    {
                        break cursor + offset;
                    }
                    read_chunk_bytes(stream, &mut encoded, body.len(), cursor)?;
                };
                if trailer_end == cursor {
                    return Ok(body);
                }
                let trailer = &encoded[cursor..trailer_end];
                if trailer.starts_with(b" ")
                    || trailer.starts_with(b"\t")
                    || !trailer.contains(&b':')
                {
                    return Err("invalidRequest");
                }
                cursor = trailer_end + 2;
            }
        }
        if body.len().saturating_add(size) > MAX_REQUEST_BYTES {
            return Err("invalidRequest");
        }
        let needed = cursor.checked_add(size + 2).ok_or("invalidRequest")?;
        while encoded.len() < needed {
            read_chunk_bytes(stream, &mut encoded, body.len(), cursor)?;
        }
        if &encoded[cursor + size..needed] != b"\r\n" {
            return Err("invalidRequest");
        }
        body.extend_from_slice(&encoded[cursor..cursor + size]);
        cursor = needed;
    }
}

fn read_chunk_bytes(
    stream: &mut TcpStream,
    encoded: &mut Vec<u8>,
    decoded_len: usize,
    cursor: usize,
) -> Result<(), &'static str> {
    if cursor.saturating_sub(decoded_len) > MAX_CHUNK_FRAMING_BYTES {
        return Err("invalidRequest");
    }
    let mut chunk = [0u8; 8192];
    let count = stream.read(&mut chunk).map_err(|_| "invalidRequest")?;
    if count == 0 {
        return Err("invalidRequest");
    }
    if encoded.len().saturating_add(count)
        > MAX_REQUEST_BYTES.saturating_add(MAX_CHUNK_FRAMING_BYTES)
    {
        return Err("invalidRequest");
    }
    encoded.extend_from_slice(&chunk[..count]);
    Ok(())
}

fn hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "host"
            | "content-length"
            | "expect"
    )
}

fn forward(
    downstream: &mut TcpStream,
    upstream: ProxyEndpoint,
    request: Request,
) -> Result<(), &'static str> {
    let body = request.body;
    let url = format!("http://{}:{}{}", upstream.host, upstream.port, request.path);
    let client = Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .timeout(None)
        .build()
        .map_err(|_| "relayUnavailable")?;
    let method = request.method.parse().map_err(|_| "invalidRequest")?;
    let mut outgoing = client.request(method, url);
    for (name, value) in request.headers {
        if hop_by_hop(&name) {
            continue;
        }
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| "invalidRequest")?;
        let value = HeaderValue::from_str(&value).map_err(|_| "invalidRequest")?;
        outgoing = outgoing.header(name, value);
    }
    let mut response = outgoing.body(body).send().map_err(|_| "relayUnavailable")?;
    let status = response.status();
    let reason = status.canonical_reason().unwrap_or("Upstream Response");
    write!(downstream, "HTTP/1.1 {} {}\r\n", status.as_u16(), reason)
        .map_err(|_| "relayUnavailable")?;
    for (name, value) in response.headers() {
        if hop_by_hop(name.as_str()) {
            continue;
        }
        let Ok(value) = value.to_str() else {
            continue;
        };
        write!(downstream, "{}: {}\r\n", name.as_str(), value).map_err(|_| "relayUnavailable")?;
    }
    downstream
        .write_all(b"Connection: close\r\n\r\n")
        .map_err(|_| "relayUnavailable")?;
    let mut chunk = [0u8; 8192];
    loop {
        let count = response.read(&mut chunk).map_err(|_| "relayUnavailable")?;
        if count == 0 {
            break;
        }
        downstream
            .write_all(&chunk[..count])
            .and_then(|_| downstream.flush())
            .map_err(|_| "relayUnavailable")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Instant};

    fn upstream(
        response_parts: Vec<(&'static [u8], Duration)>,
    ) -> (ProxyEndpoint, mpsc::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream).unwrap();
            let _ = sender.send(request.body);
            for (bytes, delay) in response_parts {
                stream.write_all(bytes).unwrap();
                stream.flush().unwrap();
                thread::sleep(delay);
            }
        });
        (
            ProxyEndpoint {
                host: "127.0.0.1".into(),
                port,
                protocol: crate::features::proxy::ProxyProtocol::Http,
            },
            receiver,
        )
    }

    fn send(port: u16, path: &str, body: &str) -> TcpStream {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        write!(
            stream,
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        stream.flush().unwrap();
        stream
    }

    fn send_chunked(port: u16, path: &str, chunks: &[&str]) -> TcpStream {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        write!(
            stream,
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n"
        )
        .unwrap();
        for chunk in chunks {
            write!(stream, "{:x}\r\n{chunk}\r\n", chunk.len()).unwrap();
        }
        stream.write_all(b"0\r\n\r\n").unwrap();
        stream.flush().unwrap();
        stream
    }

    #[test]
    fn responses_request_body_is_forwarded_without_semantic_changes() {
        let (upstream, received) = upstream(vec![(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
            Duration::ZERO,
        )]);
        let relay = CodexRelay::start(upstream).unwrap();
        let body = r#"{"model":"stale-model","input":[1],"tools":[{"type":"x"}],"reasoning":{"effort":"high"},"stream":true}"#;
        let mut stream = send(relay.port(), "/v1/responses", body);
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(
            response.starts_with("HTTP/1.1 200"),
            "unexpected relay response: {response:?}"
        );
        let forwarded: serde_json::Value =
            serde_json::from_slice(&received.recv().unwrap()).unwrap();
        assert_eq!(forwarded["model"], "stale-model");
        assert_eq!(forwarded["input"], json!([1]));
        assert_eq!(forwarded["tools"], json!([{"type":"x"}]));
        assert_eq!(forwarded["reasoning"], json!({"effort":"high"}));
        assert_eq!(forwarded["stream"], true);
    }

    #[test]
    fn localhost_absolute_form_is_normalized_before_forwarding() {
        let (upstream, received) = upstream(vec![(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
            Duration::ZERO,
        )]);
        let relay = CodexRelay::start(upstream).unwrap();
        let body = r#"{"model":"remote-model","input":"hello"}"#;
        let mut stream = send(
            relay.port(),
            "http://127.0.0.1:25721/v1/responses?source=proxy",
            body,
        );
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        let forwarded: serde_json::Value =
            serde_json::from_slice(&received.recv().unwrap()).unwrap();
        assert_eq!(forwarded["model"], "remote-model");
        assert_eq!(forwarded["input"], "hello");
    }

    #[test]
    fn non_loopback_absolute_form_is_rejected() {
        assert_eq!(
            normalize_request_target("http://example.com/v1/responses"),
            Err("invalidRequest")
        );
    }

    #[test]
    fn chunked_responses_request_is_decoded_without_semantic_changes() {
        let (upstream, received) = upstream(vec![(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
            Duration::ZERO,
        )]);
        let relay = CodexRelay::start(upstream).unwrap();
        let mut stream = send_chunked(
            relay.port(),
            "/v1/responses",
            &[r#"{"model":"old","in"#, r#"put":"hello","stream":true}"#],
        );
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        let forwarded: serde_json::Value =
            serde_json::from_slice(&received.recv().unwrap()).unwrap();
        assert_eq!(forwarded["model"], "old");
        assert_eq!(forwarded["input"], "hello");
        assert_eq!(forwarded["stream"], true);
    }

    #[test]
    fn expect_continue_is_completed_before_reading_the_body() {
        let (upstream, received) = upstream(vec![(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
            Duration::ZERO,
        )]);
        let relay = CodexRelay::start(upstream).unwrap();
        let body = r#"{"model":"old"}"#;
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, relay.port())).unwrap();
        write!(
            stream,
            "POST /responses HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nExpect: 100-continue\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.flush().unwrap();
        let mut interim = [0u8; 25];
        stream.read_exact(&mut interim).unwrap();
        assert_eq!(&interim, b"HTTP/1.1 100 Continue\r\n\r\n");
        stream.write_all(body.as_bytes()).unwrap();
        stream.flush().unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(
            response.starts_with("HTTP/1.1 200"),
            "unexpected relay response: {response:?}"
        );
        let forwarded: serde_json::Value =
            serde_json::from_slice(&received.recv().unwrap()).unwrap();
        assert_eq!(forwarded["model"], "old");
    }

    #[test]
    fn claude_path_is_passed_through_without_model_resolution() {
        let (upstream, received) = upstream(vec![(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
            Duration::ZERO,
        )]);
        let relay = CodexRelay::start(upstream).unwrap();
        let body = r#"{"model":"claude-sonnet","messages":[]}"#;
        let mut stream = send(relay.port(), "/v1/messages", body);
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert_eq!(received.recv().unwrap(), body.as_bytes());
    }

    #[test]
    fn upstream_error_status_and_body_are_passed_through() {
        let (upstream, _) = upstream(vec![(
            b"HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nX-Request-Id: fixed-test\r\nContent-Length: 26\r\n\r\n{\"error\":\"quota exceeded\"}",
            Duration::ZERO,
        )]);
        let relay = CodexRelay::start(upstream).unwrap();
        let mut stream = send(relay.port(), "/responses", r#"{"model":"old"}"#);
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 429 Too Many Requests"));
        assert!(response.contains("x-request-id: fixed-test"));
        assert!(response.ends_with(r#"{"error":"quota exceeded"}"#));
    }

    #[test]
    fn model_resolver_is_not_consulted_at_runtime() {
        let (upstream, received) = upstream(vec![(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}",
            Duration::ZERO,
        )]);
        let relay = CodexRelay::start(upstream).unwrap();
        let mut stream = send(relay.port(), "/responses", r#"{"model":"unknown"}"#);
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert_eq!(received.recv().unwrap(), br#"{"model":"unknown"}"#);
    }

    #[test]
    fn response_body_is_streamed_without_waiting_for_completion() {
        let (upstream, _) = upstream(vec![
            (
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\ndata: first\n\n",
                Duration::from_millis(250),
            ),
            (b"data: second\n\n", Duration::ZERO),
        ]);
        let relay = CodexRelay::start(upstream).unwrap();
        let mut stream = send(relay.port(), "/responses", r#"{"model":"old"}"#);
        stream
            .set_read_timeout(Some(Duration::from_millis(40)))
            .unwrap();
        let started = Instant::now();
        let mut first = Vec::new();
        while started.elapsed() < Duration::from_millis(180)
            && !first
                .windows(b"data: first".len())
                .any(|bytes| bytes == b"data: first")
        {
            let mut bytes = [0u8; 512];
            match stream.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => first.extend_from_slice(&bytes[..count]),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(error) => panic!("stream read failed: {error}"),
            }
        }
        assert!(started.elapsed() < Duration::from_millis(220));
        let first = String::from_utf8_lossy(&first);
        assert!(
            first.contains("data: first"),
            "unexpected relay response: {first}"
        );
        assert!(!first.contains("data: second"));
    }
}
