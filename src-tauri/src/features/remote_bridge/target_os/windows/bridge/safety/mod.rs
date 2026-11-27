mod frozen;
#[cfg(windows)]
mod frozen_resources;
#[cfg(windows)]
mod local_files;
#[cfg(windows)]
mod owned_process;
mod policy;
#[cfg(windows)]
mod policy_storage;
mod runtime;

#[cfg(test)]
mod tests;

use super::{WindowsBridgeInput, WindowsBridgePlan};
use crate::features::{proxy::ProxyEndpoint, remote_bridge::BridgeResult};
use frozen::{FrozenFiles, FrozenProfile, HostTrustEvidence};
use policy::{ClockSample, LeaseScope, PolicyEvidence, ReviewScope, VerifiedPolicy};
use serde::Deserialize;
use std::{
    net::{IpAddr, Ipv4Addr},
    time::{Duration, Instant},
};

pub(super) use policy::PolicyIssuer;

const VERIFY_TIME: Duration = Duration::from_secs(15);
const MAX_LISTENER_REPORT: usize = 4096;
const FRAME_PREFIX: &[u8] = b"ROUTESPAN-FORWARD/1\0";

struct Preparation<'a> {
    current: WindowsBridgeInput<'a>,
    now: ClockSample,
    directory: &'a str,
    host_trust: HostTrustEvidence<'a>,
    policy: Option<PolicyEvidence<'a>>,
}

struct PreparedForward {
    scope: LeaseScope,
    upstream: ProxyEndpoint,
    proxy_revision: u64,
    ingress_port: u16,
    profile: FrozenProfile,
    args: [String; 7],
    policy: VerifiedPolicy,
}

fn plan_lease_scope(plan: &WindowsBridgePlan<'_>, issuer: &PolicyIssuer) -> LeaseScope {
    let (host, account, ssh_port) = plan.input.identity;
    LeaseScope {
        review: ReviewScope {
            review: plan.policy.review_id.into(),
            revision: issuer.revision,
            deployment: issuer.deployment,
        },
        host: host.into(),
        account: account.into(),
        ssh_port,
        host_key_sha256: plan.input.host_key_sha256,
        config_fingerprint: plan.input.config_fingerprint.into(),
        generation: plan.input.generation,
        remote_port: plan.remote_port,
    }
}

impl PreparedForward {
    fn prepare(plan: &WindowsBridgePlan<'_>, input: Preparation<'_>) -> BridgeResult<Self> {
        // Retargeting, proxy changes, and the absence of an authenticated identity
        // fail in the existing plan. The offline layer cannot mint an identity.
        let forward = plan.forward_args(input.current, input.now.unix_seconds)?;
        let issuer = plan
            .policy
            .issuer
            .as_ref()
            .ok_or("windowsForwardPolicyUnreviewed")?;
        let scope = plan_lease_scope(plan, issuer);
        let policy = policy::verify(
            issuer,
            &scope,
            input.policy.ok_or("windowsForwardPolicyDenied")?,
            plan.policy.expires_at,
            input.now,
        )?;
        let profile = FrozenProfile::build(
            plan.input.identity,
            scope.host_key_sha256,
            input.host_trust,
            input.directory,
        )?;
        let args = profile.forward_args(&forward);
        Ok(Self {
            scope,
            upstream: plan.input.upstream.clone(),
            proxy_revision: plan.input.proxy_revision,
            ingress_port: plan.ingress_port,
            profile,
            args,
            policy,
        })
    }

    fn check_launch(
        &self,
        files: FrozenFiles<'_>,
        args: &[String],
        now: ClockSample,
    ) -> BridgeResult<()> {
        self.policy.check_time(now)?;
        self.profile.check_files(files)?;
        if args != self.args {
            return Err("sshConfigUnsafe".into());
        }
        Ok(())
    }

    fn check_current(&self, current: &Self, now: ClockSample) -> BridgeResult<()> {
        self.policy.check_time(now)?;
        current.policy.check_time(now)?;
        if self.scope != current.scope
            || self.upstream != current.upstream
            || self.proxy_revision != current.proxy_revision
            || self.args != current.args
            || current.policy.status_sequence < self.policy.status_sequence
        {
            return Err("windowsBridgeContextChanged".into());
        }
        self.profile.check_files(FrozenFiles {
            config: current.profile.config.as_bytes(),
            known_hosts: current.profile.known_hosts.as_bytes(),
            revoked_hosts: current.profile.revoked_hosts.as_bytes(),
        })
    }
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
#[serde(deny_unknown_fields)]
struct ListenerReport {
    version: u8,
    nonce: String,
    generation: u64,
    port: u16,
    ipv4_complete: bool,
    ipv6_complete: bool,
    addresses: Vec<IpAddr>,
}

#[derive(PartialEq, Eq)]
enum Phase {
    Listening,
    AwaitingReceipt,
    Verified,
    Closed,
}

// R is the pending attempt's owned resources. It stays here until every check
// passes; failure or cancellation drops it and permanently closes this attempt.
struct Verification<R> {
    prepared: PreparedForward,
    resources: Option<R>,
    listener_nonce: [u8; 16],
    receipt_nonce: [u8; 16],
    deadline: Instant,
    phase: Phase,
}

impl<R> Verification<R> {
    fn new(
        prepared: PreparedForward,
        files: FrozenFiles<'_>,
        args: &[String],
        resources: R,
        now: ClockSample,
    ) -> BridgeResult<Self> {
        prepared.check_launch(files, args, now)?;
        let mut listener_nonce = [0; 16];
        let mut receipt_nonce = [0; 16];
        getrandom::fill(&mut listener_nonce).map_err(|_| "stateUnavailable")?;
        getrandom::fill(&mut receipt_nonce).map_err(|_| "stateUnavailable")?;
        let deadline = now
            .monotonic
            .checked_add(VERIFY_TIME)
            .ok_or("sshTimeout")?
            .min(prepared.policy.deadline());
        Ok(Self {
            prepared,
            resources: Some(resources),
            listener_nonce,
            receipt_nonce,
            deadline,
            phase: Phase::Listening,
        })
    }

    fn check_time(&self, now: ClockSample) -> BridgeResult<()> {
        if now.monotonic >= self.deadline || self.phase == Phase::Closed {
            return Err("windowsForwardVerificationClosed".into());
        }
        self.prepared.policy.check_time(now)
    }

    fn cancel(&mut self) {
        self.phase = Phase::Closed;
        drop(self.resources.take());
    }

    fn listener_report(&mut self, bytes: &[u8], now: ClockSample) -> BridgeResult<()> {
        let check = || -> BridgeResult<()> {
            self.check_time(now)?;
            if self.phase != Phase::Listening || bytes.len() > MAX_LISTENER_REPORT {
                return Err("unsafeBinding".into());
            }
            let reply: ListenerReport =
                serde_json::from_slice(bytes).map_err(|_| "unsafeBinding")?;
            if reply.version != 1
                || reply.nonce != hex::encode(self.listener_nonce)
                || reply.generation != self.prepared.scope.generation
                || reply.port != self.prepared.scope.remote_port
                || !(reply.ipv4_complete && reply.ipv6_complete)
                || reply.addresses != [IpAddr::V4(Ipv4Addr::LOCALHOST)]
            {
                return Err("unsafeBinding".into());
            }
            Ok(())
        };
        let result = check();
        if result.is_ok() {
            self.phase = Phase::AwaitingReceipt;
        } else {
            self.cancel();
        }
        result
    }

    fn expected_receipt(&self) -> Vec<u8> {
        [
            FRAME_PREFIX,
            &self.prepared.scope.generation.to_be_bytes(),
            &self.prepared.scope.remote_port.to_be_bytes(),
            &self.prepared.ingress_port.to_be_bytes(),
            &self.receipt_nonce,
        ]
        .concat()
    }

    fn receipt(&mut self, bytes: &[u8], eof: bool, now: ClockSample) -> BridgeResult<()> {
        let checked = self.check_time(now).and_then(|()| {
            if self.phase == Phase::AwaitingReceipt && eof && bytes == self.expected_receipt() {
                Ok(())
            } else {
                Err("windowsForwardReceiptInvalid".into())
            }
        });
        if checked.is_ok() {
            self.phase = Phase::Verified;
        } else {
            self.cancel();
        }
        checked
    }

    fn finish(
        mut self,
        current: &PreparedForward,
        files: FrozenFiles<'_>,
        args: &[String],
        now: ClockSample,
    ) -> BridgeResult<R> {
        self.check_time(now)?;
        if self.phase != Phase::Verified {
            return Err("windowsForwardReceiptInvalid".into());
        }
        self.prepared.check_current(current, now)?;
        current.check_launch(files, args, now)?;
        self.resources
            .take()
            .ok_or_else(|| "windowsForwardVerificationClosed".into())
    }
}
