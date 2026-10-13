import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";

const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const browser = await chromium.launch({ headless: true, ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath: process.env.PROXYENV_LAYOUT_BROWSER } : {}) });
try {
  const page = await browser.newPage({ viewport: { width: 980, height: 800 }, reducedMotion: "reduce" });
  page.setDefaultTimeout(15000);
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.addInitScript(() => {
    window.isTauri = true;
    window.calls = [];
    window.cachedBridge = {
      status: "connected", target: { id: "readiness-lab", displayName: "Lab", source: "manual", sourceLabel: "ProxyEnv", configPath: "ProxyEnv", host: "lab.example.test", user: "dev", port: 22, available: true, compatibility: "compatible", canOpenVscode: true, canOpenMobaxterm: true },
      proxy: { local: { host: "127.0.0.1", port: 7897, protocol: "mixed" }, remotePort: 7897 }, cc: { local: { host: "127.0.0.1", port: 15721, protocol: "http" }, remotePort: 15721 },
      proxyStatus: "connected", ccStatus: "connected", activeProxyRevision: 1, environment: "", codexConfigured: false, claudeConfigured: false, error: null,
      sshAuth: { mode: "interactive", method: "password", authenticated: true, passwordStored: false }, reconnectState: "idle",
      postConnectStatus: "preparing", sessionEnvironmentState: "preparing", vscodeState: "preparing", codexState: "preparing", claudeState: "preparing",
      toolInstallations: [{ tool: "codex", state: "checking", extensionState: "checking" }, { tool: "claude", state: "checking", extensionState: "checking" }],
      diagnostics: { generalProxyEgress: { state: "testing", checkedAt: null }, aiRouteVerification: { state: "notTested", checkedAt: null }, serverDirect: { state: "failed", checkedAt: 1 }, serverInternet: "unreachable" },
    };
    window.__TAURI_INTERNALS__ = { invoke: async command => {
      window.calls.push(command);
      if (command === "remote_bridge_summary") return structuredClone(window.cachedBridge);
      if (command === "remote_bridge_detect_cc") return { state: "confirmed", localPort: 15721 };
      if (command === "remote_bridge_targets") return [window.cachedBridge.target];
      if (command === "remote_bridge_skills" || command === "remote_bridge_events") return [];
      if (command === "remote_bridge_log_status") return { available: true };
      if (command === "remote_bridge_session_environment_command") return '. "$HOME/.proxyenv/sessions/test/env.sh"';
      if (command === "remote_bridge_check_network") return { serverInternet: "unreachable" };
      if (command === "remote_bridge_run_diagnostics") {
        await new Promise(resolve => setTimeout(resolve, 200));
        return { running: false, observations: structuredClone(window.cachedBridge.diagnostics) };
      }
      throw Error("Unsupported synthetic IPC: " + command);
    } };
  });
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || "http://localhost:1420"}/?impeccable-review=remote-connected`);
  await page.waitForFunction(() => document.querySelector('.remote-target-name strong')?.textContent === 'Lab');
  assert.equal(await page.locator(".remote-readiness").count(), 0);
  await page.waitForFunction(() => document.querySelector('.bridge-ai-section')?.textContent.includes('正在检测安装状态'));
  for (const name of ["打开终端", "在 VS Code 中打开", "启动 MobaXterm"]) assert.equal(await page.locator(".bridge-launch-bar button").filter({ hasText: name }).isEnabled(), true);
  assert.equal(await page.locator("dialog[open]").count(), 0, "Missing tools never open an installation or authentication modal automatically");
  const refresh = page.getByRole("button", { name: "重新检测", exact: true });
  assert.equal(await refresh.isEnabled(), true, "Background preparation or local CC detection does not lock manual diagnostics");
  await refresh.click();
  await page.waitForFunction(() => window.calls.filter(command => command === 'remote_bridge_run_diagnostics').length === 1);
  await page.waitForFunction(() => !document.querySelector('.remote-status-toolbar button[aria-busy]')?.disabled);
  const shots = resolve(".impeccable/review/remote-first-connect");
  if (process.argv.includes("--screenshots")) mkdirSync(shots, { recursive: true });
  for (const [theme, width, height] of [["light", 980, 800], ["dark", 620, 620]]) {
    await page.setViewportSize({ width, height });
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    if (process.argv.includes("--screenshots")) await page.screenshot({ path: resolve(shots, `preparing-${theme}.png`) });
  }
  await page.setViewportSize({ width: 980, height: 800 });
  await page.evaluate(() => {
    const summary = window.cachedBridge;
    summary.postConnectStatus = "ready";
    for (const key of ["sessionEnvironmentState", "vscodeState", "codexState", "claudeState"]) summary[key] = "ready";
    summary.toolInstallations = [{ tool: "codex", state: "missing", extensionState: "installed" }, { tool: "claude", state: "missing", extensionState: "missing" }];
    summary.diagnostics.generalProxyEgress.state = "passed";
  });
  await page.waitForFunction(() => !document.querySelector('.bridge-ai-section')?.textContent.includes('正在检测安装状态'));
  assert.equal(await page.locator(".remote-readiness").count(), 0);
  assert.equal(await page.locator("dialog[open]").count(), 0);
  await page.evaluate(() => { window.cachedBridge.vscodeState = "warning"; window.cachedBridge.diagnostics.generalProxyEgress.state = "failed"; });
  await page.getByRole("button", { name: "高级", exact: true }).click();
  await page.getByRole("button", { name: "概览", exact: true }).click();
  assert.equal(await page.getByText("部分准备需要处理", { exact: false }).count(), 0);
  await page.evaluate(() => { window.cachedBridge.reconnectState = "retrying"; });
  await page.locator(".remote-reconnect-inline").waitFor();
  await page.evaluate(() => { window.cachedBridge.reconnectState = "idle"; window.cachedBridge.vscodeState = "ready"; window.cachedBridge.diagnostics.generalProxyEgress.state = "passed"; window.cachedBridge.proxy = null; window.cachedBridge.proxyStatus = null; window.cachedBridge.codexState = "preparing"; });
  await page.locator(".remote-reconnect-inline").waitFor({ state: "hidden" });
  await page.waitForFunction(() => document.querySelector('.bridge-ai-section')?.textContent.includes('正在检测安装状态'));
  assert.equal(await page.locator(".remote-readiness").count(), 0);
  assert.equal(await page.evaluate(() => window.calls.some(command => /tool_verify|config_apply|extension_apply|install/.test(command))), false);
  assert.deepEqual(errors, []);
  console.log("First connection UI: nonblocking preparation, working manual diagnostics, launch availability, neutral missing CLI, no aggregate warning, reconnect feedback, AI-only and light/dark narrow layout passed (synthetic IPC only).");
} finally {
  await browser.close();
}
