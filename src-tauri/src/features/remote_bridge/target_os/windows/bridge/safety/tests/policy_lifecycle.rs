use super::{policy::lifecycle::*, *};

#[cfg(windows)]
pub(super) fn drive_local_policy_stop(revoked: bool, close: impl FnOnce()) {
    let fixture = Fixture::new();
    let history = MemoryHistory::default();
    let lease = fixture.lease();
    let mut lifecycle = enroll(&fixture, &lease, &history);
    if revoked {
        let status = Status {
            sequence: 10,
            revoked: true,
            ..Fixture::status()
        };
        refresh(
            &mut lifecycle,
            &fixture,
            &lease,
            &history,
            &mut source(&fixture, status),
            fixture.clock(1),
        );
        assert!(matches!(lifecycle.state(), State::Revoked));
    } else {
        lifecycle.check_current(&history, || observe(&lease.scope, fixture.clock(30)));
        assert!(matches!(lifecycle.state(), State::Expired));
    }
    close();
}
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

#[derive(Default)]
struct MemoryHistory {
    entries: Mutex<HashMap<HistoryKey, Result<Decision, HistoryError>>>,
    fail_write: AtomicBool,
}

impl PolicyHistory for MemoryHistory {
    fn read(&self, key: &HistoryKey) -> Result<Option<Decision>, HistoryError> {
        self.entries.lock().unwrap().get(key).copied().transpose()
    }

    fn commit(
        &self,
        key: &HistoryKey,
        expected: Option<Decision>,
        next: Decision,
    ) -> Result<(), HistoryError> {
        let mut entries = self.entries.lock().unwrap();
        if entries.get(key).copied().transpose()? != expected {
            return Err(HistoryError::Conflict);
        }
        if self.fail_write.load(Ordering::Relaxed) {
            return Err(HistoryError::Unavailable);
        }
        entries.insert(key.clone(), Ok(next));
        Ok(())
    }
}

struct Source<F>(F);

impl<F> StatusSource for Source<F>
where
    F: FnMut(&StatusRequest<'_>) -> Result<StatusResponse, StatusUnavailable>,
{
    fn query(&mut self, request: &StatusRequest<'_>) -> Result<StatusResponse, StatusUnavailable> {
        (self.0)(request)
    }
}

fn issuer(f: &Fixture) -> &PolicyIssuer {
    f.review[0].issuer.as_ref().unwrap()
}

fn key(f: &Fixture, lease: &Lease) -> HistoryKey {
    HistoryKey {
        issuer_key: issuer(f).key,
        review: lease.scope.review.clone(),
    }
}

fn session<'a>(f: &'a Fixture, lease: &Lease, enrollment: Enrollment) -> PolicyLifecycle<'a> {
    PolicyLifecycle::new(
        issuer(f),
        lease.scope.clone(),
        f.review[0].expires_at,
        enrollment,
    )
}

fn owned(signed: Signed) -> OwnedRecord {
    OwnedRecord {
        payload: signed.payload,
        signature: signed.signature,
    }
}

fn response(f: &Fixture, request: &StatusRequest<'_>, status: Signed) -> StatusResponse {
    StatusResponse {
        proof: owned(f.sign(
            FRESHNESS_DOMAIN,
            FreshnessProof {
                challenge: request.challenge,
                lease_sha256: request.lease_sha256,
                status_sha256: Sha256::digest(&status.payload).into(),
            },
        )),
        status: owned(status),
    }
}

pub(super) fn source(f: &Fixture, status: Status) -> impl StatusSource + '_ {
    Source(move |request: &StatusRequest<'_>| {
        assert_eq!(request.key.issuer_key, issuer(f).key);
        assert_eq!(request.key.review, status.review);
        Ok(response(f, request, f.sign(STATUS_DOMAIN, &status)))
    })
}

fn unavailable(_: &StatusRequest<'_>) -> Result<StatusResponse, StatusUnavailable> {
    Err(StatusUnavailable)
}

fn unexpected(_: &StatusRequest<'_>) -> Result<StatusResponse, StatusUnavailable> {
    panic!("status queried after a terminal decision")
}

fn observe(scope: &LeaseScope, clock: ClockSample) -> Observation<'_> {
    Observation { scope, clock }
}

fn refresh(
    lifecycle: &mut PolicyLifecycle<'_>,
    f: &Fixture,
    lease: &Lease,
    history: &MemoryHistory,
    source: &mut impl StatusSource,
    clock: ClockSample,
) {
    lifecycle.refresh(
        f.sign(LEASE_DOMAIN, lease).record(),
        history,
        source,
        || observe(&lease.scope, clock),
    );
}

fn enroll<'a>(f: &'a Fixture, lease: &Lease, history: &MemoryHistory) -> PolicyLifecycle<'a> {
    let mut lifecycle = session(f, lease, Enrollment::IndependentlyApproved);
    refresh(
        &mut lifecycle,
        f,
        lease,
        history,
        &mut source(f, Fixture::status()),
        f.clock(0),
    );
    assert!(
        matches!(lifecycle.state(), State::Active(_)),
        "{:?}",
        lifecycle.state()
    );
    lifecycle
}

#[test]
fn policy_lifecycle_requires_explicit_enrollment_and_commit() {
    let f = Fixture::new();
    let lease = f.lease();
    let history = MemoryHistory::default();
    let mut existing = session(&f, &lease, Enrollment::Existing);
    assert!(matches!(existing.state(), State::AwaitingVerification));
    refresh(
        &mut existing,
        &f,
        &lease,
        &history,
        &mut Source(unexpected),
        f.clock(0),
    );
    assert!(matches!(existing.state(), State::StorageFault));

    let mut approved = session(&f, &lease, Enrollment::IndependentlyApproved);
    assert!(matches!(approved.state(), State::Unregistered));
    history.fail_write.store(true, Ordering::Relaxed);
    refresh(
        &mut approved,
        &f,
        &lease,
        &history,
        &mut source(&f, Fixture::status()),
        f.clock(0),
    );
    assert!(matches!(approved.state(), State::StorageFault));
    assert_eq!(history.read(&key(&f, &lease)), Ok(None));
    history.fail_write.store(false, Ordering::Relaxed);
    enroll(&f, &lease, &history);
    assert_eq!(history.read(&key(&f, &lease)).unwrap().unwrap().sequence, 9);

    let mut duplicate = session(&f, &lease, Enrollment::IndependentlyApproved);
    refresh(
        &mut duplicate,
        &f,
        &lease,
        &history,
        &mut Source(unexpected),
        f.clock(0),
    );
    assert!(matches!(duplicate.state(), State::Unconfirmed));
}

#[test]
fn policy_lifecycle_rechecks_same_decision_and_rejects_conflicts_and_rollback() {
    let f = Fixture::new();
    let lease = f.lease();
    let history = MemoryHistory::default();
    let mut lifecycle = enroll(&f, &lease, &history);
    let original = history.read(&key(&f, &lease)).unwrap().unwrap();
    let mut differently_encoded = Source(|request: &StatusRequest<'_>| {
        let json = serde_json::to_vec_pretty(&Fixture::status()).unwrap();
        Ok(response(&f, request, f.sign_bytes(STATUS_DOMAIN, json)))
    });
    refresh(
        &mut lifecycle,
        &f,
        &lease,
        &history,
        &mut differently_encoded,
        f.clock(1),
    );
    assert!(matches!(lifecycle.state(), State::Active(_)));
    assert_eq!(history.read(&key(&f, &lease)), Ok(Some(original)));

    let next = Status {
        sequence: 10,
        ..Fixture::status()
    };
    refresh(
        &mut lifecycle,
        &f,
        &lease,
        &history,
        &mut source(&f, next),
        f.clock(2),
    );
    assert!(matches!(lifecycle.state(), State::Active(_)));
    let stored = history.read(&key(&f, &lease)).unwrap().unwrap();
    assert_eq!(stored.sequence, 10);
    for status in [
        Fixture::status(),
        Status {
            sequence: 10,
            revoked: true,
            ..Fixture::status()
        },
        Status {
            sequence: 10,
            expires: 181,
            ..Fixture::status()
        },
    ] {
        let mut again = session(&f, &lease, Enrollment::Existing);
        refresh(
            &mut again,
            &f,
            &lease,
            &history,
            &mut source(&f, status),
            f.clock(3),
        );
        assert!(matches!(again.state(), State::Unconfirmed));
        assert_eq!(history.read(&key(&f, &lease)), Ok(Some(stored)));
    }
}

#[test]
fn policy_lifecycle_persists_revocation_across_instances() {
    let f = Fixture::new();
    let lease = f.lease();
    let history = MemoryHistory::default();
    let mut active = enroll(&f, &lease, &history);
    let mut verifier = session(&f, &lease, Enrollment::Existing);
    let revoked = Status {
        sequence: 10,
        revoked: true,
        ..Fixture::status()
    };
    refresh(
        &mut verifier,
        &f,
        &lease,
        &history,
        &mut source(&f, revoked),
        f.clock(1),
    );
    assert!(matches!(verifier.state(), State::Revoked));
    assert!(history.read(&key(&f, &lease)).unwrap().unwrap().revoked);
    active.check_current(&history, || observe(&lease.scope, f.clock(2)));
    assert!(matches!(active.state(), State::Revoked));

    for mode in [Enrollment::Existing, Enrollment::IndependentlyApproved] {
        let mut restarted = session(&f, &lease, mode);
        refresh(
            &mut restarted,
            &f,
            &lease,
            &history,
            &mut Source(unexpected),
            f.clock(2),
        );
        assert!(matches!(restarted.state(), State::Revoked));
    }
}

#[test]
fn policy_lifecycle_rejects_missing_corrupt_reads_and_write_failure() {
    let f = Fixture::new();
    let lease = f.lease();
    let history = MemoryHistory::default();
    enroll(&f, &lease, &history);
    let original = history.read(&key(&f, &lease)).unwrap();
    for revoked in [false, true] {
        history.fail_write.store(false, Ordering::Relaxed);
        let mut active = session(&f, &lease, Enrollment::Existing);
        refresh(
            &mut active,
            &f,
            &lease,
            &history,
            &mut source(&f, Fixture::status()),
            f.clock(0),
        );
        assert!(matches!(active.state(), State::Active(_)));
        history.fail_write.store(true, Ordering::Relaxed);
        let next = Status {
            sequence: 10,
            revoked,
            ..Fixture::status()
        };
        refresh(
            &mut active,
            &f,
            &lease,
            &history,
            &mut source(&f, next),
            f.clock(1),
        );
        assert!(matches!(active.state(), State::StorageFault));
        assert_eq!(history.read(&key(&f, &lease)), Ok(original));
    }

    for error in [
        HistoryError::Missing,
        HistoryError::Corrupt,
        HistoryError::Unavailable,
    ] {
        history
            .entries
            .lock()
            .unwrap()
            .insert(key(&f, &lease), Err(error));
        for mode in [Enrollment::Existing, Enrollment::IndependentlyApproved] {
            let mut lifecycle = session(&f, &lease, mode);
            refresh(
                &mut lifecycle,
                &f,
                &lease,
                &history,
                &mut Source(unexpected),
                f.clock(2),
            );
            assert!(matches!(lifecycle.state(), State::StorageFault));
        }
    }
    history.fail_write.store(false, Ordering::Relaxed);
    history
        .entries
        .lock()
        .unwrap()
        .insert(key(&f, &lease), Ok(original.unwrap()));
    let mut active = session(&f, &lease, Enrollment::Existing);
    refresh(
        &mut active,
        &f,
        &lease,
        &history,
        &mut source(&f, Fixture::status()),
        f.clock(2),
    );
    assert!(matches!(active.state(), State::Active(_)));
    history.entries.lock().unwrap().remove(&key(&f, &lease));
    active.check_current(&history, || observe(&lease.scope, f.clock(3)));
    assert!(matches!(active.state(), State::StorageFault));
}

#[test]
fn policy_lifecycle_concurrent_commit_cannot_overwrite_revocation() {
    let f = Fixture::new();
    let lease = f.lease();
    let history = MemoryHistory::default();
    let mut stale = enroll(&f, &lease, &history);
    let mut other = session(&f, &lease, Enrollment::Existing);
    let mut interleaved = Source(|request: &StatusRequest<'_>| {
        let revoke = Status {
            sequence: 11,
            revoked: true,
            ..Fixture::status()
        };
        refresh(
            &mut other,
            &f,
            &lease,
            &history,
            &mut source(&f, revoke),
            f.clock(1),
        );
        assert!(matches!(other.state(), State::Revoked));
        let stale = Status {
            sequence: 10,
            ..Fixture::status()
        };
        Ok(response(&f, request, f.sign(STATUS_DOMAIN, stale)))
    });
    refresh(
        &mut stale,
        &f,
        &lease,
        &history,
        &mut interleaved,
        f.clock(1),
    );
    assert!(matches!(stale.state(), State::StorageFault));
    let saved = history.read(&key(&f, &lease)).unwrap().unwrap();
    assert_eq!((saved.sequence, saved.revoked), (11, true));
}

#[test]
fn policy_lifecycle_isolates_scope_and_session_generation() {
    let f = Fixture::new();
    let lease = f.lease();
    let history = MemoryHistory::default();
    let mut active = enroll(&f, &lease, &history);
    let mut other = f.lease();
    other.scope.review.review = "isolated-review".into();
    let status = Status {
        review: other.scope.review.clone(),
        sequence: 99,
        revoked: true,
        ..Fixture::status()
    };
    let mut other_session = session(&f, &other, Enrollment::IndependentlyApproved);
    refresh(
        &mut other_session,
        &f,
        &other,
        &history,
        &mut source(&f, status),
        f.clock(0),
    );
    assert!(matches!(other_session.state(), State::Revoked));
    active.check_current(&history, || observe(&lease.scope, f.clock(1)));
    assert!(matches!(active.state(), State::Active(_)));

    let mut new_generation = f.lease();
    new_generation.scope.generation += 1;
    let mut next = session(&f, &new_generation, Enrollment::Existing);
    refresh(
        &mut next,
        &f,
        &new_generation,
        &history,
        &mut source(&f, Fixture::status()),
        f.clock(1),
    );
    assert!(matches!(next.state(), State::Active(_)));
    active.check_current(&history, || observe(&new_generation.scope, f.clock(1)));
    assert!(matches!(active.state(), State::Unconfirmed));
    next.check_current(&history, || observe(&new_generation.scope, f.clock(1)));
    assert!(matches!(next.state(), State::Active(_)));

    let mut new_revision = f.lease();
    new_revision.scope.review.revision += 1;
    let mut invalid = session(&f, &new_revision, Enrollment::IndependentlyApproved);
    refresh(
        &mut invalid,
        &f,
        &new_revision,
        &history,
        &mut Source(unexpected),
        f.clock(1),
    );
    assert!(matches!(invalid.state(), State::Unconfirmed));
}

#[test]
fn policy_lifecycle_rejects_replayed_or_mismatched_freshness_proofs() {
    let f = Fixture::new();
    let lease = f.lease();
    let history = MemoryHistory::default();
    let mut lifecycle = session(&f, &lease, Enrollment::IndependentlyApproved);
    let mut replay = None;
    let mut capture = Source(|request: &StatusRequest<'_>| {
        replay = Some(response(
            &f,
            request,
            f.sign(STATUS_DOMAIN, Fixture::status()),
        ));
        Ok(response(
            &f,
            request,
            f.sign(STATUS_DOMAIN, Fixture::status()),
        ))
    });
    refresh(
        &mut lifecycle,
        &f,
        &lease,
        &history,
        &mut capture,
        f.clock(0),
    );
    assert!(matches!(lifecycle.state(), State::Active(_)));
    let mut replayed = Source(|_: &StatusRequest<'_>| Ok(replay.take().unwrap()));
    refresh(
        &mut lifecycle,
        &f,
        &lease,
        &history,
        &mut replayed,
        f.clock(1),
    );
    assert!(matches!(lifecycle.state(), State::Unconfirmed));

    for alter in [
        |reply: &mut StatusResponse| reply.proof.signature[0] ^= 1,
        |reply: &mut StatusResponse| reply.status.signature[0] ^= 1,
    ] {
        let mut bad = Source(|request: &StatusRequest<'_>| {
            let mut reply = response(&f, request, f.sign(STATUS_DOMAIN, Fixture::status()));
            alter(&mut reply);
            Ok(reply)
        });
        let mut lifecycle = session(&f, &lease, Enrollment::Existing);
        refresh(&mut lifecycle, &f, &lease, &history, &mut bad, f.clock(1));
        assert!(matches!(lifecycle.state(), State::Unconfirmed));
    }
    for alter in [
        |proof: &mut FreshnessProof| proof.lease_sha256[0] ^= 1,
        |proof: &mut FreshnessProof| proof.status_sha256[0] ^= 1,
    ] {
        let mut bad = Source(|request: &StatusRequest<'_>| {
            let mut reply = response(&f, request, f.sign(STATUS_DOMAIN, Fixture::status()));
            let mut proof: FreshnessProof = serde_json::from_slice(&reply.proof.payload).unwrap();
            alter(&mut proof);
            reply.proof = owned(f.sign(FRESHNESS_DOMAIN, proof));
            Ok(reply)
        });
        let mut lifecycle = session(&f, &lease, Enrollment::Existing);
        refresh(&mut lifecycle, &f, &lease, &history, &mut bad, f.clock(1));
        assert!(matches!(lifecycle.state(), State::Unconfirmed));
    }
    let mut other_issuer = Source(|request: &StatusRequest<'_>| {
        let mut reply = response(&f, request, f.sign(STATUS_DOMAIN, Fixture::status()));
        let other = Ed25519KeyPair::from_seed_unchecked(&[72; 32]).unwrap();
        let message = [FRESHNESS_DOMAIN, reply.proof.payload.as_slice()].concat();
        reply.proof.signature = other.sign(&message).as_ref().to_vec();
        Ok(reply)
    });
    let mut lifecycle = session(&f, &lease, Enrollment::Existing);
    refresh(
        &mut lifecycle,
        &f,
        &lease,
        &history,
        &mut other_issuer,
        f.clock(1),
    );
    assert!(matches!(lifecycle.state(), State::Unconfirmed));
}

#[test]
fn policy_lifecycle_rejects_unavailable_expired_and_late_authority() {
    let f = Fixture::new();
    let lease = f.lease();
    let history = MemoryHistory::default();
    let mut lifecycle = enroll(&f, &lease, &history);
    refresh(
        &mut lifecycle,
        &f,
        &lease,
        &history,
        &mut Source(unavailable),
        f.clock(1),
    );
    assert!(matches!(lifecycle.state(), State::Unconfirmed));
    refresh(
        &mut lifecycle,
        &f,
        &lease,
        &history,
        &mut Source(unexpected),
        f.clock(2),
    );
    assert!(matches!(lifecycle.state(), State::Unconfirmed));

    let mut lifecycle = session(&f, &lease, Enrollment::Existing);
    refresh(
        &mut lifecycle,
        &f,
        &lease,
        &history,
        &mut source(&f, Fixture::status()),
        f.clock(30),
    );
    assert!(matches!(lifecycle.state(), State::Unconfirmed));

    let mut lifecycle = session(&f, &lease, Enrollment::Existing);
    let now = Cell::new(f.clock(0));
    let mut late = Source(|request: &StatusRequest<'_>| {
        now.set(ClockSample {
            monotonic: request.deadline,
            ..f.clock(0)
        });
        Ok(response(
            &f,
            request,
            f.sign(STATUS_DOMAIN, Fixture::status()),
        ))
    });
    lifecycle.refresh(
        f.sign(LEASE_DOMAIN, &lease).record(),
        &history,
        &mut late,
        || observe(&lease.scope, now.get()),
    );
    assert!(matches!(lifecycle.state(), State::Unconfirmed));
}

#[test]
fn policy_lifecycle_expires_on_both_clocks_without_extending_same_decision() {
    let f = Fixture::new();
    let lease = f.lease();
    for at_expiry in [
        ClockSample {
            monotonic: f.clock(29).monotonic,
            ..f.clock(30)
        },
        ClockSample {
            unix_seconds: 150,
            ..f.clock(30)
        },
    ] {
        let history = MemoryHistory::default();
        let mut lifecycle = enroll(&f, &lease, &history);
        let slow_wall = ClockSample {
            unix_seconds: 150,
            ..f.clock(20)
        };
        refresh(
            &mut lifecycle,
            &f,
            &lease,
            &history,
            &mut source(&f, Fixture::status()),
            slow_wall,
        );
        assert!(matches!(lifecycle.state(), State::Active(_)));
        lifecycle.check_current(&history, || observe(&lease.scope, slow_wall));
        assert!(matches!(lifecycle.state(), State::Active(_)));
        lifecycle.check_current(&history, || observe(&lease.scope, at_expiry));
        assert!(matches!(lifecycle.state(), State::Expired));
        refresh(
            &mut lifecycle,
            &f,
            &lease,
            &history,
            &mut Source(unexpected),
            f.clock(31),
        );
        assert!(matches!(lifecycle.state(), State::Expired));
        lifecycle.stop();
        lifecycle.stop();
        lifecycle.check_current(&history, || panic!("stopped session sampled"));
        assert!(matches!(lifecycle.state(), State::Stopped));
    }

    let shorter_lease = Lease {
        expires: 175,
        ..f.lease()
    };
    let history = MemoryHistory::default();
    let mut lifecycle = enroll(&f, &shorter_lease, &history);
    let next = Status {
        sequence: 10,
        expires: 181,
        ..Fixture::status()
    };
    let stalled_wall = |seconds| ClockSample {
        unix_seconds: 150,
        ..f.clock(seconds)
    };
    refresh(
        &mut lifecycle,
        &f,
        &shorter_lease,
        &history,
        &mut source(&f, next),
        stalled_wall(20),
    );
    assert!(matches!(lifecycle.state(), State::Active(_)));
    lifecycle.check_current(&history, || observe(&shorter_lease.scope, stalled_wall(25)));
    assert!(matches!(lifecycle.state(), State::Expired));
}

#[test]
fn policy_lifecycle_rechecks_clocks_and_scope_around_authority_and_commit() {
    let f = Fixture::new();
    let lease = f.lease();
    for rolled_back in [
        ClockSample {
            monotonic: f.clock(2).monotonic,
            ..f.clock(0)
        },
        ClockSample {
            monotonic: f.clock(0).monotonic,
            ..f.clock(2)
        },
    ] {
        let history = MemoryHistory::default();
        let mut lifecycle = enroll(&f, &lease, &history);
        lifecycle.check_current(&history, || observe(&lease.scope, f.clock(1)));
        lifecycle.check_current(&history, || observe(&lease.scope, rolled_back));
        assert!(matches!(lifecycle.state(), State::Unconfirmed));
    }

    let history = MemoryHistory::default();
    let mut lifecycle = session(&f, &lease, Enrollment::IndependentlyApproved);
    let mut changed = f.lease();
    changed.scope.config_fingerprint.push('x');
    let samples = Cell::new(0);
    lifecycle.refresh(
        f.sign(LEASE_DOMAIN, &lease).record(),
        &history,
        &mut source(&f, Fixture::status()),
        || {
            samples.set(samples.get() + 1);
            let scope = if samples.get() == 1 {
                &lease.scope
            } else {
                &changed.scope
            };
            observe(scope, f.clock(0))
        },
    );
    assert!(matches!(lifecycle.state(), State::Unconfirmed));
    assert_eq!(history.read(&key(&f, &lease)), Ok(None));

    let mut lifecycle = session(&f, &lease, Enrollment::IndependentlyApproved);
    let samples = Cell::new(0);
    lifecycle.refresh(
        f.sign(LEASE_DOMAIN, &lease).record(),
        &history,
        &mut source(&f, Fixture::status()),
        || {
            samples.set(samples.get() + 1);
            let seconds = if samples.get() < 3 { 0 } else { 30 };
            observe(&lease.scope, f.clock(seconds))
        },
    );
    assert!(matches!(lifecycle.state(), State::Expired));
    assert_eq!(history.read(&key(&f, &lease)).unwrap().unwrap().sequence, 9);
}
