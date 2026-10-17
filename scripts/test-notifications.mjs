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
test("master opt-out removes optional feedback but preserves errors and safety warnings",()=>{
  const state=harness();
  state.notify("Copied","success");state.notify("Info","info");state.notify("Required failure","error");
  state.setOptionalNotificationsEnabled(false);
  assert.deepEqual(Array.from(state.notifications,item=>item.kind),["error"]);
  assert.equal(state.notify("Optional success","success"),undefined);
  assert.equal(state.notify("Optional info","info"),undefined);
  state.notify("Security confirmation required","warning");
  assert.equal(state.notifications.length,2);
  state.setOptionalNotificationsEnabled(true);
  state.notify("Enabled again","success");
  assert.equal(state.notifications.length,3);
  state.advance(10000);assert.equal(state.notifications.length,0);
});

test("desktop consent is separate, off by default and loaded once at shell startup",()=>{
  const shell=readFileSync("src/app/AppShell.vue","utf8"),page=readFileSync("src/features/settings/components/SettingsPage.vue","utf8");
  const service=readFileSync("src-tauri/src/services/desktop_alerts.rs","utf8"),legacy=readFileSync("src-tauri/src/services/settings.rs","utf8");
  assert.match(shell,/const desktopAlerts = ref\(false\)/);
  assert.match(shell,/settingsReady = true;\s+void loadDesktopAlerts\(\)/);
  assert.match(shell,/watch\(\(\) => draftSettings.value.notificationsEnabled, setOptionalNotificationsEnabled/);
  assert.doesNotMatch(page,/onMounted|backend\.desktopAlerts|setInterval/);
  assert.match(service,/routespan-desktop-alerts\.v1\.json/);
  assert.match(service,/return Ok\(false\)/);
  assert.doesNotMatch(legacy,/desktop_alerts|desktopAlerts/);
  assert.match(page,/!settings.value.notificationsEnabled \|\| !props.desktopAlertsReady \|\| props.desktopAlertsBusy/);
  assert.match(page,/desktopDisabled.value \|\| !props.desktopAlerts/);
  assert.match(page,/@retry-desktop-alerts|retryDesktopAlerts/);
});

test("failed legacy notification writes restore durable values without discarding newer edits",async()=>{
  const shell=readFileSync("src/app/AppShell.vue","utf8");
  const source=shell.slice(shell.indexOf("async function flushSettings()"),shell.indexOf("async function loadDesktopAlerts()"));
  for(const newer of [false,true]) {
    let calls=0;
    const state={pendingSettings:{notificationsEnabled:false,notificationSound:true},saveInFlight:false,
      savedNotificationSettings:{notificationsEnabled:true,notificationSound:false},restoringNotificationSettings:false,
      settingsError:{value:""},settingsLoadError:{value:""},error:{value:""},view:{value:"settings"},
      draftSettings:{value:{notificationsEnabled:false,notificationSound:true}},exports:{}};
    state.backend={saveAppSettings:async()=>{
      if(++calls===1) {
        if(newer) {
          state.draftSettings.value.notificationsEnabled=true;
          state.pendingSettings={notificationsEnabled:true,notificationSound:true};
        }
        throw new Error("save failed");
      }
    }};
    vm.runInNewContext(`${source};exports.flush=flushSettings;`,state);
    await state.exports.flush();
    // The queued save starts synchronously in finally, then completes in this microtask.
    await Promise.resolve();
    assert.equal(state.draftSettings.value.notificationsEnabled,true);
    assert.equal(state.draftSettings.value.notificationSound,false);
    assert.equal(state.savedNotificationSettings.notificationsEnabled,true);
    assert.equal(state.savedNotificationSettings.notificationSound,false);
    assert.equal(state.restoringNotificationSettings,false);
    assert.equal(calls,newer?2:1);
  }
});

test("native delivery uses cached reconnect attention and invalidates stale activation",()=>{
  const native=readFileSync("src-tauri/src/desktop/notifications.rs","utf8");
  assert.match(native,/tray_state::listen\(sender\)/);
  assert.match(native,/receiver.recv\(\)/);
  assert.match(native,/desktop_alerts::load\(\).unwrap_or\(false\)/);
  assert.match(native,/settings::load\(\).ok\(\)/);
  assert.match(native,/ACTIVE.load\(Ordering::SeqCst\) == serial/);
  assert.match(native,/attention_generation == Some\(generation\)/);
  assert.match(native,/RemoveGroupedTagWithId/);
  assert.match(native,/RemoveActivated/);
  assert.doesNotMatch(native,/recv_timeout|sleep\(|bridge_attention|detect_prox|ssh::|unwrap_or_default\(\)/);
  assert.match(native,/<audio silent=/);
  assert.match(native,/emit\("bridge-notification-open", \(\)\)/);
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
