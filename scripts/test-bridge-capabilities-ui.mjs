import assert from "node:assert/strict";
import { createRequire } from "node:module";

const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const browser = await chromium.launch({ headless: true, ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath: process.env.PROXYENV_LAYOUT_BROWSER } : {}) });
try {
  const page = await browser.newPage({ viewport: { width: 880, height: 720 }, reducedMotion: "reduce" });
  await page.addInitScript(() => {
    window.isTauri = true;
    window.capabilityCalls = [];
    // Synthetic IPC: no real SSH, relay, credentials or configuration writes.
    window.bridgeFixture = {
      status: "connected", target: { id: "openssh|preview|aliyun-dev", displayName: "aliyun-dev", source: "openssh", sourceLabel: "OpenSSH", sshAlias: "aliyun-dev", host: "8.138.152.49", user: "lxl", port: 22, configPath: "~/.ssh/config", available: true, compatibility: "compatible", canOpenVscode: true, canOpenMobaxterm: false },
      proxy: { local: { host: "127.0.0.1", port: 10809, protocol: "mixed" }, remotePort: 23841 },
      cc: { local: { host: "127.0.0.1", port: 15721, protocol: "http" }, remotePort: 31472 },
      proxyStatus: "connected", ccStatus: "connected", activeProxyRevision: 1, environment: "", codexConfigured: true, claudeConfigured: false,
      error: null, sshAuth: { mode: "nonInteractive", method: "identityFile", authenticated: true, passwordStored: false },
    };
    window.originalRoutes = { proxy: structuredClone(window.bridgeFixture.proxy), cc: structuredClone(window.bridgeFixture.cc) };
    window.__TAURI_INTERNALS__ = { invoke: async (command, args) => {
      if (command === "remote_bridge_detect_cc") return { state: "confirmed", localPort: 15721 };
      if (command === "remote_bridge_summary") return structuredClone(window.bridgeFixture);
      if (command === "remote_bridge_set_capability") {
        window.capabilityCalls.push(args);
        return new Promise((resolve, reject) => { window.finishCapability = (success) => {
          if (!success) { reject("configurationConflict"); return; }
          const { capability, enabled } = args.change;
          window.bridgeFixture[capability] = enabled ? structuredClone(window.originalRoutes[capability]) : null;
          window.bridgeFixture[`${capability}Status`] = enabled ? "connected" : null;
          resolve(structuredClone(window.bridgeFixture));
        }; });
      }
      throw new Error("Unsupported synthetic IPC");
    } };
  });
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || "http://localhost:1420"}/?impeccable-review=remote-connected`);
  const switches = page.locator(".capability-switch");
  await switches.first().waitFor();
  for (const [index, enabled, success] of [[0, false, true], [1, false, true], [0, true, true], [1, true, false], [1, true, true]]) {
    const before = await switches.nth(index).isChecked();
    const callsBefore = await page.evaluate(() => window.capabilityCalls.length);
    await switches.nth(index).click();
    await page.waitForFunction(count => window.capabilityCalls.length === count + 1, callsBefore);
    assert.equal(await switches.nth(index).isChecked(), before, "No optimistic state changes before backend confirmation");
    assert.equal(await switches.nth(index).getAttribute("aria-busy"), "true");
    assert.equal(await switches.nth(0).isDisabled(), true);
    assert.equal(await switches.nth(1).isDisabled(), true);
    // A second native click during pending IPC must not issue another request.
    await switches.nth(index).evaluate(input => input.click());
    assert.equal(await page.evaluate(() => window.capabilityCalls.length), callsBefore + 1);
    const call = await page.evaluate(() => window.capabilityCalls.at(-1));
    assert.equal(call.change.capability, index === 0 ? "proxy" : "cc");
    assert.equal(call.change.enabled, enabled);
    assert.equal(call.confirmed, true);
    await page.evaluate(result => window.finishCapability(result), success);
    await page.waitForFunction(() => [...document.querySelectorAll(".capability-switch")].every(input => input.getAttribute("aria-busy") === "false"));
    assert.equal(await switches.nth(index).isChecked(), success ? enabled : before);
    if (index === 0) assert.equal(await switches.nth(1).isChecked(), callsBefore === 0, "General proxy changes leave AI state untouched");
  }
  assert.equal(await switches.nth(0).isChecked(), true);
  assert.equal(await switches.nth(1).isChecked(), true);
  console.log("Capability UI: independent enable/disable, controlled loading, duplicate click prevention and failure rollback passed.");
} finally {
  await browser.close();
}
