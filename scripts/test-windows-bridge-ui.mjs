import assert from "node:assert/strict";
import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";

const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const screenshots = resolve(process.env.ROUTESPAN_M34_SCREENSHOTS || "artifacts/v0.3.0/M3.4/screenshots");
mkdirSync(screenshots, { recursive: true });
const browser = await chromium.launch({
  headless: true,
  args: ["--no-proxy-server"],
  ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath: process.env.PROXYENV_LAYOUT_BROWSER } : {}),
});

try {
  const page = await browser.newPage({ viewport: { width: 880, height: 720 }, reducedMotion: "reduce" });
  await page.addInitScript(() => {
    window.isTauri = true;
    window.pendingPlatformProbes = [];
    window.unexpectedBridgeCalls = [];
    window.fixtureSummary = {
      status: "disconnected", target: null, proxy: null, cc: null, proxyStatus: null, ccStatus: null,
      activeProxyRevision: null, environment: "", codexConfigured: false, claudeConfigured: false,
      error: null, sshAuth: { mode: "nonInteractive", method: "unknown", authenticated: false, passwordStored: false },
    };
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        if (command === "remote_bridge_summary") return structuredClone(window.fixtureSummary);
        if (command === "remote_bridge_detect_cc") return { state: "notDetected", localPort: 15721 };
        if (command === "remote_bridge_clear_session_credential") return undefined;
        if (command === "remote_bridge_probe_target_os") {
          return new Promise((complete, reject) => window.pendingPlatformProbes.push({
            id: args.targetId,
            complete: (platform, windowsBridge) => complete({
              targetId: args.targetId, platform, status: platform === "windows" ? "confirmed" : "unknown", windowsBridge,
            }),
            reject,
          }));
        }
        if (command.startsWith("remote_bridge_") || command.startsWith("ssh_auth_")) {
          window.unexpectedBridgeCalls.push(command);
        }
        return undefined;
      },
    };
  });
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || "http://127.0.0.1:1434"}/?impeccable-review=remote`, { waitUntil: "domcontentloaded" });
  const platform = page.locator(".remote-platform-status");
  const platformLabel = platform.locator("strong");
  const probe = platform.getByRole("button", { name: "检查平台" });
  const candidate = page.getByRole("radio").filter({ hasText: "gpu-server" });
  const connect = page.locator(".remote-connect-action");
  const pendingCount = (count) => page.waitForFunction(expected => window.pendingPlatformProbes.length === expected, count);
  const complete = (index, value, windowsBridge) => page.evaluate(
    ({ index, value, windowsBridge }) => window.pendingPlatformProbes[index].complete(value, windowsBridge),
    { index, value, windowsBridge },
  );
  const label = (value) => page.waitForFunction(
    expected => document.querySelector(".remote-platform-label strong")?.textContent === expected,
    value,
  );
  await label("未知");

  await probe.click();
  await pendingCount(1);
  assert.ok(await connect.isDisabled());
  await platform.getByRole("button", { name: "取消", exact: true }).click();
  await complete(0, "windows", { transport: "unavailable", terminal: "unavailable" });
  await label("未知");
  assert.equal(await platform.locator(".remote-platform-windows").count(), 0);

  await probe.click();
  await pendingCount(2);
  await candidate.click();
  await complete(1, "windows", { transport: "unavailable", terminal: "unavailable" });
  await label("未知");
  await probe.click();
  await pendingCount(3);
  await page.evaluate(() => window.pendingPlatformProbes[2].reject({ code: "sshAuth" }));
  assert.match(await platform.getByRole("alert").innerText(), /SSH 无法连接.*认证/);
  assert.equal(await platformLabel.innerText(), "未知");

  await probe.click();
  await pendingCount(4);
  await complete(3, "windows", { transport: "unavailable", terminal: "unavailable" });
  await label("Windows");
  assert.ok(await connect.isDisabled());
  assert.equal(await page.locator(".remote-current-state.unavailable").innerText(), "当前不可用");
  assert.equal(await page.locator(".remote-setup-panel .switch-input").count(), 0);
  assert.match(await platform.innerText(), /实机验收/);
  await page.screenshot({ path: resolve(screenshots, "windows-blocked-light.png") });

  const setWindows = async (transport, terminal) => page.evaluate(({ transport, terminal }) => {
    Object.assign(window.fixtureSummary, {
      status: "connected",
      targetPlatform: "windows",
      target: { id: "vscode|preview|gpu-server", displayName: "gpu-server" },
      windowsBridge: { transport, terminal },
    });
  }, { transport, terminal });
  await setWindows("awaitingAuthorization", "awaitingEnvironment");
  await platform.getByText("等待授权", { exact: true }).waitFor();
  await platform.getByText("等待代理环境", { exact: true }).waitFor();
  assert.equal(await page.locator(".bridge-launch-bar").count(), 0);
  assert.ok(await connect.isDisabled());
  await page.setViewportSize({ width: 560, height: 620 });
  await page.evaluate(() => { document.documentElement.dataset.theme = "dark"; });
  await platform.scrollIntoViewIfNeeded();
  assert.ok(await platform.evaluate(element => element.scrollWidth <= element.clientWidth + 1));
  await page.screenshot({ path: resolve(screenshots, "windows-awaiting-dark.png") });

  await setWindows("preparing", "prepared");
  await platform.getByText("代理环境已准备", { exact: true }).waitFor();
  assert.equal(await page.locator(".remote-target-name .check-status").count(), 0);
  await setWindows("cleanupIncomplete", "closed");
  await platform.getByText("清理尚未完成", { exact: true }).waitFor();
  await page.screenshot({ path: resolve(screenshots, "windows-cleanup-dark.png") });
  assert.deepEqual(await page.evaluate(() => window.unexpectedBridgeCalls), []);

  await page.evaluate(() => {
    Object.assign(window.fixtureSummary, {
      status: "disconnected", targetPlatform: null, target: null, windowsBridge: null,
    });
  });
  await page.getByRole("radio").filter({ hasText: "aliyun-dev" }).click();
  await label("未知");
  await probe.click();
  await pendingCount(5);
  await complete(4, "linux");
  await label("Linux");
  assert.ok(await connect.isEnabled());
  assert.equal(await platform.locator(".remote-platform-windows").count(), 0);
  assert.deepEqual(await page.evaluate(() => window.unexpectedBridgeCalls), []);

  const localizedPlatformCopy = await page.evaluate(async () => {
    const { remoteBridgeMessages } = await import("/src/shared/i18n/remote-bridge.ts");
    return Object.fromEntries(Object.entries(remoteBridgeMessages).map(([locale, copy]) => [
      locale, [copy.rbTargetPlatform, copy.rbWindowsBridgeUnavailable, copy.rbPlatformCleanupIncomplete],
    ]));
  });
  assert.deepEqual(Object.keys(localizedPlatformCopy).sort(), ["en", "ja", "ko", "zh-CN"]);
  for (const copy of Object.values(localizedPlatformCopy)) {
    assert.ok(copy.every(message => typeof message === "string" && message.trim() !== ""));
  }
  for (const index of [0, 1, 2]) {
    assert.equal(new Set(Object.values(localizedPlatformCopy).map(copy => copy[index])).size, 4);
  }
  console.log("Windows and Linux platform IPC, cancellation, stale replies, production action gates, managed state and cleanup UI passed.");
} finally {
  await browser.close();
}
