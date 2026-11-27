use crate::features::remote_bridge::BridgeResult;
use ring::signature::{UnparsedPublicKey, ED25519};
use serde::{de::DeserializeOwned, Deserialize};
use std::time::{Duration, Instant};

pub(super) mod lifecycle;

pub(super) const LEASE_DOMAIN: &[u8] = b"RouteSpan/windows-forward-lease/v1\0";
pub(super) const STATUS_DOMAIN: &[u8] = b"RouteSpan/windows-forward-status/v1\0";
const MAX_PAYLOAD: usize = 4096;
const MAX_LEASE_SECONDS: u64 = 300;
const MAX_STATUS_SECONDS: u64 = 60;

pub(in super::super) struct PolicyIssuer {
    pub key: [u8; 32],
    pub revision: u64,
    pub deployment: u64,
}

#[derive(Clone, Copy)]
pub(super) struct SignedRecord<'a> {
    pub payload: &'a [u8],
    pub signature: &'a [u8],
}

#[derive(Clone, Debug, Deserialize, serde::Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub(super) struct ReviewScope {
    pub review: String,
    pub revision: u64,
    pub deployment: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize))]
#[serde(deny_unknown_fields)]
pub(super) struct LeaseScope {
    pub review: ReviewScope,
    pub host: String,
    pub account: String,
    pub ssh_port: u16,
    pub host_key_sha256: [u8; 32],
    pub config_fingerprint: String,
    pub generation: u64,
    pub remote_port: u16,
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
#[serde(deny_unknown_fields)]
pub(super) struct Lease {
    pub scope: LeaseScope,
    pub gateway_ports: String,
    pub issued: u64,
    pub expires: u64,
    pub changes_blocked_until: u64,
    pub minimum_status_sequence: u64,
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
#[serde(deny_unknown_fields)]
pub(super) struct Status {
    pub review: ReviewScope,
    pub sequence: u64,
    pub issued: u64,
    pub expires: u64,
    pub revoked: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ClockSample {
    pub unix_seconds: u64,
    pub monotonic: Instant,
}

pub(super) struct PolicyEvidence<'a> {
    pub lease: SignedRecord<'a>,
    pub status: SignedRecord<'a>,
    // The caller must supply its last trusted sequence, including persisted state.
    pub seen_status_sequence: u64,
}

#[derive(Clone, Debug)]
pub(super) struct VerifiedPolicy {
    checked_at: ClockSample,
    expires: u64,
    deadline: Instant,
    pub status_sequence: u64,
}

impl VerifiedPolicy {
    pub fn check_time(&self, now: ClockSample) -> BridgeResult<()> {
        if !(self.checked_at.unix_seconds..self.expires).contains(&now.unix_seconds)
            || !(self.checked_at.monotonic..self.deadline).contains(&now.monotonic)
        {
            return Err("windowsForwardPolicyDenied".into());
        }
        Ok(())
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }
}

fn signed<T: DeserializeOwned>(
    key: &[u8; 32],
    domain: &[u8],
    record: SignedRecord<'_>,
) -> BridgeResult<T> {
    if record.payload.len() > MAX_PAYLOAD || record.signature.len() != 64 {
        return Err("windowsForwardPolicyDenied".into());
    }
    let message = [domain, record.payload].concat();
    UnparsedPublicKey::new(&ED25519, key)
        .verify(&message, record.signature)
        .map_err(|_| "windowsForwardPolicyDenied")?;
    serde_json::from_slice(record.payload).map_err(|_| "windowsForwardPolicyDenied".into())
}

pub(super) fn verify(
    issuer: &PolicyIssuer,
    scope: &LeaseScope,
    evidence: PolicyEvidence<'_>,
    review_expires: u64,
    now: ClockSample,
) -> BridgeResult<VerifiedPolicy> {
    let lease: Lease = signed(&issuer.key, LEASE_DOMAIN, evidence.lease)?;
    let status: Status = signed(&issuer.key, STATUS_DOMAIN, evidence.status)?;
    check_lease(&lease, scope, review_expires, now.unix_seconds)?;
    check_status(
        &status,
        &lease,
        evidence.seen_status_sequence,
        now.unix_seconds,
    )?;
    if status.revoked {
        return Err("windowsForwardPolicyDenied".into());
    }
    verified(&lease, &status, now)
}

fn check_lease(
    lease: &Lease,
    scope: &LeaseScope,
    review_expires: u64,
    current: u64,
) -> BridgeResult<()> {
    if lease.scope != *scope
        || scope.review.revision == 0
        || scope.review.deployment == 0
        || lease.gateway_ports != "no"
        || lease.changes_blocked_until < lease.expires
        || lease.expires > review_expires
        || !(lease.issued..lease.expires).contains(&current)
        || lease.expires - lease.issued > MAX_LEASE_SECONDS
    {
        return Err("windowsForwardPolicyDenied".into());
    }
    Ok(())
}

fn check_status(
    status: &Status,
    lease: &Lease,
    seen_status_sequence: u64,
    current: u64,
) -> BridgeResult<()> {
    if status.review != lease.scope.review
        || !(status.issued..status.expires).contains(&current)
        || status.expires - status.issued > MAX_STATUS_SECONDS
        || status.sequence == 0
        || status.sequence < seen_status_sequence.max(lease.minimum_status_sequence)
    {
        return Err("windowsForwardPolicyDenied".into());
    }
    Ok(())
}

fn verified(lease: &Lease, status: &Status, now: ClockSample) -> BridgeResult<VerifiedPolicy> {
    let expires = lease.expires.min(status.expires);
    let deadline = now
        .monotonic
        .checked_add(Duration::from_secs(expires - now.unix_seconds))
        .ok_or("windowsForwardPolicyDenied")?;
    Ok(VerifiedPolicy {
        checked_at: now,
        expires,
        deadline,
        status_sequence: status.sequence,
    })
}
