use super::{
    check_lease, check_status, signed, verified, ClockSample, Lease, LeaseScope, PolicyIssuer,
    ReviewScope, SignedRecord, Status, VerifiedPolicy, LEASE_DOMAIN, MAX_STATUS_SECONDS,
    STATUS_DOMAIN,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

pub(in super::super) const FRESHNESS_DOMAIN: &[u8] = b"RouteSpan/windows-forward-status-proof/v1\0";

#[derive(Clone, Debug, Deserialize, serde::Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct HistoryKey {
    pub issuer_key: [u8; 32],
    pub review: ReviewScope,
}

#[derive(Clone, Copy, Debug, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Decision {
    pub sequence: u64,
    pub digest: [u8; 32],
    pub revoked: bool,
}

impl Decision {
    fn from_status(status: &Status) -> Self {
        let mut digest = Sha256::new();
        digest.update(status.sequence.to_be_bytes());
        digest.update(status.issued.to_be_bytes());
        digest.update(status.expires.to_be_bytes());
        digest.update([u8::from(status.revoked)]);
        Self {
            sequence: status.sequence,
            digest: digest.finalize().into(),
            revoked: status.revoked,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum HistoryError {
    Missing,
    Unavailable,
    Corrupt,
    Conflict,
}

pub(in super::super) trait PolicyHistory {
    /// None represents a confirmed never-enrolled scope. Lost enrollment data is Missing.
    fn read(&self, key: &HistoryKey) -> Result<Option<Decision>, HistoryError>;

    /// Compare and commit under the same storage lock, including repeat reads of the same decision.
    /// Success confirms the decision and enrollment marker were committed together.
    fn commit(
        &self,
        key: &HistoryKey,
        expected: Option<Decision>,
        next: Decision,
    ) -> Result<(), HistoryError>;
}

pub(in super::super) struct StatusRequest<'a> {
    pub key: &'a HistoryKey,
    pub challenge: [u8; 32],
    pub lease_sha256: [u8; 32],
    pub deadline: Instant,
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
#[serde(deny_unknown_fields)]
pub(in super::super) struct FreshnessProof {
    pub challenge: [u8; 32],
    pub lease_sha256: [u8; 32],
    pub status_sha256: [u8; 32],
}

pub(in super::super) struct OwnedRecord {
    pub payload: Vec<u8>,
    pub signature: Vec<u8>,
}

impl OwnedRecord {
    fn borrow(&self) -> SignedRecord<'_> {
        SignedRecord {
            payload: &self.payload,
            signature: &self.signature,
        }
    }
}

pub(in super::super) struct StatusResponse {
    pub status: OwnedRecord,
    pub proof: OwnedRecord,
}

pub(in super::super) struct StatusUnavailable;

pub(in super::super) trait StatusSource {
    /// The independent authority signs a fresh proof for the exact lease and its latest status.
    /// The source must honor the request deadline and bound both records before returning.
    fn query(&mut self, request: &StatusRequest<'_>) -> Result<StatusResponse, StatusUnavailable>;
}

pub(in super::super) enum Enrollment {
    Existing,
    IndependentlyApproved,
}

#[derive(Debug)]
pub(in super::super) struct Grant {
    policy: VerifiedPolicy,
    decision: Decision,
}

#[derive(Debug)]
pub(in super::super) enum State {
    Unregistered,
    AwaitingVerification,
    Active(Grant),
    Revoked,
    Expired,
    Unconfirmed,
    StorageFault,
    Stopped,
}

pub(in super::super) struct Observation<'a> {
    pub scope: &'a LeaseScope,
    pub clock: ClockSample,
}

pub(in super::super) struct PolicyLifecycle<'a> {
    issuer: &'a PolicyIssuer,
    scope: LeaseScope,
    review_expires: u64,
    clock_anchor: Option<ClockSample>,
    last_clock: Option<ClockSample>,
    state: State,
}

impl<'a> PolicyLifecycle<'a> {
    pub fn new(
        issuer: &'a PolicyIssuer,
        scope: LeaseScope,
        review_expires: u64,
        enrollment: Enrollment,
    ) -> Self {
        Self {
            issuer,
            scope,
            review_expires,
            clock_anchor: None,
            last_clock: None,
            state: match enrollment {
                Enrollment::Existing => State::AwaitingVerification,
                Enrollment::IndependentlyApproved => State::Unregistered,
            },
        }
    }

    /// Reports the last decision. Call check_current before relying on an active decision.
    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn active_at(&self, now: ClockSample) -> bool {
        matches!(&self.state, State::Active(grant) if grant.policy.check_time(now).is_ok())
            && self.last_clock.is_none_or(|last| {
                now.unix_seconds >= last.unix_seconds && now.monotonic >= last.monotonic
            })
    }

    pub fn refresh<'scope>(
        &mut self,
        lease: SignedRecord<'_>,
        history: &impl PolicyHistory,
        source: &mut impl StatusSource,
        mut observe: impl FnMut() -> Observation<'scope>,
    ) {
        let enrolling = match self.state {
            State::Unregistered => true,
            State::AwaitingVerification | State::Active(_) => false,
            _ => return,
        };
        let previous = std::mem::replace(&mut self.state, State::AwaitingVerification);
        let previous = match &previous {
            State::Active(grant) => Some(grant),
            _ => None,
        };
        self.state =
            match self.refresh_grant(lease, history, source, &mut observe, enrolling, previous) {
                Ok(grant) => State::Active(grant),
                Err(state) => state,
            };
    }

    pub fn check_current<'scope>(
        &mut self,
        history: &impl PolicyHistory,
        mut observe: impl FnMut() -> Observation<'scope>,
    ) {
        self.state = match std::mem::replace(&mut self.state, State::AwaitingVerification) {
            State::Active(grant) => match self.check_grant(&grant, history, &mut observe) {
                Ok(()) => State::Active(grant),
                Err(state) => state,
            },
            closed => closed,
        };
    }

    pub fn stop(&mut self) {
        self.state = State::Stopped;
    }

    fn key(&self) -> HistoryKey {
        HistoryKey {
            issuer_key: self.issuer.key,
            review: self.scope.review.clone(),
        }
    }

    fn observe(
        &mut self,
        observation: Observation<'_>,
        previous: Option<&Grant>,
    ) -> Result<ClockSample, State> {
        let now = observation.clock;
        if *observation.scope != self.scope
            || self.last_clock.is_some_and(|last| {
                now.unix_seconds < last.unix_seconds || now.monotonic < last.monotonic
            })
        {
            return Err(State::Unconfirmed);
        }
        if let Some(grant) = previous {
            check_deadline(&grant.policy, now)?;
        }
        self.last_clock = Some(now);
        Ok(now)
    }

    fn check_grant<'scope>(
        &mut self,
        grant: &Grant,
        history: &impl PolicyHistory,
        observe: &mut impl FnMut() -> Observation<'scope>,
    ) -> Result<(), State> {
        self.observe(observe(), Some(grant))?;
        let decision = read_history(history, &self.key())?.ok_or(State::StorageFault)?;
        if decision.revoked {
            return Err(State::Revoked);
        }
        if decision != grant.decision {
            return Err(State::Unconfirmed);
        }
        self.observe(observe(), Some(grant))?;
        Ok(())
    }

    fn refresh_grant<'scope>(
        &mut self,
        lease_record: SignedRecord<'_>,
        history: &impl PolicyHistory,
        source: &mut impl StatusSource,
        observe: &mut impl FnMut() -> Observation<'scope>,
        enrolling: bool,
        previous: Option<&Grant>,
    ) -> Result<Grant, State> {
        let start = self.observe(observe(), previous)?;
        let anchor = *self.clock_anchor.get_or_insert(start);
        let key = self.key();
        if (key.review.revision, key.review.deployment)
            != (self.issuer.revision, self.issuer.deployment)
        {
            return Err(State::Unconfirmed);
        }
        let saved = read_history(history, &key)?;
        if saved.is_some_and(|decision| decision.revoked) {
            return Err(State::Revoked);
        }
        if enrolling != saved.is_none() {
            return Err(if enrolling {
                State::Unconfirmed
            } else {
                State::StorageFault
            });
        }
        if previous.is_some_and(|grant| saved != Some(grant.decision)) {
            return Err(State::Unconfirmed);
        }

        let lease: Lease =
            signed(&self.issuer.key, LEASE_DOMAIN, lease_record).map_err(|_| State::Unconfirmed)?;
        check_lease(&lease, &self.scope, self.review_expires, start.unix_seconds)
            .map_err(|_| State::Unconfirmed)?;
        let mut challenge = [0; 32];
        getrandom::fill(&mut challenge).map_err(|_| State::Unconfirmed)?;
        let query_seconds = (lease.expires - start.unix_seconds).min(MAX_STATUS_SECONDS);
        let mut deadline = start
            .monotonic
            .checked_add(Duration::from_secs(query_seconds))
            .ok_or(State::Unconfirmed)?;
        if let Some(grant) = previous {
            deadline = deadline.min(grant.policy.deadline());
        }
        let request = StatusRequest {
            key: &key,
            challenge,
            lease_sha256: Sha256::digest(lease_record.payload).into(),
            deadline,
        };
        let response = source.query(&request).map_err(|_| State::Unconfirmed)?;
        let received = self.observe(observe(), previous)?;
        if received.monotonic >= request.deadline {
            return Err(State::Unconfirmed);
        }

        let status = validate_response(self.issuer, &request, &response)?;
        check_lease(
            &lease,
            &self.scope,
            self.review_expires,
            received.unix_seconds,
        )
        .map_err(|_| State::Unconfirmed)?;
        check_status(
            &status,
            &lease,
            saved.map_or(0, |decision| decision.sequence),
            received.unix_seconds,
        )
        .map_err(|_| State::Unconfirmed)?;
        let decision = Decision::from_status(&status);
        if saved.is_some_and(|old| old.sequence == decision.sequence && old != decision) {
            return Err(State::Unconfirmed);
        }
        history
            .commit(&key, saved, decision)
            .map_err(|_| State::StorageFault)?;
        if decision.revoked {
            return Err(State::Revoked);
        }

        let mut policy = verified(&lease, &status, received).map_err(|_| State::Unconfirmed)?;
        // A delayed wall clock must not move a signed expiry later on the monotonic clock.
        let anchored_deadline = anchor
            .monotonic
            .checked_add(Duration::from_secs(policy.expires - anchor.unix_seconds))
            .ok_or(State::Unconfirmed)?;
        policy.deadline = policy.deadline.min(anchored_deadline);
        if let Some(grant) = previous.filter(|grant| grant.decision == decision) {
            policy.deadline = policy.deadline.min(grant.policy.deadline());
        }
        let complete = self.observe(observe(), previous)?;
        check_deadline(&policy, complete)?;
        Ok(Grant { policy, decision })
    }
}

fn read_history(history: &impl PolicyHistory, key: &HistoryKey) -> Result<Option<Decision>, State> {
    let saved = history.read(key).map_err(|_| State::StorageFault)?;
    if saved.is_some_and(|decision| decision.sequence == 0) {
        return Err(State::StorageFault);
    }
    Ok(saved)
}

fn check_deadline(policy: &VerifiedPolicy, now: ClockSample) -> Result<(), State> {
    if now.unix_seconds >= policy.expires || now.monotonic >= policy.deadline() {
        return Err(State::Expired);
    }
    policy.check_time(now).map_err(|_| State::Unconfirmed)
}

fn validate_response(
    issuer: &PolicyIssuer,
    request: &StatusRequest<'_>,
    response: &StatusResponse,
) -> Result<Status, State> {
    let status = signed(&issuer.key, STATUS_DOMAIN, response.status.borrow())
        .map_err(|_| State::Unconfirmed)?;
    let proof: FreshnessProof = signed(&issuer.key, FRESHNESS_DOMAIN, response.proof.borrow())
        .map_err(|_| State::Unconfirmed)?;
    if proof.challenge != request.challenge
        || proof.lease_sha256 != request.lease_sha256
        || proof.status_sha256 != <[u8; 32]>::from(Sha256::digest(&response.status.payload))
    {
        return Err(State::Unconfirmed);
    }
    Ok(status)
}
