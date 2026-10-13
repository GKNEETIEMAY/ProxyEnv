import type { BridgeSummary } from "../features/remote-bridge/state";

export type RemoteNavState = "idle" | "connecting" | "connected" | "attention";

// Navigation describes transport only, never optional setup or model verification.
export function remoteNavigationState(summary: Pick<BridgeSummary, "status" | "reconnectState">): RemoteNavState {
  if (summary.reconnectState === "attentionRequired" || ["stale", "unavailable", "error"].includes(summary.status)) return "attention";
  if (summary.status === "connecting" || summary.reconnectState === "waiting" || summary.reconnectState === "retrying") return "connecting";
  return summary.status === "connected" ? "connected" : "idle";
}
