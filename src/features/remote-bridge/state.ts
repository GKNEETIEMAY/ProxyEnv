import { onBeforeUnmount, onMounted, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { ProxyEndpoint } from "../../shared/types";
import type { RemoteToolId } from "./tool-adapters";
export type BridgeStatus = "disconnected" | "connecting" | "connected" | "stale" | "unavailable" | "error";
export type RemoteTargetSource = "openssh" | "vscode" | "mobaxterm";
export type SshAuthMode = "nonInteractive" | "interactive";
export type SshAuthMethod = "identityFile" | "agent" | "password" | "keyboardInteractive" | "unknown";
export interface SshAuthState { mode: SshAuthMode; method: SshAuthMethod; authenticated: boolean; passwordStored: boolean }
export type SshAuthOperation = "check" | "connect";
export type SshPromptType = "password" | "keyPassphrase" | "hostKeyConfirmation" | "verificationCode" | "keyboardInteractive" | "unknown";
export type SshAuthSessionStatus = "starting" | "waitingPrompt" | "waitingUser" | "submitting" | "waitingServer" | "authenticated" | "succeeded" | "promptUnavailable" | "failed";
export interface SshAuthPrompt { id: string; type: SshPromptType; message: string; secret: boolean; attempt: number; target: string | null; fingerprint: string | null }
export interface SshPtyDiagnostic { bytesReceived: number; printableBytes: number; cprRequests: number; promptDetected: boolean; authMarkerDetected: boolean; remoteResultDetected: boolean; outputClosed: boolean }
export interface SshAuthSnapshot { sessionId: string; operation: SshAuthOperation; status: SshAuthSessionStatus; auth: SshAuthState; prompt: SshAuthPrompt | null; diagnostic: SshPtyDiagnostic; error: string | null }
export interface SshAuthOutcome { operation: SshAuthOperation; ports: PortAllocation | null; summary: BridgeSummary | null }
export interface RemoteTarget { id: string; displayName: string; source: RemoteTargetSource; sourceLabel: string; configPath: string; sshAlias: string | null; host: string | null; user: string | null; port: number | null; identityFile: string | null; available: boolean; compatibility: "compatible" | "unsupported"; unavailableReason: string | null; canOpenVscode: boolean }
export interface BridgeEndpoint { local: ProxyEndpoint; remotePort: number }
export type RemoteToolVerification = "notConfigured" | "verifyPending" | "verified" | "authenticationRequired" | "routeUnavailable" | "timedOut" | "failed";
export interface RemoteToolState { id: RemoteToolId; displayName: string; configured: boolean; verification: RemoteToolVerification; verificationSupported: boolean; supportedRouteModes: "ccSwitch"[] }
export interface ToolVerificationResult { tool: RemoteToolId; verification: RemoteToolVerification }
export interface BridgeSummary { status: BridgeStatus; target: RemoteTarget | null; proxy: BridgeEndpoint | null; cc: BridgeEndpoint | null; proxyStatus: BridgeStatus | null; ccStatus: BridgeStatus | null; activeProxyRevision: number | null; environment: string; codexConfigured: boolean; claudeConfigured: boolean; tools?: RemoteToolState[]; codexExtension?: string | null; claudeExtension?: string | null; error: string | null; sshAuth: SshAuthState }
export interface BridgeRequest { targetId: string; proxyPort: number | null; ccPort: number | null; ccLocalPort: number; expectedRevision: number }
export interface PortAllocation { proxyPort: number; ccPort: number }
export interface CcDetection { state: "confirmed" | "listeningUnknown" | "notDetected"; localPort: number }
export interface RemoteNetworkObservation { serverInternet: "reachable" | "unreachable" | "unknown" }
export interface ConfigPreview { id: string; tool: RemoteToolId; path: string; before: string; after: string; version: string; launch: string; alias:string; restore:boolean; existingConfig:boolean; routeUpdate:boolean; permissionHardening:boolean }
export type VscodeRemoteContextStatus = "detected" | "ambiguous" | "unsupported";
export type ExtensionLocationState = "locationUnknown" | "activeUnknown" | "remoteConfirmed";
export interface VscodeRemoteContext {
  status: VscodeRemoteContextStatus;
  edition: "stable" | "insiders" | "legacy" | "custom" | "unknown";
  serverRoot: string;
  serverVersion: string;
  serverVersions: string[];
  dataPath: string;
  remoteSettingsPath: string;
  extensionRoot: string;
  evidence: "defaultStableRoot" | "defaultInsidersRoot" | "legacyRoot" | "agentFolderEnvironment" | "multipleServerRoots" | "noServerRoot";
  confidence: "high" | "medium" | "low";
  candidateCount: number;
}
export interface ExtensionCapability {
  tool: RemoteToolId;
  detected: boolean;
  supported: boolean;
  version: string;
  versions: string[];
  runtimeVersion: string;
  runtimeVersions: string[];
  candidateCount: number;
  location: Exclude<ExtensionLocationState, "remoteConfirmed">;
  configuration: "configured" | "notConfigured" | "conflict" | "unknown";
}
export interface ExtensionInspection { user: string; contextHash: string; vscode: VscodeRemoteContext; extensions: ExtensionCapability[] }
export interface ExtensionPreview { id: string; alias: string; tool: RemoteToolId; path: string; version: string; runtimeVersion: string; port: number; previousPort: number | null; originalExists: boolean; restore: boolean; loginPromptChange: "add" | "unchanged" | "overrideFalse" | null }
export const emptySummary = (): BridgeSummary => ({ status:"disconnected", target:null, proxy:null, cc:null, proxyStatus:null, ccStatus:null, activeProxyRevision:null, environment:"", codexConfigured:false, claudeConfigured:false, error:null, sshAuth:{mode:"nonInteractive",method:"unknown",authenticated:false,passwordStored:false} });
export const targetLabel = (target:RemoteTarget|null|undefined) => target ? `${target.displayName} · ${target.sourceLabel}` : "";
export const remoteBackend = {
  extensionInspect: (targetId: string) => invoke<ExtensionInspection>("remote_bridge_extension_inspect", { targetId }),
  extensionPreview: (selection: { alias: string; tool: RemoteToolId; contextHash: string; remoteConfirmed: boolean; restore: boolean }) => invoke<ExtensionPreview>("remote_bridge_extension_preview", { selection }),
  extensionApply: (id: string) => invoke<void>("remote_bridge_extension_apply", { id, confirmed: true }),
  targets: () => invoke<RemoteTarget[]>("remote_bridge_targets"),
  summary: () => invoke<BridgeSummary>("remote_bridge_summary"),
  check: (targetId: string) => invoke<PortAllocation>("remote_bridge_check", { targetId }),
  checkNetwork: (targetId: string) => invoke<RemoteNetworkObservation>("remote_bridge_check_network", { targetId }),
  sshAuthBegin: (operation: SshAuthOperation, targetId: string, request: BridgeRequest | null = null) => invoke<SshAuthSnapshot>("ssh_auth_begin", { operation, targetId, request }),
  sshAuthState: (sessionId: string) => invoke<SshAuthSnapshot>("ssh_auth_state", { sessionId }),
  sshAuthSubmit: (sessionId: string, promptId: string, response: string) => invoke<SshAuthSnapshot>("ssh_auth_submit", { sessionId, promptId, response }),
  sshAuthConfirmHost: (sessionId: string, promptId: string) => invoke<SshAuthSnapshot>("ssh_auth_confirm_host", { sessionId, promptId }),
  sshAuthFinish: (sessionId: string) => invoke<SshAuthOutcome>("ssh_auth_finish", { sessionId }),
  sshAuthCancel: (sessionId: string) => invoke<void>("ssh_auth_cancel", { sessionId }),
  allocatePorts: (targetId: string, preferDefaults = true) => invoke<PortAllocation>("remote_bridge_allocate_ports", { targetId, preferDefaults }),
  detectCc: (localPort: number) => invoke<CcDetection>("remote_bridge_detect_cc", { localPort }),
  preview: (request: BridgeRequest) => invoke<BridgeSummary>("remote_bridge_preview", { request }),
  connect: (request: BridgeRequest) => invoke<BridgeSummary>("remote_bridge_connect", { request, confirmed:true }),
  disconnect: () => invoke<BridgeSummary>("remote_bridge_disconnect", { confirmed:true }),
  test: () => invoke<void>("remote_bridge_test"),
  launchProxyTerminal: () => invoke<void>("remote_bridge_launch_proxy_terminal"),
  launchManualTerminal: () => invoke<void>("remote_bridge_launch_manual_terminal"),
  clearSessionCredential: () => invoke<void>("remote_bridge_clear_session_credential"),
  configPreview: (tool: RemoteToolId) => invoke<ConfigPreview>("remote_bridge_config_preview", { tool }),
  configApply: (id: string) => invoke<void>("remote_bridge_config_apply", { id, confirmed:true }),
  configRestorePreview: (targetId:string, tool:RemoteToolId) => invoke<ConfigPreview>("remote_bridge_config_restore_preview", { targetId,tool }),
  configRestore: (id: string) => invoke<void>("remote_bridge_config_restore", { id, confirmed:true }),
  toolVerify: (tool: RemoteToolId) => invoke<ToolVerificationResult>("remote_bridge_tool_verify", { tool }),
  openVscode: (targetId:string) => invoke<void>("remote_bridge_open_vscode",{targetId}),
  openVscodeSettings: () => invoke<void>("remote_bridge_open_vscode_settings"),
  revealTargetConfig: (targetId:string) => invoke<void>("remote_bridge_reveal_target_config",{targetId}),
  openTargetConfig: (targetId:string) => invoke<void>("remote_bridge_open_target_config",{targetId}),
  launchMobaxterm: () => invoke<void>("remote_bridge_launch_mobaxterm"),
};
export function useRemoteBridge() {
  const summary = ref<BridgeSummary>(emptySummary());
  let timer: ReturnType<typeof setTimeout> | undefined;
  let disposed=false;
  async function refresh() {
    if (!isTauri()) return;
    try { const next=await remoteBackend.summary(); if (!disposed) summary.value=next; }
    catch { if (!disposed) summary.value={...summary.value,status:"unavailable"}; }
  }
  async function poll() { await refresh(); if (!disposed) timer=setTimeout(poll,2000); }
  onMounted(() => { void poll(); });
  onBeforeUnmount(() => { disposed=true; clearTimeout(timer); });
  return { summary, refresh };
}
