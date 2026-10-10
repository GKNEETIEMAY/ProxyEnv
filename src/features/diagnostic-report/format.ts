import type { Copy } from "../../shared/i18n";
import type { DiagnosticReportData } from "../../shared/types";

/** Pure formatting: one immutable safe snapshot can be rendered in any locale. */
export function formatDiagnosticReport(data: DiagnosticReportData, copy: Copy): string {
  const field = (label: string, value: string | number) => `- ${label}: ${value}`;
  const section = (label: string, lines: string[]) => `${label}\n${lines.join("\n")}`;
  const unknown = copy.reportUnknown;
  const environment = { disabled: copy.environmentDisabled, partial: copy.environmentPartial, enabled: copy.environmentEnabled, mismatch: copy.environmentMismatch };
  const tun = { notDetected: copy.tunNotDetected, possible: copy.tunPossible, detected: copy.tunDetected, unknown: copy.tunUnknown };
  const connection = {
    notTested: copy.reportNotTested, testing: copy.reportTesting, reachable: copy.reportReachable,
    partial: copy.reportPartial, unreachable: copy.reportUnreachable, localProxyUnavailable: copy.reportProxyUnavailable, unknown
  };
  const errors = {
    proxyUnavailable: copy.reportProxyUnavailable, proxyHandshakeFailed: copy.reportHandshakeFailed,
    connectTimeout: copy.reportTimeout, tlsFailed: copy.reportTlsFailed, remoteRejected: copy.reportRemoteRejected,
    httpStatus: copy.reportHttpStatus, networkError: copy.reportNetworkError, unknown
  };
  const diagnosis = {
    confirmedReady: copy.assistantConfirmedReadyTitle, environmentConfigured: copy.assistantEnvironmentConfiguredTitle,
    proxyLaunchRecommended: copy.assistantProxyLaunchRecommendedTitle, ruleSyncRecommended: copy.assistantRuleSyncRecommendedTitle,
    conflict: copy.assistantConflictTitle, unsupported: copy.assistantUnsupportedStateTitle, unknown: copy.assistantUnknownTitle
  };
  const action = { none: copy.reportNone, launchWithProxy: copy.assistantLaunchWithProxy, launchWithoutProxy: copy.assistantLaunchDirect, applyKnownRule: copy.assistantRuleSyncRecommendedTitle };
  const category = { notSelected: copy.reportNotSelected, knownRule: copy.reportKnownRule, unrecognized: copy.reportUnrecognized, unavailable: copy.assistantUnavailable };
  const protocol = { http: "HTTP", socks5: "SOCKS5", mixed: "Mixed", unknown };
  const confidence = { veryHigh: copy.reportVeryHigh, high: copy.reportHigh, medium: copy.reportMedium, low: copy.reportLow };
  const variableNames = { http: "HTTP_PROXY", https: "HTTPS_PROXY", all: "ALL_PROXY" };
  const variables = data.managedVariables.map(variable => data.os === "windows" ? variableNames[variable] : variableNames[variable].toLowerCase());
  const os = ({ windows: "Windows", macos: "macOS", linux: "Linux" } as Record<string, string>)[data.os] ?? unknown;
  const bridge = data.remoteBridge;
  const runtime = {pending:copy.rbAdvPending, preparing:copy.rbAdvPreparing, ready:copy.rbAdvReady, warning:copy.rbAdvWarning};
  const errorCategory = {network:copy.rbAdvErrorNetwork, ssh:copy.rbAdvErrorSsh, authentication:copy.rbAdvErrorAuthentication, conflict:copy.rbAdvErrorConflict, unsupported:copy.rbAdvErrorUnsupported, unavailable:copy.rbAdvErrorUnavailable, invalidResponse:copy.rbAdvErrorResponse, unknown:copy.rbAdvErrorUnknown};
  const proof = {notTested:copy.rbAdvNotVerified, testing:copy.rbCheckChecking, passed:copy.rbAdvVerified, failed:copy.rbAdvFailed};
  const bridgeDetails:string[] = [];
  if (bridge) {
    if (bridge.security) {
      bridgeDetails.push(field(copy.rbAdvBind, bridge.security.remoteBindScope === 'loopback' ? copy.rbAdvLoopback : unknown));
      bridgeDetails.push(field(copy.rbAdvProxyAuth, bridge.security.generalProxyAuth === 'none' ? copy.rbAdvNoProxyAuth : unknown));
      bridgeDetails.push(field(copy.rbAdvAiAuth, bridge.security.aiRouteAuth === 'session' ? copy.rbAdvSessionAuth : unknown));
      bridgeDetails.push(field(copy.rbAdvShell, bridge.security.shellScope === 'sessionOnly' ? copy.rbAdvSessionOnly : unknown));
    }
    if (bridge.reconnectState) bridgeDetails.push(field(copy.rbAdvRecovery, ({idle:copy.rbAdvIdle, waiting:copy.rbReconnectWaiting, retrying:copy.rbReconnectWaiting, attentionRequired:copy.rbReconnectAttention})[bridge.reconnectState]));
    if (bridge.sshAuthMethod) bridgeDetails.push(field(copy.rbAdvAuthentication, ({identityFile:copy.rbTargetAuthIdentity, agent:copy.rbTargetAuthAgent, password:copy.rbTargetAuthPassword, keyboardInteractive:"Keyboard Interactive", unknown})[bridge.sshAuthMethod]));
    if (bridge.runtimeProxyMatch) bridgeDetails.push(field(copy.rbAdvRuntimePort, ({matched:copy.rbAdvMatched, mismatch:copy.rbAdvMismatch, unknown})[bridge.runtimeProxyMatch]));
    if (bridge.postConnectStatus) bridgeDetails.push(field(copy.rbAdvPostConnect, ({idle:copy.rbAdvIdle, preparing:copy.rbAdvPreparing, ready:copy.rbAdvReady, partial:copy.rbAdvWarning})[bridge.postConnectStatus]));
    for (const [name,state] of [[copy.rbAdvTerminal,bridge.sessionEnvironmentState],["VS Code",bridge.vscodeState],["Codex",bridge.codexState],["Claude Code",bridge.claudeState],["Skills",bridge.skillsState]] as const) {
      if (state) bridgeDetails.push(field(name, runtime[state]));
    }
    if (bridge.postConnectErrorCategory) bridgeDetails.push(field(copy.rbAdvErrorCategory, errorCategory[bridge.postConnectErrorCategory]));
    for (const [name,observation] of [[copy.rbServerInternet,bridge.diagnostics?.serverDirect],[copy.rbAdvEgress,bridge.diagnostics?.generalProxyEgress],[copy.rbAdvModelRequest,bridge.diagnostics?.aiRouteVerification]] as const) {
      if (!observation) continue;
      bridgeDetails.push(field(name, proof[observation.state]));
      if (observation.checkedAt !== null) bridgeDetails.push(field(`${name} · ${copy.rbAdvLastChecked}`, new Date(observation.checkedAt).toISOString()));
      if (observation.durationMs !== null) bridgeDetails.push(field(`${name} · ${copy.rbAdvDuration}`, `${observation.durationMs} ms`));
      if (observation.errorCode) bridgeDetails.push(field(`${name} · ${copy.rbAdvErrorCategory}`, errorCategory[observation.errorCode]));
    }
    const phases = new Set(["bridge.prepare","ssh.authenticate","coreBridge.total","bridge.forwardReady","bridge.total","sessionEnv.apply","vscodeNetwork.refresh","codex.status","codex.preview","codex.apply","claude.status","claude.preview","claude.apply","postConnect.total"]);
    for (const timing of bridge.phaseTimings ?? []) {
      const outcome = ({success:copy.rbAdvSuccess, failed:copy.rbAdvFailed, unsupported:copy.rbAdvNotSupported} as Record<string,string>)[timing.outcome] ?? unknown;
      bridgeDetails.push(field(`${copy.rbAdvPhase} · ${phases.has(timing.phase) ? timing.phase : unknown}`, `${timing.durationMs} ms · ${outcome}`));
    }
  }
  return [
    copy.reportTitle,
    ...(data.remoteBridge ? [section(copy.rbTitle, [
      field(copy.rbAlias, data.remoteBridge.configured ? copy.reportDetected : copy.reportNotSelected),
      field(copy.rbCheck, data.remoteBridge.reachable ? copy.reportReachable : copy.reportUnknown),
      field(copy.reportStatus, copy.rbStates[data.remoteBridge.status]),
      field(copy.protocol, data.remoteBridge.protocol ? protocol[data.remoteBridge.protocol] : unknown),
      field(copy.rbProxy, `${data.remoteBridge.proxyStatus ? copy.rbStates[data.remoteBridge.proxyStatus] : unknown} / ${data.remoteBridge.proxyPort ?? copy.reportNone}`),
      field(copy.rbCc, `${data.remoteBridge.ccDetected ? copy.reportListening : unknown} / ${data.remoteBridge.ccStatus ? copy.rbStates[data.remoteBridge.ccStatus] : unknown} / ${data.remoteBridge.ccPort ?? copy.reportNone}`),
      field("Codex", data.remoteBridge.codexConfigured ? copy.rbApplied : copy.reportUnknown),
      field("Claude Code CLI", data.remoteBridge.claudeConfigured ? copy.rbApplied : copy.reportUnknown),
      ...([['Codex', data.remoteBridge.codexExtension], ['Claude Code', data.remoteBridge.claudeExtension]] as const).map(([name, state]) => field(`${name} · ${copy.rbExtGui}`, state === 'configured' ? copy.rbExtPending : state === 'notConfigured' ? copy.rbExtNotConfigured : state === 'conflict' ? copy.rbConfigError : copy.reportUnknown)),
      ...bridgeDetails
    ])] : []),
    `${copy.reportVersion}: ${data.appVersion}\n${copy.reportOs}: ${os} ${data.osVersion ?? unknown}`,
    section(copy.proxyClient, [field(copy.reportDetected, data.detectedCount), field(copy.reportListening, data.listeningCount),
      field(copy.reportSelected, data.hasSelection ? data.selectedClient ?? copy.reportOtherClient : copy.reportNotSelected),
      field(copy.reportStatus, data.hasSelection ? data.available ? copy.assistantAvailable : copy.assistantUnavailable : copy.reportNotSelected),
      field(copy.protocol, data.protocol ? protocol[data.protocol] : unknown), field(copy.autoConfidence, data.confidence ? confidence[data.confidence] : unknown)]),
    section(copy.windowsSystemProxy, [field(copy.reportStatus, data.systemProxyEnabled === null ? unknown : data.systemProxyEnabled ? copy.systemProxyOn : copy.systemProxyOff)]),
    section(copy.proxyEnvironment, [field(copy.reportStatus, environment[data.environment]), field(copy.reportManagedVariables, variables.join(", ") || copy.reportNone)]),
    section(copy.assistantTun, [field(copy.reportStatus, tun[data.tun])]),
    section(copy.reportConnectivity, [field(copy.reportStatus, connection[data.connectivity]), field(copy.reportSuccessfulTargets, `${data.successfulTargets} / ${data.totalTargets}`), field(copy.reportErrors, data.errorCategories.map(kind => errors[kind]).join("; ") || copy.reportNone)]),
    section(copy.assistantTitle, [field(copy.reportCategory, category[data.assistant.category]), field(copy.assistantDiagnosisState, diagnosis[data.assistant.state]), field(copy.assistantRecommendation, action[data.assistant.action])])
  ].join("\n\n");
}
