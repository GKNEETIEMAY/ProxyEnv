use super::{BridgeResult, PortAllocation, DEFAULT_CC_REMOTE_PORT};

pub(super) fn derived_port(seed: &str, round: u8, lane: u8) -> u16 {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("{seed}\n{round}"));
    let offset = usize::from(lane) * 2;
    20_000 + (u16::from_le_bytes([digest[offset], digest[offset + 1]]) % 40_001)
}

pub(super) fn first_available_port(
    preferred: impl IntoIterator<Item = u16>,
    excluded: Option<u16>,
    mut available: impl FnMut(u16) -> BridgeResult<bool>,
) -> BridgeResult<u16> {
    for candidate in preferred {
        if Some(candidate) != excluded && available(candidate)? {
            return Ok(candidate);
        }
    }
    Err("portAllocationFailed".into())
}

pub(super) fn resolve_port_pair(
    fingerprint: &str,
    local_port: Option<u16>,
    runtime_expected_proxy_port: Option<u16>,
    prefer_defaults: bool,
    mut remote_available: impl FnMut(u16) -> BridgeResult<bool>,
) -> BridgeResult<PortAllocation> {
    let mut runtime_port_conflict = false;
    let proxy_port = if let Some(expected) = runtime_expected_proxy_port {
        if remote_available(expected)? {
            expected
        } else {
            runtime_port_conflict = true;
            first_available_port(
                (0..24).map(|round| derived_port(fingerprint, round, 0)),
                None,
                &mut remote_available,
            )?
        }
    } else {
        let preferred = local_port.filter(|_| prefer_defaults).into_iter();
        first_available_port(
            preferred.chain((0..24).map(|round| derived_port(fingerprint, round, 0))),
            None,
            &mut remote_available,
        )?
    };
    let preferred_cc = prefer_defaults
        .then_some(DEFAULT_CC_REMOTE_PORT)
        .into_iter();
    let cc_port = first_available_port(
        preferred_cc.chain((0..24).map(|round| derived_port(fingerprint, round, 1))),
        Some(proxy_port),
        &mut remote_available,
    )?;
    Ok(PortAllocation {
        proxy_port,
        cc_port,
        runtime_expected_proxy_port,
        runtime_port_conflict,
    })
}
