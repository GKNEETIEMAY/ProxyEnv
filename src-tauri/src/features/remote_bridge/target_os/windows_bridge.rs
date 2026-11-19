use super::windows::{allows_direct_probe, WindowsShell, WindowsTargetProbe};
use crate::features::{
    proxy::{active, ProxyEndpoint, ProxyProtocol},
    remote_bridge::{ssh, BridgeResult},
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::RangeInclusive,
};

mod safety;

#[derive(Clone, Copy, PartialEq, Eq)]
enum GatewayPorts {
    No,
    ClientSpecified,
    Yes,
    Unknown,
}

struct ManagedHostPolicy {
    review_id: &'static str,
    host: &'static str,
    account: &'static str,
    ssh_port: u16,
    host_key_sha256: [u8; 32],
    gateway_ports: GatewayPorts,
    remote_ports: RangeInclusive<u16>,
    valid_from: u64,
    expires_at: u64,
    issuer: Option<safety::PolicyIssuer>,
}

// Only independently reviewed, repository-controlled evidence may go here.
// There is no enrolled host, config import, or remote claim source in M3.1.
const REVIEWED_TEST_HOSTS: &[ManagedHostPolicy] = &[];

impl ManagedHostPolicy {
    fn check(&self, input: &ValidatedInput<'_>, remote_port: u16, now: u64) -> BridgeResult<()> {
        if self.gateway_ports != GatewayPorts::No
            || !(self.valid_from..self.expires_at).contains(&now)
            || (self.host, self.account, self.ssh_port) != input.identity
            || self.host_key_sha256 != input.host_key_sha256
        {
            return Err("windowsForwardPolicyDenied".into());
        }
        if *self.remote_ports.start() < 1024 || !self.remote_ports.contains(&remote_port) {
            return Err("invalidPort".into());
        }
        Ok(())
    }
}

/// A backend snapshot from the same authenticated target and session generation.
/// The host key must come from SSH host verification. A prompt, Windows reply,
/// or caller-provided fingerprint is not a source of verified identity.
pub(in crate::features::remote_bridge) struct WindowsBridgeInput<'a> {
    pub target: &'a ssh::BoundTarget,
    pub effective_ssh_config: &'a str,
    pub verified_host: Option<ssh::VerifiedHostIdentity<'a>>,
    pub platform: &'a WindowsTargetProbe,
    pub generation: u64,
    pub active_proxy: &'a active::ActiveProxyContext,
}

struct ValidatedInput<'a> {
    identity: (&'a str, &'a str, u16),
    config_fingerprint: &'a str,
    host_key_sha256: [u8; 32],
    shell: WindowsShell,
    generation: u64,
    proxy_revision: u64,
    upstream: ProxyEndpoint,
}

impl<'a> WindowsBridgeInput<'a> {
    fn validate(self) -> BridgeResult<ValidatedInput<'a>> {
        let config_fingerprint = self
            .target
            .checked_config_fingerprint(self.effective_ssh_config)?;
        if !allows_direct_probe(self.effective_ssh_config)
            || self.effective_ssh_config.lines().any(|line| {
                let mut fields = line.split_ascii_whitespace();
                match fields.next() {
                    Some("localforward" | "remoteforward" | "dynamicforward" | "stdioforward") => {
                        true
                    }
                    Some("tunnel") => !matches!(fields.next(), Some("false" | "no")),
                    _ => false,
                }
            })
        {
            return Err("sshConfigUnsafe".into());
        }
        let WindowsTargetProbe::Confirmed { shell } = self.platform else {
            return Err("remoteUnsupported".into());
        };
        let candidate = self
            .active_proxy
            .available_candidate()
            .ok_or("proxyUnavailable")?;
        let upstream = active::endpoint(candidate);
        if !candidate.listening
            || !upstream
                .host
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
            || upstream.port == 0
            || upstream.protocol == ProxyProtocol::Unknown
        {
            return Err("proxyUnavailable".into());
        }
        Ok(ValidatedInput {
            identity: self.target.endpoint_identity(),
            config_fingerprint,
            host_key_sha256: self
                .verified_host
                .ok_or("hostKey")?
                .key_for(self.target, self.generation)?,
            shell: *shell,
            generation: self.generation,
            proxy_revision: self.active_proxy.revision,
            upstream,
        })
    }
}

pub(in crate::features::remote_bridge) struct WindowsBridgePlan<'a> {
    input: ValidatedInput<'a>,
    policy: &'a ManagedHostPolicy,
    remote_port: u16,
    ingress_port: u16,
    planned_at: u64,
}

impl<'a> WindowsBridgePlan<'a> {
    pub(in crate::features::remote_bridge) fn build(
        input: WindowsBridgeInput<'a>,
        review_id: &str,
        remote_port: u16,
        ingress: SocketAddr,
        now: u64,
    ) -> BridgeResult<Self> {
        Self::build_from_reviews(
            input,
            review_id,
            remote_port,
            ingress,
            now,
            REVIEWED_TEST_HOSTS,
        )
    }

    fn build_from_reviews(
        input: WindowsBridgeInput<'a>,
        review_id: &str,
        remote_port: u16,
        ingress: SocketAddr,
        now: u64,
        reviews: &'a [ManagedHostPolicy],
    ) -> BridgeResult<Self> {
        let mut matching = reviews
            .iter()
            .filter(|policy| policy.review_id == review_id);
        let policy = matching.next().ok_or("windowsForwardPolicyUnreviewed")?;
        if matching.next().is_some() {
            return Err("windowsForwardPolicyUnreviewed".into());
        }
        let input = input.validate()?;
        policy.check(&input, remote_port, now)?;
        if ingress.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST)
            || ingress.port() < 1024
            || (input.upstream.host == "127.0.0.1" && input.upstream.port == ingress.port())
        {
            return Err("unsafeBinding".into());
        }
        Ok(Self {
            input,
            policy,
            remote_port,
            ingress_port: ingress.port(),
            planned_at: now,
        })
    }

    /// Supplies one forwarding option, never a command or additional argv.
    /// A future launcher must uphold the validated SSH config and host trust.
    pub(in crate::features::remote_bridge) fn forward_args(
        &self,
        current: WindowsBridgeInput<'_>,
        now: u64,
    ) -> BridgeResult<[String; 2]> {
        let current = current.validate()?;
        if current.config_fingerprint != self.input.config_fingerprint
            || current.identity != self.input.identity
        {
            return Err("sshConfigChanged".into());
        }
        if current.host_key_sha256 != self.input.host_key_sha256 {
            return Err("hostKeyChanged".into());
        }
        if current.generation != self.input.generation || current.shell != self.input.shell {
            return Err("windowsBridgeContextChanged".into());
        }
        if current.proxy_revision != self.input.proxy_revision
            || current.upstream != self.input.upstream
        {
            return Err("activeChanged".into());
        }
        if now < self.planned_at {
            return Err("windowsForwardPolicyDenied".into());
        }
        self.policy.check(&current, self.remote_port, now)?;
        Ok([
            "-R".into(),
            ssh::reverse_forward_spec(self.remote_port, "127.0.0.1", self.ingress_port),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{
        proxy::{Confidence, ProxyCandidate},
        remote_bridge::target_os::windows::WindowsProbeReason,
    };

    pub(super) const CONFIG: &str =
        "hostname a.example\nuser alice\nport 2222\ncheckhostip no\ntunnel false\n";
    const KEY: [u8; 32] = [0x91; 32];
    const CONFIRMED: WindowsTargetProbe = WindowsTargetProbe::Confirmed {
        shell: WindowsShell::Cmd,
    };
    pub(super) const REVIEW: ManagedHostPolicy = ManagedHostPolicy {
        review_id: "review-a",
        host: "a.example",
        account: "alice",
        ssh_port: 2222,
        host_key_sha256: KEY,
        gateway_ports: GatewayPorts::No,
        remote_ports: 20500..=20502,
        valid_from: 100,
        expires_at: 200,
        issuer: None,
    };
    pub(super) const INGRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49152);

    pub(super) fn active() -> active::ActiveProxyContext {
        active::ActiveProxyContext {
            available: true,
            revision: 9,
            candidate: Some(ProxyCandidate {
                id: "fixture-proxy".into(),
                client_name: None,
                icon_key: None,
                process_name: None,
                pid: None,
                host: "127.0.0.1".into(),
                port: 7897,
                protocol: ProxyProtocol::Mixed,
                source: vec![],
                confidence: Confidence::High,
                listening: true,
            }),
            ..Default::default()
        }
    }

    pub(super) fn input<'a>(
        target: &'a ssh::BoundTarget,
        proxy: &'a active::ActiveProxyContext,
    ) -> WindowsBridgeInput<'a> {
        WindowsBridgeInput {
            target,
            effective_ssh_config: CONFIG,
            verified_host: Some(ssh::VerifiedHostIdentity::fixture(target, 23, KEY)),
            platform: &CONFIRMED,
            generation: 23,
            active_proxy: proxy,
        }
    }

    fn reviewed<'a>(
        input: WindowsBridgeInput<'a>,
        now: u64,
        reviews: &'a [ManagedHostPolicy],
    ) -> BridgeResult<WindowsBridgePlan<'a>> {
        WindowsBridgePlan::build_from_reviews(input, "review-a", 20500, INGRESS, now, reviews)
    }

    fn rejected<T>(result: BridgeResult<T>, code: &str) {
        assert_eq!(result.err().as_deref(), Some(code));
    }

    #[test]
    fn windows_bridge_plan_uses_exactly_one_fixed_loopback_forward() {
        let target = ssh::BoundTarget::fixture("alias-a", CONFIG).unwrap();
        let proxy = active();
        for port in [20500, 20502] {
            let plan = WindowsBridgePlan::build_from_reviews(
                input(&target, &proxy),
                "review-a",
                port,
                INGRESS,
                100,
                &[REVIEW],
            )
            .unwrap();
            assert_eq!(
                plan.forward_args(input(&target, &proxy), 199).unwrap(),
                ["-R", &format!("127.0.0.1:{port}:127.0.0.1:49152")]
            );
        }
    }

    #[test]
    fn windows_bridge_plan_rejects_forged_or_ambiguous_policy_claims() {
        let config = format!("{CONFIG}gatewayports no\n");
        let target = ssh::BoundTarget::fixture("alias-a", &config).unwrap();
        let proxy = active();
        let snapshot = || WindowsBridgeInput {
            effective_ssh_config: &config,
            ..input(&target, &proxy)
        };
        rejected(
            WindowsBridgePlan::build(snapshot(), "review-a", 20500, INGRESS, 150),
            "windowsForwardPolicyUnreviewed",
        );
        rejected(
            WindowsBridgePlan::build_from_reviews(
                snapshot(),
                "-R unreviewed",
                20500,
                INGRESS,
                150,
                &[REVIEW],
            ),
            "windowsForwardPolicyUnreviewed",
        );
        rejected(
            reviewed(snapshot(), 150, &[REVIEW, REVIEW]),
            "windowsForwardPolicyUnreviewed",
        );
        for forged in [
            ManagedHostPolicy {
                host: "b.example",
                ..REVIEW
            },
            ManagedHostPolicy {
                account: "bob",
                ..REVIEW
            },
            ManagedHostPolicy {
                ssh_port: 22,
                ..REVIEW
            },
            ManagedHostPolicy {
                host_key_sha256: [0x42; 32],
                ..REVIEW
            },
        ] {
            rejected(
                reviewed(snapshot(), 150, &[forged]),
                "windowsForwardPolicyDenied",
            );
        }
    }

    #[test]
    fn windows_bridge_plan_requires_gateway_ports_no_and_a_current_review() {
        let target = ssh::BoundTarget::fixture("alias-a", CONFIG).unwrap();
        let proxy = active();
        let snapshot = || input(&target, &proxy);
        for gateway_ports in [
            GatewayPorts::ClientSpecified,
            GatewayPorts::Yes,
            GatewayPorts::Unknown,
        ] {
            rejected(
                reviewed(
                    snapshot(),
                    150,
                    &[ManagedHostPolicy {
                        gateway_ports,
                        ..REVIEW
                    }],
                ),
                "windowsForwardPolicyDenied",
            );
        }
        for now in [0, 99, 200, 201] {
            rejected(
                reviewed(snapshot(), now, &[REVIEW]),
                "windowsForwardPolicyDenied",
            );
        }
        let plan = reviewed(snapshot(), 150, &[REVIEW]).unwrap();
        for now in [149, 200] {
            rejected(
                plan.forward_args(snapshot(), now),
                "windowsForwardPolicyDenied",
            );
        }
    }

    #[test]
    fn windows_bridge_plan_rejects_unapproved_ports_and_non_loopback_ingress() {
        let target = ssh::BoundTarget::fixture("alias-a", CONFIG).unwrap();
        let proxy = active();
        let build = |port, ingress| {
            WindowsBridgePlan::build_from_reviews(
                input(&target, &proxy),
                "review-a",
                port,
                ingress,
                150,
                &[REVIEW],
            )
        };
        for port in [0, 1023, 20499, 20503, u16::MAX] {
            rejected(build(port, INGRESS), "invalidPort");
        }
        for addr in [
            "0.0.0.0:49152",
            "[::]:49152",
            "[::1]:49152",
            "192.0.2.1:49152",
            "127.0.0.1:7897",
            "127.0.0.1:0",
        ] {
            rejected(build(20500, addr.parse().unwrap()), "unsafeBinding");
        }
    }

    #[test]
    fn windows_bridge_plan_revalidates_target_config_account_key_and_generation() {
        let target = ssh::BoundTarget::fixture("alias-a", CONFIG).unwrap();
        let proxy = active();
        let plan = reviewed(input(&target, &proxy), 150, &[REVIEW]).unwrap();
        let other_alias = ssh::BoundTarget::fixture("alias-b", CONFIG).unwrap();
        rejected(
            plan.forward_args(input(&other_alias, &proxy), 150),
            "sshConfigChanged",
        );
        let stale_identity = WindowsBridgeInput {
            verified_host: Some(ssh::VerifiedHostIdentity::fixture(&target, 23, KEY)),
            ..input(&other_alias, &proxy)
        };
        rejected(plan.forward_args(stale_identity, 150), "hostKey");

        for config in [
            CONFIG.replace("a.example", "b.example"),
            CONFIG.replace("alice", "bob"),
            CONFIG.replace("2222", "2223"),
            format!("{CONFIG}identitiesonly yes\n"),
        ] {
            let retargeted = ssh::BoundTarget::fixture("alias-a", &config).unwrap();
            for binding in [&target, &retargeted] {
                let current = WindowsBridgeInput {
                    effective_ssh_config: &config,
                    ..input(binding, &proxy)
                };
                rejected(plan.forward_args(current, 150), "sshConfigChanged");
            }
        }
        let changed_key = WindowsBridgeInput {
            verified_host: Some(ssh::VerifiedHostIdentity::fixture(&target, 23, [9; 32])),
            ..input(&target, &proxy)
        };
        rejected(plan.forward_args(changed_key, 150), "hostKeyChanged");
        let new_generation = WindowsBridgeInput {
            generation: 24,
            verified_host: Some(ssh::VerifiedHostIdentity::fixture(&target, 24, KEY)),
            ..input(&target, &proxy)
        };
        rejected(
            plan.forward_args(new_generation, 150),
            "windowsBridgeContextChanged",
        );
        let stale_generation = WindowsBridgeInput {
            generation: 24,
            ..input(&target, &proxy)
        };
        rejected(plan.forward_args(stale_generation, 150), "hostKey");
        let changed_shell = WindowsBridgeInput {
            platform: &WindowsTargetProbe::Confirmed {
                shell: WindowsShell::PowerShell,
            },
            ..input(&target, &proxy)
        };
        rejected(
            plan.forward_args(changed_shell, 150),
            "windowsBridgeContextChanged",
        );
    }

    #[test]
    fn windows_bridge_plan_rejects_unsupported_ssh_options_platform_and_missing_trust() {
        let proxy = active();
        for setting in [
            "proxyjump bastion",
            "proxycommand external",
            "knownhostscommand external",
            "checkhostip yes",
            "tunnel yes",
            "stdioforward host:80",
        ] {
            let config = format!("{CONFIG}{setting}\n");
            let target = ssh::BoundTarget::fixture("alias-a", &config).unwrap();
            let snapshot = WindowsBridgeInput {
                effective_ssh_config: &config,
                ..input(&target, &proxy)
            };
            rejected(reviewed(snapshot, 150, &[REVIEW]), "sshConfigUnsafe");
        }
        for extra in [
            "localforward 127.0.0.1:1 127.0.0.1:2",
            "remoteforward 30000 127.0.0.1:81",
            "dynamicforward 9999",
        ] {
            rejected(
                ssh::BoundTarget::fixture("alias-a", &format!("{CONFIG}{extra}\n")),
                "sshConfigUnsafe",
            );
        }
        let target = ssh::BoundTarget::fixture("alias-a", CONFIG).unwrap();
        let unknown = WindowsTargetProbe::Unknown {
            reason: WindowsProbeReason::NoReply,
        };
        let snapshot = WindowsBridgeInput {
            platform: &unknown,
            ..input(&target, &proxy)
        };
        rejected(reviewed(snapshot, 150, &[REVIEW]), "remoteUnsupported");
        let snapshot = WindowsBridgeInput {
            verified_host: None,
            ..input(&target, &proxy)
        };
        rejected(reviewed(snapshot, 150, &[REVIEW]), "hostKey");
    }

    #[test]
    fn windows_bridge_plan_revalidates_proxy_revision_endpoint_and_availability() {
        let target = ssh::BoundTarget::fixture("alias-a", CONFIG).unwrap();
        let original = active();
        let plan = reviewed(input(&target, &original), 150, &[REVIEW]).unwrap();
        let mut changed = active();
        changed.revision += 1;
        rejected(
            plan.forward_args(input(&target, &changed), 150),
            "activeChanged",
        );
        changed.revision = original.revision;
        changed.candidate.as_mut().unwrap().port += 1;
        rejected(
            plan.forward_args(input(&target, &changed), 150),
            "activeChanged",
        );
        changed.available = false;
        rejected(
            plan.forward_args(input(&target, &changed), 150),
            "proxyUnavailable",
        );
        changed.available = true;
        changed.candidate.as_mut().unwrap().host = "0.0.0.0".into();
        rejected(
            plan.forward_args(input(&target, &changed), 150),
            "proxyUnavailable",
        );
    }
}
