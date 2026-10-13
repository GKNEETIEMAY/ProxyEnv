import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

function harness() {
  let now=0, sequence=0;
  const timers=new Map(), watchers=[];
  const exports={};
  const code=ts.transpileModule(readFileSync("src/shared/state/notifications.ts","utf8"),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
  vm.runInNewContext(code,{exports,require:name=>{
    assert.equal(name,"vue");
    return {reactive:value=>value,watch:(source,callback)=>{watchers.push({source,callback});return ()=>{};}};
  },Date:{now:()=>now},setTimeout:(callback,delay)=>{const id=sequence++;timers.set(id,{callback,at:now+delay});return id;},clearTimeout:id=>timers.delete(id)});
  return {...exports,watchers,advance(ms){
    const end=now+ms;
    while(true) {
      const next=[...timers].filter(([,value])=>value.at<=end).sort((a,b)=>a[1].at-b[1].at)[0];
      if(!next)break;
      now=next[1].at;timers.delete(next[0]);next[1].callback();
    }
    now=end;
  }};
}
test("feedback is session-only, bounded, deduplicated and dismissible",()=>{
  const state=harness();
  assert.equal(state.notify("   "),undefined);
  const id=state.notify("Failed","error","operation");
  assert.equal(state.notify("Retry failed","error","operation"),id);
  assert.equal(state.notifications.length,1);
  assert.equal(state.notifications[0].message,"Retry failed");
  for(let i=0;i<3;i++)state.notify(`Result ${i}`,"success");
  assert.equal(state.notifications.length,3);
  assert.equal(state.notifications.some(item=>item.id===id),false);
  state.dismissNotification(state.notifications[0].id);
  assert.equal(state.notifications.length,2);
  state.clearNotifications();state.advance(20000);
  assert.equal(state.notifications.length,0);
});
test("success expires sooner than errors and hovering preserves remaining time",()=>{
  const state=harness();
  const id=state.notify("Copied","success");
  state.advance(1000);state.pauseNotification(id);state.advance(10000);
  assert.equal(state.notifications.length,1,"Timer ID zero can also pause");
  state.resumeNotification(id);state.advance(2999);
  assert.equal(state.notifications.length,1);state.advance(1);assert.equal(state.notifications.length,0);
  state.notify("Failure","error");state.notify("Attention","warning");state.notify("Info","info");
  state.advance(4000);assert.equal(state.notifications.length,2);
  state.advance(6000);assert.equal(state.notifications.length,0);
});
test("updating a hovered notice does not restart its countdown",()=>{
  const state=harness();
  const id=state.notify("First failure","error","request");
  state.pauseNotification(id);state.notify("Second failure","error","request");state.advance(20000);
  assert.equal(state.notifications.length,1);
  state.resumeNotification(id);state.advance(10000);assert.equal(state.notifications.length,0);
});
test("existing localized operation refs use the same service without initial replay",()=>{
  const state=harness();
  state.useNotification(()=>"localized feedback","error","source");
  assert.equal(state.notifications.length,0);
  state.watchers[0].callback("");assert.equal(state.notifications.length,0);
  state.watchers[0].callback("操作失败");assert.equal(state.notifications[0].message,"操作失败");
});
test("all operation surfaces use shared notifications instead of bottom error paragraphs",()=>{
  for(const path of ["application-assistant/components/ApplicationAssistantPage.vue","application-assistant/components/ApplicationProxyGuideDialog.vue","diagnostic-report/components/DiagnosticReportDialog.vue","remote-bridge/components/RemoteBridgePage.vue","remote-bridge/components/RemoteBridgeAdvancedView.vue","remote-bridge/components/RemoteToolDialog.vue","remote-bridge/components/RemoteToolInstallDialog.vue"]){
    const source=readFileSync(`src/features/${path}`,"utf8");
    assert.match(source,/useNotification\(/,path);
    assert.doesNotMatch(source,/<p v-if="(?:error|copyError|launchError|clearLogsError|modelSettingsError)"/,path);
  }
  const shell=readFileSync("src/app/AppShell.vue","utf8"), center=readFileSync("src/shared/components/NotificationCenter.vue","utf8");
  assert.match(shell,/<NotificationCenter /);
  assert.match(center,/popover="manual"/);
  assert.match(center,/prefers-reduced-motion/);
  assert.doesNotMatch(center,/v-html/);
});
