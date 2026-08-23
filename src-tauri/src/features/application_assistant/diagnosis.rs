use crate::{
    error::Result,
    features::{
        network_observation::TunObservationState,
        proxy::{
            self,
            connectivity::{self, ProxyConnectivityState},
            ProxyEnvironmentService, ProxyEnvironmentState, ProxyProtocol,
        },
    },
    services::settings,
};

use super::{
    ApplicationDiagnosis, ApplicationNetworkState, DiagnosisSummary, ManagedApplication,
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
    let input = DiagnosisInput {
        proxy_available,
        system_proxy_enabled: proxy::system_proxy_enabled(),
        proxy_environment_state: environment.state,
        tun_observation: TunObservationState::Unknown,
        known_rule: None,
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
    proxy_connectivity_state: Option<ProxyConnectivityState>,
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
    if let Some(rule_id) = &input.known_rule {
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
}
