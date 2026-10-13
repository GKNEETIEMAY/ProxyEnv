import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";

const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const browser = await chromium.launch({ headless: true, ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath: process.env.PROXYENV_LAYOUT_BROWSER } : {}) });
const shots = resolve(".impeccable/review/brand");
if (process.argv.includes("--screenshots")) mkdirSync(shots, { recursive: true });
try {
  const page = await browser.newPage({ viewport: { width: 980, height: 800 }, reducedMotion: "reduce" });
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.addInitScript(() => {
    window.isTauri = true;
    window.cachedBridge = { status: "connected", target: { id: "brand-lab", displayName: "Lab", source: "openssh", sourceLabel: "OpenSSH", configPath: "~/.ssh/config", sshAlias: "lab", host: "lab.example.test", user: "dev", port: 22, available: true, compatibility: "compatible", canOpenVscode: true, canOpenMobaxterm: true },
      proxy: { local: { host: "127.0.0.1", port: 7897, protocol: "mixed" }, remotePort: 7897 }, cc: { local: { host: "127.0.0.1", port: 15721, protocol: "http" }, remotePort: 15721 },
      proxyStatus: "connected", ccStatus: "connected", codexConfigured: true, claudeConfigured: true, environment: "", error: null, postConnectStatus: "ready",
      sshAuth: { mode: "nonInteractive", method: "agent", authenticated: true, passwordStored: false }, reconnectState: "idle", toolInstallations: [],
    };
    window.__TAURI_INTERNALS__ = { invoke: async command => {
      if (command === "remote_bridge_summary") return structuredClone(window.cachedBridge);
      if (command === "remote_bridge_detect_cc") return { state: "confirmed", localPort: 15721 };
      if (command === "remote_bridge_targets") return [window.cachedBridge.target];
      if (command === "remote_bridge_events" || command === "remote_bridge_skills") return [];
      if (command === "remote_bridge_log_status") return { available: false };
      throw Error("Unsupported synthetic IPC: " + command);
    } };
  });
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || "http://localhost:1420"}/?impeccable-review=multi-client`);
  const nav = page.locator(".primary-nav");
  await nav.waitFor();
  await page.locator('.remote-nav-status[data-state="connected"]').waitFor();
  const capture = async name => {
    if (process.argv.includes("--screenshots")) await page.screenshot({ path: resolve(shots, `${name}.png`) });
  };
  const checkHeader = async () => {
    assert.equal(await nav.locator("button").count(), 2);
    assert.equal(await page.locator('.wordmark .brand-mark').evaluate(img => img.complete && img.naturalWidth > 0), true);
    assert.equal(await page.locator('.wordmark .brand-mark').evaluate(img => getComputedStyle(img).backgroundColor), "rgba(0, 0, 0, 0)");
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    const boxes = await page.locator('.app-header').evaluate(node => [...node.querySelectorAll('button')].map(button => {
      const r = button.getBoundingClientRect(); return { left: r.left, right: r.right, width: r.width, scroll: button.scrollWidth, client: button.clientWidth };
    }));
    const width = page.viewportSize().width;
    assert.ok(boxes.every(box => box.left >= 0 && box.right <= width + 1 && box.width > 0 && box.scroll <= box.client + 1), `Header controls fit ${width}px`);
  };
  for (const [locale, appName] of [["zh-CN", "境枢 RouteSpan"], ["en", "RouteSpan"], ["ja", "RouteSpan"], ["ko", "RouteSpan"]]) {
    await page.locator('.primary-nav-actions button').last().click();
    await page.locator('.settings-tabs button').first().click();
    await page.locator('#language').selectOption(locale);
    await page.locator('.header-back-button').click();
    assert.equal(await page.locator('.wordmark strong').innerText(), appName);
    assert.equal(await page.title(), appName);
    for (const [width, theme] of [[980, "light"], [980, "dark"], [560, "light"], [560, "dark"]]) {
      await page.setViewportSize({ width, height: 800 });
      await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
      await checkHeader();
      if (locale === "zh-CN" && width === 980) await capture(`brand-local-${theme}`);
      if (width === 560 && theme === "light") await capture(`brand-narrow-${locale}`);
    }
    await page.setViewportSize({ width: 980, height: 800 });
    await page.locator('.primary-nav-actions button').last().click();
    await page.locator('.settings-tabs button').last().click();
    assert.equal(await page.locator('.about-identity h2').innerText(), appName);
    assert.equal(await page.locator('.about-identity .brand-mark').evaluate(img => img.complete && img.naturalWidth > 0), true);
    if (locale === "zh-CN") await capture("brand-about");
    await page.locator('.header-back-button').click();
    await page.locator('.primary-nav-actions button').first().click();
    await page.locator('.diagnostic-report-dialog').waitFor({ state: 'visible' });
    await page.waitForFunction(() => document.querySelector('.report-preview')?.value.startsWith("RouteSpan"));
    assert.ok((await page.locator('.report-preview').inputValue()).startsWith("RouteSpan"));
    await page.keyboard.press("Escape");
  }
  await nav.locator('button').last().click();
  await page.locator('.remote-view-switch').waitFor();
  assert.equal(await page.locator('.remote-view-switch button').count(), 2);
  await capture("brand-remote-connected");
  const box = await nav.locator('button').last().boundingBox();
  await page.evaluate(() => Object.assign(window.cachedBridge, { status: "connecting" }));
  await page.locator('.remote-nav-status[data-state="connecting"]').waitFor();
  assert.deepEqual(await nav.locator('button').last().boundingBox(), box);
  assert.deepEqual(errors, []);
  console.log("Brand UI passed: transparent Header/About mark, four localized names and slogans, local/remote hierarchy, stable status dot, report branding and light/dark header geometry down to 560px (synthetic IPC only).");
} finally { await browser.close(); }
