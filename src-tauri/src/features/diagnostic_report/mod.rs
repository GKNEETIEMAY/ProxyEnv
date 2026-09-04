//! Read-only, allowlisted report DTO. Raw paths, endpoints, errors and rule values never cross it.
use std::collections::HashSet;

use serde::Serialize;

use crate::{
    error::{ProxyEnvError, Result},
    features::{
        application_assistant::{
            self, ApplicationDiagnosis, ApplicationNetworkState, RecommendedAction,
        },
        network_observation::{self, TunObservationState},
        proxy::{
            self,
            connectivity::{self, ConnectivityErrorKind, ProxyConnectivityState},
            Confidence, ProxyEnvironmentState, ProxyEnvironmentStatus, ProxyProtocol,
            ProxyVariable,
        },
    },
    services::{redaction, settings},
};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AppCategory {
    NotSelected,
    KnownRule,
    Unrecognized,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportAction {
    None,
    LaunchWithProxy,
    LaunchWithoutProxy,
    ApplyKnownRule,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantSummary {
    category: AppCategory,
    state: ApplicationNetworkState,
    action: ReportAction,
}

impl AssistantSummary {
    fn unknown(category: AppCategory) -> Self {
        Self {
            category,
            state: ApplicationNetworkState::Unknown,
            action: ReportAction::None,
        }
    }

    fn from_diagnosis(diagnosis: &ApplicationDiagnosis, revision: u64) -> Self {
        if diagnosis.active_proxy_revision != revision {
            return Self::unknown(AppCategory::Unavailable);
        }
        Self {
            category: if diagnosis.known_rule.is_some() {
                AppCategory::KnownRule
            } else {
                AppCategory::Unrecognized
            },
            state: diagnosis.application_network_state,
            action: match diagnosis.recommended_action {
                RecommendedAction::None => ReportAction::None,
                RecommendedAction::LaunchWithProxy => ReportAction::LaunchWithProxy,
                RecommendedAction::LaunchWithoutProxy => ReportAction::LaunchWithoutProxy,
                RecommendedAction::ApplyKnownRule { .. } => ReportAction::ApplyKnownRule,
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReportData {
    app_version: String,
    os: &'static str,
    os_version: Option<String>,
    detected_count: usize,
    listening_count: usize,
    selected_client: Option<String>,
    has_selection: bool,
    available: bool,
    protocol: Option<ProxyProtocol>,
    confidence: Option<Confidence>,
    system_proxy_enabled: Option<bool>,
    environment: ProxyEnvironmentState,
    managed_variables: Vec<ProxyVariable>,
    tun: TunObservationState,
    connectivity: ProxyConnectivityState,
    successful_targets: usize,
    total_targets: usize,
    error_categories: Vec<ConnectivityErrorKind>,
    assistant: AssistantSummary,
}

fn project_report(
    status: &ProxyEnvironmentStatus,
    tun: TunObservationState,
    connection: Option<&connectivity::ProxyConnectivityResult>,
    assistant: AssistantSummary,
) -> DiagnosticReportData {
    // Group multiple listening ports of one process, without exporting process identity.
    let key = |candidate: &proxy::ProxyCandidate| {
        candidate.pid.map_or_else(
            || format!("candidate:{}", candidate.id),
            |pid| format!("process:{pid}"),
        )
    };
    let count = |listening_only: bool| {
        status
            .candidates
            .iter()
            .filter(|candidate| !listening_only || candidate.listening)
            .map(key)
            .collect::<HashSet<_>>()
            .len()
    };
    let selected = status.active_proxy.candidate.as_ref();
    let mut errors = Vec::new();
    if let Some(connection) = connection {
        for kind in connection
            .targets
            .iter()
            .filter_map(|target| target.error_kind)
        {
            if !errors.contains(&kind) {
                errors.push(kind);
            }
        }
    }
    DiagnosticReportData {
        app_version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS,
        os_version: redaction::report_version(sysinfo::System::os_version().as_deref()),
        detected_count: count(false),
        listening_count: count(true),
        selected_client: proxy::report_client_name(selected.and_then(|c| c.client_name.as_deref())),
        has_selection: selected.is_some(),
        available: status.active_proxy.available,
        protocol: selected.map(|c| c.protocol),
        confidence: selected.map(|c| c.confidence.clone()),
        system_proxy_enabled: cfg!(windows).then(proxy::system_proxy_enabled),
        environment: status.state,
        managed_variables: status.selected_variables.clone(),
        tun,
        connectivity: connection.map_or(ProxyConnectivityState::NotTested, |c| c.state),
        successful_targets: connection
            .map_or(0, |c| c.targets.iter().filter(|t| t.success).count()),
        total_targets: connection.map_or(0, |c| c.targets.len()),
        error_categories: errors,
        assistant,
    }
}

pub fn generate(application_id: Option<&str>) -> Result<DiagnosticReportData> {
    let settings = settings::load()?;
    let status = proxy::active::snapshot_status(&settings.proxy_variables)?;
    let revision = status.active_proxy.revision;
    let assistant = match application_id {
        None => AssistantSummary::unknown(AppCategory::NotSelected),
        Some(id) => application_assistant::resolve_application(id)
            .map(|application| {
                application_assistant::diagnose_report_snapshot(
                    application,
                    status.active_proxy.clone(),
                    status.state,
                )
            })
            .map(|diagnosis| AssistantSummary::from_diagnosis(&diagnosis, revision))
            .unwrap_or_else(|_| AssistantSummary::unknown(AppCategory::Unavailable)),
    };
    let connection = connectivity::get_last_result(status.active_proxy.available_candidate());
    let report = project_report(
        &status,
        network_observation::observe().state,
        connection.as_ref(),
        assistant,
    );
    if proxy::active::snapshot()?.revision != revision {
        return Err(ProxyEnvError::ActiveProxyChanged);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::proxy::active::ActiveProxyContext;

    fn status() -> ProxyEnvironmentStatus {
        let candidate = proxy::ProxyCandidate {
            id: "private-id".into(),
            client_name: Some("Clash Verge Rev".into()),
            icon_key: None,
            process_name: Some("private-process.exe".into()),
            pid: Some(12345),
            host: "secret.example.com".into(),
            port: 54321,
            protocol: ProxyProtocol::Mixed,
            source: vec![],
            confidence: Confidence::High,
            listening: true,
        };
        ProxyEnvironmentStatus {
            state: ProxyEnvironmentState::Enabled,
            entries: vec![crate::environment::EnvironmentEntry {
                name: "HTTP_PROXY".into(),
                scope: crate::environment::EnvironmentScope::User,
                value: Some(
                    "https://username:password@secret.example.com/subscription?token=private-token"
                        .into(),
                ),
                exists: true,
            }],
            selected_variables: vec![ProxyVariable::Http, ProxyVariable::Https],
            active_proxy: ActiveProxyContext {
                candidate: Some(candidate.clone()),
                available: true,
                revision: 4,
                ..Default::default()
            },
            candidates: vec![candidate],
            matches_active_proxy: true,
            snapshot_available: true,
            warning: Some(r"C:\Users\Alice\private\config.json".into()),
        }
    }

    #[test]
    fn only_allowlisted_status_fields_cross_the_report_boundary() {
        let mut status = status();
        status.active_proxy.candidate.as_mut().unwrap().client_name =
            Some("Alice token=private-token".into());
        let connection = connectivity::ProxyConnectivityResult {
            state: ProxyConnectivityState::Partial,
            endpoint: proxy::ProxyEndpoint {
                host: "secret.example.com".into(),
                port: 54321,
                protocol: ProxyProtocol::Mixed,
            },
            local_listener_ok: true,
            protocol_ok: true,
            tested_at: "private-timestamp".into(),
            total_duration_ms: 12345,
            targets: vec![connectivity::ConnectivityTargetResult {
                name: "Alice".into(),
                url: "https://secret.example.com/subscription?token=private-token".into(),
                success: false,
                status_code: None,
                duration_ms: None,
                error_kind: Some(ConnectivityErrorKind::ConnectTimeout),
            }],
        };
        let report = project_report(
            &status,
            TunObservationState::Possible,
            Some(&connection),
            AssistantSummary::unknown(AppCategory::NotSelected),
        );
        let json = serde_json::to_string(&report).unwrap();
        for secret in [
            "Alice",
            "private-token",
            "username",
            "password",
            "secret.example.com",
            "54321",
            "12345",
            "config.json",
            "private-process",
            "private-id",
            "private-timestamp",
            "subscription",
        ] {
            assert!(!json.contains(secret), "report leaked {secret}");
        }
        assert!(report.selected_client.is_none());
        assert_eq!(
            report.error_categories,
            vec![ConnectivityErrorKind::ConnectTimeout]
        );
        assert_eq!(report.total_targets, 1);
    }

    #[test]
    fn counts_clients_not_multiple_ports_and_never_invents_connectivity() {
        let mut status = status();
        let mut second_port = status.candidates[0].clone();
        second_port.id = "second-port".into();
        second_port.port = 54322;
        status.candidates.push(second_port);
        let report = project_report(
            &status,
            TunObservationState::Detected,
            None,
            AssistantSummary::unknown(AppCategory::NotSelected),
        );
        assert_eq!(report.detected_count, 1);
        assert_eq!(report.listening_count, 1);
        assert_eq!(report.connectivity, ProxyConnectivityState::NotTested);
        assert_eq!(report.total_targets, 0);
        assert_eq!(report.selected_client.as_deref(), Some("Clash Verge Rev"));
        assert_eq!(report.assistant.state, ApplicationNetworkState::Unknown);
    }

    #[test]
    fn assistant_projection_excludes_rule_ids_paths_and_stale_recommendations() {
        let diagnosis = ApplicationDiagnosis {
            active_proxy_revision: 4,
            application: application_assistant::ManagedApplication {
                id: "secret-id".into(),
                display_name: "Alice".into(),
                executable_path: r"C:\Users\Alice\App.exe".into(),
                icon_key: None,
                rule_id: Some("secret-rule".into()),
                last_action: None,
            },
            proxy_available: true,
            system_proxy_enabled: false,
            proxy_environment_state: ProxyEnvironmentState::Enabled,
            tun_observation: TunObservationState::Detected,
            known_rule: Some("secret-rule".into()),
            proxy_connectivity_state: None,
            application_network_state: ApplicationNetworkState::RuleSyncRecommended,
            recommended_action: RecommendedAction::ApplyKnownRule {
                rule_id: "secret-rule".into(),
            },
        };
        let json = serde_json::to_string(&AssistantSummary::from_diagnosis(&diagnosis, 4)).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("Alice"));
        assert!(!json.contains("App.exe"));
        let stale = AssistantSummary::from_diagnosis(&diagnosis, 5);
        assert_eq!(stale.state, ApplicationNetworkState::Unknown);
        assert!(matches!(stale.action, ReportAction::None));
    }
}
