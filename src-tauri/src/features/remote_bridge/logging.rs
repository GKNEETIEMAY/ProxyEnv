//! Bounded asynchronous JSONL logging. Only typed, redacted bridge events enter here.
//! Never subscribe to arbitrary tracing output or serialize requests/summaries.
use super::events::BridgeEvent;
#[cfg(any(test, debug_assertions))]
use super::observations::timestamp;
use crate::services::local_file;
use serde::Serialize;
use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
        Arc, OnceLock,
    },
    time::Duration,
};

const MAX_BYTES: u64 = 2 * 1024 * 1024;
const FILES: usize = 3;
static SINK: OnceLock<Sink> = OnceLock::new();
struct Sink {
    sender: SyncSender<Message>,
    directory: PathBuf,
    healthy: Arc<AtomicBool>,
}
enum Message {
    Event(BridgeEvent),
    Debug,
    Clear(mpsc::Sender<io::Result<()>>),
    Stop(mpsc::Sender<()>),
}
#[derive(Serialize)]
pub struct LogStatus {
    pub available: bool,
}

pub fn initialize(directory: PathBuf) {
    if SINK.get().is_some() {
        return;
    }
    let (sender, receiver) = mpsc::sync_channel(256);
    let healthy = Arc::new(AtomicBool::new(false));
    let worker_health = healthy.clone();
    let worker_directory = directory.clone();
    if std::thread::Builder::new().name("bridge-log".into()).spawn(move || {
        let mut writer = Writer { directory: worker_directory, limit: MAX_BYTES };
        worker_health.store(writer.prepare().is_ok(), Ordering::Release);
        while let Ok(message) = receiver.recv() {
            match message {
                Message::Stop(ack) => { let _ = ack.send(()); break; }
                Message::Clear(ack) => {
                    let result = writer.clear();
                    worker_health.store(result.is_ok(), Ordering::Release);
                    let _ = ack.send(result);
                }
                Message::Event(event) => {
                    let result = serialize_event(&event).and_then(|bytes| writer.append(bytes));
                    worker_health.store(result.is_ok(), Ordering::Release);
                }
                Message::Debug => {
                    #[cfg(debug_assertions)]
                    {
                        let bytes = serde_json::to_vec(&serde_json::json!({"timestamp":timestamp(), "level":"DEBUG", "phase":"diagnostics.run"}));
                        let result = bytes.map_err(io::Error::other).and_then(|bytes| writer.append(bytes));
                        worker_health.store(result.is_ok(), Ordering::Release);
                    }
                }
            }
        }
    }).is_err() { return; }
    let _ = SINK.set(Sink {
        sender,
        directory,
        healthy,
    });
}
pub(super) fn record(event: &BridgeEvent) {
    if let Some(sink) = SINK.get() {
        // A full queue drops a log record, never blocks SSH or relay traffic.
        let _ = sink.sender.try_send(Message::Event(event.clone()));
    }
}
pub(super) fn debug_diagnostics() {
    if cfg!(debug_assertions) {
        if let Some(sink) = SINK.get() {
            let _ = sink.sender.try_send(Message::Debug);
        }
    }
}
pub fn status() -> LogStatus {
    LogStatus {
        available: SINK
            .get()
            .is_some_and(|sink| sink.healthy.load(Ordering::Acquire)),
    }
}
pub fn directory() -> super::BridgeResult<PathBuf> {
    let sink = SINK
        .get()
        .filter(|sink| sink.healthy.load(Ordering::Acquire))
        .ok_or("logUnavailable")?;
    validate_directory(&sink.directory).map_err(|_| "logUnavailable")?;
    Ok(sink.directory.clone())
}
pub fn shutdown() {
    if let Some(sink) = SINK.get() {
        let (ack, receiver) = mpsc::channel();
        if sink.sender.try_send(Message::Stop(ack)).is_ok() {
            let _ = receiver.recv_timeout(Duration::from_secs(2));
        }
        sink.healthy.store(false, Ordering::Release);
    }
}
pub fn clear() -> super::BridgeResult<()> {
    let sink = SINK
        .get()
        .filter(|sink| sink.healthy.load(Ordering::Acquire))
        .ok_or("logUnavailable")?;
    let (ack, receiver) = mpsc::channel();
    sink.sender
        .try_send(Message::Clear(ack))
        .map_err(|_| "logBusy")?;
    receiver
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "logUnavailable")?
        .map_err(|_| "logUnavailable".into())
}
fn serialize_event(event: &BridgeEvent) -> io::Result<Vec<u8>> {
    let mut value = serde_json::to_value(event).map_err(io::Error::other)?;
    value["level"] = match event.level {
        super::events::Level::Info => "INFO",
        super::events::Level::Warning => "WARN",
        super::events::Level::Error => "ERROR",
    }
    .into();
    serde_json::to_vec(&value).map_err(io::Error::other)
}
fn validate_directory(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::other("unsafe log directory"));
    }
    // Check each existing ancestor, including Windows junctions. Do not follow redirected storage.
    for ancestor in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
        local_file::ensure_safe_directory(ancestor)?;
    }
    Ok(())
}
struct Writer {
    directory: PathBuf,
    limit: u64,
}
impl Writer {
    fn path(&self, index: usize) -> PathBuf {
        self.directory.join(format!("bridge-{index}.jsonl"))
    }
    fn prepare(&self) -> io::Result<()> {
        validate_directory(&self.directory)?;
        // Existing files must also pass the shared local-file boundary.
        for index in 0..FILES {
            local_file::safe_read(&self.path(index), self.limit)?;
        }
        Ok(())
    }
    fn append(&mut self, mut record: Vec<u8>) -> io::Result<()> {
        self.prepare()?;
        record.push(b'\n');
        if record.len() as u64 > self.limit {
            return Err(io::Error::other("oversized log record"));
        }
        let mut current = local_file::safe_read(&self.path(0), self.limit)?.unwrap_or_default();
        if current.len() as u64 + record.len() as u64 > self.limit {
            // Fixed, validated internal files only. No glob deletion or recursive cleanup.
            for index in (1..FILES).rev() {
                if let Some(previous) = local_file::safe_read(&self.path(index - 1), self.limit)? {
                    local_file::atomic_write(&self.path(index), &previous, "bridge-log")?;
                }
            }
            current.clear();
        }
        current.extend(record);
        local_file::atomic_write(&self.path(0), &current, "bridge-log")
    }
    fn clear(&mut self) -> io::Result<()> {
        // Validate every fixed target before clearing anything. Retain empty files
        // rather than deleting the directory or traversing unrelated user data.
        self.prepare()?;
        for index in 0..FILES {
            let path = self.path(index);
            if local_file::safe_read(&path, self.limit)?.is_some() {
                local_file::atomic_write(&path, b"", "bridge-log-clear")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_events_have_no_free_form_or_credential_fields() {
        let event = BridgeEvent {
            timestamp: 1,
            level: super::super::events::Level::Warning,
            component: super::super::events::Component::AiRoute,
            action: super::super::events::Action::ToolVerify,
            outcome: super::super::events::Outcome::Warning,
            error_code: Some(super::super::observations::ErrorCategory::from_code(
                "Authorization: secret",
            )),
            duration_ms: Some(4),
        };
        let bytes = serialize_event(&event).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("secret"));
        assert!(!text.contains("Authorization"));
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 7);
        assert_eq!(value["level"], "WARN");
        assert_eq!(value["errorCode"], "unknown");
    }
    #[test]
    fn rolling_files_are_bounded_and_keep_latest_records() {
        let directory = std::env::temp_dir().join(format!(
            "proxyenv-log-test-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let mut writer = Writer {
            directory: directory.clone(),
            limit: 10,
        };
        for index in 0..8 {
            writer
                .append(format!("{{\"n\":{index}}}").into_bytes())
                .unwrap();
        }
        for (file, value) in [(0, 7), (1, 6), (2, 5)] {
            assert_eq!(
                String::from_utf8(
                    local_file::safe_read(&writer.path(file), 10)
                        .unwrap()
                        .unwrap()
                )
                .unwrap(),
                format!("{{\"n\":{value}}}\n")
            );
        }
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), FILES);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn unsupported_storage_does_not_get_overwritten() {
        let directory = std::env::temp_dir().join(format!(
            "proxyenv-log-unsafe-{}-{}",
            std::process::id(),
            timestamp()
        ));
        std::fs::create_dir_all(directory.join("bridge-0.jsonl")).unwrap();
        let mut writer = Writer {
            directory: directory.clone(),
            limit: 10,
        };
        assert!(writer.append(b"{}".to_vec()).is_err());
        assert!(directory.join("bridge-0.jsonl").is_dir());
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn clear_only_empties_owned_logs_and_logging_can_continue() {
        let directory = std::env::temp_dir().join(format!(
            "proxyenv-log-clear-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let mut writer = Writer {
            directory: directory.clone(),
            limit: 10,
        };
        for _ in 0..4 {
            writer.append(b"{\"n\":1}".to_vec()).unwrap();
        }
        let foreign = directory.join("settings.json");
        local_file::atomic_write(&foreign, b"keep", "fixture").unwrap();
        writer.clear().unwrap();
        for index in 0..FILES {
            assert_eq!(
                local_file::safe_read(&writer.path(index), 10).unwrap(),
                Some(vec![])
            );
        }
        assert_eq!(std::fs::read(&foreign).unwrap(), b"keep");
        writer.append(b"{}".to_vec()).unwrap();
        assert_eq!(std::fs::read(writer.path(0)).unwrap(), b"{}\n");
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn clear_checks_all_targets_before_changing_any_log() {
        let directory = std::env::temp_dir().join(format!(
            "proxyenv-log-clear-unsafe-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let mut writer = Writer {
            directory: directory.clone(),
            limit: 10,
        };
        writer.append(b"{}".to_vec()).unwrap();
        std::fs::create_dir(directory.join("bridge-2.jsonl")).unwrap();
        assert!(writer.clear().is_err());
        assert_eq!(std::fs::read(writer.path(0)).unwrap(), b"{}\n");
        assert!(!writer.path(1).exists());
        assert!(writer.path(2).is_dir());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
