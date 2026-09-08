use super::{
    ssh, BridgeResult, PortAllocation, Request, SshAuthMethod, SshAuthMode, SshAuthState, Summary,
};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;
type SharedTerminalOutput = Arc<Mutex<TerminalOutput>>;

const AUTH_MARKER_PREFIX: &str = "__PROXYENV_SSH_AUTH_";
const MAX_TERMINAL_BYTES: usize = 65_536;
const MAX_TRANSCRIPT_CHARS: usize = 8_192;
const MAX_CONTROL_BYTES: usize = 64;
const SESSION_TTL: Duration = Duration::from_secs(180);
const PROMPT_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const COMPLETION_WAIT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Operation {
    Check,
    Connect,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    Starting,
    WaitingPrompt,
    WaitingUser,
    Submitting,
    WaitingServer,
    Authenticated,
    Succeeded,
    PromptUnavailable,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PromptType {
    Password,
    KeyPassphrase,
    HostKeyConfirmation,
    VerificationCode,
    KeyboardInteractive,
    Unknown,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthPrompt {
    pub id: String,
    #[serde(rename = "type")]
    pub prompt_type: PromptType,
    pub message: String,
    pub secret: bool,
    pub attempt: u16,
    pub target: Option<String>,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PtyDiagnostic {
    pub bytes_received: u64,
    pub printable_bytes: u64,
    pub cpr_requests: u32,
    pub prompt_detected: bool,
    pub auth_marker_detected: bool,
    pub remote_result_detected: bool,
    pub output_closed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub session_id: String,
    pub operation: Operation,
    pub status: SessionStatus,
    pub auth: SshAuthState,
    pub prompt: Option<AuthPrompt>,
    pub diagnostic: PtyDiagnostic,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub operation: Operation,
    pub ports: Option<PortAllocation>,
    pub summary: Option<Summary>,
}

struct TerminalOutput {
    bytes: Vec<u8>,
    authenticated: bool,
    marker: Vec<u8>,
    pending_echo: Option<Zeroizing<Vec<u8>>>,
    echo_match_len: usize,
    bytes_received: u64,
    printable_bytes: u64,
    cpr_requests: u32,
    remote_result_detected: bool,
    closed: bool,
}

pub struct PtyProcess {
    child: Box<dyn Child + Send>,
    _master: Box<dyn MasterPty + Send>,
    _writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl ssh::ManagedSsh for PtyProcess {
    fn is_running(&mut self) -> BridgeResult<bool> {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|_| "processFailed".into())
    }
}

enum Completion {
    Check {
        ports: PortAllocation,
    },
    Connect {
        request: Request,
        summary: Box<Summary>,
        fingerprint: String,
    },
}

struct Session {
    id: String,
    target_id: String,
    operation: Operation,
    auth: SshAuthState,
    status: SessionStatus,
    error: Option<String>,
    output: SharedTerminalOutput,
    writer: SharedWriter,
    process: Option<PtyProcess>,
    completion: Completion,
    created_at: Instant,
    attempt: u16,
    current_prompt: Option<AuthPrompt>,
    last_prompt: Option<(PromptType, String)>,
    prompt_generation: u32,
    prompt_consumed_at: usize,
    waiting_since: Instant,
    authenticated_at: Option<Instant>,
}

static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_sessions() -> BridgeResult<std::sync::MutexGuard<'static, HashMap<String, Session>>> {
    sessions().lock().map_err(|_| "stateUnavailable".into())
}

fn session_id() -> BridgeResult<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "stateUnavailable")?;
    Ok(hex::encode(bytes))
}

fn random_ports() -> BridgeResult<PortAllocation> {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).map_err(|_| "stateUnavailable")?;
    let proxy_port = 20_000 + (u16::from_le_bytes([bytes[0], bytes[1]]) % 40_001);
    let mut cc_port = 20_000 + (u16::from_le_bytes([bytes[2], bytes[3]]) % 40_001);
    if proxy_port == cc_port {
        cc_port = if cc_port == 60_000 {
            20_000
        } else {
            cc_port + 1
        };
    }
    Ok(PortAllocation {
        proxy_port,
        cc_port,
    })
}

fn command_builder(command: &std::process::Command) -> CommandBuilder {
    let mut builder = CommandBuilder::new(command.get_program());
    for argument in command.get_args() {
        builder.arg(argument);
    }
    builder
}

#[derive(Default)]
enum ControlState {
    #[default]
    Ground,
    Escape,
    Csi(Vec<u8>),
    Osc {
        escape: bool,
    },
}

#[derive(Default)]
struct TerminalControlParser {
    state: ControlState,
}

struct ParsedTerminalChunk {
    printable: Vec<u8>,
    cpr_requests: u32,
}

impl TerminalControlParser {
    fn push(&mut self, bytes: &[u8]) -> ParsedTerminalChunk {
        let mut printable = Vec::with_capacity(bytes.len());
        let mut cpr_requests = 0u32;
        for &byte in bytes {
            match &mut self.state {
                ControlState::Ground if byte == 0x1b => self.state = ControlState::Escape,
                ControlState::Ground => printable.push(byte),
                ControlState::Escape if byte == b'[' => self.state = ControlState::Csi(Vec::new()),
                ControlState::Escape if byte == b']' => {
                    self.state = ControlState::Osc { escape: false }
                }
                ControlState::Escape => self.state = ControlState::Ground,
                ControlState::Csi(sequence) => {
                    if sequence.len() >= MAX_CONTROL_BYTES {
                        self.state = ControlState::Ground;
                    } else if (0x40..=0x7e).contains(&byte) {
                        if byte == b'n' && sequence.as_slice() == b"6" {
                            cpr_requests = cpr_requests.saturating_add(1);
                        }
                        self.state = ControlState::Ground;
                    } else {
                        sequence.push(byte);
                    }
                }
                ControlState::Osc { .. } if byte == 0x07 => self.state = ControlState::Ground,
                ControlState::Osc { escape } if *escape && byte == b'\\' => {
                    self.state = ControlState::Ground
                }
                ControlState::Osc { escape } => *escape = byte == 0x1b,
            }
        }
        ParsedTerminalChunk {
            printable,
            cpr_requests,
        }
    }
}

fn suppress_confirmed_echo(output: &mut TerminalOutput, bytes: &[u8]) -> Vec<u8> {
    if output.pending_echo.is_none() {
        return bytes.to_vec();
    }
    let mut visible = Vec::with_capacity(bytes.len());
    for &byte in bytes {
        let Some((expected_byte, expected_len)) =
            output.pending_echo.as_ref().and_then(|expected| {
                expected
                    .get(output.echo_match_len)
                    .copied()
                    .map(|expected_byte| (expected_byte, expected.len()))
            })
        else {
            visible.push(byte);
            continue;
        };
        if byte == expected_byte {
            output.echo_match_len += 1;
            if output.echo_match_len == expected_len {
                output.pending_echo = None;
                output.echo_match_len = 0;
            }
        } else {
            output.pending_echo = None;
            output.echo_match_len = 0;
            visible.push(byte);
        }
    }
    visible
}

fn terminal_after_marker(bytes: &[u8], marker: &[u8]) -> Option<String> {
    let marker_end = bytes
        .windows(marker.len())
        .position(|window| window == marker)?
        .saturating_add(marker.len());
    Some(strip_terminal(&bytes[marker_end..], marker))
}

fn contains_remote_result(bytes: &[u8], marker: &[u8]) -> bool {
    terminal_after_marker(bytes, marker).is_some_and(|text| {
        text.lines().any(|line| {
            let line = line.trim();
            line.starts_with('{')
                && line.ends_with('}')
                && serde_json::from_str::<serde_json::Value>(line).is_ok()
        })
    })
}

struct SpawnedPty {
    process: PtyProcess,
    writer: SharedWriter,
    output: SharedTerminalOutput,
}

fn spawn(
    mut command: std::process::Command,
    destination: String,
    remote_command: String,
    marker: String,
) -> BridgeResult<SpawnedPty> {
    command.arg(destination).arg(remote_command);
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|_| "ptyUnavailable")?;
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|_| "ptyUnavailable")?;
    let writer = Arc::new(Mutex::new(
        pair.master.take_writer().map_err(|_| "ptyUnavailable")?,
    ));
    let child = pair
        .slave
        .spawn_command(command_builder(&command))
        .map_err(|_| "sshMissing")?;
    drop(pair.slave);
    let output = Arc::new(Mutex::new(TerminalOutput {
        bytes: Vec::new(),
        authenticated: false,
        marker: marker.into_bytes(),
        pending_echo: None,
        echo_match_len: 0,
        bytes_received: 0,
        printable_bytes: 0,
        cpr_requests: 0,
        remote_result_detected: false,
        closed: false,
    }));
    let reader_output = Arc::clone(&output);
    let reader_writer = Arc::clone(&writer);
    thread::spawn(move || {
        let mut chunk = [0u8; 2048];
        let mut control_parser = TerminalControlParser::default();
        while let Ok(count) = reader.read(&mut chunk) {
            if count == 0 {
                break;
            }
            let parsed = control_parser.push(&chunk[..count]);
            if parsed.cpr_requests > 0 {
                if let Ok(mut stream) = reader_writer.lock() {
                    for _ in 0..parsed.cpr_requests {
                        let _ = stream.write_all(b"\x1b[1;1R");
                    }
                    let _ = stream.flush();
                }
            }
            if let Ok(mut state) = reader_output.lock() {
                state.bytes_received = state.bytes_received.saturating_add(count as u64);
                state.printable_bytes = state
                    .printable_bytes
                    .saturating_add(parsed.printable.len() as u64);
                state.cpr_requests = state.cpr_requests.saturating_add(parsed.cpr_requests);
                let visible = suppress_confirmed_echo(&mut state, &parsed.printable);
                if state.bytes.len() < MAX_TERMINAL_BYTES {
                    let remaining = MAX_TERMINAL_BYTES - state.bytes.len();
                    state
                        .bytes
                        .extend_from_slice(&visible[..visible.len().min(remaining)]);
                }
                if !state.authenticated
                    && state.marker.len() <= state.bytes.len()
                    && state
                        .bytes
                        .windows(state.marker.len())
                        .any(|window| window == state.marker)
                {
                    state.authenticated = true;
                }
                state.remote_result_detected = contains_remote_result(&state.bytes, &state.marker);
            }
        }
        if let Ok(mut state) = reader_output.lock() {
            state.closed = true;
        }
    });
    Ok(SpawnedPty {
        process: PtyProcess {
            child,
            _master: pair.master,
            _writer: Arc::clone(&writer),
        },
        writer,
        output,
    })
}

fn strip_terminal(bytes: &[u8], marker: &[u8]) -> String {
    let mut clean = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            0x1b if bytes.get(index + 1) == Some(&b'[') => {
                index += 2;
                while index < bytes.len() {
                    let byte = bytes[index];
                    index += 1;
                    if (0x40..=0x7e).contains(&byte) {
                        break;
                    }
                }
            }
            0x1b if bytes.get(index + 1) == Some(&b']') => {
                index += 2;
                while index < bytes.len() {
                    if bytes[index] == 0x07 {
                        index += 1;
                        break;
                    }
                    if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
                        index += 2;
                        break;
                    }
                    index += 1;
                }
            }
            b'\r' => {
                if clean.last() != Some(&b'\n') {
                    clean.push(b'\n');
                }
                index += 1;
            }
            0x08 => {
                clean.pop();
                index += 1;
            }
            byte if byte == b'\n' || byte == b'\t' || byte >= 0x20 => {
                clean.push(byte);
                index += 1;
            }
            _ => index += 1,
        }
    }
    let text = String::from_utf8_lossy(&clean)
        .replace(std::str::from_utf8(marker).unwrap_or_default(), "");
    let chars: Vec<_> = text.chars().collect();
    let start = chars.len().saturating_sub(MAX_TRANSCRIPT_CHARS);
    chars[start..].iter().collect::<String>().trim().to_owned()
}

fn tail_lines(text: &str, count: usize) -> String {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let start = lines.len().saturating_sub(count);
    let joined = lines[start..].join("\n");
    let chars = joined.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(512);
    chars[start..].iter().collect::<String>()
}

fn current_line(text: &str) -> &str {
    text.lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or("")
}

fn fingerprint(text: &str) -> Option<String> {
    let start = text.find("SHA256:")?;
    let value = text[start..]
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(|character: char| matches!(character, '.' | ',' | ';' | ')' | ']'));
    (!value.is_empty() && value.len() <= 160).then(|| value.to_owned())
}

fn safe_passphrase_prompt(line: &str) -> String {
    let Some(start) = line.find(['\'', '"']) else {
        return "Enter passphrase for SSH key:".into();
    };
    let quote = line.as_bytes()[start] as char;
    let rest = &line[start + 1..];
    let Some(end) = rest.find(quote) else {
        return "Enter passphrase for SSH key:".into();
    };
    let name = std::path::Path::new(&rest[..end])
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("SSH key");
    format!("Enter passphrase for key '{name}':")
}

fn parse_ssh_prompt(text: &str, attempt: u16) -> Option<AuthPrompt> {
    if text.is_empty() {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    let line = current_line(text);
    let line_lower = line.to_ascii_lowercase();
    let make = |prompt_type, message: String, secret| AuthPrompt {
        id: String::new(),
        prompt_type,
        message,
        secret,
        attempt,
        target: None,
        fingerprint: None,
    };

    if lower.contains("are you sure you want to continue connecting")
        && (lower.contains("yes/no") || lower.contains("fingerprint"))
    {
        let mut prompt = make(PromptType::HostKeyConfirmation, tail_lines(text, 4), false);
        prompt.fingerprint = fingerprint(text);
        return Some(prompt);
    }
    if line_lower.contains("enter passphrase for key") {
        return Some(make(
            PromptType::KeyPassphrase,
            safe_passphrase_prompt(line),
            true,
        ));
    }
    if line_lower.contains("verification code")
        || line_lower == "otp:"
        || line_lower.contains("one-time password")
        || line_lower.contains("one time password")
    {
        return Some(make(PromptType::VerificationCode, line.to_owned(), true));
    }
    if line_lower.ends_with("password:") {
        return Some(make(PromptType::Password, line.to_owned(), true));
    }
    if ["duo", "token:", "challenge:", "response:"]
        .iter()
        .any(|pattern| line_lower.contains(pattern))
    {
        return Some(make(
            PromptType::KeyboardInteractive,
            line.to_owned(),
            !line_lower.contains("yes/no"),
        ));
    }
    if line.ends_with(':') || line.ends_with('?') {
        return Some(make(PromptType::Unknown, line.to_owned(), false));
    }
    None
}

fn classify_error(transcript: &str) -> String {
    let text = transcript.to_ascii_lowercase();
    if text.contains("remote host identification has changed")
        || text.contains("offending") && text.contains("known_hosts")
    {
        "hostKeyChanged"
    } else if text.contains("host key verification failed") || text.contains("host key") {
        "hostKey"
    } else if text.contains("forwarding") || text.contains("remote port forwarding failed") {
        "forwardDenied"
    } else if text.contains("permission denied")
        || text.contains("authentication failed")
        || text.contains("password")
    {
        "sshAuth"
    } else {
        "sshFailed"
    }
    .into()
}

fn valid_response(response: &str) -> bool {
    !response.is_empty()
        && response.len() <= 4096
        && !response
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, b'\r' | b'\n' | 0))
}

fn matching_prompt<'a>(
    prompt: Option<&'a AuthPrompt>,
    prompt_id: &str,
) -> BridgeResult<&'a AuthPrompt> {
    prompt
        .filter(|prompt| prompt.id == prompt_id)
        .ok_or_else(|| "sshAuthPromptChanged".into())
}

fn transcript(output: &SharedTerminalOutput) -> BridgeResult<(String, bool, bool, usize)> {
    let output = output.lock().map_err(|_| "stateUnavailable")?;
    Ok((
        strip_terminal(&output.bytes, &output.marker),
        output.authenticated,
        output.closed,
        output.bytes.len(),
    ))
}

fn terminal_delta(output: &SharedTerminalOutput, start: usize) -> BridgeResult<String> {
    let output = output.lock().map_err(|_| "stateUnavailable")?;
    let bytes = output.bytes.get(start..).unwrap_or_default();
    Ok(strip_terminal(bytes, &output.marker))
}

fn diagnostic(output: &SharedTerminalOutput, prompt_detected: bool) -> BridgeResult<PtyDiagnostic> {
    let output = output.lock().map_err(|_| "stateUnavailable")?;
    Ok(PtyDiagnostic {
        bytes_received: output.bytes_received,
        printable_bytes: output.printable_bytes,
        cpr_requests: output.cpr_requests,
        prompt_detected,
        auth_marker_detected: output.authenticated,
        remote_result_detected: output.remote_result_detected,
        output_closed: output.closed,
    })
}

fn parse_json_result(output: &SharedTerminalOutput, operation: &str) -> BridgeResult<()> {
    let output = output.lock().map_err(|_| "stateUnavailable")?;
    let text = terminal_after_marker(&output.bytes, &output.marker).ok_or("remoteUnsupported")?;
    let json_line = text
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with('{'))
        .ok_or("remoteUnsupported")?;
    ssh::parse_remote_output(operation, json_line).map(|_| ())
}

fn tunnel_remote_command(ports: &[u16], marker: &str) -> String {
    let checks = ports
        .iter()
        .map(|port| {
            format!(
                "lines=\"$(ss -H -ltn 2>/dev/null)\"; printf \"%s\\n\" \"$lines\" | grep -Eq \"127\\.0\\.0\\.1:{port}([[:space:]]|$)\" || ready=0; printf \"%s\\n\" \"$lines\" | grep -Eq \"(^|[[:space:]])(\\*|0\\.0\\.0\\.0|\\[::\\]):{port}([[:space:]]|$)\" && exit 74 || :;"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "sh -c 'set -u; i=0; while [ \"$i\" -lt 12 ]; do ready=1; {checks} if [ \"$ready\" -eq 1 ]; then printf \"\\n{}\\n\"; exec sh -c \"while :; do sleep 3600; done\"; fi; i=$((i+1)); sleep 1; done; exit 73'",
        marker
    )
}

fn interactive_check_remote_command(ports: &[u16], marker: &str) -> String {
    let ports = ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "sh -c 'set -eu; printf \"\\n{marker}\\n\"; fail() {{ printf \"{{\\\"error\\\":\\\"%s\\\"}}\\n\" \"$1\"; exit 0; }}; [ \"$(uname -s)\" = Linux ] || fail remoteUnsupported; [ \"$(id -u)\" != 0 ] || fail rootForbidden; for utility in ss awk sha256sum mktemp flock sync cmp stat sed grep cut cp mv cat unlink; do command -v \"$utility\" >/dev/null 2>&1 || fail dependencyMissing; done; for number in {ports}; do entries=\"$(ss -H -ltn \"sport = :$number\")\" || fail remoteUnsupported; [ -z \"$entries\" ] || fail portInUse; done; printf \"{{\\\"verified\\\":true}}\\n\"'"
    )
}

pub fn begin(
    operation: Operation,
    target_id: String,
    request: Option<Request>,
) -> BridgeResult<Snapshot> {
    cleanup();
    ssh::validate_target(&target_id)?;
    let id = session_id()?;
    let marker = format!("{AUTH_MARKER_PREFIX}{id}__");
    let (mut command, destination) = ssh::interactive_target_command(&target_id)?;
    let (remote_command, completion) = match operation {
        Operation::Check => {
            command.arg("-oClearAllForwardings=yes");
            let ports = random_ports()?;
            (
                interactive_check_remote_command(&[ports.proxy_port, ports.cc_port], &marker),
                Completion::Check { ports },
            )
        }
        Operation::Connect => {
            let request = request.ok_or("invalidRequest")?;
            if request.target_id != target_id {
                return Err("invalidTarget".into());
            }
            let plan = super::prepare_interactive_connect(&request)?;
            command.arg("-oClearAllForwardings=no");
            for (remote, host, local) in &plan.endpoints {
                let host = if host.contains(':') {
                    format!("[{host}]")
                } else {
                    host.clone()
                };
                command
                    .arg("-R")
                    .arg(format!("127.0.0.1:{remote}:{host}:{local}"));
            }
            let ports = plan
                .endpoints
                .iter()
                .map(|endpoint| endpoint.0)
                .collect::<Vec<_>>();
            (
                tunnel_remote_command(&ports, &marker),
                Completion::Connect {
                    request,
                    summary: Box::new(plan.summary),
                    fingerprint: plan.fingerprint,
                },
            )
        }
    };
    let spawned = spawn(command, destination, remote_command, marker)?;
    let session = Session {
        id: id.clone(),
        target_id,
        operation,
        auth: SshAuthState {
            mode: SshAuthMode::Interactive,
            method: SshAuthMethod::Unknown,
            authenticated: false,
            password_stored: false,
        },
        status: SessionStatus::Starting,
        error: None,
        output: spawned.output,
        writer: spawned.writer,
        process: Some(spawned.process),
        completion,
        created_at: Instant::now(),
        attempt: 1,
        current_prompt: None,
        last_prompt: None,
        prompt_generation: 0,
        prompt_consumed_at: 0,
        waiting_since: Instant::now(),
        authenticated_at: None,
    };
    if operation == Operation::Connect {
        super::mark_interactive_connecting()?;
    }
    lock_sessions()?.insert(id.clone(), session);
    state(&id)
}

pub fn state(session_id: &str) -> BridgeResult<Snapshot> {
    let mut sessions = lock_sessions()?;
    let session = sessions
        .get_mut(session_id)
        .ok_or("sshAuthSessionMissing")?;
    let (terminal_text, authenticated, output_closed, output_len) = transcript(&session.output)?;
    session.auth.authenticated = authenticated;
    if authenticated && session.authenticated_at.is_none() {
        session.authenticated_at = Some(Instant::now());
    }
    if authenticated && session.auth.method == SshAuthMethod::Unknown {
        session.auth.method = ssh::non_interactive_auth_method(&session.target_id);
    }
    let exit = session
        .process
        .as_mut()
        .ok_or("sshAuthSessionMissing")?
        .child
        .try_wait()
        .map_err(|_| "processFailed")?;
    let remote_result_ready = session
        .output
        .lock()
        .map_err(|_| "stateUnavailable")?
        .remote_result_detected;
    if session.status == SessionStatus::PromptUnavailable {
        // Preserve the actionable diagnostic state after terminating the stuck PTY.
    } else {
        let completion_timed_out = matches!(session.completion, Completion::Check { .. })
            && session
                .authenticated_at
                .is_some_and(|started| started.elapsed() >= COMPLETION_WAIT_TIMEOUT);
        match (&session.completion, exit, output_closed) {
            (Completion::Check { .. }, _, _) if authenticated && remote_result_ready => {
                match parse_json_result(&session.output, "check") {
                    Ok(()) => session.status = SessionStatus::Succeeded,
                    Err(code) => {
                        session.status = SessionStatus::Failed;
                        session.error = Some(code);
                    }
                }
            }
            (Completion::Connect { .. }, None, _) if authenticated => {
                session.status = SessionStatus::Succeeded;
            }
            (_, Some(_), true) => {
                session.status = SessionStatus::Failed;
                session.error = Some(classify_error(&terminal_text));
            }
            (_, _, _) if completion_timed_out => {
                session.status = SessionStatus::Failed;
                session.error = Some("sshAuthCompletionTimeout".into());
                let _ = session
                    .process
                    .as_mut()
                    .ok_or("sshAuthSessionMissing")?
                    .child
                    .kill();
            }
            (_, _, _) if authenticated => session.status = SessionStatus::Authenticated,
            _ => {
                if session.current_prompt.is_none() && output_len > session.prompt_consumed_at {
                    let delta = terminal_delta(&session.output, session.prompt_consumed_at)?;
                    if let Some(mut prompt) = parse_ssh_prompt(&delta, session.attempt) {
                        let rejected = delta
                            .to_ascii_lowercase()
                            .contains("permission denied, please try again");
                        session.prompt_generation = session.prompt_generation.saturating_add(1);
                        prompt.id = format!("{}:{}", session.id, session.prompt_generation);
                        session.last_prompt = Some((prompt.prompt_type, prompt.message.clone()));
                        session.current_prompt = Some(prompt);
                        session.status = SessionStatus::WaitingUser;
                        if rejected {
                            session.error = Some("sshAuthRejected".into());
                        }
                    }
                }
                if session.current_prompt.is_some() {
                    session.status = SessionStatus::WaitingUser;
                } else if session.waiting_since.elapsed() >= PROMPT_WAIT_TIMEOUT {
                    session.status = SessionStatus::PromptUnavailable;
                    session.error = Some("sshAuthPromptUnavailable".into());
                    let _ = session
                        .process
                        .as_mut()
                        .ok_or("sshAuthSessionMissing")?
                        .child
                        .kill();
                } else if matches!(session.status, SessionStatus::Submitting) {
                    session.status = SessionStatus::WaitingServer;
                } else if session.status != SessionStatus::WaitingServer {
                    session.status = SessionStatus::WaitingPrompt;
                }
            }
        }
    }
    let prompt_detected = session.current_prompt.is_some() || session.last_prompt.is_some();
    let prompt = (session.status == SessionStatus::WaitingUser)
        .then(|| session.current_prompt.clone())
        .flatten();
    let terminal_diagnostic = diagnostic(&session.output, prompt_detected)?;
    Ok(Snapshot {
        session_id: session.id.clone(),
        operation: session.operation,
        status: session.status,
        auth: session.auth,
        prompt,
        diagnostic: terminal_diagnostic,
        error: session.error.clone(),
    })
}

fn write_response(
    session: &mut Session,
    response: &[u8],
    method: SshAuthMethod,
    suppress_echo: bool,
) -> BridgeResult<()> {
    let (_, _, _, output_len) = transcript(&session.output)?;
    session.auth.method = method;
    session.status = SessionStatus::Submitting;
    session.error = None;
    session.attempt = session.attempt.saturating_add(1);
    session.prompt_consumed_at = output_len;
    session.current_prompt = None;
    session.waiting_since = Instant::now();
    if suppress_echo {
        let mut output = session.output.lock().map_err(|_| "stateUnavailable")?;
        output.pending_echo = Some(Zeroizing::new(response.to_vec()));
        output.echo_match_len = 0;
    }
    let mut writer = session.writer.lock().map_err(|_| "stateUnavailable")?;
    writer
        .write_all(response)
        .and_then(|_| writer.write_all(b"\r"))
        .and_then(|_| writer.flush())
        .map_err(|_| "processFailed".into())
}

pub fn submit(session_id: &str, prompt_id: &str, response: String) -> BridgeResult<Snapshot> {
    if !valid_response(&response) {
        return Err("sshAuthInputInvalid".into());
    }
    let mut response = Zeroizing::new(response);
    {
        let mut sessions = lock_sessions()?;
        let session = sessions
            .get_mut(session_id)
            .ok_or("sshAuthSessionMissing")?;
        if session.auth.authenticated || session.status != SessionStatus::WaitingUser {
            return Err("sshAuthSessionClosed".into());
        }
        let prompt = matching_prompt(session.current_prompt.as_ref(), prompt_id)?;
        if prompt.prompt_type == PromptType::HostKeyConfirmation {
            return Err("sshAuthInputInvalid".into());
        }
        let method = match prompt.prompt_type {
            PromptType::Password | PromptType::KeyPassphrase => SshAuthMethod::Password,
            _ => SshAuthMethod::KeyboardInteractive,
        };
        write_response(session, response.as_bytes(), method, prompt.secret)?;
    }
    response.clear();
    state(session_id)
}

pub fn confirm_host(session_id: &str, prompt_id: &str) -> BridgeResult<Snapshot> {
    {
        let mut sessions = lock_sessions()?;
        let session = sessions
            .get_mut(session_id)
            .ok_or("sshAuthSessionMissing")?;
        if session.auth.authenticated || session.status != SessionStatus::WaitingUser {
            return Err("sshAuthSessionClosed".into());
        }
        let prompt = matching_prompt(session.current_prompt.as_ref(), prompt_id)?;
        if prompt.prompt_type != PromptType::HostKeyConfirmation {
            return Err("sshAuthInputInvalid".into());
        }
        write_response(session, b"yes", SshAuthMethod::Unknown, false)?;
    }
    state(session_id)
}

pub fn finish(session_id: &str) -> BridgeResult<Outcome> {
    let snapshot = state(session_id)?;
    if snapshot.status != SessionStatus::Succeeded {
        return Err(snapshot.error.unwrap_or_else(|| "sshAuthPending".into()));
    }
    let mut session = lock_sessions()?
        .remove(session_id)
        .ok_or("sshAuthSessionMissing")?;
    match session.completion {
        Completion::Check { ports } => {
            super::complete_interactive_check(session.auth);
            Ok(Outcome {
                operation: Operation::Check,
                ports: Some(ports),
                summary: None,
            })
        }
        Completion::Connect {
            request,
            summary,
            fingerprint,
        } => {
            let process = session.process.take().ok_or("processFailed")?;
            let summary = super::complete_interactive_connect(
                request,
                *summary,
                fingerprint,
                process,
                session.auth,
            )?;
            Ok(Outcome {
                operation: Operation::Connect,
                ports: None,
                summary: Some(summary),
            })
        }
    }
}

pub fn cancel(session_id: &str) -> BridgeResult<()> {
    let session = lock_sessions()?
        .remove(session_id)
        .ok_or("sshAuthSessionMissing")?;
    if session.operation == Operation::Connect {
        super::cancel_interactive_connect();
    }
    Ok(())
}

pub fn cleanup() {
    if let Ok(mut sessions) = sessions().try_lock() {
        let cancelled_connect = sessions.values().any(|session| {
            session.operation == Operation::Connect && session.created_at.elapsed() >= SESSION_TTL
        });
        sessions.retain(|_, session| session.created_at.elapsed() < SESSION_TTL);
        drop(sessions);
        if cancelled_connect {
            super::cancel_interactive_connect();
        }
    }
}

pub fn shutdown() {
    if let Ok(mut sessions) = sessions().try_lock() {
        sessions.clear();
    }
    super::cancel_interactive_connect();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_output() -> TerminalOutput {
        TerminalOutput {
            bytes: Vec::new(),
            authenticated: false,
            marker: b"test-marker".to_vec(),
            pending_echo: None,
            echo_match_len: 0,
            bytes_received: 0,
            printable_bytes: 0,
            cpr_requests: 0,
            remote_result_detected: false,
            closed: false,
        }
    }

    #[test]
    fn terminal_output_removes_control_sequences_and_private_marker() {
        let marker = b"__PROXYENV_SSH_AUTH_test__";
        let raw = b"\x1b]0;private title\x07\x1b[2JPassword: __PROXYENV_SSH_AUTH_test__\r\n";
        let clean = strip_terminal(raw, marker);
        assert_eq!(clean, "Password:");
        assert!(!clean.contains("PROXYENV"));
    }

    #[test]
    fn terminal_output_is_bounded() {
        let raw = vec![b'a'; MAX_TRANSCRIPT_CHARS + 100];
        assert_eq!(
            strip_terminal(&raw, b"marker").chars().count(),
            MAX_TRANSCRIPT_CHARS
        );
    }

    #[test]
    fn remote_result_detection_accepts_only_complete_json_lines() {
        assert!(contains_remote_result(
            b"noise\r\nmarker\r\n{\"verified\":true}\r\n",
            b"marker"
        ));
        assert!(!contains_remote_result(
            b"{\"verified\":true}\r\nmarker\r\n",
            b"marker"
        ));
        assert!(!contains_remote_result(
            b"marker\r\nprintf '{\"verified\":true}'\r\n",
            b"marker"
        ));
        assert!(!contains_remote_result(
            b"marker\r\n{\"verified\":true",
            b"marker"
        ));
    }

    #[test]
    fn prompt_parser_classifies_common_openssh_questions() {
        for value in ["student@host's password:", "Password:"] {
            let prompt = parse_ssh_prompt(value, 1).expect("password prompt");
            assert_eq!(prompt.prompt_type, PromptType::Password);
            assert!(prompt.secret);
        }
        let passphrase = parse_ssh_prompt(
            "Enter passphrase for key '/home/student/.ssh/id_ed25519':",
            1,
        )
        .expect("passphrase prompt");
        assert_eq!(passphrase.prompt_type, PromptType::KeyPassphrase);
        assert_eq!(passphrase.message, "Enter passphrase for key 'id_ed25519':");
        assert!(!passphrase.message.contains("/home/student"));

        for value in ["Verification code:", "OTP:", "One-time password:"] {
            let prompt = parse_ssh_prompt(value, 2).expect("verification prompt");
            assert_eq!(prompt.prompt_type, PromptType::VerificationCode);
            assert!(prompt.secret);
        }
        let challenge = parse_ssh_prompt("Duo two-factor login\nToken:", 2)
            .expect("keyboard-interactive prompt");
        assert_eq!(challenge.prompt_type, PromptType::KeyboardInteractive);
    }

    #[test]
    fn prompt_parser_handles_fragmented_host_confirmation_and_unknown_prompts() {
        let fragmented = ["student@host's pass", "word:"].concat();
        assert_eq!(
            parse_ssh_prompt(&fragmented, 1)
                .expect("fragmented prompt")
                .prompt_type,
            PromptType::Password
        );

        let host = "The authenticity of host 'gpu01' can't be established.\nED25519 key fingerprint is SHA256:abc123.\nAre you sure you want to continue connecting (yes/no/[fingerprint])?";
        let prompt = parse_ssh_prompt(host, 1).expect("host confirmation");
        assert_eq!(prompt.prompt_type, PromptType::HostKeyConfirmation);
        assert_eq!(prompt.fingerprint.as_deref(), Some("SHA256:abc123"));
        assert!(!prompt.secret);

        assert_eq!(
            parse_ssh_prompt("University access policy accepted?", 1)
                .expect("unknown prompt")
                .prompt_type,
            PromptType::Unknown
        );
    }

    #[test]
    fn submitted_response_echo_is_suppressed_without_swallowing_the_next_prompt() {
        let mut output = test_output();
        output.pending_echo = Some(Zeroizing::new(b"secret-response".to_vec()));

        let first = suppress_confirmed_echo(&mut output, b"secret-");
        let second = suppress_confirmed_echo(&mut output, b"response\r\nVerification code:");

        assert!(first.is_empty());
        assert_eq!(second, b"\r\nVerification code:");
        assert!(output.pending_echo.is_none());
        assert_eq!(
            parse_ssh_prompt(&String::from_utf8_lossy(&second), 2)
                .expect("next prompt")
                .prompt_type,
            PromptType::VerificationCode
        );
    }

    #[test]
    fn terminal_control_parser_answers_complete_and_fragmented_cursor_queries() {
        let mut complete = TerminalControlParser::default();
        let parsed = complete.push(b"\x1b[6n");
        assert_eq!(parsed.cpr_requests, 1);
        assert!(parsed.printable.is_empty());

        let mut fragmented = TerminalControlParser::default();
        assert_eq!(fragmented.push(b"\x1b[").cpr_requests, 0);
        assert_eq!(fragmented.push(b"6").cpr_requests, 0);
        let parsed = fragmented.push(b"n");
        assert_eq!(parsed.cpr_requests, 1);
        assert!(parsed.printable.is_empty());
    }

    #[test]
    fn fragmented_ansi_output_still_exposes_a_password_prompt() {
        let mut parser = TerminalControlParser::default();
        let mut visible = Vec::new();
        for chunk in [
            b"\x1b[".as_slice(),
            b"31mPass".as_slice(),
            b"word:".as_slice(),
        ] {
            visible.extend(parser.push(chunk).printable);
        }
        let text = String::from_utf8(visible).expect("utf8 prompt");
        assert_eq!(
            parse_ssh_prompt(&text, 1)
                .expect("password prompt")
                .prompt_type,
            PromptType::Password
        );
    }

    #[test]
    fn response_must_match_the_session_owned_prompt_identifier() {
        let prompt = AuthPrompt {
            id: "session-a:2".into(),
            prompt_type: PromptType::VerificationCode,
            message: "Verification code:".into(),
            secret: true,
            attempt: 2,
            target: None,
            fingerprint: None,
        };
        assert!(matching_prompt(Some(&prompt), "session-a:2").is_ok());
        assert_eq!(
            matching_prompt(Some(&prompt), "session-a:1").unwrap_err(),
            "sshAuthPromptChanged"
        );
        assert_eq!(
            matching_prompt(None, "session-a:2").unwrap_err(),
            "sshAuthPromptChanged"
        );
    }

    #[test]
    fn tunnel_command_only_contains_validated_numeric_ports() {
        let command = tunnel_remote_command(&[23841, 31472], "marker");
        assert!(command.contains("127\\.0\\.0\\.1:23841"));
        assert!(command.contains("127\\.0\\.0\\.1:31472"));
        assert!(command.contains("while :; do sleep 3600"));
    }

    #[test]
    fn interactive_check_is_a_bounded_fixed_command_without_stdin_payload() {
        let command = interactive_check_remote_command(&[23841, 31472], "safe-marker");
        assert!(command.contains("safe-marker"));
        assert!(command.contains("for number in 23841 31472"));
        assert!(command.contains("dependencyMissing"));
        assert!(command.contains("portInUse"));
        assert!(command.contains("{\\\"verified\\\":true}"));
        assert!(!command.contains("sh -s"));
        assert!(command.len() < 2_048);
    }

    #[test]
    fn authentication_errors_keep_host_key_auth_and_forwarding_separate() {
        assert_eq!(classify_error("Host key verification failed"), "hostKey");
        assert_eq!(
            classify_error("WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!"),
            "hostKeyChanged"
        );
        assert_eq!(
            classify_error("Permission denied (publickey,password)"),
            "sshAuth"
        );
        assert_eq!(
            classify_error("remote port forwarding failed for listen port"),
            "forwardDenied"
        );
        assert_eq!(classify_error("connection reset"), "sshFailed");
    }

    #[test]
    fn authentication_response_rejects_multiline_or_unbounded_input() {
        assert!(valid_response("one-time-response"));
        assert!(!valid_response(""));
        assert!(!valid_response("first\nsecond"));
        assert!(!valid_response("first\rsecond"));
        assert!(!valid_response(&"x".repeat(4097)));
    }
}
