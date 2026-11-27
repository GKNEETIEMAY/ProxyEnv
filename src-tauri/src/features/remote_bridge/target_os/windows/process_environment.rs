use crate::features::proxy::{ProxyEndpoint, ProxyProtocol};
use std::{cmp::Ordering, ffi::OsString, os::windows::ffi::OsStrExt};
use windows::Win32::Globalization::{
    CompareStringOrdinal, CSTR_EQUAL, CSTR_GREATER_THAN, CSTR_LESS_THAN,
};

const HTTP: &str = "HTTP_PROXY";
const HTTPS: &str = "HTTPS_PROXY";
const ALL: &str = "ALL_PROXY";
const BYPASS: &str = "NO_PROXY";
const LOOPBACK: [&str; 3] = ["localhost", "127.0.0.1", "::1"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::features::remote_bridge) enum EnvironmentError {
    InvalidEndpoint,
    InvalidEntry,
}

/// An owned Unicode environment block for CreateProcessW.
pub(in crate::features::remote_bridge) struct ProcessEnvironment(Vec<u16>);

impl ProcessEnvironment {
    pub fn prepare(
        endpoint: &ProxyEndpoint,
        inherited: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Result<Self, EnvironmentError> {
        if endpoint.host != "127.0.0.1"
            || endpoint.port == 0
            || endpoint.protocol == ProxyProtocol::Unknown
        {
            return Err(EnvironmentError::InvalidEndpoint);
        }

        let proxy_names = [HTTP, HTTPS, ALL, BYPASS].map(wide);
        let [_, _, _, bypass_name] = &proxy_names;
        let mut entries = Vec::new();
        let mut bypass = Vec::<String>::new();
        let mut bypass_all = false;
        for (name, value) in inherited {
            let name: Vec<_> = name.encode_wide().collect();
            let value: Vec<_> = value.encode_wide().collect();
            if !valid_name(&name) || value.contains(&0) {
                return Err(EnvironmentError::InvalidEntry);
            }
            if compare_names(&name, bypass_name) == Ordering::Equal {
                let value =
                    String::from_utf16(&value).map_err(|_| EnvironmentError::InvalidEntry)?;
                bypass_all |= value.trim() == "*";
                for entry in value
                    .split(',')
                    .map(str::trim)
                    .filter(|entry| !entry.is_empty())
                {
                    add_bypass(&mut bypass, entry);
                }
            } else if !proxy_names
                .iter()
                .any(|proxy| compare_names(&name, proxy) == Ordering::Equal)
            {
                entries.push((name, value));
            }
        }

        let authority = format!("{}:{}", endpoint.host, endpoint.port);
        if matches!(
            endpoint.protocol,
            ProxyProtocol::Http | ProxyProtocol::Mixed
        ) {
            let url = wide(&format!("http://{authority}"));
            entries.push((wide(HTTP), url.clone()));
            entries.push((wide(HTTPS), url));
        }
        if matches!(
            endpoint.protocol,
            ProxyProtocol::Socks5 | ProxyProtocol::Mixed
        ) {
            entries.push((wide(ALL), wide(&format!("socks5h://{authority}"))));
        }
        for host in LOOPBACK {
            add_bypass(&mut bypass, host);
        }
        let bypass = if bypass_all {
            "*".into()
        } else {
            bypass.join(",")
        };
        entries.push((wide(BYPASS), wide(&bypass)));

        entries.sort_by(|(left, _), (right, _)| compare_names(left, right));
        let mut block = Vec::new();
        for (name, value) in entries {
            block.extend(name);
            block.push(u16::from(b'='));
            block.extend(value);
            block.push(0);
        }
        block.push(0);
        Ok(Self(block))
    }

    pub fn block(&self) -> &[u16] {
        &self.0
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

fn valid_name(name: &[u16]) -> bool {
    const EQUALS: u16 = b'=' as u16;
    if name.is_empty() || name.contains(&0) {
        return false;
    }
    if let [EQUALS, drive, colon] = name {
        return u8::try_from(*drive).is_ok_and(|drive| drive.is_ascii_alphabetic())
            && *colon == u16::from(b':');
    }
    !name.contains(&EQUALS)
}

fn add_bypass(entries: &mut Vec<String>, host: &str) {
    if !entries.iter().any(|entry| entry.eq_ignore_ascii_case(host)) {
        entries.push(host.into());
    }
}

fn compare_names(left: &[u16], right: &[u16]) -> Ordering {
    // SAFETY: both inputs are live, explicitly sized UTF-16 slices.
    match unsafe { CompareStringOrdinal(left, right, true) } {
        CSTR_LESS_THAN => Ordering::Less,
        CSTR_EQUAL => Ordering::Equal,
        CSTR_GREATER_THAN => Ordering::Greater,
        _ => left.cmp(right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(protocol: ProxyProtocol, port: u16) -> ProxyEndpoint {
        ProxyEndpoint {
            host: "127.0.0.1".into(),
            port,
            protocol,
        }
    }

    fn inherited<'a>(
        entries: &'a [(&'a str, &'a str)],
    ) -> impl Iterator<Item = (OsString, OsString)> + 'a {
        entries
            .iter()
            .map(|(name, value)| ((*name).into(), (*value).into()))
    }

    fn entries(environment: &ProcessEnvironment) -> Vec<String> {
        assert_eq!(
            &environment.block()[environment.block().len() - 2..],
            &[0, 0]
        );
        environment
            .block()
            .split(|unit| *unit == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| String::from_utf16(entry).unwrap())
            .collect()
    }

    #[test]
    fn managed_environment_maps_protocols_without_mixing_schemes() {
        let original = [
            ("http_proxy", "previous"),
            ("HTTPS_proxy", "previous"),
            ("all_proxy", "previous"),
        ];
        for (protocol, expected) in [
            (
                ProxyProtocol::Http,
                vec![
                    "HTTPS_PROXY=http://127.0.0.1:20500",
                    "HTTP_PROXY=http://127.0.0.1:20500",
                ],
            ),
            (
                ProxyProtocol::Socks5,
                vec!["ALL_PROXY=socks5h://127.0.0.1:20500"],
            ),
            (
                ProxyProtocol::Mixed,
                vec![
                    "ALL_PROXY=socks5h://127.0.0.1:20500",
                    "HTTPS_PROXY=http://127.0.0.1:20500",
                    "HTTP_PROXY=http://127.0.0.1:20500",
                ],
            ),
        ] {
            let block =
                ProcessEnvironment::prepare(&endpoint(protocol, 20500), inherited(&original))
                    .unwrap();
            let mut expected = expected;
            expected.push("NO_PROXY=localhost,127.0.0.1,::1");
            assert_eq!(entries(&block), expected);
        }
    }

    #[test]
    fn managed_environment_removes_case_conflicts_and_preserves_other_entries() {
        let block = ProcessEnvironment::prepare(
            &endpoint(ProxyProtocol::Http, 20500),
            inherited(&[
                ("Path", "合成路徑"),
                ("EMPTY", ""),
                ("=C:", "C:\\fixture"),
                ("HtTp_ProXy", "x"),
                ("HTTP_PROXY", "y"),
                ("all_PROXY", ""),
                ("no_proxy", " .example.test, LOCALHOST,192.0.2.0/24 "),
                ("NO_PROXY", ".EXAMPLE.TEST, ,127.0.0.1,::1"),
            ]),
        )
        .unwrap();
        assert_eq!(
            entries(&block),
            [
                "=C:=C:\\fixture",
                "EMPTY=",
                "HTTPS_PROXY=http://127.0.0.1:20500",
                "HTTP_PROXY=http://127.0.0.1:20500",
                "NO_PROXY=.example.test,LOCALHOST,192.0.2.0/24,127.0.0.1,::1",
                "Path=合成路徑",
            ]
        );
    }

    #[test]
    fn managed_environment_preserves_standalone_wildcard_and_handles_empty_bypass() {
        for (original, expected) in [
            ("*", "NO_PROXY=*"),
            (" , ", "NO_PROXY=localhost,127.0.0.1,::1"),
            ("*,.test", "NO_PROXY=*,.test,localhost,127.0.0.1,::1"),
        ] {
            let environment = ProcessEnvironment::prepare(
                &endpoint(ProxyProtocol::Socks5, 20500),
                inherited(&[("No_Proxy", original)]),
            )
            .unwrap();
            assert!(entries(&environment).iter().any(|entry| entry == expected));
        }
    }

    #[test]
    fn managed_environment_rejects_malformed_input_and_unconfirmed_endpoints() {
        let valid = endpoint(ProxyProtocol::Http, 20500);
        for entries in [[("", "x")], [("A=B", "x")], [("A", "x\0y")], [("=??", "x")]] {
            assert_eq!(
                ProcessEnvironment::prepare(&valid, inherited(&entries)).err(),
                Some(EnvironmentError::InvalidEntry)
            );
        }
        for endpoint in [
            endpoint(ProxyProtocol::Unknown, 20500),
            endpoint(ProxyProtocol::Http, 0),
            ProxyEndpoint {
                host: "0.0.0.0".into(),
                ..valid.clone()
            },
            ProxyEndpoint {
                host: "user:secret@127.0.0.1".into(),
                ..valid
            },
        ] {
            assert_eq!(
                ProcessEnvironment::prepare(&endpoint, []).err(),
                Some(EnvironmentError::InvalidEndpoint)
            );
        }
    }

    #[test]
    fn managed_environment_copies_are_isolated() {
        let baseline = [("OTHER", "original"), ("HTTP_PROXY", "original")];
        let input: Vec<_> = inherited(&baseline).collect();
        let first =
            ProcessEnvironment::prepare(&endpoint(ProxyProtocol::Http, 20001), input.clone())
                .unwrap();
        let second =
            ProcessEnvironment::prepare(&endpoint(ProxyProtocol::Socks5, 20002), input.clone())
                .unwrap();
        assert_eq!(input, inherited(&baseline).collect::<Vec<_>>());
        assert!(entries(&first).contains(&"HTTP_PROXY=http://127.0.0.1:20001".into()));
        assert!(entries(&second).contains(&"ALL_PROXY=socks5h://127.0.0.1:20002".into()));
        assert_eq!(entries(&first).last(), entries(&second).last());
    }
}
