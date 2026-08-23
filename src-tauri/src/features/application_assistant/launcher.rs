use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    error::{ProxyEnvError, Result},
    features::proxy::{plan::build_proxy_environment_plan, ProxyEndpoint, ProxyVariable},
};

use super::{LaunchApplicationResult, LaunchEnvironmentMode, ManagedApplication};

const PROXY_VARIABLES_ALL_CASES: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
];

const ALL_PROXY_VARIABLES: &[ProxyVariable] = &[
    ProxyVariable::Http,
    ProxyVariable::Https,
    ProxyVariable::All,
];

pub fn launch(
    application: &ManagedApplication,
    environment_mode: LaunchEnvironmentMode,
    endpoint: Option<&ProxyEndpoint>,
) -> Result<LaunchApplicationResult> {
    let executable_path = validate_executable(&application.executable_path)?;
    let mut command = Command::new(&executable_path);
    configure_environment(&mut command, environment_mode, endpoint)?;
    let child = command
        .spawn()
        .map_err(|error| ProxyEnvError::ApplicationLaunch(error.to_string()))?;
    Ok(LaunchApplicationResult {
        pid: child.id(),
        executable_path,
        environment_mode,
    })
}

fn configure_environment(
    command: &mut Command,
    environment_mode: LaunchEnvironmentMode,
    endpoint: Option<&ProxyEndpoint>,
) -> Result<()> {
    match environment_mode {
        LaunchEnvironmentMode::Inherit => {}
        LaunchEnvironmentMode::ClearProxyVariables => clear_proxy_variables(command),
        LaunchEnvironmentMode::UseCurrentProxy => {
            let endpoint = endpoint.ok_or(ProxyEnvError::ActiveProxyMissing)?;
            let plan = build_proxy_environment_plan(endpoint, ALL_PROXY_VARIABLES)?;
            clear_proxy_variables(command);
            for variable in plan.variables {
                if let Some(value) = variable.value {
                    command.env(variable.name, value);
                }
            }
        }
    }
    Ok(())
}

fn clear_proxy_variables(command: &mut Command) {
    for name in PROXY_VARIABLES_ALL_CASES {
        command.env_remove(name);
    }
}

fn validate_executable(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(ProxyEnvError::InvalidApplication(
            "the executable path must be absolute".into(),
        ));
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| ProxyEnvError::InvalidApplication(error.to_string()))?;
    let metadata = canonical
        .metadata()
        .map_err(|error| ProxyEnvError::InvalidApplication(error.to_string()))?;
    if !metadata.is_file() {
        return Err(ProxyEnvError::InvalidApplication(
            "the selected path is not a file".into(),
        ));
    }
    if !is_supported_executable(&canonical) {
        return Err(ProxyEnvError::InvalidApplication(
            "the selected file is not a supported executable".into(),
        ));
    }
    Ok(canonical)
}

#[cfg(windows)]
fn is_supported_executable(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

#[cfg(not(windows))]
fn is_supported_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, ffi::OsString};

    use super::*;
    use crate::features::proxy::ProxyProtocol;

    fn endpoint(protocol: ProxyProtocol) -> ProxyEndpoint {
        ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: 7897,
            protocol,
        }
    }

    fn configured_environment(command: &Command) -> HashMap<OsString, Option<OsString>> {
        command
            .get_envs()
            .map(|(name, value)| (name.to_owned(), value.map(OsStr::to_owned)))
            .collect()
    }

    fn environment_entry<'a>(
        environment: &'a HashMap<OsString, Option<OsString>>,
        name: &str,
    ) -> Option<&'a Option<OsString>> {
        #[cfg(windows)]
        {
            environment.iter().find_map(|(key, value)| {
                key.to_string_lossy()
                    .eq_ignore_ascii_case(name)
                    .then_some(value)
            })
        }
        #[cfg(not(windows))]
        {
            environment.get(OsStr::new(name))
        }
    }

    fn environment_value<'a>(
        environment: &'a HashMap<OsString, Option<OsString>>,
        name: &str,
    ) -> Option<&'a OsStr> {
        environment_entry(environment, name).and_then(|value| value.as_deref())
    }

    #[test]
    fn inherit_keeps_the_command_environment_unchanged() {
        let mut command = Command::new("example");
        command.env("HTTP_PROXY", "parent-value");
        command.env("PROXYENV_KEEP", "keep-me");
        let before = configured_environment(&command);

        configure_environment(&mut command, LaunchEnvironmentMode::Inherit, None).unwrap();

        assert_eq!(configured_environment(&command), before);
    }

    #[test]
    fn current_proxy_overrides_stale_values_and_preserves_other_environment() {
        let mut command = Command::new("example");
        for name in PROXY_VARIABLES_ALL_CASES {
            command.env(name, "stale");
        }
        command.env("PROXYENV_KEEP", "keep-me");

        configure_environment(
            &mut command,
            LaunchEnvironmentMode::UseCurrentProxy,
            Some(&endpoint(ProxyProtocol::Mixed)),
        )
        .unwrap();
        let environment = configured_environment(&command);

        #[cfg(windows)]
        {
            assert_eq!(
                environment_value(&environment, "HTTP_PROXY"),
                Some(OsStr::new("http://127.0.0.1:7897"))
            );
            assert_eq!(
                environment_value(&environment, "HTTPS_PROXY"),
                Some(OsStr::new("http://127.0.0.1:7897"))
            );
            assert_eq!(
                environment_value(&environment, "ALL_PROXY"),
                Some(OsStr::new("socks5://127.0.0.1:7897"))
            );
        }
        #[cfg(not(windows))]
        {
            assert_eq!(
                environment_value(&environment, "http_proxy"),
                Some(OsStr::new("http://127.0.0.1:7897"))
            );
            assert_eq!(
                environment_value(&environment, "https_proxy"),
                Some(OsStr::new("http://127.0.0.1:7897"))
            );
            assert_eq!(
                environment_value(&environment, "all_proxy"),
                Some(OsStr::new("socks5://127.0.0.1:7897"))
            );
            for name in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"] {
                assert!(environment_entry(&environment, name).is_some_and(Option::is_none));
            }
        }
        assert_eq!(
            environment_value(&environment, "PROXYENV_KEEP"),
            Some(OsStr::new("keep-me"))
        );
    }

    #[test]
    fn clear_proxy_variables_removes_both_cases_only() {
        let mut command = Command::new("example");
        for name in PROXY_VARIABLES_ALL_CASES {
            command.env(name, "stale");
        }
        command.env("PROXYENV_KEEP", "keep-me");

        configure_environment(
            &mut command,
            LaunchEnvironmentMode::ClearProxyVariables,
            None,
        )
        .unwrap();
        let environment = configured_environment(&command);

        for name in PROXY_VARIABLES_ALL_CASES {
            assert!(environment_entry(&environment, name).is_some_and(Option::is_none));
        }
        assert_eq!(
            environment_value(&environment, "PROXYENV_KEEP"),
            Some(OsStr::new("keep-me"))
        );
    }

    #[test]
    fn use_current_proxy_requires_a_known_endpoint() {
        let mut command = Command::new("example");
        assert!(
            configure_environment(&mut command, LaunchEnvironmentMode::UseCurrentProxy, None)
                .is_err()
        );
        assert!(configure_environment(
            &mut command,
            LaunchEnvironmentMode::UseCurrentProxy,
            Some(&endpoint(ProxyProtocol::Unknown))
        )
        .is_err());
    }

    #[test]
    fn validates_absolute_executable_files_only() {
        assert!(validate_executable(Path::new("relative-application.exe")).is_err());
        assert!(validate_executable(&std::env::current_dir().unwrap()).is_err());
        assert_eq!(
            validate_executable(&std::env::current_exe().unwrap()).unwrap(),
            std::env::current_exe().unwrap().canonicalize().unwrap()
        );
    }

    #[cfg(windows)]
    #[test]
    fn child_process_receives_each_launch_environment_mode() {
        run_child_probe(
            LaunchEnvironmentMode::Inherit,
            None,
            Some("stale-http"),
            Some("stale-all"),
        );
        run_child_probe(
            LaunchEnvironmentMode::UseCurrentProxy,
            Some(&endpoint(ProxyProtocol::Mixed)),
            Some("http://127.0.0.1:7897"),
            Some("socks5://127.0.0.1:7897"),
        );
        run_child_probe(LaunchEnvironmentMode::ClearProxyVariables, None, None, None);
    }

    #[cfg(windows)]
    fn run_child_probe(
        mode: LaunchEnvironmentMode,
        endpoint: Option<&ProxyEndpoint>,
        expected_http: Option<&str>,
        expected_all: Option<&str>,
    ) {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "features::application_assistant::launcher::tests::child_environment_probe",
            "--ignored",
        ]);
        command.env("HTTP_PROXY", "stale-http");
        command.env("HTTPS_PROXY", "stale-http");
        command.env("ALL_PROXY", "stale-all");
        command.env("PROXYENV_KEEP", "keep-me");
        command.env("PROXYENV_EXPECT_HTTP", expected_http.unwrap_or("<absent>"));
        command.env("PROXYENV_EXPECT_ALL", expected_all.unwrap_or("<absent>"));
        configure_environment(&mut command, mode, endpoint).unwrap();

        let status = command.status().unwrap();
        assert!(status.success());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "executed as a child by child_process_receives_each_launch_environment_mode"]
    fn child_environment_probe() {
        assert_eq!(std::env::var("PROXYENV_KEEP").as_deref(), Ok("keep-me"));
        assert_expected_environment("HTTP_PROXY", "PROXYENV_EXPECT_HTTP");
        assert_expected_environment("HTTPS_PROXY", "PROXYENV_EXPECT_HTTP");
        assert_expected_environment("ALL_PROXY", "PROXYENV_EXPECT_ALL");
    }

    #[cfg(windows)]
    fn assert_expected_environment(name: &str, expected_name: &str) {
        let expected = std::env::var(expected_name).unwrap();
        if expected == "<absent>" {
            assert!(std::env::var_os(name).is_none(), "{name} should be absent");
        } else {
            assert_eq!(std::env::var(name).as_deref(), Ok(expected.as_str()));
        }
    }
}
