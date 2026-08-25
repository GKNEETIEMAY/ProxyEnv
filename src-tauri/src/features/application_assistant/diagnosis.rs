use crate::{
    error::Result,
    features::{
        network_observation::{self, TunObservationState},
        proxy::{
            self,
            connectivity::{self, ProxyConnectivityState},
            ProxyEnvironmentService, ProxyEnvironmentState, ProxyProtocol,
        },
    },
    services::settings,
};

use super::{
    rules, ApplicationDiagnosis, ApplicationNetworkState, DiagnosisSummary, ManagedApplication,
    RecommendedAction,
};

pub fn diagnose(application: ManagedApplication) -> Result<ApplicationDiagnosis> {
    let settings = settings::load()?;
    let candidates = proxy::detect()?;
    let active_candidate = candidates
        .iter()
        .find(|candidate| candidate.listening)
        .cloned();
    let environment =
        ProxyEnvironmentService::status(&settings.proxy_variables, active_candidate.clone())?;
    let proxy_available = active_candidate.as_ref().is_some_and(|candidate| {
        candidate.listening && !matches!(candidate.protocol, ProxyProtocol::Unknown)
    });
    let proxy_connectivity_state =
        connectivity::get_last_result(active_candidate.as_ref()).map(|result| result.state);
    let active_endpoint = active_candidate
        .as_ref()
        .map(|candidate| proxy::ProxyEndpoint {
            host: candidate.host.clone(),
            port: candidate.port,
            protocol: candidate.protocol,
        });
    let rule_preview =
        rules::preview_application(&application.executable_path, active_endpoint.as_ref());
    let rule_inspection = match rule_preview.state {
        rules::RulePreviewState::NoMatchingRule => RuleInspectionState::None,
        rules::RulePreviewState::AmbiguousRule => RuleInspectionState::Ambiguous,
        rules::RulePreviewState::Ready => RuleInspectionState::Ready,
        rules::RulePreviewState::AlreadyCurrent => RuleInspectionState::AlreadyCurrent,
        _ => RuleInspectionState::Unavailable,
    };
    let input = DiagnosisInput {
        proxy_available,
        system_proxy_enabled: proxy::system_proxy_enabled(),
        proxy_environment_state: environment.state,
        tun_observation: network_observation::observe().state,
        known_rule: rule_preview.rule_id,
        rule_inspection,
        proxy_connectivity_state,
    };
    Ok(build_diagnosis(application, input))
}

#[derive(Debug, Clone)]
struct DiagnosisInput {
    proxy_available: bool,
    system_proxy_enabled: bool,
    proxy_environment_state: ProxyEnvironmentState,
    tun_observation: TunObservationState,
    known_rule: Option<String>,
    rule_inspection: RuleInspectionState,
    proxy_connectivity_state: Option<ProxyConnectivityState>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum RuleInspectionState {
    None,
    Ready,
    AlreadyCurrent,
    Unavailable,
    Ambiguous,
}

fn build_diagnosis(application: ManagedApplication, input: DiagnosisInput) -> ApplicationDiagnosis {
    let (application_network_state, recommended_action, summary) = decide(&input);
    ApplicationDiagnosis {
        application,
        proxy_available: input.proxy_available,
        system_proxy_enabled: input.system_proxy_enabled,
        proxy_environment_state: input.proxy_environment_state,
        tun_observation: input.tun_observation,
        known_rule: input.known_rule,
        proxy_connectivity_state: input.proxy_connectivity_state,
        application_network_state,
        recommended_action,
        summary,
    }
}

fn decide(
    input: &DiagnosisInput,
) -> (ApplicationNetworkState, RecommendedAction, DiagnosisSummary) {
    let proxy_failed = matches!(
        input.proxy_connectivity_state,
        Some(ProxyConnectivityState::Unreachable | ProxyConnectivityState::LocalProxyUnavailable)
    );
    if !input.proxy_available || proxy_failed {
        return (
            ApplicationNetworkState::Unsupported,
            RecommendedAction::None,
            DiagnosisSummary::Unsupported,
        );
    }
    if matches!(input.rule_inspection, RuleInspectionState::Ambiguous) {
        return (
            ApplicationNetworkState::Conflict,
            RecommendedAction::None,
            DiagnosisSummary::Unsupported,
        );
    }
    if matches!(input.rule_inspection, RuleInspectionState::AlreadyCurrent) {
        return (
            ApplicationNetworkState::Ready,
            RecommendedAction::None,
            DiagnosisSummary::Normal,
        );
    }
    if matches!(input.rule_inspection, RuleInspectionState::Unavailable) {
        return (
            ApplicationNetworkState::Unsupported,
            RecommendedAction::None,
            DiagnosisSummary::Unsupported,
        );
    }
    if let (RuleInspectionState::Ready, Some(rule_id)) = (input.rule_inspection, &input.known_rule)
    {
        return (
            ApplicationNetworkState::RuleSyncRecommended,
            RecommendedAction::ApplyKnownRule {
                rule_id: rule_id.clone(),
            },
            DiagnosisSummary::KnownApplicationRule,
        );
    }
    if matches!(
        input.proxy_environment_state,
        ProxyEnvironmentState::Enabled
    ) {
        return (
            ApplicationNetworkState::Ready,
            RecommendedAction::None,
            DiagnosisSummary::Normal,
        );
    }
    (
        ApplicationNetworkState::ProxyLaunchRecommended,
        RecommendedAction::LaunchWithProxy,
        DiagnosisSummary::CanLaunchWithProxy,
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn application() -> ManagedApplication {
        ManagedApplication {
            id: "example".into(),
            display_name: "Example".into(),
            executable_path: PathBuf::from(r"C:\Apps\Example.exe"),
            icon_key: None,
            rule_id: None,
            last_action: None,
        }
    }

    fn input() -> DiagnosisInput {
        DiagnosisInput {
            proxy_available: true,
            system_proxy_enabled: false,
            proxy_environment_state: ProxyEnvironmentState::Disabled,
            tun_observation: TunObservationState::Unknown,
            known_rule: None,
            rule_inspection: RuleInspectionState::None,
            proxy_connectivity_state: None,
        }
    }

    #[test]
    fn recommends_proxy_launch_when_a_usable_proxy_is_available() {
        let diagnosis = build_diagnosis(application(), input());
        assert_eq!(
            diagnosis.application_network_state,
            ApplicationNetworkState::ProxyLaunchRecommended
        );
        assert_eq!(
            diagnosis.recommended_action,
            RecommendedAction::LaunchWithProxy
        );
        assert_eq!(diagnosis.summary, DiagnosisSummary::CanLaunchWithProxy);
    }

    #[test]
    fn reports_ready_when_environment_already_matches_the_active_proxy() {
        let mut input = input();
        input.proxy_environment_state = ProxyEnvironmentState::Enabled;
        let diagnosis = build_diagnosis(application(), input);
        assert_eq!(
            diagnosis.application_network_state,
            ApplicationNetworkState::Ready
        );
        assert_eq!(diagnosis.recommended_action, RecommendedAction::None);
        assert_eq!(diagnosis.summary, DiagnosisSummary::Normal);
    }

    #[test]
    fn known_rule_takes_priority_over_generic_launch_advice() {
        let mut input = input();
        input.known_rule = Some("example-rule".into());
        input.rule_inspection = RuleInspectionState::Ready;
        let diagnosis = build_diagnosis(application(), input);
        assert_eq!(
            diagnosis.application_network_state,
            ApplicationNetworkState::RuleSyncRecommended
        );
        assert_eq!(
            diagnosis.recommended_action,
            RecommendedAction::ApplyKnownRule {
                rule_id: "example-rule".into()
            }
        );
        assert_eq!(diagnosis.summary, DiagnosisSummary::KnownApplicationRule);
    }

    #[test]
    fn ambiguous_rules_never_recommend_an_automatic_change() {
        let mut input = input();
        input.rule_inspection = RuleInspectionState::Ambiguous;
        let diagnosis = build_diagnosis(application(), input);
        assert_eq!(
            diagnosis.application_network_state,
            ApplicationNetworkState::Conflict
        );
        assert_eq!(diagnosis.recommended_action, RecommendedAction::None);
        assert_eq!(diagnosis.summary, DiagnosisSummary::Unsupported);
    }

    #[test]
    fn an_already_current_rule_is_reported_as_ready() {
        let mut input = input();
        input.known_rule = Some("example-rule".into());
        input.rule_inspection = RuleInspectionState::AlreadyCurrent;
        let diagnosis = build_diagnosis(application(), input);
        assert_eq!(
            diagnosis.application_network_state,
            ApplicationNetworkState::Ready
        );
        assert_eq!(diagnosis.recommended_action, RecommendedAction::None);
    }

    #[test]
    fn an_unreadable_known_rule_never_recommends_a_write() {
        let mut input = input();
        input.known_rule = Some("example-rule".into());
        input.rule_inspection = RuleInspectionState::Unavailable;
        let diagnosis = build_diagnosis(application(), input);
        assert_eq!(
            diagnosis.application_network_state,
            ApplicationNetworkState::Unsupported
        );
        assert_eq!(diagnosis.recommended_action, RecommendedAction::None);
    }

    #[test]
    fn unavailable_proxy_does_not_recommend_changing_the_application() {
        let mut missing = input();
        missing.proxy_available = false;
        let diagnosis = build_diagnosis(application(), missing);
        assert_eq!(
            diagnosis.application_network_state,
            ApplicationNetworkState::Unsupported
        );
        assert_eq!(diagnosis.recommended_action, RecommendedAction::None);

        let mut failed = input();
        failed.proxy_connectivity_state = Some(ProxyConnectivityState::Unreachable);
        let diagnosis = build_diagnosis(application(), failed);
        assert_eq!(
            diagnosis.application_network_state,
            ApplicationNetworkState::Unsupported
        );
        assert_eq!(diagnosis.recommended_action, RecommendedAction::None);
    }

    #[test]
    fn partial_connectivity_does_not_become_a_total_proxy_failure() {
        let mut input = input();
        input.proxy_connectivity_state = Some(ProxyConnectivityState::Partial);
        let diagnosis = build_diagnosis(application(), input);
        assert_eq!(
            diagnosis.recommended_action,
            RecommendedAction::LaunchWithProxy
        );
    }

    #[test]
    fn preserves_system_proxy_and_tun_as_independent_observations() {
        let mut input = input();
        input.system_proxy_enabled = true;
        input.tun_observation = TunObservationState::Detected;

        let diagnosis = build_diagnosis(application(), input);

        assert!(diagnosis.system_proxy_enabled);
        assert_eq!(diagnosis.tun_observation, TunObservationState::Detected);
    }
}
