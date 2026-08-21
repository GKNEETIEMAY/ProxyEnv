import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, EnvironmentStatus, ProxyCandidate, ProxyEndpoint } from "../types";

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
  disableProxyEnvironment: () => invoke<EnvironmentStatus>("disable_proxy_environment"),
  restoreProxyEnvironment: () => invoke<EnvironmentStatus>("restore_proxy_environment"),
  detectProxies: () => invoke<ProxyCandidate[]>("detect_proxies"),
  appSettings: () => invoke<AppSettings>("get_app_settings"),
  saveAppSettings: (settings: AppSettings) => invoke<AppSettings>("save_app_settings", { settings })
};
