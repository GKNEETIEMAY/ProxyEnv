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
  detect(preview: ConfigPreview | undefined): boolean;
  inspect(summary: BridgeSummary): RemoteToolInspection;
  preview(targetId: string, restoring: boolean): Promise<ConfigPreview>;
  apply(previewId: string): Promise<void>;
  restore(previewId: string): Promise<void>;
  launch(summary: BridgeSummary): string;
  verify(): Promise<ToolVerificationResult>;
  supportedRouteModes(): readonly RemoteToolRouteMode[];
  compatibility(preview: ConfigPreview | undefined): RemoteToolCompatibility;
  configureLabel(copy: RemoteBridgeCopy): string;
  verifyLabel(copy: RemoteBridgeCopy): string;
  restoreLabel(copy: RemoteBridgeCopy): string;
  impact(copy: RemoteBridgeCopy, restoring: boolean): string;
  renderExtensionPreview(port: number): string;
}

interface AdapterDefinition {
  id: RemoteToolId;
  displayName: string;
  extensionPath: string;
  launchCommand: string;
  verificationSupported: boolean;
  configured(summary: BridgeSummary): boolean;
  configureLabel(copy: RemoteBridgeCopy): string;
  verifyLabel(copy: RemoteBridgeCopy): string;
  restoreLabel(copy: RemoteBridgeCopy): string;
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
    launchCommand: "codex --profile proxyenv_bridge",
    verificationSupported: false,
    configured: (summary) => summary.codexConfigured,
    configureLabel: (copy) => copy.rbCodex,
    verifyLabel: (copy) => copy.rbVerifyClaude,
    restoreLabel: (copy) => copy.rbRestoreCodex,
    impact: (copy, restoring) => restoring ? copy.rbExtRestoreImpact : copy.rbExtCodexImpact,
    renderExtensionPreview: (port) => `model_provider = "proxyenv_bridge"\n[model_providers.proxyenv_bridge]\nname = "ProxyEnv CC Switch"\nbase_url = "http://127.0.0.1:${port}/v1"\nwire_api = "responses"\nrequires_openai_auth = false`,
  }),
  createAdapter({
    id: "claude",
    displayName: "Claude Code CLI",
    extensionPath: "~/.vscode-server/data/Machine/settings.json",
    launchCommand: 'claude --settings "$HOME/.claude/proxyenv-bridge.json"',
    verificationSupported: true,
    configured: (summary) => summary.claudeConfigured,
    configureLabel: (copy) => copy.rbClaude,
    verifyLabel: (copy) => copy.rbVerifyClaude,
    restoreLabel: (copy) => copy.rbRestoreClaude,
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
