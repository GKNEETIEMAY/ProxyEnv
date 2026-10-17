import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";
import { copyModuleUrl } from "./remote-tool-test-modules.mjs";

const compile = source => ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }).outputText;
const { trayNavigationAction, trayCapabilityIntent } = await import(`data:text/javascript;base64,${Buffer.from(compile(readFileSync("src/app/navigation.ts", "utf8"))).toString("base64")}`);
const { remoteBridgeMessages } = await import(copyModuleUrl);
const shell = readFileSync("src/app/AppShell.vue", "utf8");
const page = readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue", "utf8");
const tray = readFileSync("src-tauri/src/desktop/tray.rs", "utf8");
const projection = readFileSync("src-tauri/src/features/remote_bridge/tray_state.rs", "utf8");
const ref = value => ({ value });
const connected = extra => ({ status: "connected", target: { id: "example" }, reconnectState: "idle", proxy: {}, cc: null, ...extra });

test("tray IPC accepts only navigation/capability intents, never targets or shell commands", () => {
  for (const value of ["local", "remote", "proxy", "cc"]) assert.equal(trayNavigationAction(value), value);
  for (const value of [undefined, null, 1, {}, { action: "proxy", targetId: "other" }, "ssh host", "disconnect", "quit"]) assert.equal(trayNavigationAction(value), null);
});

test("capability intent reads the current summary and cannot establish a new SSH connection", () => {
  const summary = connected();
  assert.deepEqual(trayCapabilityIntent(summary, "proxy"), { targetId: "example", capability: "proxy", enabled: false });
  assert.deepEqual(trayCapabilityIntent(summary, "cc"), { targetId: "example", capability: "cc", enabled: true });
  assert.equal(summary.cc, null);
  for (const status of ["disconnected", "connecting", "stale", "unavailable", "error"]) assert.equal(trayCapabilityIntent(connected({ status }), "proxy"), null);
  for (const reconnectState of ["waiting", "retrying", "attentionRequired"]) assert.equal(trayCapabilityIntent(connected({ reconnectState }), "proxy"), null);
  assert.equal(trayCapabilityIntent(connected({ target: null }), "proxy"), null);
  assert.equal(trayCapabilityIntent(connected({ postConnectStatus: "preparing" }), "proxy"), null);
});

function navigationHarness() {
  const calls = [];
  const context = { trayNavigationAction, view: ref("local"), updateExclusive: ref(false), modal: false, calls,
    document: { querySelector: () => context.modal ? {} : null },
    copy: ref({ rbCapabilityBusy: "busy" }), notify: (...args) => calls.push(["notice", ...args]),
    openLocal: () => { context.view.value = "local"; },
    openRemote: () => { context.view.value = context.updateExclusive.value ? "settings" : "remote"; },
    refreshRemoteBridge: async () => { calls.push(["refresh"]); }, nextTick: async () => {},
    remotePage: ref({ requestTrayCapability: action => calls.push(["request", action]) }) };
  runInNewContext(compile(shell.slice(shell.indexOf("let trayNavigationRevision"), shell.indexOf("async function copyEndpoint("))), context);
  return context;
}

test("tray navigation restores the requested workspace and hands capability changes to the UI", async () => {
  const context = navigationHarness();
  await context.handleTrayNavigation("remote"); assert.equal(context.view.value, "remote");
  await context.handleTrayNavigation("local"); assert.equal(context.view.value, "local");
  await context.handleTrayNavigation("cc"); assert.deepEqual(context.calls, [["refresh"], ["request", "cc"]]);
});

test("modal ownership and update exclusion prevent tray capability bypass", async () => {
  const context = navigationHarness();
  context.modal = true;
  await context.handleTrayNavigation("proxy"); assert.equal(context.view.value, "local");
  context.modal = false; context.updateExclusive.value = true;
  await context.handleTrayNavigation("cc"); assert.equal(context.view.value, "settings");
  assert.ok(!context.calls.some(([type]) => type === "request"));
});

test("update exclusion acquired during refresh also prevents opening confirmation", async () => {
  const context = navigationHarness();
  context.refreshRemoteBridge = async () => { context.updateExclusive.value = true; };
  await context.handleTrayNavigation("proxy");
  assert.equal(context.view.value, "settings");
  assert.ok(!context.calls.some(([type]) => type === "request"));
});

test("requesting a tray change only opens confirmation, respecting busy and unavailable capabilities", () => {
  const calls = [];
  const context = { trayCapabilityIntent, busy: ref(false), props: { summary: connected(), copy: {} },
    document: { querySelector: () => null }, notify: (...args) => calls.push(["notice", ...args]),
    proxyAvailable: ref(false), ccUsable: ref(false), advancedView: ref(true), trayChange: ref(),
    trayConfirmation: ref({ showModal: () => calls.push(["dialog"]) }) };
  runInNewContext(compile(page.slice(page.indexOf("function requestTrayCapability("), page.indexOf("function closeTrayConfirmation()"))), context);
  context.requestTrayCapability("proxy");
  assert.equal(context.advancedView.value, false);
  assert.deepEqual(JSON.parse(JSON.stringify(context.trayChange.value)), { targetId: "example", capability: "proxy", enabled: false });
  assert.deepEqual(calls, [["dialog"]], "Disabling an existing lane does not require its local endpoint to remain available");
  context.requestTrayCapability("cc");
  assert.equal(calls.filter(([type]) => type === "dialog").length, 1, "Unavailable local routes cannot be enabled");
  context.ccUsable.value = true; context.busy.value = true;
  context.requestTrayCapability("cc");
  assert.equal(calls.filter(([type]) => type === "dialog").length, 1);
});

test("a slower tray request cannot overwrite a newer navigation or open a dialog on another page", async () => {
  const context = navigationHarness();
  let finish;
  context.refreshRemoteBridge = () => new Promise(resolve => { finish = resolve; });
  const previous = context.handleTrayNavigation("proxy");
  await context.handleTrayNavigation("local");
  finish(); await previous;
  assert.equal(context.view.value, "local");
  assert.ok(!context.calls.some(([type]) => type === "request"));
});

function confirmationHarness(current) {
  let task;
  const calls = [];
  const context = { trayCapabilityIntent, busy: ref(false), trayChange: ref({ targetId: "example", capability: "proxy", enabled: false }),
    props: { summary: connected(), copy: { rbTrayStateChanged: "changed" }, updateLocked: false },
    closeTrayConfirmation: () => calls.push(["close"]),
    perform: action => { task = action(); },
    remoteBackend: { summary: async () => current },
    emit: (...args) => calls.push(args), notify: (...args) => calls.push(["notice", ...args]),
    changeCapability: async (...args) => calls.push(["change", ...args]) };
  runInNewContext(compile(page.slice(page.indexOf("function confirmTrayCapability()"), page.indexOf("watch(() => [props.summary.target?.id"))), context);
  return { context, calls, done: () => task };
}

test("confirmation revalidates the target, capability state, reconnect and update gate", async () => {
  for (const summary of [connected({ target: { id: "different" } }), connected({ proxy: null }), connected({ status: "disconnected" }), connected({ reconnectState: "retrying" })]) {
    const harness = confirmationHarness(summary);
    harness.context.confirmTrayCapability(); await harness.done();
    assert.ok(!harness.calls.some(([type]) => type === "change"));
  }
  const harness = confirmationHarness(connected());
  harness.context.confirmTrayCapability(); await harness.done();
  assert.deepEqual(harness.calls, [["close"], ["change", "example", "proxy", false]]);
  const locked = confirmationHarness(connected());
  locked.context.props.updateLocked = true;
  locked.context.confirmTrayCapability(); await assert.rejects(locked.done(), error => error === "updateInProgress");
});

test("native tray consumes cached change notifications and revalidates only actions", () => {
  const navigate = tray.slice(tray.indexOf("fn navigate"), tray.indexOf("pub fn setup"));
  assert.match(navigate, /show_main_window\(app\)/);
  assert.match(navigate, /tray_state::can_change\(\)/);
  assert.match(navigate, /app.emit\("tray-navigation", action\)/);
  assert.doesNotMatch(tray, /set_capability|remote_bridge::connect|ssh::|invoke\(/);
  assert.match(projection, /source.try_lock\(\)/);
  assert.match(projection, /bridge_changes_busy\(\)/);
  const snapshot = projection.slice(projection.indexOf("pub(crate) fn snapshot()"), projection.indexOf("pub(crate) fn subscribe()"));
  assert.match(snapshot, /store\(\).tray.snapshot\(\)/);
  assert.doesNotMatch(snapshot, /try_lock|busy|refresh|discover/);
  assert.match(tray, /tray_state::subscribe\(\)/);
  assert.match(tray, /changes.recv_timeout/);
  assert.ok(tray.indexOf("tray_state::subscribe()") < tray.indexOf("let snapshot = tray_state::snapshot()"));
  assert.doesNotMatch(tray, /Connection::Busy|正在处理|thread::sleep/);
  assert.doesNotMatch(projection.split("#[cfg(test)]")[0], /\.host|\.port|token|fingerprint\(|summary\(\)|TcpStream|ssh::/);
  assert.match(shell, /listen<unknown>\("tray-navigation"/);
  assert.match(page, /defineExpose\(\{ requestTrayCapability \}\)/);
  assert.match(page, /ref="trayConfirmation"[\s\S]*?@submit.prevent="confirmTrayCapability"/);
  assert.match(page, /async function changeCapability[\s\S]*?remoteBackend.setCapability/);
});

test("four locales provide explicit confirmation and state-change recovery", () => {
  for (const locale of ["zh-CN", "en", "ja", "ko"]) {
    const copy = remoteBridgeMessages[locale];
    assert.match(copy.rbTrayEnableCapability, /\{capability\}/);
    assert.match(copy.rbTrayPauseCapability, /\{capability\}/);
    assert.ok(copy.rbTrayStateChanged && copy.rbOverviewCapabilityHelp && copy.rbCancel && copy.rbConfirm);
  }
});
