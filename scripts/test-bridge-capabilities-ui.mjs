import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";

const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const screenshots = process.argv.includes("--screenshots") ? resolve(".debug-tmp/remote-layout") : null;
if (screenshots) mkdirSync(screenshots, { recursive: true });
const browser = await chromium.launch({ headless: true, ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath: process.env.PROXYENV_LAYOUT_BROWSER } : {}) });
try {
  const page = await browser.newPage({ viewport: { width: 880, height: 720 }, reducedMotion: "reduce" });
  await page.addInitScript(() => {
    window.isTauri = true;
    window.capabilityCalls = [];
    window.skillCalls = [];
    window.toolCalls = [];
    window.environmentCalls = 0;
    window.copiedCommands = [];
    window.mobaCalls = 0;
    Object.defineProperty(navigator, 'clipboard', { configurable:true, value:{ writeText:async value => window.copiedCommands.push(value) } });
    // Synthetic IPC: no real SSH, relay, credentials or configuration writes.
    window.bridgeFixture = {
      status: "connected", target: { id: "manual-preview", displayName: "aliyun-dev", source: "manual", sourceLabel: "ProxyEnv", sshAlias: null, host: "8.138.152.49", user: "lxl", port: 22, configPath: "ProxyEnv", available: true, compatibility: "compatible", canOpenVscode: true, canOpenMobaxterm: false },
      proxy: { local: { host: "127.0.0.1", port: 10809, protocol: "mixed" }, remotePort: 23841 },
      cc: { local: { host: "127.0.0.1", port: 15721, protocol: "http" }, remotePort: 31472 },
      proxyStatus: "connected", ccStatus: "connected", activeProxyRevision: 1, environment: "", codexConfigured: true, claudeConfigured: false,
      postConnectStatus: "preparing", codexState: "preparing", claudeState: "pending",
      error: null, sshAuth: { mode: "nonInteractive", method: "identityFile", authenticated: true, passwordStored: false },
    };
    window.originalRoutes = { proxy: structuredClone(window.bridgeFixture.proxy), cc: structuredClone(window.bridgeFixture.cc) };
    window.__TAURI_INTERNALS__ = { invoke: async (command, args) => {
      if (command === "remote_bridge_detect_cc") return { state: "confirmed", localPort: 15721 };
      if (command === "remote_bridge_summary") return structuredClone(window.bridgeFixture);
      if (command === 'remote_bridge_launch_mobaxterm') { window.mobaCalls += 1; return; }
      if (command === 'remote_bridge_session_environment_command') {
        window.environmentCalls += 1;
        return new Promise((resolve, reject) => { window.finishEnvironment = success => success
          ? resolve('source "$HOME/.config/proxyenv/session-preview/env.sh"') : reject('bridgeUnavailable'); });
      }
      if (["remote_bridge_config_preview", "remote_bridge_config_restore_preview"].includes(command)) window.toolCalls.push(command);
      if (["remote_bridge_enable_skill", "remote_bridge_disable_skill"].includes(command)) {
        window.skillCalls.push({ command, args });
        return new Promise((resolve, reject) => { window.finishSkill = (success) => {
          if (!success) { reject("skillConflict"); return; }
          const enabled = command === "remote_bridge_enable_skill";
          resolve({ id: args.id, tool: "codex", name: "release-notes", hash: "review-1", fileCount: 3, totalSize: 4100, state: enabled ? "synced" : "notSynced", enabled });
        }; });
      }
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
  await page.locator('.bridge-launch-bar').waitFor();
  assert.equal(await page.locator('.bridge-launch-bar .bridge-launch-option button').first().isDisabled(), false, 'A supported connected manual target can open VS Code');
  const metadata = await page.locator('.remote-target-metadata').innerText();
  assert.ok(metadata.includes('lxl@8.138.152.49'), 'SSH metadata includes the selected user and host');
  assert.ok(!metadata.includes(':22'), 'Default SSH port is omitted from display labels');
  const toolSwitches = page.locator('.bridge-tool .switch-input');
  await page.waitForFunction(() => [...document.querySelectorAll('.bridge-tool .switch-input')].every(input => input.getAttribute('aria-busy') === 'true'));
  assert.equal(await page.locator('.bridge-tool .check-status[data-state="checking"]').count(), 2, 'Both AI tools show their own checking status');
  for (const [width, height, theme] of [[560,620,'dark'], [880,720,'light']]) {
    await page.setViewportSize({ width, height });
    await page.evaluate(value => document.documentElement.dataset.theme = value, theme);
    await page.locator('.bridge-tool-grid').scrollIntoViewIfNeeded();
    for (const card of await page.locator('.bridge-tool').all()) {
      assert.equal(await card.locator('.check-status').isVisible(), true);
      assert.equal(await card.evaluate(element => element.scrollWidth <= element.clientWidth + 1), true, 'Checking text does not overflow its card');
    }
    if (screenshots) await page.screenshot({ path: resolve(screenshots, `ai-tool-checking-${theme}.png`) });
  }
  for (const index of [0, 1]) {
    assert.equal(await toolSwitches.nth(index).isDisabled(), true, 'Checking blocks enabling and disabling');
    assert.equal(await toolSwitches.nth(index).isChecked(), index === 0, 'Checking preserves the actual configuration state');
    await toolSwitches.nth(index).evaluate(input => input.click());
  }
  await page.locator('.remote-view-switch button').nth(1).click();
  const advancedTools = page.locator('.remote-tool-access-list .remote-tool-access-control .switch-input');
  for (const input of await advancedTools.all()) {
    assert.equal(await input.isDisabled(), true, 'Advanced uses the same checking lock');
    await input.evaluate(input => input.click());
  }
  assert.equal(await page.evaluate(() => window.toolCalls.length), 0, 'Blocked clicks never issue configuration requests');
  await page.locator('.remote-view-switch button').first().click();
  await page.evaluate(() => window.bridgeFixture.codexState = 'ready');
  await page.waitForFunction(() => document.querySelector('.bridge-tool .switch-input').getAttribute('aria-busy') === 'false');
  assert.equal(await toolSwitches.first().isDisabled(), false, 'Completed Codex unlocks independently of Claude and VS Code');
  assert.equal(await toolSwitches.nth(1).isDisabled(), true);
  await page.evaluate(() => { window.bridgeFixture.claudeState = 'warning'; window.bridgeFixture.postConnectStatus = 'partial'; });
  await page.waitForFunction(() => [...document.querySelectorAll('.bridge-tool .switch-input')].every(input => !input.disabled));
  assert.equal(await page.locator('.bridge-tool .check-status[data-state="checking"]').count(), 0, 'Warning exits checking instead of leaving the toggle stuck');
  // Reconnect reuses the same runtime states and locks both tools again.
  await page.evaluate(() => { window.bridgeFixture.codexState = 'preparing'; window.bridgeFixture.claudeState = 'preparing'; });
  await page.waitForFunction(() => [...document.querySelectorAll('.bridge-tool .switch-input')].every(input => input.disabled));
  await page.evaluate(() => { window.bridgeFixture.codexState = 'ready'; window.bridgeFixture.claudeState = 'warning'; window.bridgeFixture.postConnectStatus = 'ready'; });
  await page.waitForFunction(() => [...document.querySelectorAll('.capability-switch')].every(input => !input.disabled));
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
    if (index === 0) assert.equal(await page.locator('.bridge-launch-copy').isDisabled(), !(success ? enabled : before), 'Copy follows General Proxy availability independently of AI routing');
    if (index === 0) assert.equal(await switches.nth(1).isChecked(), callsBefore === 0, "General proxy changes leave AI state untouched");
  }
  assert.equal(await switches.nth(0).isChecked(), true);
  assert.equal(await switches.nth(1).isChecked(), true);
  const copySetup = page.locator('.bridge-launch-copy');
  assert.equal(await copySetup.isVisible(), true, 'Terminal setup copy is directly available in Overview');
  assert.equal(await copySetup.isDisabled(), false);
  const copyBox = await copySetup.boundingBox();
  const launchBox = await page.locator('.bridge-launch-bar').boundingBox();
  assert.ok(Math.abs(copyBox.x + copyBox.width - launchBox.x - launchBox.width) <= 1, 'Copy is aligned to the bottom bar right edge');
  await page.evaluate(() => window.bridgeFixture.target.canOpenMobaxterm = true);
  const moba = page.locator('.bridge-launch-option button').nth(1);
  await page.waitForFunction(() => !document.querySelectorAll('.bridge-launch-option button')[1].disabled);
  await moba.click();
  await page.waitForFunction(() => window.mobaCalls === 1);
  await page.waitForFunction(() => !document.querySelector('.bridge-launch-copy').disabled);
  assert.equal(await page.locator('.remote-view-switch button').first().getAttribute('aria-pressed'), 'true', 'Launching Moba keeps Overview open');
  await page.locator('.bridge-launch-option').nth(1).locator('.help-tooltip').hover();
  await page.getByRole('tooltip').waitFor();
  assert.match(await page.getByRole('tooltip').innerText(), /底部操作栏右侧的复制按钮/);
  await page.mouse.move(0,0);
  await page.getByRole('tooltip').waitFor({state:'hidden'});
  for (const success of [true,false,true]) {
    const callsBefore = await page.evaluate(() => window.environmentCalls);
    const copiesBefore = await page.evaluate(() => window.copiedCommands.length);
    const boxBefore = await copySetup.boundingBox();
    await copySetup.click();
    await page.waitForFunction(count => window.environmentCalls === count + 1, callsBefore);
    assert.equal(await copySetup.isDisabled(), true, 'Pending copy locks duplicate requests');
    await copySetup.evaluate(button => button.click());
    assert.equal(await page.evaluate(() => window.environmentCalls), callsBefore + 1);
    await page.evaluate(result => window.finishEnvironment(result), success);
    await page.waitForFunction(() => !document.querySelector('.bridge-launch-copy').disabled);
    assert.equal(await page.evaluate(() => window.copiedCommands.length), copiesBefore + Number(success));
    assert.equal(await copySetup.evaluate(button => button.classList.contains('is-copied')), success, 'Only completed copies show success');
    assert.deepEqual(await copySetup.boundingBox(), boxBefore, 'Copy feedback does not resize or move the fixed button');
    if (success) assert.equal(await page.evaluate(() => window.copiedCommands.at(-1)), 'source "$HOME/.config/proxyenv/session-preview/env.sh"', 'Copy uses the shared backend command, not displayed exports or AI credentials');
  }
  const manager = page.locator('#bridge-skills-manager');
  const manage = page.locator('.bridge-skills-heading button');
  const skillsHelp = page.locator('.bridge-skills-heading .help-tooltip');
  await skillsHelp.hover();
  await page.getByRole('tooltip').waitFor();
  assert.match(await page.getByRole('tooltip').innerText(), /取消同步仅移除 ProxyEnv 管理的副本/);
  await page.mouse.move(0, 0);
  await page.getByRole('tooltip').waitFor({state:'hidden'});
  assert.equal(await manager.isVisible(), false, 'Skills are collapsed by default');
  assert.equal(await manage.innerText(), '', 'Manage is a compact gear-only button');
  assert.equal(await manage.getAttribute('aria-label'), '管理');
  // Force a scrollbar threshold crossing; expansion must not narrow the cards.
  await page.setViewportSize({width:880,height:720});
  const content = page.locator('.remote-overview-page .remote-fields');
  const thresholdHeight = await content.evaluate(element => 720 + element.scrollHeight - element.clientHeight + 80);
  await page.setViewportSize({width:880,height:thresholdHeight});
  await content.evaluate(element => element.scrollTop = 0);
  const geometry = () => page.evaluate(() => {
    const fields = document.querySelector('.remote-overview-page .remote-fields');
    const selectors = ['.app-header', '.remote-bridge-page', '.remote-workspace-simple', '.remote-workspace-heading', '.remote-selected-target', '.bridge-capabilities', '.bridge-ai-section', '.bridge-launch-bar'];
    return { overflow:fields.scrollHeight > fields.clientHeight,
      boxes:selectors.map(selector => {
        const rect = document.querySelector(selector).getBoundingClientRect();
        return {x:rect.x,width:rect.width};
      }) };
  });
  const collapsedGeometry = await geometry();
  assert.equal(collapsedGeometry.overflow, false, 'Collapsed fixture fits without scrolling');
  assert.equal(await content.evaluate(element => getComputedStyle(element).scrollbarGutter), 'stable');
  assert.equal(await content.evaluate(element => getComputedStyle(element).overflowY), 'scroll', 'The scroll lane is allocated before expansion, including embedded browsers without gutters');
  const transparentScrollbar = 'rgba(0, 0, 0, 0) rgba(0, 0, 0, 0)';
  assert.equal(await content.evaluate(element => getComputedStyle(element).scrollbarColor), transparentScrollbar, 'Collapsed Skills hide the scroll thumb without removing its lane');
  await content.hover();
  assert.equal(await content.evaluate(element => getComputedStyle(element).scrollbarColor), transparentScrollbar, 'Hover does not reveal the scrollbar while collapsed');
  assert.equal(await content.evaluate(element => getComputedStyle(element, '::-webkit-scrollbar-button').display), 'none', 'No top/bottom scrollbar arrow buttons');
  await manage.click();
  await page.waitForFunction(() => document.querySelector('.bridge-skills-manage').getAttribute('aria-expanded') === 'true');
  await page.waitForFunction(value => getComputedStyle(document.querySelector('.remote-overview-page .remote-fields')).scrollbarColor !== value, transparentScrollbar, {timeout:2000});
  assert.notEqual(await content.evaluate(element => getComputedStyle(element).scrollbarColor), transparentScrollbar, 'Expanded Skills retain the subtle draggable scrollbar');
  const expandedGeometry = await geometry();
  assert.equal(expandedGeometry.overflow, true, 'Expanded fixture requires a scrollbar');
  assert.deepEqual(expandedGeometry.boxes, collapsedGeometry.boxes, 'Manage expansion keeps card and footer widths and horizontal positions stable');
  assert.equal(await page.locator('.bridge-skills-list').evaluate(element => getComputedStyle(element).scrollbarGutter), 'stable', 'Filtering cannot shift Skills columns when its scrollbar changes');
  assert.equal(await page.locator('.bridge-skills-list').evaluate(element => getComputedStyle(element).overflowY), 'scroll');
  await manage.click();
  assert.deepEqual((await geometry()).boxes, collapsedGeometry.boxes, 'Collapsing does not shift the page back');
  // Older/embedded scrollbar implementations must not rely on scrollbar-gutter.
  await content.evaluate(element => element.style.scrollbarGutter = 'auto');
  const fallbackCollapsed = await geometry();
  await manage.click();
  assert.deepEqual((await geometry()).boxes, fallbackCollapsed.boxes, 'Without gutter support, expanding still cannot squeeze the whole page');
  await manage.click();
  assert.deepEqual((await geometry()).boxes, fallbackCollapsed.boxes, 'Without gutter support, collapsing stays fixed too');
  await content.evaluate(element => element.style.removeProperty('scrollbar-gutter'));
  await manage.click();
  await page.setViewportSize({width:880,height:720});
  assert.equal(await manage.getAttribute('aria-expanded'), 'true');
  assert.equal(await page.locator('.remote-view-switch button').first().getAttribute('aria-pressed'), 'true', 'Manage never navigates to Advanced');
  assert.deepEqual(await page.locator('.bridge-skills-table thead th').allTextContents(), ['Skills','Codex','Claude Code'], 'Agent names appear once in the table header');
  assert.equal(await manager.getByText('可分别管理每个智能体。', {exact:false}).count(), 0, 'Safety explanation is in help rather than permanent body copy');
  if (screenshots) {
    for (const [width, height, theme] of [[880,720,'light'], [560,620,'dark']]) {
      await page.setViewportSize({ width, height });
      await page.evaluate(value => document.documentElement.dataset.theme = value, theme);
      await manager.scrollIntoViewIfNeeded();
      for (const row of await page.locator('.bridge-skill-row').all()) {
        assert.ok((await row.boundingBox()).height <= 40, 'Ordinary skill rows stay compact at desktop and narrow widths');
      }
      assert.equal(await manager.evaluate(element => element.scrollWidth <= element.clientWidth + 1), true, 'Compact table never overflows the page');
      const copyRect = await page.locator('.bridge-launch-copy').boundingBox();
      assert.ok(copyRect.x + copyRect.width <= width && copyRect.y + copyRect.height <= height, 'Terminal copy stays within the fixed footer at desktop and narrow widths');
      assert.equal(await page.locator('.bridge-launch-bar').evaluate(element => element.scrollWidth <= element.clientWidth + 1), true, 'Launch bar never overflows horizontally');
      await page.screenshot({ path: resolve(screenshots, `skills-table-${theme}.png`) });
    }
    await page.setViewportSize({width:880,height:720});
    await page.evaluate(() => document.documentElement.dataset.theme = 'light');
  }
  const search = page.locator('.bridge-skills-search input');
  await search.fill('release');
  assert.equal(await page.locator('.bridge-skill-row').count(), 1);
  const skillAction = page.locator('.bridge-skill-row .bridge-skill-action').first();
  for (const [enabled, success] of [[false, true], [true, false], [true, true]]) {
    const before = await skillAction.getAttribute('aria-label');
    const checkedBefore = await skillAction.isChecked();
    const callsBefore = await page.evaluate(() => window.skillCalls.length);
    await skillAction.click();
    await page.waitForFunction(count => window.skillCalls.length === count + 1, callsBefore);
    assert.equal(await skillAction.getAttribute('aria-label'), before, 'Skill state waits for the backend');
    assert.equal(await skillAction.isChecked(), checkedBefore, 'Switch does not toggle before confirmation');
    assert.equal(await skillAction.getAttribute('aria-busy'), 'true');
    await skillAction.evaluate(button => button.click());
    assert.equal(await page.evaluate(() => window.skillCalls.length), callsBefore + 1);
    const call = await page.evaluate(() => window.skillCalls.at(-1));
    assert.equal(call.command, enabled ? 'remote_bridge_enable_skill' : 'remote_bridge_disable_skill');
    assert.equal(call.args.id, 'codex|release-notes', 'Skills use the existing agent-specific ID');
    await page.evaluate(result => window.finishSkill(result), success);
    await page.waitForFunction(() => document.querySelector('.bridge-skill-action').getAttribute('aria-busy') === 'false');
    const after = await skillAction.getAttribute('aria-label');
    if (!success) assert.equal(after, before, 'Failure preserves the prior enabled state');
    else assert.notEqual(after, before);
    assert.equal(await skillAction.isChecked(), success ? enabled : checkedBefore);
    assert.equal(await page.locator('.bridge-skill-agent-copy .check-status').nth(1).getAttribute('data-state'), 'healthy', 'Codex changes leave Claude sync untouched');
  }
  await search.fill('rust');
  assert.equal(await page.locator('.bridge-skill-action').nth(1).isDisabled(), true, 'An agent without a local link cannot sync');
  await search.fill('');
  const barTop = await page.locator('.bridge-launch-bar').evaluate(bar => bar.getBoundingClientRect().top);
  await page.locator('.remote-workspace > .remote-fields').evaluate(element => element.scrollTop = element.scrollHeight);
  assert.equal(await page.locator('.bridge-launch-bar').evaluate(bar => bar.getBoundingClientRect().top), barTop, 'Content scroll never moves launch actions');
  for (const image of await page.locator('.bridge-launch-bar img').all()) assert.equal(await image.evaluate(img => img.complete && img.naturalWidth > 0), true, 'Bundled app icon must load');
  await manage.click();
  assert.equal(await manager.isVisible(), false);
  const disconnect = page.locator('.remote-disconnect');
  assert.equal(await disconnect.locator('svg').count(), 1, 'Disconnect has a matching line icon');
  assert.equal(await disconnect.evaluate(element => {
    const probe = document.createElement('span'); probe.style.color = 'var(--danger)'; element.append(probe);
    const matches = getComputedStyle(element).color === getComputedStyle(probe).color;
    probe.remove(); return matches;
  }), true, 'Disconnect uses the danger text token');
  await disconnect.click();
  await page.locator('dialog[open] #remote-confirm-title').waitFor();
  assert.equal(await page.evaluate(() => window.bridgeFixture.status), 'connected', 'Clicking disconnect still requires confirmation');
  await page.locator('dialog[open]').getByRole('button', {name:'取消',exact:true}).click();
  console.log("Capability UI: independent enable/disable, controlled loading, duplicate click prevention and failure rollback passed.");
  console.log("AI tool UI: independent checking indicators, enable/disable locks in Overview/Advanced, warning recovery and reconnect passed.");
  console.log("Skills UI: collapsed summary, inline search, independent sync/unsync, failure handling, persistent launch actions and bundled app icons passed.");
  console.log("Skills scrolling: expand/collapse across scrollbar threshold preserves content and footer geometry.");
  console.log("Terminal copy: fixed right-side button, Moba help, shared command, proxy-only gating, duplicate lock and error handling passed.");
} finally {
  await browser.close();
}
