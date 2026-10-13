import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";

const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const screenshots = process.argv.includes("--screenshots") ? resolve(".impeccable/review/remote-advanced") : null;
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
    window.egressCalls = 0;
    window.verifyCalls = [];
    window.eventsCalls = 0;
    window.bridgeEvents = [];
    window.clearCredentialCalls = 0;
    Object.defineProperty(navigator, 'clipboard', { configurable:true, value:{ writeText:async value => window.copiedCommands.push(value) } });
    // Synthetic IPC: no real SSH, relay, credentials or configuration writes.
    window.bridgeFixture = {
      status: "connected", target: { id: "manual-preview", displayName: "aliyun-dev", source: "manual", sourceLabel: "ProxyEnv", sshAlias: null, host: "8.138.152.49", user: "lxl", port: 22, configPath: "ProxyEnv", available: true, compatibility: "compatible", canOpenVscode: true, canOpenMobaxterm: false },
      proxy: { local: { host: "127.0.0.1", port: 10809, protocol: "mixed" }, remotePort: 23841 },
      cc: { local: { host: "127.0.0.1", port: 15721, protocol: "http" }, remotePort: 31472 },
      proxyStatus: "connected", ccStatus: "connected", activeProxyRevision: 1, environment: "", codexConfigured: true, claudeConfigured: false,
      postConnectStatus: "preparing", codexState: "preparing", claudeState: "pending",
      error: null, sshAuth: { mode: "nonInteractive", method: "identityFile", authenticated: true, passwordStored: false },
      diagnostics: {generalProxyEgress:{state:"notTested",checkedAt:null,errorCode:null,durationMs:null}, aiRouteVerification:{state:"notTested",checkedAt:null,errorCode:null,durationMs:null}},
      security: {generalProxyAuth:"none",aiRouteAuth:"session",remoteBindScope:"loopback",shellScope:"sessionOnly"},
    };
    window.originalRoutes = { proxy: structuredClone(window.bridgeFixture.proxy), cc: structuredClone(window.bridgeFixture.cc) };
    window.__TAURI_INTERNALS__ = { invoke: async (command, args) => {
      if (command === "remote_bridge_detect_cc") return { state: "confirmed", localPort: 15721 };
      if (command === "remote_bridge_summary") return structuredClone(window.bridgeFixture);
      if (command === "remote_bridge_events") { window.eventsCalls++; if(window.eventPollDelayMs) await new Promise(resolve=>setTimeout(resolve,window.eventPollDelayMs)); return structuredClone(window.bridgeEvents); }
      if (command === "remote_bridge_log_status") { if(window.logStatusDelayMs) await new Promise(resolve=>setTimeout(resolve,window.logStatusDelayMs)); return {available:window.logsAvailable !== false}; }
      if (command === "remote_bridge_open_log_directory") { window.logsOpened = (window.logsOpened ?? 0) + 1; return; }
      if (command === "remote_bridge_clear_logs") {
        window.clearLogCalls = (window.clearLogCalls ?? 0) + 1;
        return new Promise((resolve,reject) => { window.finishClearLogs = success => success ? resolve() : reject({code:"logUnavailable"}); });
      }
      if (command === "remote_bridge_run_diagnostics") {
        const ipc = window.__TAURI_INTERNALS__.invoke;
        const jobs=[ipc("remote_bridge_test")];
        for (const tool of window.bridgeFixture.tools ?? []) {
          if (tool.configured && tool.verificationSupported) jobs.push(ipc("remote_bridge_tool_verify",{tool:tool.id}));
        }
        await Promise.allSettled(jobs);
        return {running:false,observations:structuredClone(window.bridgeFixture.diagnostics),ssh:"connected",vscode:"warning",serverInternet:"reachable"};
      }
      if (command === "remote_bridge_check_network") return {serverInternet:"reachable"};
      if (command === "remote_bridge_clear_session_credential") { window.clearCredentialCalls++; window.bridgeFixture.sshAuth.passwordStored = false; return; }
      if (command === "remote_bridge_test") {
        window.egressCalls++;
        window.bridgeFixture.diagnostics.generalProxyEgress.state = "testing";
        return new Promise((resolve,reject) => { window.finishEgress = success => {
          window.bridgeFixture.diagnostics.generalProxyEgress = {state:success?"passed":"failed",checkedAt:Date.now(),errorCode:success?null:"network",durationMs:183};
          window.bridgeEvents.unshift({timestamp:Date.now(),level:success?"info":"error",component:"generalProxy",action:"egressTest",outcome:success?"success":"failed",errorCode:success?null:"network",durationMs:183});
          success ? resolve() : reject("networkFailed");
        }; });
      }
      if (command === "remote_bridge_tool_verify") {
        window.verifyCalls.push(args.tool);
        if(window.modelVerifyError) {
          window.bridgeFixture.diagnostics.aiRouteVerification = {state:"failed",checkedAt:Date.now(),errorCode:"network",durationMs:45};
          window.bridgeEvents.unshift({timestamp:Date.now(),level:"error",component:"aiRoute",action:"toolVerify",outcome:"failed",errorCode:"network",durationMs:45});
          throw 'networkFailed';
        }
        window.bridgeFixture.tools.find(tool => tool.id === args.tool).verification = "verified";
        window.bridgeFixture.diagnostics.aiRouteVerification = {state:"passed",checkedAt:Date.now(),errorCode:null,durationMs:45};
        return {tool:args.tool,verification:"verified"};
      }
      if (command === 'remote_bridge_launch_mobaxterm') { window.mobaCalls += 1; return; }
      if (command === 'remote_bridge_session_environment_command') {
        window.environmentCalls += 1;
        return new Promise((resolve, reject) => { window.finishEnvironment = (success,command='source "$HOME/.config/proxyenv/session-preview/env.sh"') => success
          ? resolve(command) : reject('bridgeUnavailable'); });
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
  const advanced = page.locator('.bridge-advanced-view');
  await advanced.waitFor({state:'visible'});
  assert.equal(await advanced.locator('.switch-input').count(),0,'Advanced is diagnostic-only; tool controls remain in Overview');
  assert.doesNotMatch(await advanced.locator('[aria-labelledby="advanced-environment-title"]').innerText(),/AI 工具|Codex|Claude Code|Skills/,'Advanced does not repeat Overview AI tool or Skills detection');
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
    assert.equal(await page.locator('.remote-workspace > .remote-fields').evaluate(element=>element.disabled),false,'Overview copy also leaves the rest of the page enabled');
    assert.equal(await page.locator('.remote-workspace > .remote-fields').evaluate(element=>getComputedStyle(element).opacity),'1');
    await copySetup.evaluate(button => button.click());
    assert.equal(await page.evaluate(() => window.environmentCalls), callsBefore + 1);
    await page.evaluate(result => window.finishEnvironment(result), success);
    await page.waitForFunction(() => !document.querySelector('.bridge-launch-copy').disabled);
    assert.equal(await page.evaluate(() => window.copiedCommands.length), copiesBefore + Number(success));
    assert.equal(await copySetup.evaluate(button => button.classList.contains('is-copied')), success, 'Only completed copies show success');
    assert.deepEqual(await copySetup.boundingBox(), boxBefore, 'Copy feedback does not resize or move the fixed button');
    if (success) assert.equal(await page.evaluate(() => window.copiedCommands.at(-1)), 'source "$HOME/.config/proxyenv/session-preview/env.sh"', 'Copy uses the shared backend command, not displayed exports or AI credentials');
  }
  await page.waitForFunction(()=>!document.querySelector('.bridge-launch-copy').classList.contains('is-copied'),{},{timeout:3000});
  await page.evaluate(() => {
    window.bridgeFixture.sessionEnvironmentState = "ready";
    window.bridgeFixture.vscodeState = "warning";
    window.bridgeFixture.runtimeProxyMatch = "mismatch";
    window.bridgeFixture.sshAuth.passwordStored = true;
    window.bridgeFixture.timings = [{phase:"bridgePrepare",durationMs:84,outcome:"ready"}];
    window.bridgeFixture.claudeConfigured = true;
    window.bridgeFixture.claudeState = "ready";
    window.bridgeFixture.tools = [
      {id:"codex",configured:true,verification:"verifyPending",verificationSupported:false},
      {id:"claude",configured:true,verification:"verifyPending",verificationSupported:true}
    ];
  });
  await page.locator('.remote-view-switch button').nth(1).click();
  const links = page.locator('.advanced-card').filter({has:page.locator('#advanced-links-title')});
  const egressRow = links.locator('.advanced-status-row').filter({hasText:'实际出口'});
  const modelRow = links.locator('.advanced-status-row').filter({hasText:'模型请求'});
  await advanced.waitFor({state:'visible'});
  for(const width of [1280,880,560]) {
    await page.setViewportSize({width,height:720});
    if(width>760) {
      const cards=await advanced.locator(':scope > .advanced-card').all();
      for(let i=0;i<cards.length;i+=2) {
        assert.equal((await cards[i].boundingBox()).height,(await cards[i+1].boundingBox()).height,'Each left/right card pair has equal height');
      }
    }
    for(const pair of await links.locator('.advanced-proof-grid').all()) {
      const [left,right] = await pair.locator('.advanced-status-row').all();
      const a = await left.boundingBox(), b = await right.boundingBox();
      assert.equal(a.y,b.y,`${width}: paired link checks share the same row`);
      assert.ok(b.x>=a.x+a.width,'Link verification is a left/right layout');
      assert.equal(await pair.evaluate(element=>element.scrollWidth<=element.clientWidth+1),true,'Paired verification never overflows');
    }
  }
  await page.setViewportSize({width:880,height:720});
  await page.waitForFunction(() => document.querySelector('.bridge-advanced-view')?.textContent.includes('不匹配'));
  const userPortRow=advanced.locator('.advanced-status-row').filter({hasText:'用户代理端口'});
  assert.equal(await userPortRow.locator('.check-status').getAttribute('data-state'),'warning','A settings mismatch is not proof of extension failure');
  const authRow=links.locator('.advanced-status-row').filter({hasText:'认证'});
  assert.equal(await authRow.locator('.check-status').getAttribute('data-state'),'healthy','Authenticated SSH uses the shared green check');
  await page.evaluate(()=>{window.bridgeFixture.runtimeProxyMatch='unknown';window.bridgeFixture.sshAuth.authenticated=false;});
  await page.waitForFunction(()=>document.querySelector('.bridge-advanced-view')?.textContent.includes('无法确认'));
  assert.equal(await userPortRow.locator('.check-status').getAttribute('data-state'),'idle','Unknown user settings must not be shown as a failed runtime probe');
  assert.equal(await authRow.locator('.check-status').getAttribute('data-state'),'idle','No green authentication check without backend evidence');
  await page.evaluate(()=>{window.bridgeFixture.runtimeProxyMatch='mismatch';window.bridgeFixture.sshAuth.authenticated=true;});
  await page.waitForFunction(() => document.querySelector('.advanced-command')?.textContent.includes('检测中'));
  await page.evaluate(() => window.finishEnvironment(true));
  await page.locator('.advanced-command code').waitFor();
  assert.equal(await page.locator('.advanced-command code').innerText(),'source "$HOME/.config/proxyenv/session-preview/env.sh"','Command preview uses the actual existing API, never a fabricated session path');
  const longCommand = `source "$HOME/.config/proxyenv/${'long-session-'.repeat(100)}/env.sh"`;
  const commandRow = page.locator('.advanced-command');
  assert.equal(await commandRow.locator('button').innerText(),'','Inline copy is an accessible icon-only action');
  assert.equal(await commandRow.locator('button svg').count(),1);
  assert.match(await advanced.innerText(),/需手动执行/);
  for(const width of [1280,880,560]) {
    await page.setViewportSize({width,height:720});
    const before = await commandRow.boundingBox();
    await page.locator('.remote-view-switch button').first().click();
    const callsBefore = await page.evaluate(() => window.environmentCalls);
    await page.locator('.remote-view-switch button').nth(1).click();
    await page.waitForFunction(count => window.environmentCalls > count,callsBefore);
    assert.deepEqual(await commandRow.boundingBox(),before,'Loading does not resize the terminal command row');
    await page.evaluate(command => window.finishEnvironment(true,command),longCommand);
    await page.waitForFunction(command => document.querySelector('.advanced-command code')?.textContent === command,longCommand);
    assert.deepEqual(await commandRow.boundingBox(),before,'Long commands cannot grow the row in either direction');
    assert.equal(await commandRow.locator('code').evaluate(element => {
      const style = getComputedStyle(element);
      return style.whiteSpace === 'nowrap' && style.textOverflow === 'ellipsis' && element.scrollWidth > element.clientWidth;
    }),true,'Long commands stay on one line with ellipsis');
    assert.equal(await commandRow.locator('code').getAttribute('title'),longCommand,'Hover retains the complete command');
    assert.equal(before.height,32,'Terminal command row has a fixed height');
    const [codeBox,buttonBox] = await Promise.all([commandRow.locator('code').boundingBox(),commandRow.locator('button').boundingBox()]);
    assert.equal(codeBox.x,before.x,'Configuration path stays left-aligned');
    assert.ok(Math.abs(buttonBox.x+buttonBox.width-before.x-before.width)<1,'Copy icon stays at the right edge');
    assert.equal(buttonBox.width,32,'Copy icon keeps a fixed width');
  }
  const copiesBeforeLong = await page.evaluate(() => window.copiedCommands.length);
  const feedbackBeforeCopy = await page.locator('.remote-bridge-page .remote-feedback').allTextContents();
  const commandBeforeCopy = await commandRow.boundingBox();
  await commandRow.locator('button').click();
  assert.equal(await page.locator('.remote-workspace > .remote-fields').evaluate(element=>element.disabled),false,'Copy must not disable or dim the entire connected page');
  assert.equal(await page.locator('.remote-workspace > .remote-fields').evaluate(element=>getComputedStyle(element).opacity),'1','Pending copy leaves the page opacity unchanged');
  assert.equal(await page.locator('dialog[open]').count(),0,'Copy does not create a blurred modal backdrop');
  await page.evaluate(command => window.finishEnvironment(true,command),longCommand);
  await page.waitForFunction(count => window.copiedCommands.length===count+1,copiesBeforeLong);
  assert.equal(await page.evaluate(() => window.copiedCommands.at(-1)),longCommand,'Copy retains the full command, not the truncated preview');
  assert.equal(await commandRow.locator('button').getAttribute('aria-label'),'已复制');
  assert.equal(await commandRow.locator('button').evaluate(button=>button.classList.contains('is-copied')),true,'Only successful copy shows the shared green feedback');
  if(screenshots) await page.screenshot({path:resolve(screenshots,'advanced-copy-success.png')});
  assert.deepEqual(await page.locator('.remote-bridge-page .remote-feedback').allTextContents(),feedbackBeforeCopy,'Copy feedback stays on the button; existing unrelated feedback is unchanged');
  assert.deepEqual(await commandRow.boundingBox(),commandBeforeCopy,'Successful copy does not move the command row');
  await page.waitForFunction(()=>!document.querySelector('.advanced-copy').classList.contains('is-copied'),{},{timeout:3000});
  assert.equal(await commandRow.locator('button').getAttribute('aria-label'),'复制终端配置命令','Copy resets without requiring another action');
  await page.setViewportSize({width:880,height:720});
  await page.locator('.remote-view-switch button').first().click();
  await page.locator('.remote-view-switch button').nth(1).click();
  await page.waitForFunction(() => document.querySelector('.advanced-command')?.textContent.includes('检测中'));
  await page.evaluate(() => window.finishEnvironment(true));
  await page.locator('.advanced-command code').waitFor();
  assert.equal(await advanced.locator('.capability-switch,.switch-input').count(),0);
  assert.equal(await egressRow.locator('.check-status').getAttribute('data-state'),'idle','A connected tunnel is not verified egress');
  assert.equal(await modelRow.locator('.check-status').getAttribute('data-state'),'idle','A configured tool is not a verified model request');
  assert.equal(await page.evaluate(() => window.egressCalls + window.verifyCalls.length),0,'Viewing Advanced sends no test requests');
  const run = links.locator('.advanced-card-actions button');
  for (const success of [true,false]) {
    const before = await page.evaluate(() => window.egressCalls);
    await run.click();
    await page.waitForFunction(count => window.egressCalls === count + 1,before);
    assert.equal(await page.evaluate(()=>window.verifyCalls.length),before+1,'Model checks start without waiting for proxy egress');
    await page.waitForFunction(() => [...document.querySelectorAll('#advanced-links-title ~ .advanced-group .advanced-status-row')].find(row=>row.textContent.includes('实际出口'))?.querySelector('.check-status')?.dataset.state === 'checking');
    assert.equal(await run.isDisabled(),true);
    await run.evaluate(button => button.click());
    assert.equal(await page.evaluate(() => window.egressCalls),before+1,'Diagnostics block duplicate runs');
    await page.evaluate(result => window.finishEgress(result),success);
    await run.waitFor({state:'visible'});
    await page.waitForFunction(() => ![...document.querySelectorAll('.advanced-card-actions button')][0].disabled);
    await page.waitForFunction(expected => [...document.querySelectorAll('#advanced-links-title ~ .advanced-group .advanced-status-row')].find(row=>row.textContent.includes('实际出口'))?.querySelector('.check-status')?.dataset.state === expected,success?'healthy':'failed');
    assert.equal(await egressRow.locator('.check-status').getAttribute('data-state'),success?'healthy':'failed');
  }
  assert.deepEqual(await page.evaluate(() => window.verifyCalls),['claude','claude'],'Only configured tools with verification support are tested, even if egress fails');
  await page.evaluate(()=>{window.modelVerifyError=true;});
  await run.click();
  await page.waitForFunction(()=>window.egressCalls===3);
  await page.waitForFunction(()=>[...document.querySelectorAll('#advanced-links-title ~ .advanced-group .advanced-status-row')].find(row=>row.textContent.includes('模型请求'))?.querySelector('.check-status')?.dataset.state==='failed');
  assert.doesNotMatch(await modelRow.innerText(),/网络错误/,'Failure reasons do not appear beneath Model request');
  await page.evaluate(()=>window.finishEgress(false));
  await page.waitForFunction(()=>![...document.querySelectorAll('.advanced-card-actions button')][0].disabled);
  await page.waitForFunction(()=>[...document.querySelectorAll('.advanced-event-list li')].some(row=>row.textContent.includes('验证失败 · 网络错误')));
  const failedModelEvent=advanced.locator('.advanced-event-list li').filter({hasText:'模型请求验证'}).first();
  assert.equal(await failedModelEvent.locator('.check-status').getAttribute('data-state'),'failed','Recent events carry the red failure icon and reason');
  await page.evaluate(()=>{window.modelVerifyError=false;window.bridgeFixture.tools.find(tool=>tool.id==='claude').configured=false;window.bridgeFixture.claudeConfigured=false;});
  await run.click();
  await page.waitForFunction(()=>window.egressCalls===4);
  assert.equal(await page.evaluate(()=>window.verifyCalls.length),3,'An off model switch skips verification');
  await page.evaluate(()=>window.finishEgress(false));
  await page.waitForFunction(()=>![...document.querySelectorAll('.advanced-card-actions button')][0].disabled);
  await page.evaluate(()=>{window.bridgeFixture.tools.find(tool=>tool.id==='claude').configured=true;window.bridgeFixture.claudeConfigured=true;});
  const openLogs = advanced.getByRole('button',{name:'打开日志目录',exact:true});
  await openLogs.click();
  await page.waitForFunction(() => window.logsOpened === 1);
  const clearButton=advanced.locator('.advanced-clear-logs');
  const clearDialog=page.locator('.advanced-clear-logs-dialog');
  assert.equal(await openLogs.evaluate(button=>button.closest('.advanced-log-action').nextElementSibling.classList.contains('advanced-clear-logs')),true,'Clear logs follows Open log folder');
  assert.equal(await page.locator('.advanced-log-action .help-tooltip').count(),0,'Log help is consolidated in the Diagnostics title');
  assert.equal(await advanced.locator('.advanced-diagnostics > .advanced-note').count(),0,'Report explanation is not duplicated in the body');
  await clearButton.click();
  if(screenshots) await page.screenshot({path:resolve(screenshots,'clear-logs-confirm-light.png')});
  await clearDialog.getByRole('button',{name:'取消',exact:true}).click();
  assert.equal(await page.evaluate(()=>window.clearLogCalls??0),0,'Cancel does not touch logs');
  await clearButton.click();
  const confirmClear=clearDialog.getByRole('button',{name:'清除日志',exact:true});
  await confirmClear.click();
  await page.waitForFunction(()=>window.clearLogCalls===1);
  assert.equal(await clearButton.isDisabled(),true);
  assert.equal(await clearDialog.getByRole('button',{name:'取消',exact:true}).isDisabled(),true);
  await clearDialog.getByRole('button',{name:'正在清除…',exact:true}).evaluate(button=>button.click());
  assert.equal(await page.evaluate(()=>window.clearLogCalls),1,'Pending clears prevent duplicate operations');
  await page.evaluate(()=>window.finishClearLogs(false));
  await clearDialog.getByRole('alert').waitFor();
  assert.match(await clearDialog.getByRole('alert').innerText(),/未能清除日志/);
  await confirmClear.click();
  await page.waitForFunction(()=>window.clearLogCalls===2);
  await page.evaluate(()=>window.finishClearLogs(true));
  await clearDialog.waitFor({state:'hidden'});
  assert.equal(await clearButton.innerText(),'日志已清除');
  assert.equal(await clearButton.evaluate(button=>button.classList.contains('is-cleared')),true);
  await page.evaluate(() => window.logsAvailable = false);
  await page.waitForFunction(() => [...document.querySelectorAll('.advanced-diagnostics button')].find(button=>button.textContent==='打开日志目录')?.disabled);
  assert.equal(await openLogs.isDisabled(),true,'Unavailable log storage cannot expose a dead action');
  assert.equal(await clearButton.isDisabled(),true,'Unavailable log storage also disables clearing');
  await page.locator('#advanced-diagnostics-title .help-tooltip').hover();
  await page.getByRole('tooltip').waitFor();
  assert.match(await page.getByRole('tooltip').innerText(),/本地日志暂不可用/);
  assert.match(await page.getByRole('tooltip').innerText(),/生成报告不会发起新的网络请求/);
  await page.mouse.move(0,0);
  await page.getByRole('tooltip').waitFor({state:'hidden'});
  await page.evaluate(() => window.logsAvailable = true);
  await page.locator('.remote-view-switch button').first().click();
  await page.locator('.remote-view-switch button').nth(1).click();
  assert.equal(await egressRow.locator('.check-status').getAttribute('data-state'),'failed','Cached proof survives tab navigation');
  await page.locator('.advanced-event-list li').first().waitFor();
  assert.match(await page.locator('.advanced-event-list').innerText(),/实际出口测试/,'Recent events come from the backend snapshot');
  await page.evaluate(() => {
    window.eventPollDelayMs = 120;
    window.logStatusDelayMs = 80;
    window.stableEventList = document.querySelector('.advanced-event-list');
    window.eventListRemovals = 0;
    window.logDisableFlashes = 0;
    window.advancedObserver = new MutationObserver(records => {
      for(const record of records) {
        if(record.type==='childList') for(const node of record.removedNodes) {
          if(node===window.stableEventList || node.contains?.(window.stableEventList)) window.eventListRemovals++;
        }
        if(record.type==='attributes' && record.target.matches('.advanced-log-action button') && record.target.disabled) window.logDisableFlashes++;
      }
    });
    window.advancedObserver.observe(document.querySelector('.bridge-advanced-view'),{subtree:true,childList:true,attributes:true,attributeFilter:['disabled']});
  });
  const eventCallsStable = await page.evaluate(()=>window.eventsCalls);
  await page.waitForTimeout(4600); // Multiple fresh, but logically identical, summary snapshots.
  assert.equal(await page.evaluate(()=>window.eventListRemovals),0,'Unchanged summary polling never removes the event list');
  assert.equal(await page.evaluate(()=>window.logDisableFlashes),0,'Unchanged summary polling never flashes the log action disabled');
  assert.equal(await page.evaluate(()=>window.stableEventList===document.querySelector('.advanced-event-list')),true,'The same event-list DOM survives polling');
  assert.ok(await page.evaluate(()=>window.eventsCalls)>=eventCallsStable+2,'Background event refresh still runs');
  await page.evaluate(()=>{window.advancedObserver.disconnect();window.eventPollDelayMs=0;window.logStatusDelayMs=0;});
  await page.evaluate(() => { window.bridgeFixture.activeProxyRevision++; window.bridgeFixture.diagnostics.generalProxyEgress = {state:'notTested',checkedAt:null,errorCode:null,durationMs:null}; });
  await page.waitForFunction(() => [...document.querySelectorAll('#advanced-links-title ~ .advanced-group .advanced-status-row')].find(row=>row.textContent.includes('实际出口'))?.querySelector('.check-status')?.dataset.state === 'idle');
  assert.equal(await advanced.getByRole('button',{name:'打开 SSH 配置',exact:true}).count(),0,'Manual targets expose no source configuration file');
  const cacheClearsBefore = await page.evaluate(() => window.clearCredentialCalls);
  await advanced.getByRole('button',{name:'清除本次 SSH 凭据缓存',exact:true}).click();
  await page.waitForFunction(count => window.clearCredentialCalls === count + 1,cacheClearsBefore);
  await page.waitForFunction(() => document.querySelector('.bridge-advanced-view')?.textContent.includes('未缓存'));
  assert.equal(await advanced.getByRole('button',{name:'清除本次 SSH 凭据缓存',exact:true}).count(),0,'No session-security action when no credential is cached');
  for(const label of ['Shell 修改','SSH 凭据缓存']) {
    const row=advanced.locator('.advanced-status-row').filter({hasText:label});
    assert.equal(await row.locator('.check-status').getAttribute('data-state'),'healthy',`${label} confirms a safe policy with the shared green check`);
  }
  const securityCard=advanced.locator('.advanced-card').filter({has:page.locator('#advanced-security-title')});
  const manualCard=advanced.locator('.advanced-card').filter({has:page.locator('#advanced-manual-title')});
  assert.equal((await securityCard.boundingBox()).height,(await manualCard.boundingBox()).height,'Second-row cards stay equal height without a cache action');
  await advanced.getByRole('button',{name:'查看运行耗时',exact:true}).click();
  await page.locator('.advanced-timing-dialog[open]').waitFor();
  assert.match(await page.locator('.advanced-timing-dialog').innerText(),/84 ms/);
  assert.equal(await page.locator('.advanced-timing-dialog').evaluate(dialog=>parseFloat(getComputedStyle(dialog).padding)),20);
  assert.equal(await page.locator('.advanced-timing-dialog form').evaluate(form=>parseFloat(getComputedStyle(form).padding)),0);
  if(screenshots) await page.screenshot({path:resolve(screenshots,'advanced-timing-light.png')});
  await page.keyboard.press('Escape');
  await advanced.getByRole('button',{name:'诊断报告',exact:true}).click();
  await page.locator('.diagnostic-report-dialog[open]').waitFor();
  assert.equal(await page.locator('.diagnostic-report-dialog').count(),1,'Advanced reuses the existing report dialog');
  await page.keyboard.press('Escape');
  // Both tabs reuse one fixed shell and one header, including when content was scrolled.
  for (const [width,height,theme] of [[1280,800,'light'],[880,720,'light'],[560,620,'dark']]) {
    await page.setViewportSize({width,height});
    await page.evaluate(value => document.documentElement.dataset.theme=value,theme);
    await page.locator('.remote-view-switch button').first().click();
    const shellGeometry = () => page.evaluate(() => ['.remote-workspace-connected','.remote-workspace-heading','.remote-status-toolbar','.remote-selected-target','.remote-server-direct'].map(selector => {
      const {x,y,width,height} = document.querySelector(selector).getBoundingClientRect();
      return {x,y,width,height};
    }));
    const before = await shellGeometry();
    await page.evaluate(() => window.sharedBridgeHeader = document.querySelector('.remote-selected-target'));
    await page.locator('.remote-workspace-connected > .remote-fields').evaluate(element => element.scrollTop=element.scrollHeight);
    assert.deepEqual(await shellGeometry(),before,'Overview scroll leaves the shared header fixed');
    await page.locator('.remote-view-switch button').nth(1).click();
    assert.deepEqual(await shellGeometry(),before,`${width}/${theme}: Advanced keeps the same shell and header geometry`);
    assert.equal(await page.locator('.remote-workspace-connected > .remote-fields').evaluate(element => element.scrollTop),0,'New tab starts at its content top');
    assert.equal(await page.evaluate(() => window.sharedBridgeHeader === document.querySelector('.remote-selected-target')),true,'Both views retain the same header instance');
    await page.locator('.remote-workspace-connected > .remote-fields').evaluate(element => element.scrollTop=element.scrollHeight);
    assert.deepEqual(await shellGeometry(),before,'Advanced scroll leaves the shared header fixed');
    await page.locator('.remote-view-switch button').first().click();
    assert.deepEqual(await shellGeometry(),before,'Returning to Overview cannot shift the shell or toolbar');
  }
  await page.locator('.remote-view-switch button').nth(1).click();
  if (screenshots) for (const [width,height,theme] of [[1280,1080,'light'],[880,720,'light'],[560,620,'dark']]) {
    await page.setViewportSize({width,height});
    await page.evaluate(value => document.documentElement.dataset.theme=value,theme);
    await page.locator('.remote-workspace-connected > .remote-fields').evaluate(element => element.scrollTop=0);
    assert.equal(await advanced.evaluate(element => element.scrollWidth<=element.clientWidth+1),true);
    await page.screenshot({path:resolve(screenshots,`advanced-${theme}-${width}.png`)});
    if(width===560){
      await page.locator('.remote-workspace-connected > .remote-fields').evaluate(element=>element.scrollTop=element.scrollHeight);
      await page.screenshot({path:resolve(screenshots,'advanced-dark-560-lower.png')});
    }
  }
  await page.setViewportSize({width:880,height:720});
  await page.evaluate(() => document.documentElement.dataset.theme='light');
  await page.locator('.remote-view-switch button').first().click();
  assert.equal(await advanced.isVisible(),false,'Returning to Overview hides the whole Advanced view including its native dialog');
  const eventCallsBefore = await page.evaluate(() => window.eventsCalls);
  await page.waitForTimeout(2200);
  assert.equal(await page.evaluate(() => window.eventsCalls),eventCallsBefore,'Hidden Advanced does not keep polling events');
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
  console.log("AI tool UI: independent checking indicators and enable/disable locks in Overview only, warning recovery and reconnect passed.");
  console.log("Advanced UI: independent egress/request proof, explicit diagnostic runs, capability/support gates, failure handling, stale-proof reset, real credential action, existing report dialog and timings passed.");
  console.log("Advanced polling: delayed identical snapshots preserve the event-list DOM and never flash log controls disabled.");
  console.log("Skills UI: collapsed summary, inline search, independent sync/unsync, failure handling, persistent launch actions and bundled app icons passed.");
  console.log("Skills scrolling: expand/collapse across scrollbar threshold preserves content and footer geometry.");
  console.log("Terminal copy: fixed right-side button, Moba help, shared command, proxy-only gating, duplicate lock and error handling passed.");
} finally {
  await browser.close();
}
