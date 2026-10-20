import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import ts from "typescript";

// Compile pure TS modules in memory: no generated files and no DOM/network dependency.
async function moduleUrl(path, replacements = {}) {
  let source = await readFile(new URL(path, import.meta.url), "utf8");
  for (const [from, to] of Object.entries(replacements)) source = source.replace(from, to);
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } });
  return `data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`;
}
const remoteLabels = await moduleUrl("../src/shared/i18n/remote-bridge.ts");
const labels = await moduleUrl("../src/shared/i18n/diagnostic-report.ts");
const desktopLabels = await moduleUrl("../src/shared/i18n/desktop.ts");
const { messages } = await import(await moduleUrl("../src/shared/i18n/index.ts", { '"./diagnostic-report"': JSON.stringify(labels), '"./remote-bridge"': JSON.stringify(remoteLabels), '"./desktop"': JSON.stringify(desktopLabels) }));
const eventPresentation = await moduleUrl("../src/features/remote-bridge/event-presentation.ts");
const { bridgeEventStatus, bridgeErrorLabels } = await import(eventPresentation);
const { formatDiagnosticReport } = await import(await moduleUrl("../src/features/diagnostic-report/format.ts", { '"../remote-bridge/event-presentation"': JSON.stringify(eventPresentation) }));
const data = {
  appVersion: "0.1.3", os: "windows", osVersion: "11 (26100)", detectedCount: 2, listeningCount: 1,
  selectedClient: "Clash Verge Rev", hasSelection: true, available: true, protocol: "mixed", confidence: "high",
  systemProxyEnabled: true, environment: "enabled", managedVariables: ["http", "https"], tun: "possible",
  connectivity: "partial", successfulTargets: 2, totalTargets: 3, errorCategories: ["connectTimeout"],
  assistant: { category: "unrecognized", state: "environmentConfigured", action: "none" }
};

test("client configuration failures retain specific safe reasons in events and reports in all locales", () => {
  const reasons = {invalidLocalProfile:"rbProfileInvalid", unsafePath:"rbAdvErrorUnsafePath", dependencyMissing:"rbAdvErrorDependencyMissing", writeFailed:"rbAdvErrorWriteFailed", verificationFailed:"rbAdvErrorVerificationFailed", rolledBack:"rbAdvErrorRolledBack", rollbackFailed:"rbAdvErrorRollbackFailed"};
  for (const copy of Object.values(messages)) {
    for (const [errorCode, key] of Object.entries(reasons)) {
      const event = {action:"toolSetup", outcome:"failed", errorCode};
      assert.equal(bridgeEventStatus(event, copy), `${copy.rbAdvConfigurationFailed} · ${copy[key]}`);
      assert.equal(bridgeErrorLabels(copy)[errorCode], copy[key]);
      const report = formatDiagnosticReport({...data, remoteBridge:{configured:true, reachable:true, status:"connected", protocol:"mixed", proxyPort:7897, ccDetected:true, ccPort:15721, codexConfigured:false, claudeConfigured:true, postConnectErrorCategory:errorCode}}, copy);
      assert.ok(report.includes(copy[key]));
      assert.doesNotMatch(report, /undefined/);
    }
    for (const action of ["toolVerify", "egressTest", "serverDirect"]) {
      assert.equal(bridgeEventStatus({action, outcome:"failed", errorCode:"network"}, copy), `${copy.rbAdvFailed} · ${copy.rbAdvErrorNetwork}`);
    }
    for (const action of ["toolSetup", "environmentSetup", "vscodeSetup"]) {
      assert.equal(bridgeEventStatus({action, outcome:"failed", errorCode:null}, copy), copy.rbAdvConfigurationFailed);
      assert.equal(bridgeEventStatus({action, outcome:"success", errorCode:null}, copy), copy.rbAdvSuccess);
    }
    assert.equal(bridgeEventStatus({action:"connect", outcome:"failed", errorCode:null}, copy), copy.rbAdvOperationFailed);
    assert.equal(bridgeEventStatus({action:"toolSetup", outcome:"failed", errorCode:"unknown"}, copy), `${copy.rbAdvConfigurationFailed} · ${copy.rbAdvErrorUnknown}`);
    assert.equal(bridgeEventStatus({action:"toolSetup", outcome:"failed", errorCode:"private-token"}, copy), `${copy.rbAdvConfigurationFailed} · ${copy.rbAdvErrorUnknown}`);
  }
});

test("official clients without a CC Switch route stay unconfigured with an explicit reason", () => {
  for (const copy of Object.values(messages)) {
    for (const outcome of ["warning","failed"]) {
      const event={action:"toolSetup",outcome,errorCode:"localRouteInactive"};
      assert.equal(bridgeEventStatus(event,copy),`${copy.rbNotConfigured} · ${copy.rbLocalRouteInactive}`);
      assert.ok(!bridgeEventStatus(event,copy).includes(copy.rbAdvConfigurationFailed));
      assert.ok(!bridgeEventStatus(event,copy).includes(copy.rbAdvErrorUnknown));
    }
    const report=formatDiagnosticReport({...data,remoteBridge:{configured:true,reachable:true,status:"connected",codexConfigured:false,claudeConfigured:false,postConnectErrorCategory:"localRouteInactive"}},copy);
    assert.ok(report.includes(copy.rbLocalRouteInactive));
  }
});

test("VS Code proxy configuration states use specific localized descriptions", () => {
  for (const copy of Object.values(messages)) {
    for (const [state,key] of Object.entries({notSet:'rbAdvProxyNotSet',disabled:'rbAdvProxyDisabled',invalidSettings:'rbAdvProxyInvalid',readFailed:'rbAdvProxyReadFailed',unsupportedProxy:'rbAdvProxyUnsupported',notCompared:'rbAdvProxyNotCompared',unknown:'rbAdvPortUnknown'})) {
      const report=formatDiagnosticReport({...data,remoteBridge:{runtimeProxyMatch:state}},copy);
      assert.ok(report.includes(copy[key]),`${state} must have a specific localized report description`);
    }
  }
});

test("all four locales format the same snapshot without mutating its diagnosis", () => {
  const before = JSON.stringify(data);
  const titles = { en: "RouteSpan Diagnostic Report", "zh-CN": "RouteSpan 诊断报告", ja: "RouteSpan 診断レポート", ko: "RouteSpan 진단 보고서" };
  for (const [locale, copy] of Object.entries(messages)) {
    const report = formatDiagnosticReport(data, copy);
    assert.ok(report.startsWith(titles[locale]));
    for (const value of ["0.1.3", "Clash Verge Rev", "Mixed", "2 / 3", "HTTP_PROXY", "HTTPS_PROXY", copy.assistantEnvironmentConfiguredTitle, copy.reportTimeout]) assert.ok(report.includes(value), `${locale}: ${value}`);
    assert.ok(!report.includes("undefined"));
    assert.ok(!report.includes("ALL_PROXY"));
  }
  assert.equal(JSON.stringify(data), before);
});

test("CLI installation states and versions are localized separately from route configuration",()=>{
  const snapshot={...data,remoteBridge:{configured:true,reachable:true,status:"connected",protocol:null,proxyPort:null,ccDetected:true,ccPort:15721,codexConfigured:false,claudeConfigured:true,toolInstallations:[{tool:"codex",state:"missing",version:null,executablePath:"/home/private/bin/codex",token:"private-token"},{tool:"claude",state:"installed",version:"2.1.227"}]}};
  const before=JSON.stringify(snapshot);
  for(const copy of Object.values(messages)) {
    const report=formatDiagnosticReport(snapshot,copy);
    assert.ok(report.includes(copy.rbInstallMissing));
    assert.ok(report.includes(`${copy.rbInstallInstalled} · 2.1.227`));
    assert.doesNotMatch(report,/private-token|\/home\/private|executablePath|undefined/);
  }
  assert.equal(JSON.stringify(snapshot),before);
});

test("unknown observations do not become successful results", () => {
  const snapshot = { ...data, selectedClient: null, hasSelection: false, available: false, protocol: null, confidence: null,
    connectivity: "notTested", successfulTargets: 0, totalTargets: 0, errorCategories: [], tun: "unknown", systemProxyEnabled: null,
    assistant: { category: "notSelected", state: "unknown", action: "none" } };
  for (const copy of Object.values(messages)) {
    const report = formatDiagnosticReport(snapshot, copy);
    assert.ok(report.includes(copy.reportNotTested));
    assert.ok(report.includes(copy.reportNotSelected));
    assert.ok(!report.includes("undefined"));
  }
});

test("copied reports omit dialog-only snapshot and privacy notices in every locale", () => {
  for (const copy of Object.values(messages)) {
    const report = formatDiagnosticReport(data, copy);
    assert.ok(!report.includes(copy.reportSnapshot));
    assert.ok(!report.includes(copy.reportPrivacy));
    assert.ok(report.includes(copy.assistantRecommendation));
  }
});

test("OS-specific variable casing is independent of output locale", () => {
  for (const copy of Object.values(messages)) {
    const report = formatDiagnosticReport({ ...data, os: "linux" }, copy);
    assert.ok(report.includes("http_proxy"));
    assert.ok(!report.includes("HTTP_PROXY"));
  }
});

test("every diagnostic enum and error category has a localized expression", () => {
  const states = ["confirmedReady", "environmentConfigured", "proxyLaunchRecommended", "ruleSyncRecommended", "conflict", "unsupported", "unknown"];
  const connectivity = ["notTested", "testing", "reachable", "partial", "unreachable", "localProxyUnavailable", "unknown"];
  for (const copy of Object.values(messages)) for (let i = 0; i < states.length; i++) {
    const report = formatDiagnosticReport({ ...data, connectivity: connectivity[i],
      errorCategories: ["proxyUnavailable", "proxyHandshakeFailed", "connectTimeout", "tlsFailed", "remoteRejected", "httpStatus", "networkError", "unknown"],
      assistant: { ...data.assistant, state: states[i] } }, copy);
    assert.ok(!report.includes("undefined"));
  }
});

test("report collection depends on non-probing active-state APIs", async () => {
  const source = await readFile(new URL("../src-tauri/src/features/diagnostic_report/mod.rs", import.meta.url), "utf8");
  assert.ok(source.includes("active::snapshot_status("));
  assert.ok(source.includes("active::snapshot()?"));
  assert.ok(!/active::(?:status|context|with_current)\(/.test(source));
  assert.ok(!source.includes("::diagnose_application"));
  const diagnosis = await readFile(new URL("../src-tauri/src/features/application_assistant/diagnosis.rs", import.meta.url), "utf8");
  const snapshotPath = diagnosis.split("fn diagnose_snapshot(")[1].split("#[derive")[0];
  assert.ok(!/active::(?:status|context)|inspect_endpoint|test_current_proxy|::detect\(/.test(snapshotPath));
});


test("remote bridge report contains only localized allowlisted summaries", () => {
  const remoteBridge = {configured:true,reachable:true,status:"stale",protocol:"mixed",proxyPort:17897,ccDetected:true,ccPort:25721,codexConfigured:true,claudeConfigured:false,
    alias:"private-server",username:"private-user",apiKey:"secret-fixture",path:"/home/private",environment:"secret-env"};
  for (const copy of Object.values(messages)) {
    const report=formatDiagnosticReport({...data,remoteBridge},copy);
    for(const value of [copy.rbTitle,copy.rbStates.stale,"17897","25721"]) assert.ok(report.includes(value));
    for(const secret of ["private-server","private-user","secret-fixture","/home/private","secret-env"]) assert.ok(!report.includes(secret));
    assert.ok(!report.includes("undefined"));
  }
});

test("cached remote diagnostics and runtime summaries are localized without leaking extra fields", () => {
  const remoteBridge = {configured:true,reachable:true,status:"connected",protocol:"mixed",proxyPort:7897,ccDetected:true,ccPort:15721,codexConfigured:true,claudeConfigured:true,
    reconnectState:"idle",sshAuthMethod:"password",runtimeProxyMatch:"matched",postConnectStatus:"partial",sessionEnvironmentState:"ready",vscodeState:"warning",codexState:"ready",claudeState:"ready",skillsState:"ready",postConnectErrorCategory:"conflict",
    diagnostics:{generalProxyEgress:{state:"failed",checkedAt:1791260000000,errorCode:"network",durationMs:183},aiRouteVerification:{state:"passed",checkedAt:1791260001000,errorCode:null,durationMs:45}},
    security:{generalProxyAuth:"none",aiRouteAuth:"session",remoteBindScope:"loopback",shellScope:"sessionOnly"},
    phaseTimings:[{phase:"ssh.authenticate",durationMs:84,outcome:"success"},{phase:"private-token",durationMs:6,outcome:"/home/private"}],
    password:"private-password",host:"private-host",request:"private-prompt"};
  const before = JSON.stringify(remoteBridge);
  for (const copy of Object.values(messages)) {
    const report = formatDiagnosticReport({...data,remoteBridge},copy);
    for (const text of [copy.rbAdvFailed,copy.rbAdvVerified,copy.rbAdvErrorNetwork,copy.rbAdvErrorConflict,copy.rbAdvRuntimePort,copy.rbAdvAuthentication,copy.rbAdvNoProxyAuth,copy.rbAdvSessionAuth,copy.rbAdvLoopback,copy.rbAdvSessionOnly,"183 ms","45 ms","ssh.authenticate"]) assert.ok(report.includes(text));
    for (const secret of ["private-token","private-password","private-host","private-prompt","/home/private","undefined"]) assert.ok(!report.includes(secret));
  }
  assert.equal(JSON.stringify(remoteBridge),before);
});

test("plugin-only report keeps CLI and extension installation independent",()=>{
  const remoteBridge={configured:true,reachable:true,status:"connected",codexConfigured:true,claudeConfigured:true,toolInstallations:[
    {tool:"codex",state:"missing",version:null,extensionState:"installed",extensionVersion:"26.825.2",path:"/home/private",token:"secret-fixture"},
    {tool:"claude",state:"missing",version:null,extensionState:"installed",extensionVersion:"2.1.252"},
  ]};
  for(const copy of Object.values(messages)) {
    const report=formatDiagnosticReport({...data,remoteBridge},copy);
    for(const text of [copy.rbInstallMissing,copy.rbInstallInstalled,copy.rbInstallExtension,"26.825.2","2.1.252"]) assert.ok(report.includes(text));
    assert.doesNotMatch(report,/\/home\/private|secret-fixture|undefined/);
  }
});
