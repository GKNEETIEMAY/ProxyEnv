struct DiscoveryTransport;
impl super::ssh::ManagedSsh for DiscoveryTransport {
    fn is_running(&mut self) -> super::BridgeResult<bool> {
        Ok(true)
    }
}
fn automatic_setup_fixture() -> super::Store {
    use super::*;
    Store {
        child: Some(Box::new(DiscoveryTransport)),
        summary: Summary {
            cc: Some(Endpoint {
                local: ProxyEndpoint {
                    host: "127.0.0.1".into(),
                    port: 15721,
                    protocol: ProxyProtocol::Http,
                },
                remote_port: 15721,
            }),
            cc_status: Some(Status::Connected),
            tool_installations: tool_adapter::unknown_installations(),
            ..Summary::default()
        },
        ..Store::default()
    }
}
#[test]
fn automatic_egress_requires_a_live_unchecked_network_lane() {
    use super::*;
    let mut summary = Summary {
        proxy: Some(Endpoint {
            local: ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: 7897,
                protocol: ProxyProtocol::Mixed,
            },
            remote_port: 7897,
        }),
        proxy_status: Some(Status::Connected),
        ..Summary::default()
    };
    assert!(should_auto_proxy_test(&summary, true));
    assert!(!should_auto_proxy_test(&summary, false));
    for status in [
        None,
        Some(Status::Connecting),
        Some(Status::Disconnected),
        Some(Status::Error),
    ] {
        summary.proxy_status = status;
        assert!(!should_auto_proxy_test(&summary, true));
    }
    summary.proxy_status = Some(Status::Connected);
    for state in [
        DiagnosticState::Testing,
        DiagnosticState::Passed,
        DiagnosticState::Failed,
    ] {
        summary.diagnostics.general_proxy_egress.state = state;
        assert!(!should_auto_proxy_test(&summary, true));
    }
    summary.diagnostics.general_proxy_egress.reset();
    assert!(should_auto_proxy_test(&summary, true));
    summary.proxy = None;
    assert!(!should_auto_proxy_test(&summary, true));
    // An AI-only bridge never triggers a general-network or model request.
    let ai_only = automatic_setup_fixture();
    assert!(!should_auto_proxy_test(&ai_only.summary, true));
    assert_eq!(
        ai_only.summary.diagnostics.ai_route_verification.state,
        DiagnosticState::NotTested
    );
}
#[test]
fn automatic_setup_requires_an_enabled_route_and_compatible_cli_or_plugin() {
    use super::*;
    use tool_adapter::{RemoteToolId as T, RemoteToolInstallState as I};
    for tool in [T::Codex, T::Claude] {
        for cli in [
            I::Unknown,
            I::Checking,
            I::Installed,
            I::Missing,
            I::Unsupported,
            I::Broken,
        ] {
            for plugin in [
                I::Unknown,
                I::Checking,
                I::Installed,
                I::Missing,
                I::Unsupported,
                I::Broken,
            ] {
                let mut state = automatic_setup_fixture();
                let item = state
                    .summary
                    .tool_installations
                    .iter_mut()
                    .find(|item| item.tool == tool)
                    .unwrap();
                item.state = cli;
                item.extension_state = plugin;
                let eligible = cli != I::Checking
                    && plugin != I::Checking
                    && (cli == I::Installed || plugin == I::Installed);
                assert_eq!(
                    automatic_tool_setup_allowed(&state, tool),
                    eligible,
                    "{tool:?}/{cli:?}/{plugin:?}"
                );
                state.summary.cc_status = Some(Status::Disconnected);
                assert!(!automatic_tool_setup_allowed(&state, tool));
                state.summary.cc_status = Some(Status::Connected);
                state.summary.cc = None;
                assert!(!automatic_tool_setup_allowed(&state, tool));
            }
        }
    }
}
#[test]
fn automatic_setup_preserves_explicit_off_and_skips_already_configured_tools() {
    use super::*;
    use tool_adapter::{RemoteToolId as T, RemoteToolInstallState as I};
    let mut state = automatic_setup_fixture();
    for item in &mut state.summary.tool_installations {
        item.state = I::Installed;
    }
    state.disabled_tools.push(T::Codex);
    assert!(!automatic_tool_setup_allowed(&state, T::Codex));
    assert!(automatic_tool_setup_allowed(&state, T::Claude));
    // Discovery and a new transport do not clear the user's per-tool OFF.
    state.summary.tool_installations = tool_adapter::unknown_installations();
    state.summary.tool_installations[0].extension_state = I::Installed;
    state.child = Some(Box::new(DiscoveryTransport));
    assert!(!automatic_tool_setup_allowed(&state, T::Codex));
    state.disabled_tools.clear();
    assert!(automatic_tool_setup_allowed(&state, T::Codex));
    state.summary.codex_configured = true;
    assert!(!automatic_tool_setup_allowed(&state, T::Codex));
    state.child = None;
    assert!(!automatic_tool_setup_allowed(&state, T::Claude));
}
#[test]
fn installation_snapshot_invalidation_and_generation_guards_are_independent_of_core_bridge() {
    use super::*;
    let target:RemoteTarget=serde_json::from_value(json!({"id":"discovery-target","displayName":"Test","source":"manual","sourceLabel":"Test","configPath":"","sshAlias":null,"host":"test.example","user":null,"port":22,"identityFile":null,"authenticationMethod":"agent","available":true,"compatibility":"compatible","unavailableReason":null,"canOpenVscode":false,"canOpenMobaxterm":false})).unwrap();
    let mut state = Store {
        child: Some(Box::new(DiscoveryTransport)),
        connection_generation: 7,
        tool_discovery_ticket: 9,
        target_fingerprint: Some("first".into()),
        tool_discovery_fingerprint: Some("first".into()),
        ..Store::default()
    };
    state.summary.target = Some(target);
    state.summary.status = Status::Connected;
    state.summary.codex_configured = true;
    state.summary.tool_installations = vec![
        tool_adapter::project_installation(tool_adapter::RemoteToolId::Codex, false, false, None),
        tool_adapter::project_installation(
            tool_adapter::RemoteToolId::Claude,
            true,
            true,
            Some("2.1.227"),
        ),
    ];
    assert!(discovery_context_current(
        &state,
        "discovery-target",
        "first",
        7,
        9
    ));
    for (target, fingerprint, generation, ticket) in [
        ("other", "first", 7, 9),
        ("discovery-target", "other", 7, 9),
        ("discovery-target", "first", 6, 9),
        ("discovery-target", "first", 7, 8),
    ] {
        assert!(!discovery_context_current(
            &state,
            target,
            fingerprint,
            generation,
            ticket
        ));
    }
    invalidate_tool_discovery(&mut state, Some("first"));
    assert_eq!(
        state.summary.tool_installations[0].state,
        tool_adapter::RemoteToolInstallState::Missing
    );
    let report = serde_json::to_value(project_report(&state)).unwrap();
    assert_eq!(report["toolInstallations"][1]["version"], "2.1.227");
    assert!(!report.to_string().contains("test.example"));
    invalidate_tool_discovery(&mut state, Some("changed"));
    assert!(state
        .summary
        .tool_installations
        .iter()
        .all(|tool| tool.state == tool_adapter::RemoteToolInstallState::Unknown));
    assert!(!discovery_context_current(
        &state,
        "discovery-target",
        "first",
        7,
        9
    ));
    assert_eq!(state.summary.status, Status::Connected);
    assert!(state.summary.codex_configured);
    assert!(state.child.is_some());
}
#[test]
fn terminal_command_is_a_read_only_session_scoped_projection() {
    let mut summary = super::Summary {
        proxy: Some(super::Endpoint {
            local: super::ProxyEndpoint {
                host: "127.0.0.1".into(),
                port: 7897,
                protocol: super::ProxyProtocol::Mixed,
            },
            remote_port: 7897,
        }),
        proxy_status: Some(super::Status::Connected),
        session_environment_state: super::RuntimeState::Ready,
        ..super::Summary::default()
    };
    let command = super::projected_session_command(&summary, true, Some("session-one"));
    assert_eq!(
        command.as_deref(),
        Some(". \"$HOME/.proxyenv/sessions/session-one/env.sh\"")
    );
    assert_eq!(
        super::projected_session_command(&summary, true, Some("session-one")),
        command
    );
    assert_ne!(
        super::projected_session_command(&summary, true, Some("session-two")),
        command
    );
    assert!(super::projected_session_command(&summary, false, Some("session-one")).is_none());
    assert!(super::projected_session_command(&summary, true, None).is_none());
    for state in [
        super::RuntimeState::Pending,
        super::RuntimeState::Preparing,
        super::RuntimeState::Warning,
    ] {
        summary.session_environment_state = state;
        assert!(super::projected_session_command(&summary, true, Some("session-one")).is_none());
    }
    summary.session_environment_state = super::RuntimeState::Ready;
    summary.proxy_status = Some(super::Status::Disconnected);
    assert!(super::projected_session_command(&summary, true, Some("session-one")).is_none());
    summary.proxy_status = Some(super::Status::Connected);
    summary.proxy = None;
    assert!(super::projected_session_command(&summary, true, Some("session-one")).is_none());
}
use super::*;

#[test]
fn profile_polling_alternates_even_when_one_tool_cannot_sync() {
    use tool_adapter::RemoteToolId::{Claude, Codex};
    assert_eq!(profile_poll_order(Codex), [Claude, Codex]);
    assert_eq!(profile_poll_order(Claude), [Codex, Claude]);
}

#[test]
fn summary_exposes_independent_codex_sync_state_after_reenable() {
    use settings::ProfileSyncState;
    let mut summary = Summary {
        claude_profile_state: ProfileSyncState::InvalidLocalProfile,
        ..Summary::default()
    };
    let adapter = tool_adapter::by_id(tool_adapter::RemoteToolId::Codex);
    for synced in [true, false, true] {
        if synced {
            adapter.apply(&mut summary);
            summary.codex_profile_state = ProfileSyncState::Synced;
        } else {
            adapter.restore(&mut summary);
            summary.codex_profile_state = ProfileSyncState::NotStarted;
        }
        let value = serde_json::to_value(exposed_summary(&summary)).unwrap();
        assert_eq!(value["codexConfigured"], synced);
        assert_eq!(
            value["codexProfileState"],
            if synced { "synced" } else { "notStarted" }
        );
        assert_eq!(value["claudeProfileState"], "invalidLocalProfile");
        assert_eq!(value["tools"][0]["configured"], synced);
    }
}

#[test]
fn inactive_local_routes_are_reported_as_unconfigured_warnings() {
    let mut state = Store::default();
    for tool in [
        tool_adapter::RemoteToolId::Codex,
        tool_adapter::RemoteToolId::Claude,
    ] {
        record_inactive_tool_route(&mut state, tool);
        let event = serde_json::to_value(&state.events.recent(1)[0]).unwrap();
        assert_eq!(event["action"], "toolSetup");
        assert_eq!(event["outcome"], "warning");
        assert_eq!(event["level"], "warning");
        assert_eq!(event["errorCode"], "localRouteInactive");
        assert!(!tool_adapter::by_id(tool).configured(&state.summary));
        assert!(state.summary.post_connect_error.is_none());
    }
}

#[test]
fn report_projection_omits_untrusted_text_and_preserves_cached_proof() {
    let mut state = Store::default();
    state.summary.environment = "secret-password /home/private/user".into();
    state.summary.error = Some("https://private.invalid/?token=secret".into());
    state.summary.post_connect_error = Some("Authorization: secret-token".into());
    state.summary.codex_extension = Some("/home/private/settings.json".into());
    state.summary.claude_extension = Some("configured".into());
    state.summary.timings = vec![
        PhaseTiming {
            phase: "ssh.authenticate".into(),
            duration_ms: 4,
            outcome: "success".into(),
        },
        PhaseTiming {
            phase: "secret-token".into(),
            duration_ms: 6,
            outcome: "secret-password".into(),
        },
    ];
    let ticket = state.summary.diagnostics.general_proxy_egress.begin();
    state
        .summary
        .diagnostics
        .general_proxy_egress
        .finish(ticket, None, 12);
    let report = serde_json::to_value(project_report(&state)).unwrap();
    assert_eq!(
        report["diagnostics"]["generalProxyEgress"]["state"],
        "passed"
    );
    assert_eq!(report["phaseTimings"][0]["phase"], "ssh.authenticate");
    assert_eq!(report["phaseTimings"][1]["phase"], "unknown");
    assert_eq!(report["postConnectErrorCategory"], "unknown");
    let text = report.to_string();
    for forbidden in [
        "private",
        "secret-token",
        "secret-password",
        "Authorization",
        "environment",
        "target",
        "revision",
        "passwordStored",
    ] {
        assert!(!text.contains(forbidden), "report leaked {forbidden}");
    }
}

#[test]
fn diagnostic_report_preserves_verified_snapshot_during_store_contention() {
    let source = StateStore::default();
    {
        let mut state = source.lock().unwrap();
        state.summary.status = Status::Connected;
        state.reachable = true;
        state.summary.codex_configured = true;
        state.summary.claude_configured = true;
        state.summary.runtime_proxy_match = RuntimeProxyMatch::Matched;
        state.summary.tool_installations = vec![tool_adapter::project_installation(
            tool_adapter::RemoteToolId::Claude,
            true,
            true,
            Some("2.1.227"),
        )];
        let ticket = state.summary.diagnostics.general_proxy_egress.begin();
        state
            .summary
            .diagnostics
            .general_proxy_egress
            .finish(ticket, None, 12);
        state.summary.environment = "private-host secret-token /home/private".into();
    }
    let expected = serde_json::to_value(source.report()).unwrap();
    assert_eq!(expected["status"], "connected");
    assert_eq!(expected["toolInstallations"][0]["version"], "2.1.227");
    let _busy = source.lock().unwrap();
    for _ in 0..3 {
        assert_eq!(
            serde_json::to_value(source.report()).unwrap(),
            expected,
            "A busy bridge must not replace verified report data with defaults"
        );
    }
    assert!(!expected.to_string().contains("private"));
    assert!(!expected.to_string().contains("secret-token"));
}

#[test]
fn diagnostic_report_publishes_complete_updates_and_clears_previous_session() {
    let source = StateStore::default();
    {
        let mut state = source.lock().unwrap();
        state.summary.status = Status::Connected;
        state.summary.codex_configured = true;
    }
    let previous = serde_json::to_value(source.report()).unwrap();
    let mut state = source.try_lock().unwrap();
    state.summary.status = Status::Stale;
    state.summary.codex_configured = false;
    assert_eq!(serde_json::to_value(source.report()).unwrap(), previous);
    drop(state);
    let current = source.report();
    assert_eq!(current.status, Status::Stale);
    assert!(!current.codex_configured);
    {
        let mut state = source.lock().unwrap();
        *state = Store::default();
    }
    let current = source.report();
    assert_eq!(current.status, Status::Disconnected);
    assert!(!current.codex_configured);
    assert_eq!(
        current.diagnostics.general_proxy_egress.state,
        DiagnosticState::NotTested
    );
    let other = StateStore::default();
    assert_eq!(other.report().status, Status::Disconnected);
}

#[test]
fn cc_route_discovery_tracks_changed_ports_and_excludes_foreign_listeners() {
    use crate::features::proxy::{listeners::TcpListener, processes::RunningProcess};
    let processes = std::collections::HashMap::from([
        (
            1,
            RunningProcess {
                pid: 1,
                name: "cc-switch.exe".into(),
            },
        ),
        (
            2,
            RunningProcess {
                pid: 2,
                name: "other.exe".into(),
            },
        ),
    ]);
    let listener = |port, pid| TcpListener {
        host: "127.0.0.1".into(),
        port,
        pids: vec![pid],
    };
    let old = listener(15721, 1);
    let new = listener(15722, 1);
    let foreign = listener(7897, 2);
    assert_eq!(
        cc_route_ports(&[old.clone(), new.clone(), old], &processes, 15721),
        vec![15721, 15722]
    );
    assert_eq!(
        cc_route_ports(&[new, foreign.clone()], &processes, 15721),
        vec![15722]
    );
    assert!(cc_route_ports(&[foreign], &processes, 15721).is_empty());
}

#[test]
fn cc_route_health_requires_bounded_healthy_json_without_redirects() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    for (status, body, expected) in [
        (200, r#"{"status":"healthy"}"#.to_owned(), true),
        (200, r#"{"status":"unhealthy"}"#.to_owned(), false),
        (200, "not json".to_owned(), false),
        (200, " ".repeat(4097), false),
        (302, r#"{"status":"healthy"}"#.to_owned(), false),
    ] {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = server.local_addr().unwrap().port();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 4096];
            assert!(stream.read(&mut request).unwrap() > 0);
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        assert_eq!(cc_route_healthy(&client, port), expected);
        worker.join().unwrap();
    }
}
#[test]
fn route_actions_depend_only_on_their_own_capability_and_ssh() {
    for unrelated in [Status::Stale, Status::Unavailable, Status::Connected] {
        assert!(capability_ready(true, Some(Status::Connected)));
        assert_eq!(
            capability_ready(true, Some(unrelated)),
            unrelated == Status::Connected
        );
    }
    assert!(!capability_ready(false, Some(Status::Connected)));
    assert!(!capability_ready(true, None));
}

#[test]
fn remote_environment_obeys_protocol_and_uses_remote_socks_dns() {
    let endpoint = |protocol| Endpoint {
        local: ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: 10809,
            protocol,
        },
        remote_port: 17897,
    };
    assert_eq!(
        remote_environment(&endpoint(ProxyProtocol::Http)).unwrap(),
        "unset HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY\nexport HTTP_PROXY=http://127.0.0.1:17897\nexport HTTPS_PROXY=http://127.0.0.1:17897\nexport NO_PROXY=localhost,127.0.0.1,::1"
    );
    assert_eq!(
        remote_environment(&endpoint(ProxyProtocol::Socks5)).unwrap(),
        "unset HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY\nexport ALL_PROXY=socks5h://127.0.0.1:17897\nexport NO_PROXY=localhost,127.0.0.1,::1"
    );
    assert_eq!(
        remote_environment(&endpoint(ProxyProtocol::Mixed))
            .unwrap()
            .lines()
            .count(),
        5
    );
    assert!(remote_environment(&endpoint(ProxyProtocol::Unknown)).is_err());
}
#[test]
fn codex_projection_contains_profile_and_claude_remains_route_only() {
    let codex: toml::Value = toml::from_str(
        &tool_adapter::by_name("codex")
            .unwrap()
            .preview(25721, Some("deepseek-flash"))
            .content,
    )
    .unwrap();
    assert_eq!(codex["model_provider"].as_str(), Some("proxyenv_bridge"));
    assert_eq!(codex["model"].as_str(), Some("deepseek-flash"));
    assert_eq!(
        codex["model_catalog_json"].as_str(),
        Some("proxyenv-codex-model-catalog.json")
    );
    assert_eq!(codex.as_table().unwrap().len(), 4);
    let claude: serde_json::Value = serde_json::from_str(
        &tool_adapter::by_name("claude")
            .unwrap()
            .preview(25721, None)
            .content,
    )
    .unwrap();
    assert_eq!(claude["env"]["ANTHROPIC_AUTH_TOKEN"], "PROXY_MANAGED");
}
#[test]
fn remote_ports_exclude_privileged_range() {
    assert!(port(1023).is_err());
    assert!(port(1024).is_ok());
    assert!(port(65535).is_ok());
}
#[test]
fn remote_port_fallbacks_are_repeatable_and_independent() {
    assert_eq!(DEFAULT_CC_REMOTE_PORT, 15_721);
    assert_eq!(
        derived_port("same-ssh-target", 0, 0),
        derived_port("same-ssh-target", 0, 0)
    );
    assert_ne!(
        derived_port("same-ssh-target", 0, 0),
        derived_port("same-ssh-target", 0, 1)
    );
    assert!((20_000..=60_000).contains(&derived_port("same-ssh-target", 0, 0)));
}

#[test]
fn runtime_proxy_port_and_local_upstream_are_independent() {
    let expected = 7897;
    for active in [expected, 10809, 2080] {
        let allocation =
            resolve_port_pair("fixture", Some(active), Some(expected), true, |_| Ok(true)).unwrap();
        assert_eq!(allocation.proxy_port, expected);
        assert_eq!(allocation.cc_port, DEFAULT_CC_REMOTE_PORT);
        assert!(!allocation.runtime_port_conflict);
    }
    let no_runtime = resolve_port_pair("fixture", Some(10809), None, true, |_| Ok(true)).unwrap();
    assert_eq!(no_runtime.proxy_port, 10809);
    assert_eq!(no_runtime.runtime_expected_proxy_port, None);
    let switched =
        resolve_port_pair("fixture", Some(2080), Some(expected), true, |_| Ok(true)).unwrap();
    assert_eq!(switched.proxy_port, expected);
}

#[test]
fn occupied_runtime_proxy_port_falls_back_without_moving_cc() {
    let expected = 7897;
    let fallback = derived_port("fixture", 0, 0);
    let result = resolve_port_pair("fixture", Some(10809), Some(expected), true, |port| {
        Ok(port != expected)
    })
    .unwrap();
    assert_eq!(result.proxy_port, fallback);
    assert!(result.runtime_port_conflict);
    assert_eq!(result.cc_port, DEFAULT_CC_REMOTE_PORT);
    assert_eq!(
        runtime_proxy_match(result.runtime_expected_proxy_port, Some(result.proxy_port)),
        RuntimeProxyMatch::Mismatch
    );
    let cc_conflict = resolve_port_pair("fixture", Some(10809), Some(expected), true, |port| {
        Ok(port != DEFAULT_CC_REMOTE_PORT)
    })
    .unwrap();
    assert_eq!(cc_conflict.proxy_port, expected);
    assert_eq!(
        runtime_proxy_match(
            cc_conflict.runtime_expected_proxy_port,
            Some(cc_conflict.proxy_port)
        ),
        RuntimeProxyMatch::Matched
    );
    assert_eq!(cc_conflict.cc_port, derived_port("fixture", 0, 1));
    assert!(!cc_conflict.runtime_port_conflict);
    let shared_preference = resolve_port_pair(
        "fixture",
        Some(10809),
        Some(DEFAULT_CC_REMOTE_PORT),
        true,
        |_| Ok(true),
    )
    .unwrap();
    assert_eq!(shared_preference.proxy_port, DEFAULT_CC_REMOTE_PORT);
    assert_ne!(shared_preference.cc_port, shared_preference.proxy_port);
    let no_runtime =
        resolve_port_pair("fixture", Some(10809), None, true, |port| Ok(port != 10809)).unwrap();
    assert_eq!(no_runtime.proxy_port, fallback);
    assert_eq!(no_runtime.cc_port, DEFAULT_CC_REMOTE_PORT);
    assert_eq!(
        runtime_proxy_match(None, Some(no_runtime.proxy_port)),
        RuntimeProxyMatch::Unknown
    );
}
#[test]
fn revision_changes_never_retarget_an_existing_tunnel() {
    let original = Endpoint {
        local: ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: 10809,
            protocol: ProxyProtocol::Mixed,
        },
        remote_port: 17897,
    };
    let summary = Summary {
        proxy: Some(original),
        active_proxy_revision: Some(8),
        ..Summary::default()
    };
    let mut context = active::ActiveProxyContext {
        available: true,
        revision: 8,
        ..Default::default()
    };
    assert_eq!(
        observed_status(&summary, Some(&context), true),
        Status::Connected
    );
    context.revision = 9;
    assert_eq!(
        observed_status(&summary, Some(&context), true),
        Status::Stale
    );
    context.available = false;
    assert_eq!(
        observed_status(&summary, Some(&context), true),
        Status::Unavailable
    );
    assert_eq!(summary.proxy.unwrap().local.port, 10809);
    assert_eq!(
        observed_status(&Summary::default(), None, true),
        Status::Connected
    );
    assert_eq!(
        observed_status(&Summary::default(), None, false),
        Status::Unavailable
    );
}
#[test]
fn request_cannot_override_local_endpoint_or_add_commands() {
    let valid = json!({"targetId":"openssh|example|dev-vm","proxyPort":23841,"ccPort":null,"ccLocalPort":15721,"expectedRevision":8});
    assert!(serde_json::from_value::<Request>(valid.clone()).is_ok());
    let mut value = valid;
    value["localHost"] = json!("0.0.0.0");
    assert!(serde_json::from_value::<Request>(value).is_err());
    for invalid in [-1, 65536] {
        let value = json!({"targetId":"openssh|example|dev-vm","proxyPort":invalid,"ccPort":null,"ccLocalPort":15721,"expectedRevision":8});
        assert!(serde_json::from_value::<Request>(value).is_err());
    }
}
#[test]
fn report_does_not_expose_target_or_paths() {
    let value = serde_json::to_string(&Report::default()).unwrap();
    for forbidden in ["alias", "host", "path", "before", "after", "environment"] {
        assert!(!value.contains(forbidden));
    }
}
