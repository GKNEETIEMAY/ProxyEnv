import { invoke } from "@tauri-apps/api/core";
import type { EnvironmentStatus, ProxyCandidate } from "../types";

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
  disableProxyEnvironment: () => invoke<EnvironmentStatus>("disable_proxy_environment"),
  detectProxies: () => invoke<ProxyCandidate[]>("detect_proxies")
};
