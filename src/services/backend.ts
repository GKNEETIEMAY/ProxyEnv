import { invoke } from "@tauri-apps/api/core";
import type { EnvironmentStatus, ProxyCandidate } from "../types";

export const backend = {
  environmentStatus: () => invoke<EnvironmentStatus>("get_environment_status"),
  enableProxyEnvironment: () => invoke<EnvironmentStatus>("enable_proxy_environment"),
  disableProxyEnvironment: () => invoke<EnvironmentStatus>("disable_proxy_environment"),
  detectProxies: () => invoke<ProxyCandidate[]>("detect_proxies")
};

