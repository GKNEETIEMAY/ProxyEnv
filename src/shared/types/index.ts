export type ProxyProtocol = "http" | "socks5" | "mixed" | "unknown";
export type DetectionSource =
  | "windowsSystemProxy"
  | "processListener"
  | "protocolProbe"
  | "clientConfig"
  | "composite";
export type Confidence = "veryHigh" | "high" | "medium" | "low";

export interface EnvironmentEntry {
  name: string;
  value: string | null;
  exists: boolean;
}

export interface EnvironmentStatus {
  state: "disabled" | "partial" | "enabled" | "mismatch";
  entries: EnvironmentEntry[];
  selectedVariables: ManagedProxyVariable[];
  activeCandidate?: ProxyCandidate;
  candidates: ProxyCandidate[];
  matchesActiveProxy: boolean;
  snapshotAvailable: boolean;
  warning?: string;
}

export interface ProxyEndpoint {
  host: string;
  port: number;
  protocol: ProxyProtocol;
}

export interface ProxyCandidate {
  id: string;
  clientName?: string;
  iconKey?: string;
  processName?: string;
  pid?: number;
  host: string;
  port: number;
  protocol: ProxyProtocol;
  source: DetectionSource[];
  confidence: Confidence;
  listening: boolean;
}

export type AppLanguage = "system" | "zh-CN" | "en" | "ja" | "ko";
export type AppTheme = "system" | "light" | "dark";
export type ManagedProxyVariable = "http" | "https" | "all";

export interface AppSettings {
  language: AppLanguage;
  theme: AppTheme;
  launchAtStartup: boolean;
  silentStart: boolean;
  closeToTray: boolean;
  proxyVariables: ManagedProxyVariable[];
}
