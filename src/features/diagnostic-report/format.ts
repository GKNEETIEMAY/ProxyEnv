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
  return [
    copy.reportTitle,
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
