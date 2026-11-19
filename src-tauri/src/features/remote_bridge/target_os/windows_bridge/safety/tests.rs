use super::{
    policy::{Lease, SignedRecord, Status, LEASE_DOMAIN, STATUS_DOMAIN},
    *,
};
use crate::features::remote_bridge::{
    base64_bytes, ssh,
    target_os::windows_bridge::tests::{active, input, CONFIG, INGRESS, REVIEW},
};
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{cell::Cell, rc::Rc};

#[cfg(windows)]
mod process;

const DIR: &str = "C:/RouteSpan fixture";

struct Signed {
    payload: Vec<u8>,
    signature: Vec<u8>,
}

impl Signed {
    fn record(&self) -> SignedRecord<'_> {
        SignedRecord {
            payload: &self.payload,
            signature: &self.signature,
        }
    }
}

struct Fixture {
    target: ssh::BoundTarget,
    proxy: crate::features::proxy::active::ActiveProxyContext,
    review: [super::super::ManagedHostPolicy; 1],
    key: Ed25519KeyPair,
    blob: Vec<u8>,
    known: String,
    start: Instant,
}

fn blob(value: u8) -> Vec<u8> {
    [b"\0\0\0\x0bssh-ed25519\0\0\0\x20".as_slice(), &[value; 32]].concat()
}

impl Fixture {
    fn new() -> Self {
        // The signing seed and host-key-shaped blob are fixtures only.
        let key = Ed25519KeyPair::from_seed_unchecked(&[71; 32]).unwrap();
        let blob = blob(13);
        let review = super::super::ManagedHostPolicy {
            host_key_sha256: Sha256::digest(&blob).into(),
            issuer: Some(PolicyIssuer {
                key: key.public_key().as_ref().try_into().unwrap(),
                revision: 2,
                deployment: 7,
            }),
            ..REVIEW
        };
        Self {
            target: ssh::BoundTarget::fixture("alias-a", CONFIG).unwrap(),
            proxy: active(),
            review: [review],
            key,
            known: format!("[a.example]:2222 ssh-ed25519 {}\n", base64_bytes(&blob)),
            blob,
            start: Instant::now(),
        }
    }

    fn clock(&self, seconds: u64) -> ClockSample {
        ClockSample {
            unix_seconds: 150 + seconds,
            monotonic: self.start + Duration::from_secs(seconds),
        }
    }

    fn input(&self) -> WindowsBridgeInput<'_> {
        WindowsBridgeInput {
            verified_host: Some(ssh::VerifiedHostIdentity::fixture(
                &self.target,
                23,
                self.review[0].host_key_sha256,
            )),
            ..input(&self.target, &self.proxy)
        }
    }

    fn plan(&self) -> WindowsBridgePlan<'_> {
        WindowsBridgePlan::build_from_reviews(
            self.input(),
            "review-a",
            20500,
            INGRESS,
            150,
            &self.review,
        )
        .unwrap()
    }

    fn review_scope() -> ReviewScope {
        ReviewScope {
            review: "review-a".into(),
            revision: 2,
            deployment: 7,
        }
    }

    fn lease(&self) -> Lease {
        Lease {
            scope: LeaseScope {
                review: Self::review_scope(),
                host: "a.example".into(),
                account: "alice".into(),
                ssh_port: 2222,
                host_key_sha256: self.review[0].host_key_sha256,
                config_fingerprint: self
                    .target
                    .checked_config_fingerprint(CONFIG)
                    .unwrap()
                    .into(),
                generation: 23,
                remote_port: 20500,
            },
            gateway_ports: "no".into(),
            issued: 140,
            expires: 190,
            changes_blocked_until: 190,
            minimum_status_sequence: 9,
        }
    }

    fn status() -> Status {
        Status {
            review: Self::review_scope(),
            sequence: 9,
            issued: 148,
            expires: 180,
            revoked: false,
        }
    }

    fn sign(&self, domain: &[u8], data: impl Serialize) -> Signed {
        self.sign_bytes(domain, serde_json::to_vec(&data).unwrap())
    }

    fn sign_bytes(&self, domain: &[u8], payload: Vec<u8>) -> Signed {
        let sig = self.key.sign(&[domain, payload.as_slice()].concat());
        Signed {
            payload,
            signature: sig.as_ref().to_vec(),
        }
    }

    fn trust(&self) -> HostTrustEvidence<'_> {
        HostTrustEvidence {
            reviewed_public_blob: &self.blob,
            known_hosts: Some(&self.known),
            revoked_blobs: Some(&[]),
        }
    }

    fn preparation<'a>(&'a self, lease: &'a Signed, status: &'a Signed) -> Preparation<'a> {
        Preparation {
            current: self.input(),
            now: self.clock(0),
            directory: DIR,
            host_trust: self.trust(),
            policy: Some(PolicyEvidence {
                lease: lease.record(),
                status: status.record(),
                seen_status_sequence: 9,
            }),
        }
    }

    fn prepare(&self, lease: Lease, status: Status) -> BridgeResult<PreparedForward> {
        PreparedForward::prepare(
            &self.plan(),
            self.preparation(
                &self.sign(LEASE_DOMAIN, lease),
                &self.sign(STATUS_DOMAIN, status),
            ),
        )
    }

    fn prepared(&self) -> PreparedForward {
        self.prepare(self.lease(), Self::status()).unwrap()
    }

    fn verification<R>(&self, resources: R) -> Verification<R> {
        let actual = self.prepared();
        Verification::new(
            self.prepared(),
            files(&actual),
            &actual.args,
            resources,
            self.clock(0),
        )
        .unwrap()
    }
}

fn files(prepared: &PreparedForward) -> FrozenFiles<'_> {
    FrozenFiles {
        config: prepared.profile.config.as_bytes(),
        known_hosts: prepared.profile.known_hosts.as_bytes(),
        revoked_hosts: prepared.profile.revoked_hosts.as_bytes(),
    }
}

fn rejected<T>(result: BridgeResult<T>, code: &str) {
    assert_eq!(result.err().as_deref(), Some(code));
}

fn listener<R>(verification: &Verification<R>) -> ListenerReport {
    ListenerReport {
        version: 1,
        nonce: hex::encode(verification.listener_nonce),
        generation: verification.prepared.scope.generation,
        port: verification.prepared.scope.remote_port,
        ipv4_complete: true,
        ipv6_complete: true,
        addresses: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
    }
}

fn report<R>(verification: &Verification<R>) -> Vec<u8> {
    serde_json::to_vec(&listener(verification)).unwrap()
}

#[test]
fn safety_requires_verified_handshake_independent_policy_and_explicit_revocation_material() {
    let f = Fixture::new();
    let lease = f.sign(LEASE_DOMAIN, f.lease());
    let status = f.sign(STATUS_DOMAIN, Fixture::status());
    let mut input = f.preparation(&lease, &status);
    input.current.verified_host = None;
    rejected(PreparedForward::prepare(&f.plan(), input), "hostKey");
    let mut input = f.preparation(&lease, &status);
    input.policy = None;
    rejected(
        PreparedForward::prepare(&f.plan(), input),
        "windowsForwardPolicyDenied",
    );
    let mut input = f.preparation(&lease, &status);
    input.host_trust.revoked_blobs = None;
    rejected(PreparedForward::prepare(&f.plan(), input), "hostKey");
    let mut input = f.preparation(&lease, &status);
    input.host_trust.known_hosts = None;
    rejected(PreparedForward::prepare(&f.plan(), input), "hostKey");
    rejected(
        WindowsBridgePlan::build(f.input(), "review-a", 20500, INGRESS, 150),
        "windowsForwardPolicyUnreviewed",
    );
}

#[test]
fn safety_requires_one_exact_unrevoked_reviewed_host_key() {
    let f = Fixture::new();
    let lease = f.sign(LEASE_DOMAIN, f.lease());
    let status = f.sign(STATUS_DOMAIN, Fixture::status());
    for known in [
        f.known.repeat(2),
        format!("@revoked {}", f.known),
        f.known.replace("a.example", "b.example"),
        f.known.replace("2222", "22"),
        f.known.replace("ssh-ed25519", "ssh-rsa"),
        format!("{},*", f.known.trim_end()),
        String::new(),
    ] {
        let mut input = f.preparation(&lease, &status);
        input.host_trust.known_hosts = Some(&known);
        rejected(PreparedForward::prepare(&f.plan(), input), "hostKey");
    }
    let mut input = f.preparation(&lease, &status);
    let revoked = [f.blob.as_slice()];
    input.host_trust.revoked_blobs = Some(&revoked);
    rejected(PreparedForward::prepare(&f.plan(), input), "hostKeyChanged");
    let mut input = f.preparation(&lease, &status);
    let replacement = blob(17);
    input.host_trust.reviewed_public_blob = &replacement;
    rejected(PreparedForward::prepare(&f.plan(), input), "hostKey");
    let mut input = f.preparation(&lease, &status);
    input.host_trust.reviewed_public_blob = &f.blob[..f.blob.len() - 1];
    rejected(PreparedForward::prepare(&f.plan(), input), "hostKey");
}

#[test]
fn safety_rejects_unsigned_tampered_and_ambiguous_signed_policy() {
    let f = Fixture::new();
    let status = f.sign(STATUS_DOMAIN, Fixture::status());
    let mut bad = f.sign(LEASE_DOMAIN, f.lease());
    bad.signature[0] ^= 1;
    for lease in [
        bad,
        f.sign(STATUS_DOMAIN, f.lease()),
        f.sign_bytes(LEASE_DOMAIN, vec![b'x'; 4097]),
        f.sign_bytes(
            LEASE_DOMAIN,
            b"{\"gateway_ports\":\"no\",\"gateway_ports\":\"yes\"}".to_vec(),
        ),
    ] {
        rejected(
            PreparedForward::prepare(&f.plan(), f.preparation(&lease, &status)),
            "windowsForwardPolicyDenied",
        );
    }
    let lease = f.sign(LEASE_DOMAIN, f.lease());
    let mut unsigned = f.preparation(&lease, &status);
    unsigned.policy.as_mut().unwrap().status.signature = &[];
    rejected(
        PreparedForward::prepare(&f.plan(), unsigned),
        "windowsForwardPolicyDenied",
    );
    let mut tampered = lease.payload.clone();
    tampered.push(b' ');
    let mut input = f.preparation(&lease, &status);
    input.policy.as_mut().unwrap().lease.payload = &tampered;
    rejected(
        PreparedForward::prepare(&f.plan(), input),
        "windowsForwardPolicyDenied",
    );
}

#[test]
fn safety_policy_binds_exact_mode_deployment_revision_scope_and_maintenance_window() {
    let f = Fixture::new();
    for mode in ["yes", "clientspecified", "unknown", "", "NO"] {
        let lease = Lease {
            gateway_ports: mode.into(),
            ..f.lease()
        };
        rejected(
            f.prepare(lease, Fixture::status()),
            "windowsForwardPolicyDenied",
        );
    }
    let alterations: &[fn(&mut Lease)] = &[
        |l| l.scope.review.revision += 1,
        |l| l.scope.review.deployment += 1,
        |l| l.scope.generation += 1,
        |l| l.scope.remote_port += 1,
        |l| l.scope.account = "bob".into(),
        |l| l.scope.host = "b.example".into(),
        |l| l.scope.config_fingerprint = "forged".into(),
        |l| l.scope.host_key_sha256 = [1; 32],
        |l| l.expires = 150,
        |l| l.issued = 151,
        |l| l.changes_blocked_until = l.expires - 1,
        |l| l.expires = 201,
    ];
    for alter in alterations {
        let mut lease = f.lease();
        alter(&mut lease);
        rejected(
            f.prepare(lease, Fixture::status()),
            "windowsForwardPolicyDenied",
        );
    }
}

#[test]
fn safety_policy_rejects_revocation_sequence_rollback_expiry_and_clock_changes() {
    let f = Fixture::new();
    for status in [
        Status {
            revoked: true,
            ..Fixture::status()
        },
        Status {
            sequence: 8,
            ..Fixture::status()
        },
        Status {
            expires: 150,
            ..Fixture::status()
        },
        Status {
            issued: 151,
            ..Fixture::status()
        },
        Status {
            expires: 221,
            ..Fixture::status()
        },
        Status {
            review: ReviewScope {
                deployment: 8,
                ..Fixture::review_scope()
            },
            ..Fixture::status()
        },
    ] {
        rejected(f.prepare(f.lease(), status), "windowsForwardPolicyDenied");
    }
    let lease = f.sign(LEASE_DOMAIN, f.lease());
    let status = f.sign(STATUS_DOMAIN, Fixture::status());
    let mut input = f.preparation(&lease, &status);
    input.policy.as_mut().unwrap().seen_status_sequence = 10;
    rejected(
        PreparedForward::prepare(&f.plan(), input),
        "windowsForwardPolicyDenied",
    );
    let prepare = f.prepared();
    for now in [
        f.clock(30),
        ClockSample {
            unix_seconds: 149,
            ..f.clock(0)
        },
        ClockSample {
            unix_seconds: 150,
            ..f.clock(30)
        },
    ] {
        rejected(
            prepare.check_launch(files(&prepare), &prepare.args, now),
            "windowsForwardPolicyDenied",
        );
    }
}

#[test]
fn safety_frozen_launch_rejects_extra_forwarding_hooks_key_changes_and_injected_paths() {
    let f = Fixture::new();
    let p = f.prepared();
    p.check_launch(files(&p), &p.args, f.clock(0)).unwrap();
    assert_eq!(
        p.args,
        [
            "-F",
            "C:/RouteSpan fixture/ssh_config",
            "-N",
            "-T",
            "-R",
            "127.0.0.1:20500:127.0.0.1:49152",
            "routespan-forward"
        ]
    );
    for setting in [
        "LocalForward 1 host:2",
        "RemoteForward 3 host:4",
        "DynamicForward 5",
        "Include extra",
        "Match exec external",
        "LocalCommand external",
        "RemoteCommand external",
        "StrictHostKeyChecking no",
        "GlobalKnownHostsFile alternate",
        "RevokedHostKeys none",
        "UpdateHostKeys yes",
        "ProxyJump jump",
        "KnownHostsCommand external",
        "ClearAllForwardings yes",
    ] {
        let changed = format!("{}\n{setting}\n", p.profile.config);
        let snapshot = FrozenFiles {
            config: changed.as_bytes(),
            ..files(&p)
        };
        rejected(
            p.check_launch(snapshot, &p.args, f.clock(0)),
            "sshConfigUnsafe",
        );
    }
    for snapshot in [
        FrozenFiles {
            known_hosts: b"alternate",
            ..files(&p)
        },
        FrozenFiles {
            revoked_hosts: b"altered",
            ..files(&p)
        },
    ] {
        rejected(
            p.check_launch(snapshot, &p.args, f.clock(0)),
            "sshConfigUnsafe",
        );
    }
    for (pos, value) in [
        (0, "-R"),
        (1, "other-config"),
        (4, "-L"),
        (5, "0.0.0.0:20500:127.0.0.1:49152"),
        (5, "127.0.0.1:20503:127.0.0.1:49152"),
        (6, "another-target"),
    ] {
        let mut args = p.args.clone();
        args[pos] = value.into();
        rejected(
            p.check_launch(files(&p), &args, f.clock(0)),
            "sshConfigUnsafe",
        );
    }
    let mut args = p.args.to_vec();
    args.extend(["-R".into(), "127.0.0.1:20501:127.0.0.1:49152".into()]);
    rejected(
        p.check_launch(files(&p), &args, f.clock(0)),
        "sshConfigUnsafe",
    );
    let lease = f.sign(LEASE_DOMAIN, f.lease());
    let status = f.sign(STATUS_DOMAIN, Fixture::status());
    for dir in [
        "C:/../unsafe",
        "C:/two\nlines",
        "C:/contains%h",
        "C:/a\"b",
        "C:/NUL",
        "C://undeclared",
        "//remote/profile",
    ] {
        let mut input = f.preparation(&lease, &status);
        input.directory = dir;
        rejected(
            PreparedForward::prepare(&f.plan(), input),
            "sshConfigUnsafe",
        );
    }
}

struct Resource(Rc<Cell<usize>>);

impl Drop for Resource {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn safety_listener_parser_rejects_wildcards_non_loopback_incomplete_or_mismatched_reports() {
    let f = Fixture::new();
    for addr in [
        "0.0.0.0",
        "::",
        "::1",
        "127.0.0.2",
        "::ffff:127.0.0.1",
        "192.0.2.9",
        "2001:db8::1",
    ] {
        let closed = Rc::new(Cell::new(0));
        let mut gate = f.verification(Resource(closed.clone()));
        let mut reply = listener(&gate);
        reply.addresses.push(addr.parse().unwrap());
        rejected(
            gate.listener_report(&serde_json::to_vec(&reply).unwrap(), f.clock(0)),
            "unsafeBinding",
        );
        assert_eq!(closed.get(), 1);
    }
    let mutations: &[fn(&mut ListenerReport)] = &[
        |r| r.ipv4_complete = false,
        |r| r.ipv6_complete = false,
        |r| r.addresses.clear(),
        |r| r.addresses.push(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        |r| r.generation += 1,
        |r| r.port += 1,
        |r| r.nonce.push('0'),
        |r| r.version += 1,
    ];
    for mutate in mutations {
        let mut gate = f.verification(());
        let mut reply = listener(&gate);
        mutate(&mut reply);
        rejected(
            gate.listener_report(&serde_json::to_vec(&reply).unwrap(), f.clock(0)),
            "unsafeBinding",
        );
    }
    for bad in [vec![b' '; 4097], b"{} {}".to_vec(), b"{}".to_vec()] {
        let mut gate = f.verification(());
        rejected(gate.listener_report(&bad, f.clock(0)), "unsafeBinding");
    }
}

#[test]
fn safety_receipt_is_one_shot_exact_bounded_and_invalid_after_cancel() {
    let f = Fixture::new();
    for variant in 0..6 {
        let closed = Rc::new(Cell::new(0));
        let mut gate = f.verification(Resource(closed.clone()));
        gate.listener_report(&report(&gate), f.clock(0)).unwrap();
        let mut frame = gate.expected_receipt();
        match variant {
            0 => *frame.last_mut().unwrap() ^= 1,
            1 => {
                frame.pop();
            }
            2 => frame.push(0),
            3 => {} // Complete prefix without EOF.
            4 => gate.receipt(&frame, true, f.clock(0)).unwrap(), // Replay.
            5 => gate.cancel(),
            _ => unreachable!(),
        }
        assert!(gate.receipt(&frame, variant != 3, f.clock(0)).is_err());
        assert_eq!(closed.get(), 1);
    }
    let mut gate = f.verification(());
    gate.listener_report(&report(&gate), f.clock(0)).unwrap();
    rejected(
        gate.receipt(&gate.expected_receipt(), true, f.clock(15)),
        "windowsForwardVerificationClosed",
    );
    let other = f.verification(());
    let mut gate = f.verification(());
    gate.listener_report(&report(&gate), f.clock(0)).unwrap();
    rejected(
        gate.receipt(&other.expected_receipt(), true, f.clock(0)),
        "windowsForwardReceiptInvalid",
    );
}

#[test]
fn safety_simulated_upstream_opens_only_after_all_checks_and_current_policy() {
    let f = Fixture::new();
    let opens = Cell::new(0);
    let dropped = Rc::new(Cell::new(0));
    let attempt = |gate: Verification<Resource>, current: &PreparedForward| {
        gate.finish(current, files(current), &current.args, f.clock(0))
            .inspect(|_| opens.set(opens.get() + 1))
    };
    let current = f.prepared();
    assert!(attempt(f.verification(Resource(dropped.clone())), &current).is_err());
    let mut gate = f.verification(Resource(dropped.clone()));
    gate.listener_report(&report(&gate), f.clock(0)).unwrap();
    assert!(attempt(gate, &current).is_err());
    let mut gate = f.verification(Resource(dropped.clone()));
    gate.listener_report(&report(&gate), f.clock(0)).unwrap();
    gate.receipt(&gate.expected_receipt(), true, f.clock(0))
        .unwrap();
    let mut stale = f.prepared();
    stale.proxy_revision += 1;
    assert!(attempt(gate, &stale).is_err());
    assert_eq!((opens.get(), dropped.get()), (0, 3));

    let mut gate = f.verification(Resource(dropped.clone()));
    gate.listener_report(&report(&gate), f.clock(0)).unwrap();
    gate.receipt(&gate.expected_receipt(), true, f.clock(0))
        .unwrap();
    let mut older = f.prepared();
    older.policy.status_sequence -= 1;
    assert!(attempt(gate, &older).is_err());
    assert_eq!((opens.get(), dropped.get()), (0, 4));

    let mut gate = f.verification(Resource(dropped.clone()));
    gate.listener_report(&report(&gate), f.clock(0)).unwrap();
    gate.receipt(&gate.expected_receipt(), true, f.clock(0))
        .unwrap();
    let tampered = FrozenFiles {
        config: b"changed after launch",
        ..files(&current)
    };
    rejected(
        gate.finish(&current, tampered, &current.args, f.clock(0)),
        "sshConfigUnsafe",
    );
    assert_eq!((opens.get(), dropped.get()), (0, 5));

    let mut gate = f.verification(Resource(dropped.clone()));
    gate.listener_report(&report(&gate), f.clock(0)).unwrap();
    gate.receipt(&gate.expected_receipt(), true, f.clock(0))
        .unwrap();
    assert_eq!(opens.get(), 0);
    let resource = attempt(gate, &current).unwrap();
    assert_eq!((opens.get(), dropped.get()), (1, 5));
    drop(resource);
    assert_eq!(dropped.get(), 6);
}
