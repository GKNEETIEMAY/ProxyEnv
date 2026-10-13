import type { BridgeSummary, RemoteRuntimeState } from "./state";

export type ReadinessState = "preparing" | "ready" | "attention";
export type ReadinessItemId = "terminal" | "vscode" | "tools" | "egress";
export interface ReadinessItem { id: ReadinessItemId; state: ReadinessState }
export interface RemoteReadiness { state: ReadinessState; items: ReadinessItem[] }

// A presentation of the current backend snapshot, not a second lifecycle or probe.
export function remoteReadiness(summary: BridgeSummary): RemoteReadiness {
  const fallback = summary.postConnectStatus === "preparing" ? "preparing" : "ready";
  const runtime = (state?: RemoteRuntimeState): ReadinessState =>
    state === "warning" ? "attention" : state === "pending" || state === "preparing" ? "preparing" : state === "ready" ? "ready" : fallback;
  const items: ReadinessItem[] = [];
  if (summary.proxy) items.push({ id: "terminal", state: runtime(summary.sessionEnvironmentState) });
  if (summary.target?.canOpenVscode) items.push({ id: "vscode", state: runtime(summary.vscodeState) });
  if (summary.cc) {
    const states = [runtime(summary.codexState), runtime(summary.claudeState)];
    const checking = summary.toolInstallations?.some(tool => tool.state === "checking" || tool.extensionState === "checking");
    // An absent CLI is normal: the user may use only its extension, or neither.
    items.push({ id: "tools", state: checking || states.includes("preparing") ? "preparing" : states.includes("attention") ? "attention" : "ready" });
  }
  if (summary.proxy) {
    const egress = summary.diagnostics?.generalProxyEgress.state;
    items.push({ id: "egress", state: egress === "passed" ? "ready" : egress === "testing" ? "preparing" : egress === "failed" ? "attention" : summary.postConnectStatus === "preparing" ? "preparing" : "attention" });
  }
  const state = items.some(item => item.state === "preparing") ? "preparing"
    : items.some(item => item.state === "attention") || summary.skillsState === "warning" ? "attention" : "ready";
  return { state, items };
}

