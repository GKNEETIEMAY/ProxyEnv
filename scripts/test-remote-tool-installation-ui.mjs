import assert from "node:assert/strict";
import {createRequire} from "node:module";
import {mkdirSync} from "node:fs";
import {resolve} from "node:path";
const require=createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const {chromium}=require("playwright");
const browser=await chromium.launch({headless:true,...(process.env.PROXYENV_LAYOUT_BROWSER ? {executablePath:process.env.PROXYENV_LAYOUT_BROWSER}:{})});
const directory=resolve(".impeccable/review/remote-installation");
mkdirSync(directory,{recursive:true});
try {
  const page=await browser.newPage({viewport:{width:980,height:800},reducedMotion:"reduce"});
  const errors=[];page.on("pageerror",error=>errors.push(error.message));
  await page.addInitScript(() => {
    window.isTauri=true; window.discoverCalls=0; window.routeWrites=0;window.terminalCalls=[];window.copies=[];window.ipcCalls=[];
    window.cachedBridge={status:"connected",target:{id:"install-test",displayName:"Lab",source:"manual",host:"lab.example.test",user:"dev",port:22,available:true,compatibility:"compatible",canOpenVscode:true},
      proxy:{local:{host:"127.0.0.1",port:7897,protocol:"mixed"},remotePort:7897},cc:{local:{host:"127.0.0.1",port:15721,protocol:"http"},remotePort:15721},proxyStatus:"connected",ccStatus:"connected",activeProxyRevision:1,environment:"",codexConfigured:false,claudeConfigured:true,error:null,
      sshAuth:{mode:"nonInteractive",method:"agent",authenticated:true,passwordStored:false},codexState:"ready",claudeState:"ready",postConnectStatus:"ready",sessionEnvironmentState:"ready",sessionEnvironmentCommand:'. "$HOME/.proxyenv/sessions/test/env.sh"',
      toolInstallations:[{tool:"codex",state:"missing",version:null,checkedAt:1,errorCode:"cliMissing"},{tool:"claude",state:"installed",version:"2.1.227",checkedAt:1,errorCode:null}],installCapabilities:{curl:true,npm:false,brew:false,nodeVersion:null}};
    Object.defineProperty(navigator,"clipboard",{value:{writeText:async text => window.copies.push(text)}});
    window.__TAURI_INTERNALS__={invoke:async (command,args)=>{
      window.ipcCalls.push(command);
      if(command==="remote_bridge_summary")return structuredClone(window.cachedBridge);
      if(command==="remote_bridge_detect_cc")return {state:"confirmed",localPort:15721};
      if(command==="remote_bridge_targets")return [window.cachedBridge.target];
      if(command==="remote_bridge_events")return [];
      if(command==="remote_bridge_log_status")return {available:true};
      if(command==="remote_bridge_discover_tools") {
        window.discoverCalls++;window.cachedBridge.toolInstallations[0].state="checking";
        await new Promise(resolve=>setTimeout(resolve,600));
        window.cachedBridge.toolInstallations[0]={tool:"codex",state:"installed",version:"0.134.0",checkedAt:2,errorCode:null};
        return {tools:structuredClone(window.cachedBridge.toolInstallations),capabilities:window.cachedBridge.installCapabilities};
      }
      if(command==="remote_bridge_launch_proxy_terminal" || command==="remote_bridge_launch_manual_terminal") {window.terminalCalls.push(command);return;}
      if(command==="remote_bridge_config_restore_preview")return {id:"restore",restore:true};
      if(command==="remote_bridge_config_restore") {await new Promise(resolve=>setTimeout(resolve,20));window.cachedBridge.codexConfigured=false;return;}
      if(command==="remote_bridge_config_apply" || command==="remote_bridge_config_preview") {
        window.routeWrites++;
        if (!window.allowPluginWrites) throw Error("Unexpected route write");
        if(command==="remote_bridge_config_preview") return {id:args.tool,version:"0.151.0"};
        await new Promise(resolve=>setTimeout(resolve,20));
        window.cachedBridge[args.id==="codex" ? "codexConfigured" : "claudeConfigured"]=true;
        return;
      }
      throw Error("Unsupported synthetic IPC");
    }};
  });
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || "http://localhost:1420"}/?impeccable-review=remote-connected`);
  const card=page.locator('.bridge-tool').first(), dialog=page.locator('.remote-install-dialog');
  await card.getByText("CLI 未安装",{exact:true}).waitFor();
  assert.equal(await card.locator('.bridge-tool-status').getAttribute('data-state'),'warning');
  const heading=page.locator('.remote-workspace-heading h1');
  await heading.focus();
  assert.equal(await heading.evaluate(node=>getComputedStyle(node).outlineStyle),'none',"Programmatic heading focus must not draw a black frame");
  assert.equal(await card.getByRole("switch").isEnabled(),false);
  assert.equal(await page.locator('.bridge-tool').nth(1).getByRole("switch").isEnabled(),true,"Other tool remains independent");
  await card.getByRole("button",{name:"查看安装方式",exact:true}).click();
  assert.equal(await dialog.isVisible(),true);
  assert.equal(await dialog.getByRole("button",{name:"复制命令 · npm",exact:true}).isEnabled(),true,"Missing prerequisite does not hide the method");
  assert.ok(await dialog.getByText("远端未检测到：npm",{exact:true}).isVisible());
  const button=dialog.getByRole("button",{name:"复制命令 · Native Installer",exact:true});
  const before=await button.boundingBox();await button.click();
  assert.deepEqual(await dialog.locator('.install-copy').first().boundingBox(),before,"Copy feedback does not shift layout");
  assert.ok(await dialog.locator('.install-copy').first().evaluate(node=>node.classList.contains('is-copied')));
  assert.equal(await page.evaluate(()=>window.copies[0]),"curl -fsSL https://chatgpt.com/codex/install.sh | sh");
  await dialog.getByRole("button",{name:"打开终端",exact:true}).click();
  assert.deepEqual(await page.evaluate(()=>window.terminalCalls),["remote_bridge_launch_proxy_terminal"]);
  const pathHelp=dialog.locator('.install-path-help');
  assert.equal(await pathHelp.getAttribute("open"),null,"PATH troubleshooting starts collapsed");
  await pathHelp.locator('summary').click();
  assert.equal(await pathHelp.locator('.install-path-recipe').count(),1,"Codex does not assume a native ~/.local/bin install");
  await pathHelp.getByRole("button",{name:"复制命令 · npm 全局安装 · 当前终端生效",exact:true}).click();
  assert.match(await page.evaluate(()=>window.copies.at(-1)),/npm prefix -g/);
  assert.equal(await page.evaluate(()=>window.routeWrites),0,"PATH copy does not execute or configure the remote host");
  for(const [name,width,height,theme] of [["dialog-light",980,800,"light"],["dialog-dark",980,800,"dark"],["dialog-narrow",620,620,"light"]]) {
    await page.setViewportSize({width,height});
    await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
    const box=await dialog.boundingBox();assert.ok(box.x>=0 && box.y>=0 && box.x+box.width<=width+1 && box.y+box.height<=height+1);
    assert.ok(await dialog.evaluate(node=>node.scrollWidth<=node.clientWidth+1),"No horizontal dialog clipping");
    if(process.env.PROXYENV_LAYOUT_CAPTURE!=="0") await page.screenshot({path:resolve(directory,`${name}.png`)});
  }
  await page.keyboard.press("Escape");assert.equal(await dialog.isVisible(),false);
  assert.equal(await card.getByRole("button",{name:"查看安装方式",exact:true}).evaluate(node=>node===document.activeElement),true,"Esc returns focus to opener");
  await page.setViewportSize({width:980,height:800});
  await card.getByRole("button",{name:"查看安装方式",exact:true}).click();
  await dialog.getByRole("button",{name:"重新检测",exact:true}).click();
  assert.equal(await dialog.getByRole("button",{name:"正在检测安装状态",exact:true}).isEnabled(),false);
  await card.getByText("CLI 已安装 · 0.134.0",{exact:true}).waitFor();
  assert.equal(await card.getByRole("switch").isEnabled(),true);
  assert.equal(await card.getByRole("switch").isChecked(),false,"Discovery never auto-enables a route");
  assert.equal(await page.evaluate(()=>window.routeWrites),0);
  await page.keyboard.press("Escape");
  for(const [index,state] of [[0,'warning'],[1,'healthy']]) {
    const entry=page.locator('.bridge-tool').nth(index);
    const badge=entry.locator('.bridge-tool-status');
    assert.equal(await badge.getAttribute('data-state'),state);
    const badgeBox=await badge.boundingBox(), versionBox=await entry.locator('.bridge-tool-installation').first().boundingBox();
    assert.ok(versionBox.y >= badgeBox.y+badgeBox.height,"Configuration badge is above installation metadata");
    assert.equal(await badge.evaluate(node=>getComputedStyle(node).backgroundColor),'rgba(0, 0, 0, 0)',"Configuration status has no tinted container");
  }
  if(process.env.PROXYENV_LAYOUT_CAPTURE!=="0") {
    for(const [name,width,height,theme] of [["overview-light",980,800,"light"],["overview-dark-narrow",620,620,"dark"]]) {
      await page.setViewportSize({width,height});
      await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
      await page.screenshot({path:resolve(directory,`${name}.png`)});
    }
    await page.setViewportSize({width:980,height:800});
    await page.evaluate(()=>document.documentElement.dataset.theme="light");
  }
  await page.evaluate(()=>{window.cachedBridge.codexConfigured=true;window.cachedBridge.toolInstallations[0].state="missing";window.cachedBridge.toolInstallations[0].version=null;});
  await card.getByText("CLI 未安装",{exact:true}).waitFor();
  await card.getByText("已有路由配置",{exact:true}).waitFor();
  assert.equal(await card.getByRole("switch").isChecked(),true);assert.equal(await card.getByRole("switch").isEnabled(),true);
  await card.getByRole("switch").click();
  try { await page.waitForFunction(()=>document.querySelector('.bridge-tool .switch-input')?.checked===false,null,{timeout:8000}); }
  catch(cause) { console.log(await page.evaluate(()=>({configured:window.cachedBridge.codexConfigured,commands:window.ipcCalls.slice(-8),error:document.querySelector('.remote-error')?.textContent,card:document.querySelector('.bridge-tool')?.textContent})));throw cause; }
  assert.equal(await card.getByRole("switch").isEnabled(),false,"Missing CLI cannot be enabled after restoration");
  await page.evaluate(()=>{window.cachedBridge.proxy=null;window.cachedBridge.proxyStatus=null;});
  await page.waitForFunction(()=>document.querySelector('.bridge-capability')?.classList.contains('is-off'));
  await card.getByRole("button",{name:"查看安装方式",exact:true}).click();
  await dialog.getByText("网络桥接未开启，终端将使用服务器自身网络。",{exact:false}).waitFor();
  await dialog.getByRole("button",{name:"打开终端",exact:true}).click();
  assert.deepEqual(await page.evaluate(()=>window.terminalCalls),["remote_bridge_launch_proxy_terminal","remote_bridge_launch_manual_terminal"]);
  await page.keyboard.press("Escape");
  // Both dialogs consume the same capability snapshot; Node is a separate Claude prerequisite.
  for(const [npm,nodeVersion] of [[false,null],[true,"17.9.0"],[true,"18.0.0"],[false,"22.0.0"]]) {
    await page.evaluate(({npm,nodeVersion})=>{window.cachedBridge.installCapabilities={curl:true,npm,brew:false,nodeVersion};},{npm,nodeVersion});
    for(const index of [0,1]) {
      await page.locator('.bridge-tool').nth(index).getByRole("button",{name:"查看安装方式",exact:true}).click();
      assert.equal(await dialog.locator('.install-path-help').getAttribute("open"),null,"Each opening resets disclosure");
      if(index===1 && npm && nodeVersion==='18.0.0') {
        await dialog.getByText(/已检测到 CLI 安装成功/).waitFor();
        await dialog.locator('.install-path-help summary').click();
        assert.equal(await dialog.locator('.install-path-recipe').count(),3);
        await dialog.getByRole("button",{name:"复制命令 · Claude 原生安装 · 保存到 Bash",exact:true}).click();
        assert.match(await page.evaluate(()=>window.copies.at(-1)),/grep -qxF/);
        for(const [width,height,theme] of [[980,800,"light"],[620,620,"dark"]]) {
          await page.setViewportSize({width,height});
          await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
          assert.ok(await dialog.evaluate(node=>node.scrollWidth<=node.clientWidth+1));
          if(process.env.PROXYENV_LAYOUT_CAPTURE!=="0") await page.screenshot({path:resolve(directory,`path-claude-${theme}.png`)});
        }
        await page.setViewportSize({width:980,height:800});
        await page.evaluate(()=>document.documentElement.dataset.theme="light");
      }
      await dialog.getByText("远端未检测到：npm",{exact:true}).waitFor({state:npm ? "hidden" : "visible"});
      await dialog.getByText("远端未检测到：Node.js 18+",{exact:true}).waitFor({state:index===1 && Number(nodeVersion?.split(".")[0] ?? 0)<18 ? "visible" : "hidden"});
      await dialog.getByRole("button",{name:"关闭",exact:true}).click();
    }
  }
  // Extension-only users can enable the existing shared profile without installing a CLI.
  await page.evaluate(()=>{
    window.allowPluginWrites=true;
    window.cachedBridge.codexConfigured=false;window.cachedBridge.claudeConfigured=false;
    for(const tool of window.cachedBridge.toolInstallations) {tool.state="missing";tool.version=null;tool.extensionState="installed";tool.extensionVersion=tool.tool==="codex" ? "26.825.2" : "2.1.252";}
  });
  for(const index of [0,1]) {
    const entry=page.locator('.bridge-tool').nth(index);
    await entry.getByText("CLI 未安装",{exact:true}).waitFor();
    await entry.getByText(/VS Code 插件 已安装/).waitFor();
    const lines=entry.locator('.bridge-tool-installation');
    assert.equal(await lines.count(),2);
    const cliBox=await lines.nth(0).boundingBox(), extensionBox=await lines.nth(1).boundingBox();
    assert.ok(extensionBox.y >= cliBox.y + cliBox.height,"CLI and extension metadata occupy separate rows");
    assert.equal(await entry.getByRole("switch").isEnabled(),true);
    await entry.getByRole("button",{name:"查看安装方式",exact:true}).click();
    await dialog.getByText(/无需安装独立 CLI 即可启用接入/).waitFor();
    await dialog.getByRole("button",{name:"关闭",exact:true}).click();
    await entry.getByRole("switch").click();
    try { await page.waitForFunction(index=>document.querySelectorAll('.bridge-tool input[role="switch"]')[index]?.checked===true,index,{timeout:8000}); }
    catch(cause) { console.log(await page.evaluate(()=>({codex:window.cachedBridge.codexConfigured,claude:window.cachedBridge.claudeConfigured,commands:window.ipcCalls.slice(-12),writes:window.routeWrites,error:document.querySelector('.remote-error')?.textContent})));throw cause; }
  }
  assert.equal(await page.evaluate(()=>window.routeWrites),4,"Both tools reuse existing preview/apply commands");
  await page.getByRole("button",{name:"高级",exact:true}).click();
  const advanced=page.locator('.bridge-advanced-view');
  await advanced.getByRole("heading",{name:"环境与配置",exact:true}).waitFor();
  assert.equal(await advanced.getByText("CLI 安装状态",{exact:true}).count(),0);
  assert.equal(await advanced.locator('.advanced-card').count(),6);
  if(process.env.PROXYENV_LAYOUT_CAPTURE!=="0") await page.screenshot({path:resolve(directory,"advanced-installation.png")});
  assert.deepEqual(errors,[]);
  console.log("Installation UI: missing/installed/restorable states, tool independence, copy, recheck, terminal selection, Esc/focus, light/dark/narrow and Advanced passed (synthetic IPC only).");
} finally {await browser.close();}
