import type { BridgeCapability, BridgeSummary } from "../features/remote-bridge/state";

export type RemoteNavState = "idle" | "connecting" | "connected" | "attention";

// Navigation describes transport only, never optional setup or model verification.
export function remoteNavigationState(summary: Pick<BridgeSummary, "status" | "reconnectState">): RemoteNavState {
  if (summary.reconnectState === "attentionRequired" || ["stale", "unavailable", "error"].includes(summary.status)) return "attention";
  if (summary.status === "connecting" || summary.reconnectState === "waiting" || summary.reconnectState === "retrying") return "connecting";
  return summary.status === "connected" ? "connected" : "idle";
}

export function trayNavigationAction(payload: unknown): "local" | "remote" | BridgeCapability | null {
  return payload === "local" || payload === "remote" || payload === "proxy" || payload === "cc" ? payload : null;
}

// A tray intent never starts SSH and never guesses state from the menu's last render.
export function trayCapabilityIntent(summary: BridgeSummary, capability: BridgeCapability) {
  if (!summary.target || summary.status !== "connected"
    || summary.reconnectState && summary.reconnectState !== "idle"
    || summary.postConnectStatus === "preparing") return null;
  return { targetId: summary.target.id, capability, enabled: !summary[capability] };
}
