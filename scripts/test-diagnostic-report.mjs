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
const labels = await moduleUrl("../src/shared/i18n/diagnostic-report.ts");
const { messages } = await import(await moduleUrl("../src/shared/i18n/index.ts", { '"./diagnostic-report"': JSON.stringify(labels) }));
const { formatDiagnosticReport } = await import(await moduleUrl("../src/features/diagnostic-report/format.ts"));
const data = {
  appVersion: "0.1.3", os: "windows", osVersion: "11 (26100)", detectedCount: 2, listeningCount: 1,
  selectedClient: "Clash Verge Rev", hasSelection: true, available: true, protocol: "mixed", confidence: "high",
  systemProxyEnabled: true, environment: "enabled", managedVariables: ["http", "https"], tun: "possible",
  connectivity: "partial", successfulTargets: 2, totalTargets: 3, errorCategories: ["connectTimeout"],
  assistant: { category: "unrecognized", state: "environmentConfigured", action: "none" }
};

test("all four locales format the same snapshot without mutating its diagnosis", () => {
  const before = JSON.stringify(data);
  const titles = { en: "ProxyEnv Diagnostic Report", "zh-CN": "ProxyEnv 诊断报告", ja: "ProxyEnv 診断レポート", ko: "ProxyEnv 진단 보고서" };
  for (const [locale, copy] of Object.entries(messages)) {
    const report = formatDiagnosticReport(data, copy);
    assert.ok(report.startsWith(titles[locale]));
    for (const value of ["0.1.3", "Clash Verge Rev", "Mixed", "2 / 3", "HTTP_PROXY", "HTTPS_PROXY", copy.assistantEnvironmentConfiguredTitle, copy.reportTimeout]) assert.ok(report.includes(value), `${locale}: ${value}`);
    assert.ok(!report.includes("undefined"));
    assert.ok(!report.includes("ALL_PROXY"));
  }
  assert.equal(JSON.stringify(data), before);
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
