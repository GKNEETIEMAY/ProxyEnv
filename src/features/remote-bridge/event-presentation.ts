import type { RemoteBridgeCopy } from "../../shared/i18n/remote-bridge";
import type { BridgeErrorCategory, BridgeEvent } from "./state";

/** Shared by recent events and the safe diagnostic report. Never display raw errors. */
export function bridgeErrorLabels(copy: RemoteBridgeCopy): Record<BridgeErrorCategory, string> {
  return {
    network: copy.rbAdvErrorNetwork, ssh: copy.rbAdvErrorSsh,
    authentication: copy.rbAdvErrorAuthentication, conflict: copy.rbAdvErrorConflict,
    unsupported: copy.rbAdvErrorUnsupported, unavailable: copy.rbAdvErrorUnavailable,
    invalidResponse: copy.rbAdvErrorResponse, unknown: copy.rbAdvErrorUnknown,
    invalidLocalProfile: copy.rbProfileInvalid, unsafePath: copy.rbAdvErrorUnsafePath,
    dependencyMissing: copy.rbAdvErrorDependencyMissing, writeFailed: copy.rbAdvErrorWriteFailed,
    verificationFailed: copy.rbAdvErrorVerificationFailed, rolledBack: copy.rbAdvErrorRolledBack,
    rollbackFailed: copy.rbAdvErrorRollbackFailed,
  };
}

export function bridgeEventStatus(event: Pick<BridgeEvent, "action" | "outcome" | "errorCode">, copy: RemoteBridgeCopy): string {
  let outcome = {success:copy.rbAdvSuccess, warning:copy.rbAdvWarning, failed:copy.rbAdvOperationFailed, started:copy.rbAdvStarted}[event.outcome];
  if (event.outcome === "failed") {
    if (["toolSetup", "environmentSetup", "vscodeSetup"].includes(event.action)) outcome = copy.rbAdvConfigurationFailed;
    else if (["toolVerify", "egressTest", "serverDirect"].includes(event.action)) outcome = copy.rbAdvFailed;
  }
  return event.errorCode ? `${outcome} · ${bridgeErrorLabels(copy)[event.errorCode] ?? copy.rbAdvErrorUnknown}` : outcome;
}
