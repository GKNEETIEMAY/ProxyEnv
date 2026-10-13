//! Token-free cleanup ownership survives an application crash. Session secrets do not.
use super::{ssh, BridgeResult, Endpoint};
use crate::services::local_file;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fs, path::PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    target: String,
    session: String,
    fingerprint: String,
}
fn path() -> BridgeResult<PathBuf> {
    dirs::data_local_dir()
        .map(|root| root.join("ProxyEnv/vscode-network-owners.json"))
        .ok_or_else(|| "stateUnavailable".into())
}
fn load() -> BridgeResult<Vec<Owner>> {
    let Some(bytes) = local_file::safe_read(&path()?, 32768).map_err(|_| "stateUnavailable")?
    else {
        return Ok(Vec::new());
    };
    let owners: Vec<Owner> = serde_json::from_slice(&bytes).map_err(|_| "stateUnavailable")?;
    if owners.len() > 32
        || owners.iter().any(|owner| {
            owner.target.is_empty()
                || owner.target.len() > 256
                || owner.target.chars().any(char::is_control)
                || owner.session.len() != 32
                || !owner.session.bytes().all(|b| b.is_ascii_hexdigit())
                || owner.fingerprint.len() != 64
                || !owner.fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
        })
    {
        return Err("stateUnavailable".into());
    }
    Ok(owners)
}
fn save(owners: &[Owner]) -> BridgeResult<()> {
    let path = path()?;
    fs::create_dir_all(path.parent().ok_or("stateUnavailable")?).map_err(|_| "stateUnavailable")?;
    let bytes = serde_json::to_vec(owners).map_err(|_| "stateUnavailable")?;
    local_file::atomic_write(&path, &bytes, "vscode-network-owners")
        .map_err(|_| "stateUnavailable".into())
}
fn restore_owner(owner: &Owner, fingerprint: &str) -> BridgeResult<()> {
    if owner.fingerprint != fingerprint {
        return Err("sshConfigChanged".into());
    }
    ssh::extension_remote(
        &owner.target,
        &json!({
            "operation":"network-restore", "sessionId":owner.session
        }),
    )?;
    Ok(())
}
// Caller holds the bridge store lock, serializing apply/restore and connection changes.
pub(super) fn is_managed(target: &str) -> BridgeResult<bool> {
    Ok(load()?.iter().any(|owner| owner.target == target))
}
pub(super) fn restore(target: &str) -> BridgeResult<()> {
    let mut owners = load()?;
    let Some(index) = owners.iter().position(|owner| owner.target == target) else {
        return Ok(());
    };
    restore_owner(&owners[index], &ssh::fingerprint(target)?)?;
    owners.remove(index);
    save(&owners)
}
pub(super) fn apply(
    target: &str,
    session: &str,
    fingerprint: &str,
    proxy: Option<&Endpoint>,
) -> BridgeResult<()> {
    let mut owners = load()?;
    if let Some(index) = owners.iter().position(|owner| owner.target == target) {
        let owner = &owners[index];
        if owner.fingerprint != fingerprint {
            return Err("sshConfigChanged".into());
        }
        if owner.session != session {
            restore_owner(owner, fingerprint)?;
            owners.remove(index);
            save(&owners)?;
        }
    }
    if !owners.iter().any(|owner| owner.target == target) {
        if owners.len() >= 32 {
            return Err("stateUnavailable".into());
        }
        owners.push(Owner {
            target: target.into(),
            session: session.into(),
            fingerprint: fingerprint.into(),
        });
        // Persist before sending: a lost SSH reply must not lose restoration ownership.
        save(&owners)?;
    }
    ssh::extension_remote(
        target,
        &json!({"operation":"network-apply", "sessionId":session,
            "proxyPort":proxy.map(|endpoint| endpoint.remote_port),
            "proxyProtocol":proxy.map(|endpoint| endpoint.local.protocol)}),
    )?;
    Ok(())
}
