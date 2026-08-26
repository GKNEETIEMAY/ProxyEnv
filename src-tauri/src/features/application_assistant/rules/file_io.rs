use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use chrono::Utc;
use sha2::{Digest, Sha256};

static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(0);

pub fn operation_id() -> String {
    let sequence = NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed);
    format!(
        "{}-{}-{sequence}",
        Utc::now().timestamp_millis(),
        std::process::id()
    )
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn atomic_write(destination: &Path, bytes: &[u8], operation_id: &str) -> std::io::Result<()> {
    crate::services::local_file::atomic_write(destination, bytes, operation_id)
}
