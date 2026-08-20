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
  enabled: boolean;
  entries: EnvironmentEntry[];
  warning?: string;
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
