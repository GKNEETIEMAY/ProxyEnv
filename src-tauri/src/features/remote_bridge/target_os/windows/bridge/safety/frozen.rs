use crate::features::remote_bridge::{base64_bytes, ssh, BridgeResult};
use sha2::{Digest, Sha256};

const HOST_ALIAS: &str = "routespan-forward";
const ED25519_PREFIX: &[u8] = b"\0\0\0\x0bssh-ed25519\0\0\0\x20";

pub(super) struct HostTrustEvidence<'a> {
    pub reviewed_public_blob: &'a [u8],
    pub known_hosts: Option<&'a str>,
    pub revoked_blobs: Option<&'a [&'a [u8]]>,
}

#[derive(Clone)]
pub(super) struct FrozenProfile {
    pub config_path: String,
    pub config: String,
    pub known_hosts: String,
    pub revoked_hosts: String,
}

#[derive(Clone, Copy)]
pub(super) struct FrozenFiles<'a> {
    pub config: &'a [u8],
    pub known_hosts: &'a [u8],
    pub revoked_hosts: &'a [u8],
}

fn public_key(blob: &[u8]) -> BridgeResult<String> {
    if !blob.starts_with(ED25519_PREFIX) || blob.len() != ED25519_PREFIX.len() + 32 {
        return Err("hostKey".into());
    }
    Ok(format!("ssh-ed25519 {}", base64_bytes(blob)))
}

fn safe_profile_directory(directory: &str) -> bool {
    let bytes = directory.as_bytes();
    if !(4..=220).contains(&bytes.len()) || !bytes[0].is_ascii_alphabetic() || &bytes[1..3] != b":/"
    {
        return false;
    }
    directory[3..].split('/').all(|part| {
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        !part.is_empty()
            && !part.starts_with(' ')
            && !part.ends_with([' ', '.'])
            && !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            && !(stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.as_bytes()[3].is_ascii_digit())
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b" ._-".contains(&byte))
    })
}

impl FrozenProfile {
    pub fn build(
        (host, account, port): (&str, &str, u16),
        reviewed_fingerprint: [u8; 32],
        evidence: HostTrustEvidence<'_>,
        directory: &str,
    ) -> BridgeResult<Self> {
        if !ssh::safe_host(host)
            || !ssh::safe_name(account)
            || port == 0
            || !safe_profile_directory(directory)
        {
            return Err("sshConfigUnsafe".into());
        }
        let key = public_key(evidence.reviewed_public_blob)?;
        if <[u8; 32]>::from(Sha256::digest(evidence.reviewed_public_blob)) != reviewed_fingerprint {
            return Err("hostKey".into());
        }
        let known = evidence
            .known_hosts
            .filter(|text| text.len() <= 4096)
            .ok_or("hostKey")?;
        let label = if port == 22 {
            host.to_owned()
        } else {
            format!("[{host}]:{port}")
        };
        let expected = format!("{label} {key}");
        let mut entries = known
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'));
        if entries.next() != Some(expected.as_str()) || entries.next().is_some() {
            return Err("hostKey".into());
        }

        let revoked = evidence
            .revoked_blobs
            .filter(|keys| keys.len() <= 64)
            .ok_or("hostKey")?;
        let mut revoked_hosts = String::new();
        for blob in revoked {
            let key = public_key(blob)?;
            if *blob == evidence.reviewed_public_blob {
                return Err("hostKeyChanged".into());
            }
            revoked_hosts.push_str(&key);
            revoked_hosts.push('\n');
        }

        // This allowlist is rendered from typed inputs. No raw SSH option is copied.
        let config = format!(
            "Host {HOST_ALIAS}\nHostName {host}\nUser {account}\nPort {port}\n\
             StrictHostKeyChecking yes\nHostKeyAlgorithms ssh-ed25519\n\
             UserKnownHostsFile \"{directory}/known_hosts\"\nGlobalKnownHostsFile none\n\
             RevokedHostKeys \"{directory}/revoked_hosts\"\nVerifyHostKeyDNS no\nUpdateHostKeys no\n\
             CheckHostIP no\nKnownHostsCommand none\nProxyJump none\nProxyCommand none\n\
             ControlMaster no\nControlPath none\nForwardAgent no\nForwardX11 no\nAddKeysToAgent no\n\
             IdentityFile none\nIdentityAgent none\nIdentitiesOnly yes\nBatchMode yes\n\
             PasswordAuthentication no\nKbdInteractiveAuthentication no\nHostbasedAuthentication no\n\
             PermitLocalCommand no\nRemoteCommand none\nTunnel no\n\
             ExitOnForwardFailure yes\nConnectTimeout 8\nConnectionAttempts 1\n\
             ServerAliveInterval 5\nServerAliveCountMax 2\nForkAfterAuthentication no\n\
             ClearAllForwardings no\n"
        );
        Ok(Self {
            config_path: format!("{directory}/ssh_config"),
            config,
            known_hosts: format!("{expected}\n"),
            revoked_hosts,
        })
    }

    pub fn files(&self) -> FrozenFiles<'_> {
        FrozenFiles {
            config: self.config.as_bytes(),
            known_hosts: self.known_hosts.as_bytes(),
            revoked_hosts: self.revoked_hosts.as_bytes(),
        }
    }

    pub fn check_files(&self, files: FrozenFiles<'_>) -> BridgeResult<()> {
        if files.config != self.config.as_bytes()
            || files.known_hosts != self.known_hosts.as_bytes()
            || files.revoked_hosts != self.revoked_hosts.as_bytes()
        {
            return Err("sshConfigUnsafe".into());
        }
        Ok(())
    }

    pub fn forward_args(&self, forward: &[String; 2]) -> [String; 7] {
        [
            "-F".into(),
            self.config_path.clone(),
            "-N".into(),
            "-T".into(),
            forward[0].clone(),
            forward[1].clone(),
            HOST_ALIAS.into(),
        ]
    }
}
