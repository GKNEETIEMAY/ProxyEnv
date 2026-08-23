import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, ApplicationDiagnosis, EnvironmentStatus, LaunchApplicationResult, ManagedApplication, ProxyCandidate, ProxyEndpoint, ProxyEndpointInspection, RuleApplyResult, RuleChangePlan, RuleChangePreview, RuleRestoreResult, RunningApplication, TunObservation } from "../types";

export const backend = {
  environmentStatus: () => invoke<EnvironmentStatus>("get_environment_status"),
  enableProxyEnvironment: (proxy?: ProxyCandidate) => invoke<EnvironmentStatus>("enable_proxy_environment", proxy
    ? { host: proxy.host, port: proxy.port, protocol: proxy.protocol }
    : {}),
  syncProxyEnvironment: (proxy: ProxyCandidate) => invoke<EnvironmentStatus>("sync_proxy_environment", {
    host: proxy.host,
    port: proxy.port,
    protocol: proxy.protocol
  }),
  syncManualProxyEnvironment: (endpoint: ProxyEndpoint) => invoke<EnvironmentStatus>("sync_manual_proxy_environment", { endpoint }),
  inspectProxyEndpoint: (endpoint: ProxyEndpoint) => invoke<ProxyEndpointInspection>("inspect_proxy_endpoint", { endpoint }),
  disableProxyEnvironment: () => invoke<EnvironmentStatus>("disable_proxy_environment"),
  restoreProxyEnvironment: () => invoke<EnvironmentStatus>("restore_proxy_environment"),
  detectProxies: () => invoke<ProxyCandidate[]>("detect_proxies"),
  tunObservation: () => invoke<TunObservation>("get_tun_observation"),
  runningApplications: () => invoke<RunningApplication[]>("list_running_applications"),
  diagnoseApplication: (application: ManagedApplication) => invoke<ApplicationDiagnosis>("diagnose_application", { application }),
  previewApplicationRuleFix: (application: ManagedApplication) => invoke<RuleChangePreview>("preview_application_rule_fix", { application }),
  applyApplicationRuleFix: (application: ManagedApplication, expectedPlan: RuleChangePlan) => invoke<RuleApplyResult>("apply_application_rule_fix", { application, expectedPlan, confirmed: true }),
  restoreApplicationRuleChange: (backupId: string) => invoke<RuleRestoreResult>("restore_application_rule_change", { backupId, confirmed: true }),
  launchApplicationWithProxy: (application: ManagedApplication) => invoke<LaunchApplicationResult>("launch_application_with_current_proxy", { application }),
  launchApplicationWithoutProxy: (application: ManagedApplication) => invoke<LaunchApplicationResult>("launch_application_without_proxy", { application }),
  appSettings: () => invoke<AppSettings>("get_app_settings"),
  saveAppSettings: (settings: AppSettings) => invoke<AppSettings>("save_app_settings", { settings })
};
