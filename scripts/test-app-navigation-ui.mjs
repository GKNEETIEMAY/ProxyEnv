import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";

const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const browser = await chromium.launch({ headless: true, ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath: process.env.PROXYENV_LAYOUT_BROWSER } : {}) });
const shots = resolve(".impeccable/review/app-navigation");
if (process.argv.includes("--screenshots")) mkdirSync(shots, { recursive: true });
try {
  const page = await browser.newPage({ viewport: { width: 980, height: 800 }, reducedMotion: "reduce" });
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.addInitScript(() => {
    window.isTauri = true;
    window.cachedBridge = { status: "disconnected", target: null, proxy: null, cc: null, proxyStatus: null, ccStatus: null,
      codexConfigured: false, claudeConfigured: false, environment: "", error: null,
      sshAuth: { mode: "nonInteractive", method: "agent", authenticated: false, passwordStored: false }, reconnectState: "idle" };
    window.__TAURI_INTERNALS__ = { invoke: async command => {
      if (command === "remote_bridge_summary") return structuredClone(window.cachedBridge);
      if (command === "remote_bridge_detect_cc") return { state: "confirmed", localPort: 15721 };
      if (command === "remote_bridge_targets" || command === "remote_bridge_events" || command === "remote_bridge_skills") return [];
      if (command === "remote_bridge_log_status") return { available: false };
      throw Error("Unsupported synthetic IPC: " + command);
    } };
  });
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || "http://localhost:1420"}/?impeccable-review=multi-client`);
  const nav = page.locator(".primary-nav"), local = nav.locator("button").first(), remote = nav.locator("button").last();
  const dot = remote.locator(".remote-nav-status");
  await local.waitFor();
  assert.equal(await page.locator('.update-available-button').count(), 0, 'No arrow before a newer release is known');
  assert.equal(await nav.locator("button").count(), 2);
  assert.equal(await local.getAttribute("aria-current"), "page");
  assert.equal(await dot.getAttribute("data-state"), "idle");
  assert.equal(await dot.evaluate(node => getComputedStyle(node).visibility), "hidden");
  const localViews = page.locator('.local-view-switch button');
  assert.equal(await localViews.first().getAttribute('aria-pressed'), 'true');
  await localViews.last().click();
  await page.evaluate(() => { window.retainedRemotePage = document.querySelector('.remote-bridge-page'); });
  const remoteBox = await remote.boundingBox();
  for (const [status, reconnectState, postConnectStatus, expected] of [
    ["connecting", "idle", "idle", "connecting"], ["connected", "idle", "preparing", "connected"],
    ["connected", "idle", "partial", "connected"], ["connected", "waiting", "ready", "connecting"],
    ["connected", "retrying", "ready", "connecting"], ["connected", "attentionRequired", "ready", "attention"],
    ["stale", "idle", "ready", "attention"], ["unavailable", "idle", "ready", "attention"],
    ["error", "idle", "ready", "attention"], ["disconnected", "idle", "idle", "idle"],
  ]) {
    await page.evaluate(data => Object.assign(window.cachedBridge, data), { status, reconnectState, postConnectStatus });
    await page.waitForFunction(expected => document.querySelector('.remote-nav-status')?.dataset.state === expected, expected);
    assert.equal(await local.getAttribute("aria-current"), "page", "Background transport never changes the workspace");
    assert.deepEqual(await remote.boundingBox(), remoteBox, "Indicator changes do not move navigation");
  }
  await page.locator('.primary-nav-actions button').last().click();
  await page.locator('.header-back-button').click();
  assert.equal(await local.getAttribute("aria-current"), "page");
  assert.equal(await localViews.last().getAttribute('aria-pressed'), 'true', 'Local advanced view survives Settings');
  await remote.click();
  assert.equal(await remote.getAttribute("aria-current"), "page");
  assert.equal(await page.locator('.remote-view-switch').count(), 0, "Disconnected workspace has no usage tabs");
  await page.locator('.primary-nav-actions button').last().click();
  await page.keyboard.press("Escape");
  assert.equal(await remote.getAttribute("aria-current"), "page");
  await local.click();
  await page.locator('.home-page').getByRole('button', { name: '检查应用', exact: true }).click();
  assert.equal(await nav.count(), 0, "Assistant is contextual, not a third primary workspace");
  await page.locator('.header-back-button').click();
  assert.equal(await local.getAttribute("aria-current"), "page");
  await remote.click();
  await page.evaluate(() => Object.assign(window.cachedBridge, { status: "connected", reconnectState: "idle",
    target: { id: "navigation-test", displayName: "Lab", source: "manual", host: "lab.example.test", user: "dev", port: 22, available: true, compatibility: "compatible", canOpenVscode: true, canOpenMobaxterm: false },
    sshAuth: { mode: "nonInteractive", method: "agent", authenticated: true, passwordStored: false } }));
  await page.locator('.remote-view-switch').waitFor();
  assert.equal(await page.locator('.remote-view-switch button').count(), 2);
  await page.locator('.remote-view-switch button').last().click();
  await local.click(); await remote.click();
  assert.equal(await localViews.last().getAttribute('aria-pressed'), 'true', 'Local advanced view survives Assistant and workspace switching');
  assert.equal(await page.locator('.remote-view-switch button').last().getAttribute('aria-pressed'), 'true', "The remote view is not remounted on workspace changes");
  assert.equal(await page.evaluate(() => window.retainedRemotePage === document.querySelector('.remote-bridge-page')), true);
  await page.locator('.primary-nav-actions button').first().click();
  await page.locator('.diagnostic-report-dialog').waitFor({ state: 'visible' });
  await page.keyboard.press('Escape');
  assert.equal(await remote.getAttribute('aria-current'), 'page', "Dialog Escape never navigates the underlying workspace");

  await page.locator('.primary-nav-actions button').last().click();
  await page.locator('.settings-tabs button').last().click();
  await page.locator('.check-update-button').click();
  await page.locator('.update-available-button').waitFor();
  await page.locator('.header-back-button').click();
  const update = page.locator('.update-available-button');
  assert.equal(await update.evaluate(node => node.previousElementSibling?.className), 'wordmark');
  assert.equal(await update.evaluate(node => node.nextElementSibling?.className), 'titlebar-drag-zone');
  await update.click();
  assert.equal(await page.locator('.settings-tabs button').last().getAttribute('aria-current'), 'page', 'Arrow opens About');
  assert.equal(await page.locator('.install-update-button').count(), 0, 'Manual-only update cannot install');
  await page.locator('.header-back-button').click();

  for (const [locale, names] of [["zh-CN", ["本机环境", "远程桥接"]], ["en", ["Local environment", "Remote bridge"]], ["ja", ["ローカル環境", "リモートブリッジ"]], ["ko", ["로컬 환경", "원격 브리지"]]]) {
    await page.locator('.primary-nav-actions button').last().click();
    await page.locator('.settings-tabs button').first().click();
    await page.locator('#language').selectOption(locale);
    await page.locator('.header-back-button').click();
    assert.equal(await local.innerText(), names[0]);
    assert.equal(await remote.innerText(), names[1]);
    assert.ok((await remote.getAttribute('aria-label')).startsWith(names[1] + ' · '));
    for (const [scale, theme] of [[1, 'light'], [1.25, 'dark'], [1.5, 'light']]) {
      // Logical space at 125/150% desktop scaling; real Windows DPI acceptance remains separate.
      await page.setViewportSize({ width: Math.floor(980 / scale), height: Math.floor(800 / scale) });
      await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
      const geometry = await page.locator('.app-header').evaluate(node => [...node.querySelectorAll('button')].map(button => {
        const rect = button.getBoundingClientRect(); return { left: rect.left, right: rect.right, width: rect.width, scroll: button.scrollWidth, client: button.clientWidth };
      }));
      assert.ok(geometry.every(box => box.left >= 0 && box.right <= Math.floor(980 / scale) + 1 && box.width > 0 && box.scroll <= box.client + 1), `${locale}/${scale}: header controls remain visible without clipping`);
      if (process.argv.includes('--screenshots') && scale === 1.5) await page.screenshot({ path: resolve(shots, `${locale}-compact.png`) });
    }
  }
  assert.deepEqual(errors, []);
  console.log('Navigation UI passed: default local, two primary workspaces, background transport states, stable indicator geometry, contextual return, modal Escape, retained remote view and four-language light/dark/scaled logical viewports (synthetic IPC only).');
} finally { await browser.close(); }
