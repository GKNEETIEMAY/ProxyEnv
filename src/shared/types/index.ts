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

export interface ProxyEndpointInspection {
  listening: boolean;
  detectedProtocol: ProxyProtocol;
  protocolMatches: boolean;
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

export type TunObservationState = "notDetected" | "possible" | "detected" | "unknown";
export type TunEvidenceKind = "tunnelInterfaceType" | "virtualAdapterName" | "interfaceOperational" | "broadRoute" | "enumerationUnavailable";

export interface TunEvidence {
  kind: TunEvidenceKind;
  interfaceName?: string;
  detail: string;
}

export interface TunObservation {
  state: TunObservationState;
  interfaceName?: string;
  description?: string;
  evidence: TunEvidence[];
}

export interface RunningApplication {
  pid: number;
  applicationId?: string;
  processName: string;
  displayName: string;
  executablePath?: string;
  iconAvailable: boolean;
}

export type ApplicationActionKind = "launchWithProxy" | "launchWithoutProxy";

export interface ManagedApplication {
  id: string;
  displayName: string;
  executablePath: string;
  iconKey: string | null;
  ruleId: string | null;
  lastAction: ApplicationActionKind | null;
}

export type ApplicationNetworkState = "confirmedReady" | "environmentConfigured" | "proxyLaunchRecommended" | "ruleSyncRecommended" | "conflict" | "unsupported" | "unknown";
export type RecommendedAction = "none" | "launchWithProxy" | "launchWithoutProxy" | { applyKnownRule: { ruleId: string } };

export interface ApplicationDiagnosis {
  application: ManagedApplication;
  proxyAvailable: boolean;
  systemProxyEnabled: boolean;
  proxyEnvironmentState: EnvironmentStatus["state"];
  tunObservation: TunObservationState;
  knownRule?: string;
  proxyConnectivityState?: "notTested" | "testing" | "reachable" | "partial" | "unreachable" | "localProxyUnavailable" | "unknown";
  applicationNetworkState: ApplicationNetworkState;
  recommendedAction: RecommendedAction;
}

export type ConfigValue = string | number | boolean | null;

export interface RuleChangePlan {
  ruleId: string;
  targetFile: string;
  fieldPath: string[];
  oldValue: ConfigValue;
  newValue: ConfigValue;
  restartRequired: boolean;
}

export interface RuleChangePreview {
  state: "ready" | "alreadyCurrent" | "noMatchingRule" | "ambiguousRule" | "activeProxyMissing" | "unsupportedProxyProtocol" | "fileMissing" | "readFailed" | "parseFailed" | "fieldMissing" | "unsupportedFieldValue";
  ruleId?: string;
  targetFile?: string;
  plan?: RuleChangePlan;
}

export interface RuleBackup {
  id: string;
  ruleId: string;
  targetFile: string;
  fieldPath: string[];
  oldValue: ConfigValue;
  appliedValue: ConfigValue;
  beforeHash: string;
  afterHash: string;
  createdAt: string;
}

export interface RuleApplyResult {
  state: "applied" | "confirmationRequired" | "conflict" | "previewUnavailable" | "backupFailed" | "writeFailed" | "verificationFailed";
  backup?: RuleBackup;
  restartRequired: boolean;
}

export interface RuleRestoreResult {
  state: "restored" | "confirmationRequired" | "backupMissing" | "conflict" | "writeFailed" | "verificationFailed";
  backupId?: string;
  restartRequired: boolean;
}

export interface LaunchApplicationResult {
  pid: number;
  executablePath: string;
  environmentMode: "inherit" | "useCurrentProxy" | "clearProxyVariables";
}
