use super::*;
use crate::features::remote_bridge::{
    base64_bytes, target_os::windows_bridge::safety::frozen::HostTrustEvidence,
};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

fn fixture() -> (PathBuf, FrozenProfile) {
    let mut nonce = [0; 12];
    getrandom::fill(&mut nonce).unwrap();
    let path = std::env::current_dir()
        .unwrap()
        .join(format!("routespan-frozen-{}", hex::encode(nonce)));
    let directory = path.to_str().unwrap().replace('\\', "/");
    let key = [b"\0\0\0\x0bssh-ed25519\0\0\0\x20".as_slice(), &[17; 32]].concat();
    let known = format!("[fixture.invalid]:2222 ssh-ed25519 {}", base64_bytes(&key));
    let profile = FrozenProfile::build(
        ("fixture.invalid", "fixture", 2222),
        Sha256::digest(&key).into(),
        HostTrustEvidence {
            reviewed_public_blob: &key,
            known_hosts: Some(&known),
            revoked_blobs: Some(&[]),
        },
        &directory,
    )
    .unwrap();
    (path, profile)
}

#[test]
fn frozen_resources_hold_verified_files_and_release_idempotently() {
    let (path, profile) = fixture();
    let mut resources = FrozenResources::create(&path, profile).unwrap();
    resources.validate().unwrap();
    let config = path.join(FILES[0]);
    assert!(fs::OpenOptions::new().write(true).open(&config).is_err());
    assert!(fs::rename(&config, path.join("other")).is_err());
    resources.validate().unwrap();
    assert!(resources.close().is_empty());
    assert!(!path.exists());
    assert!(resources.close().is_empty());
    assert!(resources.validate().is_err());
}

#[test]
fn frozen_resources_preserve_foreign_objects_after_failed_setup() {
    let (path, profile) = fixture();
    fs::create_dir(&path).unwrap();
    let Err(error) = FrozenResources::create(&path, profile) else {
        panic!("existing directory accepted")
    };
    assert_eq!(error.error.kind(), io::ErrorKind::AlreadyExists);
    assert!(error.pending.is_empty());
    fs::remove_dir(&path).unwrap();

    let (path, profile) = fixture();
    let mut resources = FrozenResources::new(&path, profile).unwrap();
    let foreign = path.join(FILES[1]);
    fs::write(&foreign, b"fixture collision").unwrap();
    assert!(resources.populate().is_err());
    assert!(resources.validate().is_err());
    let pending = resources.close();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].name, ".");
    assert!(!path.join(FILES[0]).exists());
    assert_eq!(fs::read(&foreign).unwrap(), b"fixture collision");
    fs::remove_file(&foreign).unwrap();
    assert!(resources.close().is_empty());
}

#[test]
fn frozen_resources_refuse_replacement_and_retry_owned_cleanup() {
    let (path, profile) = fixture();
    let mut resources = FrozenResources::create(&path, profile).unwrap();
    let config = path.join(FILES[0]);
    let original = path.join("test parked original");
    let id = resources.files[0].id;
    resources.files[0].reader.take();
    fs::rename(&config, &original).unwrap();
    let directory = resources.directory.as_ref().unwrap();
    let mut replacement = directory
        .file(
            FILES[0],
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ,
            true,
        )
        .unwrap();
    replacement
        .write_all(b"same path, different object")
        .unwrap();
    assert_ne!(directory.validate(&replacement, false).unwrap(), id);
    drop(replacement);
    assert!(resources.validate().is_err());
    let pending = resources.close();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, Some(id));
    assert_eq!(pending[0].name, FILES[0]);
    assert_eq!(pending[0].error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(fs::read(&config).unwrap(), b"same path, different object");
    fs::remove_file(&config).unwrap();
    fs::rename(&original, &config).unwrap();
    let retry = resources.close();
    assert!(retry.is_empty(), "{retry:?}");
    assert!(!path.exists());
}
