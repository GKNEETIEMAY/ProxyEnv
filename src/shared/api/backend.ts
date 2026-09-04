import { invoke } from "@tauri-apps/api/core";
import type { ActiveProxyContext, AppSettings, ApplicationDiagnosis, EnvironmentStatus, LaunchApplicationResult, ManagedApplication, ProxyCandidate, ProxyEndpoint, ProxyEndpointInspection, RuleApplyResult, RuleChangePlan, RuleChangePreview, RuleRestoreResult, RunningApplication, TunObservation } from "../types";

export const backend = {
  environmentStatus: () => invoke<EnvironmentStatus>("get_environment_status"),
  enableProxyEnvironment: (expectedRevision: number) => invoke<EnvironmentStatus>("enable_proxy_environment", { expectedRevision }),
  syncProxyEnvironment: (expectedRevision: number) => invoke<EnvironmentStatus>("sync_proxy_environment", { expectedRevision }),
  selectActiveProxy: (candidateId: string) => invoke<EnvironmentStatus>("select_active_proxy", { candidateId }),
  activeProxyContext: () => invoke<ActiveProxyContext>("get_active_proxy_context"),
  syncManualProxyEnvironment: (endpoint: ProxyEndpoint) => invoke<EnvironmentStatus>("sync_manual_proxy_environment", { endpoint }),
  inspectProxyEndpoint: (endpoint: ProxyEndpoint) => invoke<ProxyEndpointInspection>("inspect_proxy_endpoint", { endpoint }),
  disableProxyEnvironment: () => invoke<EnvironmentStatus>("disable_proxy_environment"),
  restoreProxyEnvironment: () => invoke<EnvironmentStatus>("restore_proxy_environment"),
  detectProxies: () => invoke<ProxyCandidate[]>("detect_proxies"),
  tunObservation: () => invoke<TunObservation>("get_tun_observation"),
  runningApplications: () => invoke<RunningApplication[]>("list_running_applications"),
  pickApplication: () => invoke<ManagedApplication | null>("pick_application"),
  renewApplicationAuthorization: (applicationId: string) => invoke<ManagedApplication>("renew_application_authorization", { applicationId }),
  diagnoseApplication: (applicationId: string) => invoke<ApplicationDiagnosis>("diagnose_application", { applicationId }),
  previewApplicationRuleFix: (applicationId: string, expectedRevision: number) => invoke<RuleChangePreview>("preview_application_rule_fix", { applicationId, expectedRevision }),
  applyApplicationRuleFix: (applicationId: string, expectedPlan: RuleChangePlan, expectedRevision: number) => invoke<RuleApplyResult>("apply_application_rule_fix", { applicationId, expectedPlan, expectedRevision, confirmed: true }),
  restoreApplicationRuleChange: (backupId: string) => invoke<RuleRestoreResult>("restore_application_rule_change", { backupId, confirmed: true }),
  launchApplicationWithProxy: (applicationId: string, expectedRevision: number) => invoke<LaunchApplicationResult>("launch_application_with_current_proxy", { applicationId, expectedRevision }),
  launchApplicationWithoutProxy: (applicationId: string) => invoke<LaunchApplicationResult>("launch_application_without_proxy", { applicationId }),
  restartApplicationWithoutProxy: (applicationId: string, pid: number) => invoke<LaunchApplicationResult>("restart_application_without_proxy", { applicationId, pid }),
  appSettings: () => invoke<AppSettings>("get_app_settings"),
  saveAppSettings: (settings: AppSettings) => invoke<AppSettings>("save_app_settings", { settings })
};
