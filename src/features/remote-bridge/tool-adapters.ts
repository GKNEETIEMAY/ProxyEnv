import type { RemoteBridgeCopy } from "../../shared/i18n/remote-bridge";
import { remoteBackend, type BridgeSummary, type ConfigPreview, type RemoteToolState, type RemoteToolVerification, type ToolVerificationResult } from "./state";

export type RemoteToolId = "codex" | "claude";
export type RemoteToolRouteMode = "ccSwitch";
export type RemoteToolCompatibility = "compatible" | "unsupported";
export interface RemoteToolInspection {
  configured: boolean;
  verification: RemoteToolVerification;
  verificationSupported: boolean;
}

export interface RemoteToolAdapter {
  readonly id: RemoteToolId;
  readonly displayName: string;
  readonly extensionPath: string;
  readonly usesVscodeRemoteSettings: boolean;
  readonly restoreRequiresVscodeContext: boolean;
  detect(preview: ConfigPreview | undefined): boolean;
  inspect(summary: BridgeSummary): RemoteToolInspection;
  preview(targetId: string, restoring: boolean): Promise<ConfigPreview>;
  apply(previewId: string): Promise<void>;
  restore(previewId: string): Promise<void>;
  launch(summary: BridgeSummary): string;
  verify(): Promise<ToolVerificationResult>;
  supportedRouteModes(): readonly RemoteToolRouteMode[];
  compatibility(preview: ConfigPreview | undefined): RemoteToolCompatibility;
  verifyLabel(copy: RemoteBridgeCopy): string;
  configHint(copy: RemoteBridgeCopy): string;
  impact(copy: RemoteBridgeCopy, restoring: boolean): string;
  renderExtensionPreview(port: number): string;
}

interface AdapterDefinition {
  id: RemoteToolId;
  displayName: string;
  extensionPath: string;
  usesVscodeRemoteSettings: boolean;
  restoreRequiresVscodeContext: boolean;
  launchCommand: string;
  verificationSupported: boolean;
  configured(summary: BridgeSummary): boolean;
  verifyLabel(copy: RemoteBridgeCopy): string;
  configHint(copy: RemoteBridgeCopy): string;
  impact(copy: RemoteBridgeCopy, restoring: boolean): string;
  renderExtensionPreview(port: number): string;
}

function createAdapter(definition: AdapterDefinition): RemoteToolAdapter {
  const state = (summary: BridgeSummary): RemoteToolState | undefined => summary.tools?.find((tool) => tool.id === definition.id);
  const configured = (summary: BridgeSummary): boolean => state(summary)?.configured ?? definition.configured(summary);
  const verification = (summary: BridgeSummary): RemoteToolVerification => state(summary)?.verification ?? (configured(summary) ? "verifyPending" : "notConfigured");
  return {
    ...definition,
    detect: (preview) => Boolean(preview?.version),
    inspect: (summary) => ({ configured: configured(summary), verification: verification(summary), verificationSupported: state(summary)?.verificationSupported ?? definition.verificationSupported }),
    preview: (targetId, restoring) => restoring
      ? remoteBackend.configRestorePreview(targetId, definition.id)
      : remoteBackend.configPreview(definition.id),
    apply: (previewId) => remoteBackend.configApply(previewId),
    restore: (previewId) => remoteBackend.configRestore(previewId),
    launch: (summary) => configured(summary) ? definition.launchCommand : "",
    verify: () => remoteBackend.toolVerify(definition.id),
    supportedRouteModes: () => ["ccSwitch"],
    compatibility: (preview) => preview?.version ? "compatible" : "unsupported",
  };
}

export const remoteToolAdapters: readonly RemoteToolAdapter[] = [
  createAdapter({
    id: "codex",
    displayName: "Codex CLI",
    extensionPath: "~/.codex/config.toml",
    usesVscodeRemoteSettings: false,
    restoreRequiresVscodeContext: false,
    launchCommand: "codex",
    verificationSupported: false,
    configured: (summary) => summary.codexConfigured,
    verifyLabel: (copy) => copy.rbVerifyClaude,
    configHint: (copy) => copy.rbExtCodexImpact,
    impact: (copy, restoring) => restoring ? copy.rbExtRestoreImpact : copy.rbExtCodexImpact,
    renderExtensionPreview: (port) => `model_provider = "proxyenv_bridge"\n[model_providers.proxyenv_bridge]\nname = "ProxyEnv CC Switch"\nbase_url = "http://127.0.0.1:${port}/v1"\nwire_api = "responses"\nrequires_openai_auth = false`,
  }),
  createAdapter({
    id: "claude",
    displayName: "Claude Code CLI",
    extensionPath: "",
    usesVscodeRemoteSettings: true,
    restoreRequiresVscodeContext: true,
    launchCommand: "claude",
    verificationSupported: true,
    configured: (summary) => summary.claudeConfigured,
    verifyLabel: (copy) => copy.rbVerifyClaude,
    configHint: (copy) => copy.rbClaudeConfigHint,
    impact: (copy, restoring) => restoring ? copy.rbExtRestoreImpact : copy.rbExtClaudeImpact,
    renderExtensionPreview: (port) => JSON.stringify({
      "claudeCode.environmentVariables": [
        { name: "ANTHROPIC_BASE_URL", value: `http://127.0.0.1:${port}` },
        { name: "ANTHROPIC_AUTH_TOKEN", value: "PROXY_MANAGED" },
      ],
      "claudeCode.disableLoginPrompt": true,
    }, null, 2),
  }),
];

export function getRemoteToolAdapter(id: RemoteToolId): RemoteToolAdapter {
  const adapter = remoteToolAdapters.find((candidate) => candidate.id === id);
  if (!adapter) throw new Error("invalid remote tool adapter");
  return adapter;
}
