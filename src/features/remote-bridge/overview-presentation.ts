import type { CheckState } from "../../shared/types";
import type { RemoteBridgeCopy } from "../../shared/i18n/remote-bridge";
import type { BridgeStatus, BridgeSummary, DiagnosticState, RemoteSkill } from "./state";
import type { RemoteToolId } from "./tool-adapters";

export const bridgeEndpointLabel = (host: string, port: number) => `${host.includes(":") ? `[${host}]` : host}:${port}`;

export function diagnosticCheckState(state: DiagnosticState | undefined): CheckState {
  return state === "passed" ? "healthy" : state === "failed" ? "failed" : state === "testing" ? "checking" : "idle";
}

export function serverDirectCheckState(summary: BridgeSummary): CheckState {
  if (!summary.target || !summary.sshAuth.authenticated || !["connected", "stale", "unavailable"].includes(summary.status)) return "idle";
  const state = diagnosticCheckState(summary.diagnostics?.serverDirect?.state);
  if (state === "idle" || state === "checking") return state;
  // A failed probe is not proof that the server cannot reach the internet.
  const internet = summary.diagnostics?.serverInternet;
  return internet === "reachable" ? "healthy" : internet === "unreachable" ? "failed" : "warning";
}

export function overviewServerDirectStatus(copy: RemoteBridgeCopy, state: CheckState): { state: CheckState; label: string } {
  if (state === "healthy") return { state, label: copy.rbOverviewDirectAvailable };
  if (state === "failed") return { state: "warning", label: copy.rbOverviewDirectUnavailable };
  if (state === "warning") return { state: "idle", label: copy.rbOverviewDirectUnknown };
  if (state === "checking") return { state, label: copy.rbCheckChecking };
  return { state: "idle", label: copy.rbCheckIdle };
}

export function bridgeCapabilityState(status: BridgeStatus | null, enabled: boolean): CheckState {
  if (!enabled) return "disabled";
  if (status === "connected") return "healthy";
  if (status === "connecting") return "checking";
  if (status === "error" || status === "unavailable") return "failed";
  return "warning";
}

export function overviewToolChecking(summary: BridgeSummary, id: RemoteToolId): boolean {
  if (!summary.cc) return false;
  const state = id === "codex" ? summary.codexState : summary.claudeState;
  // Completed tools must not wait for unrelated post-connect jobs.
  return state === "pending" || state === "preparing" || (state === undefined && summary.postConnectStatus === "preparing");
}

export function overviewToolLabel(copy: RemoteBridgeCopy, summary: BridgeSummary, id: RemoteToolId, configured: boolean): string {
  if (overviewToolChecking(summary, id)) return copy.rbCheckChecking;
  if (!configured) return copy.rbNotConfigured;
  if (!summary.cc || summary.ccStatus !== "connected") return copy.rbToolRouteUnavailable;
  const profile = id === "claude" ? summary.claudeProfileState : undefined;
  if (profile === "conflict" || profile === "remoteChanged") return copy.rbProfileConflict;
  if (profile === "restartRequired") return copy.rbOverviewRestartRequired;
  if (profile === "localChanged") return copy.rbProfileLocalChanged;
  if (profile === "invalidLocalProfile") return copy.rbProfileInvalid;
  if (profile === "remoteUnavailable") return copy.rbToolRouteUnavailable;
  // No Codex profile-sync evidence is returned by BridgeSummary. Do not invent it.
  return profile === "synced" ? copy.rbOverviewSynced : copy.rbOverviewConfigured;
}

export function overviewSkillStatus(copy: RemoteBridgeCopy, skill?: RemoteSkill): { state: CheckState; label: string } {
  if (!skill) return { state: "disabled", label: copy.rbSkillNotLinked };
  if (!skill.enabled) return { state: "disabled", label: copy.rbAccessDisabled };
  const label = ({ synced: copy.rbSkillSynced, notSynced: copy.rbSkillNotSynced, localChanged: copy.rbSkillChanged, conflict: copy.rbSkillConflict, unavailable: copy.rbSkillUnavailable })[skill.state];
  const state = skill.state === "synced" ? "healthy" : skill.state === "unavailable" ? "failed" : "warning";
  return { state, label };
}
