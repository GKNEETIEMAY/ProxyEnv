use crate::error::{ProxyEnvError, Result};

use super::{ProxyEndpoint, ProxyProtocol, ProxyVariable};

#[cfg(windows)]
const MANAGED_VARIABLES: &[&str] = &["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"];
#[cfg(windows)]
const DISPLAY_VARIABLES: &[&str] = &["NO_PROXY"];

#[cfg(any(target_os = "linux", target_os = "macos"))]
const MANAGED_VARIABLES: &[&str] = &["http_proxy", "https_proxy", "all_proxy"];
#[cfg(any(target_os = "linux", target_os = "macos"))]
const DISPLAY_VARIABLES: &[&str] = &["no_proxy"];

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
const MANAGED_VARIABLES: &[&str] = &["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"];
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
const DISPLAY_VARIABLES: &[&str] = &["NO_PROXY"];

/// One environment variable in a proxy plan.
///
/// `None` means the variable must be absent. This lets persistent environment
/// updates and child-process launch environments consume the same plan.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ProxyEnvironmentVariablePlan {
    pub name: String,
    pub value: Option<String>,
}

/// A deterministic, side-effect-free description of the proxy variables that
/// should exist for an endpoint.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ProxyEnvironmentPlan {
    pub variables: Vec<ProxyEnvironmentVariablePlan>,
}

pub(crate) fn build_proxy_environment_plan(
    endpoint: &ProxyEndpoint,
    selected: &[ProxyVariable],
) -> Result<ProxyEnvironmentPlan> {
    validate_endpoint(endpoint)?;

    let host = format_host(endpoint.host.trim());
    let variables = MANAGED_VARIABLES
        .iter()
        .map(|name| {
            let variable = variable_for_name(name);
            let value = variable
                .filter(|variable| selected.contains(variable))
                .and_then(|variable| protocol_scheme(endpoint.protocol, variable))
                .map(|scheme| format!("{scheme}://{host}:{}", endpoint.port));
            ProxyEnvironmentVariablePlan {
                name: (*name).to_owned(),
                value,
            }
        })
        .collect();

    Ok(ProxyEnvironmentPlan { variables })
}

pub(crate) fn managed_variable_names() -> Vec<String> {
    MANAGED_VARIABLES
        .iter()
        .map(|name| (*name).to_owned())
        .collect()
}

pub(crate) fn all_proxy_variable_names() -> Vec<String> {
    MANAGED_VARIABLES
        .iter()
        .chain(DISPLAY_VARIABLES)
        .map(|name| (*name).to_owned())
        .collect()
}

pub(crate) fn is_managed_variable(name: &str) -> bool {
    MANAGED_VARIABLES.contains(&name)
}

pub(crate) fn variable_matches_name(variable: ProxyVariable, name: &str) -> bool {
    match variable {
        ProxyVariable::Http => name.eq_ignore_ascii_case("HTTP_PROXY"),
        ProxyVariable::Https => name.eq_ignore_ascii_case("HTTPS_PROXY"),
        ProxyVariable::All => name.eq_ignore_ascii_case("ALL_PROXY"),
    }
}

fn validate_endpoint(endpoint: &ProxyEndpoint) -> Result<()> {
    if endpoint.host.trim().is_empty() {
        return Err(ProxyEnvError::InvalidProxyEndpoint(
            "host cannot be empty".into(),
        ));
    }
    if endpoint.port == 0 {
        return Err(ProxyEnvError::InvalidProxyEndpoint(
            "port must be between 1 and 65535".into(),
        ));
    }
    if matches!(endpoint.protocol, ProxyProtocol::Unknown) {
        return Err(ProxyEnvError::InvalidProxyEndpoint(
            "protocol must be HTTP, SOCKS5, or Mixed".into(),
        ));
    }
    Ok(())
}

fn format_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    }
}

fn variable_for_name(name: &str) -> Option<ProxyVariable> {
    [
        ProxyVariable::Http,
        ProxyVariable::Https,
        ProxyVariable::All,
    ]
    .into_iter()
    .find(|variable| variable_matches_name(*variable, name))
}

fn protocol_scheme(protocol: ProxyProtocol, variable: ProxyVariable) -> Option<&'static str> {
    match (protocol, variable) {
        (ProxyProtocol::Http, ProxyVariable::Http | ProxyVariable::Https) => Some("http"),
        (ProxyProtocol::Socks5, ProxyVariable::All) => Some("socks5"),
        (ProxyProtocol::Mixed, ProxyVariable::Http | ProxyVariable::Https) => Some("http"),
        (ProxyProtocol::Mixed, ProxyVariable::All) => Some("socks5"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(protocol: ProxyProtocol) -> ProxyEndpoint {
        ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: 7897,
            protocol,
        }
    }

    fn value(plan: &ProxyEnvironmentPlan, variable: ProxyVariable) -> Option<&str> {
        plan.variables
            .iter()
            .find(|entry| variable_matches_name(variable, &entry.name))
            .and_then(|entry| entry.value.as_deref())
    }

    #[test]
    fn maps_http_to_http_and_https_variables() {
        let plan = build_proxy_environment_plan(
            &endpoint(ProxyProtocol::Http),
            &[
                ProxyVariable::Http,
                ProxyVariable::Https,
                ProxyVariable::All,
            ],
        )
        .unwrap();

        assert_eq!(
            value(&plan, ProxyVariable::Http),
            Some("http://127.0.0.1:7897")
        );
        assert_eq!(
            value(&plan, ProxyVariable::Https),
            Some("http://127.0.0.1:7897")
        );
        assert_eq!(value(&plan, ProxyVariable::All), None);
    }

    #[test]
    fn maps_socks5_to_all_proxy_only() {
        let plan = build_proxy_environment_plan(
            &endpoint(ProxyProtocol::Socks5),
            &[
                ProxyVariable::Http,
                ProxyVariable::Https,
                ProxyVariable::All,
            ],
        )
        .unwrap();

        assert_eq!(value(&plan, ProxyVariable::Http), None);
        assert_eq!(value(&plan, ProxyVariable::Https), None);
        assert_eq!(
            value(&plan, ProxyVariable::All),
            Some("socks5://127.0.0.1:7897")
        );
    }

    #[test]
    fn maps_mixed_to_both_compatible_schemes() {
        let plan = build_proxy_environment_plan(
            &endpoint(ProxyProtocol::Mixed),
            &[
                ProxyVariable::Http,
                ProxyVariable::Https,
                ProxyVariable::All,
            ],
        )
        .unwrap();

        assert_eq!(
            value(&plan, ProxyVariable::Http),
            Some("http://127.0.0.1:7897")
        );
        assert_eq!(
            value(&plan, ProxyVariable::Https),
            Some("http://127.0.0.1:7897")
        );
        assert_eq!(
            value(&plan, ProxyVariable::All),
            Some("socks5://127.0.0.1:7897")
        );
    }

    #[test]
    fn selected_variables_limit_the_plan() {
        let plan =
            build_proxy_environment_plan(&endpoint(ProxyProtocol::Mixed), &[ProxyVariable::Https])
                .unwrap();

        assert_eq!(value(&plan, ProxyVariable::Http), None);
        assert_eq!(
            value(&plan, ProxyVariable::Https),
            Some("http://127.0.0.1:7897")
        );
        assert_eq!(value(&plan, ProxyVariable::All), None);
    }

    #[test]
    fn brackets_ipv6_hosts() {
        let plan = build_proxy_environment_plan(
            &ProxyEndpoint {
                host: "::1".into(),
                port: 10808,
                protocol: ProxyProtocol::Socks5,
            },
            &[ProxyVariable::All],
        )
        .unwrap();

        assert_eq!(
            value(&plan, ProxyVariable::All),
            Some("socks5://[::1]:10808")
        );
    }

    #[test]
    fn rejects_unknown_protocol_and_invalid_endpoint() {
        assert!(build_proxy_environment_plan(
            &endpoint(ProxyProtocol::Unknown),
            &[ProxyVariable::Http]
        )
        .is_err());
        assert!(build_proxy_environment_plan(
            &ProxyEndpoint {
                host: "  ".into(),
                port: 7897,
                protocol: ProxyProtocol::Http,
            },
            &[ProxyVariable::Http]
        )
        .is_err());
        assert!(build_proxy_environment_plan(
            &ProxyEndpoint {
                host: "localhost".into(),
                port: 0,
                protocol: ProxyProtocol::Http,
            },
            &[ProxyVariable::Http]
        )
        .is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_uses_uppercase_variable_names() {
        assert_eq!(
            managed_variable_names(),
            ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"]
        );
        assert_eq!(all_proxy_variable_names()[3], "NO_PROXY");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn unix_desktop_platforms_use_lowercase_variable_names() {
        assert_eq!(
            managed_variable_names(),
            ["http_proxy", "https_proxy", "all_proxy"]
        );
        assert_eq!(all_proxy_variable_names()[3], "no_proxy");
    }
}
