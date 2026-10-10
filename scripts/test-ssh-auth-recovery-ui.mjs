import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { mkdirSync } from "node:fs";

const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const browser = await chromium.launch({headless:true,...(process.env.PROXYENV_LAYOUT_BROWSER ? {executablePath:process.env.PROXYENV_LAYOUT_BROWSER} : {})});
try {
  const page = await browser.newPage({viewport:{width:880,height:720},reducedMotion:"reduce"});
  page.setDefaultTimeout(15000);
  await page.addInitScript(() => {
    window.isTauri = true;
    window.finishCalls = 0;
    window.networkProbeCalls = 0;
    window.diagnosticCalls = 0;
    window.commandCalls = 0;
    window.copiedCommands = [];
    Object.defineProperty(navigator,'clipboard',{value:{writeText:async value => {window.copiedCommands.push(value);}}});
    window.cachedBridge = JSON.parse(sessionStorage.getItem('cachedBridge') || 'null');
    window.__TAURI_INTERNALS__ = {invoke:async command => {
      const snapshot = {
        sessionId:"synthetic-session",operation:"check",status:"succeeded",
        auth:{mode:"interactive",method:"password",authenticated:true,passwordStored:false},prompt:null,error:null,
        diagnostic:{bytesReceived:100,printableBytes:100,cprRequests:0,promptDetected:true,authMarkerDetected:true,remoteResultDetected:true,outputClosed:true},
      };
      if(command === "ssh_auth_begin" || command === "ssh_auth_state") return snapshot;
      if(command === "ssh_auth_finish") {window.finishCalls++; throw {code:"sshFailed",phase:"sshAuthentication",target:"ssh",retryable:true};}
      if(command === "ssh_auth_cancel") return;
      if(command === "remote_bridge_detect_cc") return {state:"confirmed",localPort:15721};
      if(command === "remote_bridge_summary" && window.cachedBridge) return structuredClone(window.cachedBridge);
      if(command === "remote_bridge_check_network") {window.networkProbeCalls++; return {serverInternet:"reachable"};}
      if(command === "remote_bridge_session_environment_command") {window.commandCalls++; return '. "$HOME/.proxyenv/sessions/test-session/env.sh"';}
      if(command === "remote_bridge_run_diagnostics") {
        window.diagnosticCalls++;
        window.cachedBridge.diagnostics.serverDirect = {state:"passed",checkedAt:5678,errorCode:null,durationMs:10};
        window.cachedBridge.diagnostics.serverInternet = "reachable";
        return {running:false,observations:structuredClone(window.cachedBridge.diagnostics),serverInternet:"reachable"};
      }
      throw new Error("Unsupported synthetic IPC");
    }};
  });
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || "http://localhost:1420"}/?impeccable-review=remote-auth-unavailable`);
  const dialog = page.locator('.remote-auth-dialog');
  await dialog.locator('.primary-action').click();
  await page.waitForFunction(() => window.finishCalls === 1);
  await dialog.getByRole('button',{name:'关闭',exact:true}).waitFor();
  assert.equal(await dialog.locator('.check-status').getAttribute('data-state'),'failed');
  assert.equal(await dialog.getByRole('button',{name:'关闭',exact:true}).isEnabled(),true);
  assert.equal(await dialog.locator('.primary-action').isEnabled(),true,'Post-auth failure permits retry');
  assert.equal(await dialog.locator('.remote-button-spinner').count(),0,'Failed completion stops loading');
  if(process.argv.includes('--screenshots')) {
    const directory=resolve('.impeccable/review/remote-advanced');
    mkdirSync(directory,{recursive:true});
    await page.screenshot({path:resolve(directory,'ssh-post-auth-failure.png')});
  }
  await dialog.getByRole('button',{name:'关闭',exact:true}).click();
  await dialog.waitFor({state:'hidden'});
  assert.equal(await page.locator('.remote-workspace > .remote-fields').isDisabled(),false,'Returning releases the main page');
  assert.equal(await page.locator('.remote-connect-action').isEnabled(),true,'The same target can be retried');
  console.log('SSH authentication recovery: post-auth error, retry, close and unlocked main page passed (synthetic IPC only).');
  await page.evaluate(() => sessionStorage.setItem('cachedBridge',JSON.stringify({
    status:'connected',target:{id:'manual-preview',displayName:'Lab',source:'manual',host:'lab.example.test',user:'dev',port:22,available:true,compatibility:'compatible',canOpenVscode:true},
    proxy:{local:{host:'127.0.0.1',port:7897,protocol:'mixed'},remotePort:7897},cc:null,proxyStatus:'connected',ccStatus:null,activeProxyRevision:1,environment:'',codexConfigured:false,claudeConfigured:false,error:null,
    sshAuth:{mode:'interactive',method:'password',authenticated:true,passwordStored:false},
    sessionEnvironmentState:'ready',sessionEnvironmentCommand:'. "$HOME/.proxyenv/sessions/test-session/env.sh"',
    diagnostics:{serverDirect:{state:'failed',checkedAt:1234,errorCode:'network',durationMs:10},serverInternet:'unreachable',generalProxyEgress:{state:'notTested',checkedAt:null},aiRouteVerification:{state:'notTested',checkedAt:null}},
  })));
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || "http://localhost:1420"}/?impeccable-review=remote-connected`);
  await page.waitForFunction(() => document.querySelector('.remote-server-direct .check-status > span')?.textContent === '不可用',null,{timeout:15000});
  await page.reload();
  await page.waitForFunction(() => document.querySelector('.remote-server-direct .check-status > span')?.textContent === '不可用',null,{timeout:15000});
  assert.equal(await page.evaluate(() => window.networkProbeCalls),0,'Reopening reads the cached result instead of changing it or opening SSH');
  await page.getByRole('button',{name:'重新检测',exact:true}).click();
  await page.waitForFunction(() => document.querySelector('.remote-server-direct .check-status > span')?.textContent === '可用',null,{timeout:15000});
  assert.equal(await page.evaluate(() => window.diagnosticCalls),1,'Overview refresh uses the same parallel diagnostics command as Advanced');
  assert.equal(await page.evaluate(() => window.networkProbeCalls),0,'No separate page-owned server probe');
  await page.getByRole('button',{name:'高级',exact:true}).click();
  await page.getByRole('button',{name:'概览',exact:true}).click();
  assert.equal(await page.locator('.remote-server-direct .check-status > span').textContent(),'可用');
  console.log('Server direct status: shared diagnostic proof survives reopening and tab changes; manual refresh uses unified diagnostics.');
  await page.locator('.bridge-launch-copy').click();
  await page.getByRole('button',{name:'高级',exact:true}).click();
  assert.equal(await page.locator('.advanced-copy').evaluate(button => button.classList.contains('is-copied')),true,'Copy feedback survives tab navigation');
  assert.equal(await page.locator('.advanced-copy').isEnabled(),true,'Environment copy remains available');
  assert.equal(await page.locator('.advanced-command code').textContent(),'. "$HOME/.proxyenv/sessions/test-session/env.sh"');
  await page.locator('.advanced-copy').click();
  const manualCopy=page.locator('.advanced-manual button').first();
  assert.equal(await manualCopy.evaluate(button => button.classList.contains('is-copied')),true,'Manual tools share the same feedback');
  await manualCopy.click();
  await page.getByRole('button',{name:'概览',exact:true}).click();
  assert.equal(await page.locator('.bridge-launch-copy').evaluate(button => button.classList.contains('is-copied')),true);
  assert.equal(await page.evaluate(() => window.commandCalls),0,'Navigation and ready-command copies never reload or prepare remote configuration');
  assert.deepEqual(await page.evaluate(() => window.copiedCommands),Array(3).fill('. "$HOME/.proxyenv/sessions/test-session/env.sh"'));
  await page.waitForFunction(() => !document.querySelector('.bridge-launch-copy')?.classList.contains('is-copied'),null,{timeout:5000});
  console.log('Terminal environment: all three copy entries work locally, retain shared feedback across tabs and expire normally.');
  await page.evaluate(() => {
    const bridge=JSON.parse(sessionStorage.getItem('cachedBridge'));
    delete bridge.sessionEnvironmentCommand;
    sessionStorage.setItem('cachedBridge',JSON.stringify(bridge));
  });
  await page.reload();
  await page.waitForFunction(() => document.querySelector('.remote-target-name strong')?.textContent === 'Lab');
  await page.getByRole('button',{name:'高级',exact:true}).click();
  await page.waitForFunction(() => document.querySelector('.advanced-command code')?.textContent === '. "$HOME/.proxyenv/sessions/test-session/env.sh"');
  const hydratedCalls=await page.evaluate(() => window.commandCalls);
  assert.ok(hydratedCalls>=1,'A ready snapshot without its command hydrates the preview');
  await page.locator('.advanced-copy').click();
  await page.getByRole('button',{name:'概览',exact:true}).click();
  await page.getByRole('button',{name:'高级',exact:true}).click();
  await page.waitForTimeout(2200);
  assert.equal(await page.evaluate(() => window.commandCalls),hydratedCalls,'Copies, navigation and snapshot polling reuse the confirmed command');
  assert.equal(await page.locator('.advanced-command code').textContent(),'. "$HOME/.proxyenv/sessions/test-session/env.sh"');
  console.log('Terminal preview: missing snapshot commands hydrate once and remain visible across copies, navigation and polling.');
  const portRow=page.locator('.advanced-status-row').filter({hasText:'用户代理端口'});
  for (const [state,label,port] of [
    ['matched','7897 · 匹配',7897],['mismatch','10809 · 不匹配',10809],
    ['notSet','未设置',null],['disabled','配置中已关闭',null],
    ['invalidSettings','代理配置格式错误',null],['readFailed','设置文件无法读取',null],
    ['unsupportedProxy','代理地址暂不支持',null],['notCompared','7897 · 未比较网络桥接',7897],
  ]) {
    await page.evaluate(([state,port]) => {window.cachedBridge.runtimeProxyMatch=state;window.cachedBridge.runtimeExpectedProxyPort=port;},[state,port]);
    await page.waitForFunction(label => [...document.querySelectorAll('.advanced-status-row')].some(row=>row.textContent.includes('用户代理端口')&&row.querySelector('.check-status > span')?.textContent===label),label);
    assert.equal(await portRow.locator('.check-status > span').textContent(),label);
  }
  console.log('VS Code user proxy: explicit ports and distinct configuration states render without suggesting extension failure.');
  const diagnostics=page.locator('.advanced-diagnostics');
  assert.deepEqual(await diagnostics.locator('.advanced-group h3').allTextContents(),['诊断工具','日志管理']);
  assert.equal(await diagnostics.locator('.advanced-tool-actions').count(),2);
  for(const width of [880,1440]) {
    await page.setViewportSize({width,height:width===880?720:1000});
    const cards=await page.locator('.bridge-advanced-view > section').evaluateAll(nodes=>nodes.map(card=>({
      id:card.getAttribute('aria-labelledby'),x:card.getBoundingClientRect().x,y:card.getBoundingClientRect().y,
    })));
    assert.equal(cards[3].id,'advanced-events-title');
    assert.equal(cards[4].id,'advanced-manual-title');
    assert.ok(Math.abs(cards[2].y-cards[3].y)<1&&cards[2].x<cards[3].x,'Recent events sits beside security');
    assert.ok(Math.abs(cards[4].y-cards[5].y)<1&&cards[4].x<cards[5].x,'Manual tools sits beside diagnostics');
    const layout=await diagnostics.evaluate(card=>[...card.querySelectorAll('.advanced-tool-actions')].map(group=>({
      columns:getComputedStyle(group).gridTemplateColumns.split(' ').length,
      buttons:[...group.querySelectorAll('button')].map(button=>({width:button.getBoundingClientRect().width,height:button.getBoundingClientRect().height})),
    })));
    assert.ok(layout.every(group=>group.columns===2&&group.buttons.length===2));
    assert.ok(layout.every(group=>Math.abs(group.buttons[0].width-group.buttons[1].width)<1&&group.buttons.every(button=>button.height>=32)));
  }
  assert.equal(await diagnostics.getByRole('button',{name:'打开日志目录',exact:true}).isDisabled(),true,'Unavailable logs remain disabled');
  console.log('Diagnostic card: shared grouped layout has two equal-width actions per row at compact and wide window sizes.');
  if(process.argv.includes('--screenshots')) {
    const directory=resolve('.impeccable/review/remote-advanced');
    mkdirSync(directory,{recursive:true});
    await page.setViewportSize({width:880,height:720});
    await diagnostics.scrollIntoViewIfNeeded();
    await page.screenshot({path:resolve(directory,'diagnostic-card-compact.png')});
    await page.setViewportSize({width:1440,height:1000});
    await page.screenshot({path:resolve(directory,'diagnostic-card-wide.png')});
  }
} finally {
  await browser.close();
}
