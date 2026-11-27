use super::*;
use crate::features::remote_bridge::target_os::windows_bridge::safety::policy::ReviewScope;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

const CHILD_PATH: &str = "ROUTESPAN_POLICY_STORAGE_FIXTURE";

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        Self(
            std::env::current_dir()
                .unwrap()
                .join(format!("routespan-history-{}", hex::encode(nonce))),
        )
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // The fixture alone creates the directory and consumes any staged fixture output.
        for name in [LOCK, DATA] {
            let _ = fs::remove_file(self.0.join(name));
        }
        let removed = fs::remove_dir(&self.0);
        if !std::thread::panicking() {
            removed.unwrap();
        }
    }
}

fn key(revision: u64) -> HistoryKey {
    HistoryKey {
        issuer_key: [11; 32],
        review: ReviewScope {
            review: "synthetic".into(),
            revision,
            deployment: 7,
        },
    }
}

fn decision(sequence: u64, revoked: bool) -> Decision {
    Decision {
        sequence,
        digest: [sequence as u8; 32],
        revoked,
    }
}

#[test]
fn protected_history_cas_isolates_scopes_and_preserves_revocation() {
    let root = Scratch::new();
    let first = PolicyStorage::initialize(&root.0).unwrap();
    let second = PolicyStorage::open(&root.0).unwrap();
    let main = key(1);
    let other = key(2);
    let allowed = decision(9, false);
    let revoked = decision(10, true);
    assert_eq!(first.read(&main), Ok(None));
    first.commit(&main, None, allowed).unwrap();
    assert_eq!(second.read(&main), Ok(Some(allowed)));
    assert_eq!(second.read(&main), Ok(Some(allowed)));
    assert_eq!(second.read(&other), Ok(None));
    second.commit(&other, None, allowed).unwrap();
    let _lock = first.lock(false).unwrap();
    assert_eq!(
        second.commit(&main, Some(allowed), revoked),
        Err(HistoryError::Conflict)
    );
    drop(_lock);
    second.commit(&main, Some(allowed), revoked).unwrap();
    assert_eq!(
        first.commit(&main, Some(allowed), decision(11, false)),
        Err(HistoryError::Conflict)
    );
    assert_eq!(
        first.commit(&main, Some(revoked), decision(11, false)),
        Err(HistoryError::Conflict)
    );
    assert_eq!(second.read(&other), Ok(Some(allowed)));
    assert_eq!(first.read(&main), Ok(Some(revoked)));
}

#[test]
fn protected_history_never_reinitializes_missing_or_unreadable_data() {
    let root = Scratch::new();
    let store = PolicyStorage::initialize(&root.0).unwrap();
    let other_root = Scratch::new();
    let other = PolicyStorage::initialize(&other_root.0).unwrap();
    let key = key(1);
    store.commit(&key, None, decision(9, true)).unwrap();
    let data = root.0.join(DATA);
    let protected = fs::read(&data).unwrap();
    fs::write(other_root.0.join(DATA), &protected).unwrap();
    assert_eq!(other.read(&key), Err(HistoryError::Corrupt));

    for invalid in [Vec::new(), b"damaged".to_vec(), protected[1..].to_vec()] {
        fs::write(&data, invalid).unwrap();
        assert_eq!(store.read(&key), Err(HistoryError::Corrupt));
        assert_eq!(
            store.commit(&key, None, decision(10, false)),
            Err(HistoryError::Corrupt)
        );
    }
    fs::remove_file(&data).unwrap();
    assert_eq!(store.read(&key), Err(HistoryError::Missing));
    assert!(PolicyStorage::initialize(&root.0).is_err());
    fs::write(&data, protected).unwrap();
    fs::remove_file(root.0.join(LOCK)).unwrap();
    assert_eq!(store.read(&key), Err(HistoryError::Missing));
}

#[test]
fn protected_history_restarts_after_interrupted_stage_and_failed_publish() {
    let key = key(1);
    let revoked = decision(9, true);
    if let Some(path) = std::env::var_os(CHILD_PATH) {
        let store = PolicyStorage::open(Path::new(&path)).unwrap();
        assert_eq!(store.read(&key), Ok(Some(revoked)));
        let _lock = store.lock(false).unwrap();
        let mut next = store.load().unwrap();
        next.entries[0].decision = decision(10, true);
        let _staged = store.stage(&next).unwrap();
        std::process::exit(77);
    }

    let root = Scratch::new();
    let store = PolicyStorage::initialize(&root.0).unwrap();
    store.commit(&key, None, revoked).unwrap();
    let blocked = store
        .directory
        .file(DATA, GENERIC_READ.0, FILE_SHARE_MODE(0), false)
        .unwrap();
    let unpublished = Snapshot {
        version: 1,
        entries: vec![Entry {
            key: key.clone(),
            decision: decision(10, true),
        }],
    };
    assert_eq!(
        store.save(&unpublished, true),
        Err(HistoryError::Unavailable)
    );
    drop(blocked);
    assert_eq!(store.read(&key), Ok(Some(revoked)));
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 2);
    drop(store);

    let name = format!(
        "{}::protected_history_restarts_after_interrupted_stage_and_failed_publish",
        module_path!()
    );
    let name = name.split_once("::").unwrap().1;
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name])
        .env(CHILD_PATH, &root.0)
        .spawn()
        .unwrap();
    let limit = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= limit {
            let _ = child.kill();
            let _ = child.wait();
            panic!("policy storage fixture child timed out");
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(status.code(), Some(77));
    let reopened = PolicyStorage::open(&root.0).unwrap();
    assert_eq!(reopened.read(&key), Ok(Some(revoked)));
    let staged: Vec<_> = fs::read_dir(&root.0)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".stage"))
        .collect();
    assert_eq!(staged.len(), 1);
    let name = staged[0].file_name();
    let stage = reopened
        .directory
        .file(
            name.to_str().unwrap(),
            GENERIC_READ.0 | DELETE.0,
            FILE_SHARE_MODE(0),
            false,
        )
        .unwrap();
    delete_by_handle(&stage).unwrap();
}
