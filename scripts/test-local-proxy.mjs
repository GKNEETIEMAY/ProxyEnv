import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

function module(path) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(readFileSync(path, "utf8"), { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText, { exports });
  return exports;
}
const { ConnectivitySession, connectivityExpired, connectivityTone, connectivityTtlMs } = module("src/features/proxy/connectivity.ts");
const { RefreshEpoch } = module("src/features/proxy/refresh-epoch.ts");
const { localProxyMessages } = module("src/shared/i18n/local-proxy.ts");
const { groupProxyCandidates, supportsProxyVariable } = module("src/features/proxy/client-presentation.ts");

test("application IPC exposes only selection, authorization and revision-checked proxy launch", () => {
  const commands = readFileSync("src-tauri/src/commands/application_assistant.rs", "utf8");
  const registered = readFileSync("src-tauri/src/lib.rs", "utf8");
  const expected = ["list_running_applications", "pick_application", "renew_application_authorization", "launch_application_with_current_proxy"];
  assert.deepEqual([...commands.matchAll(/#\[tauri::command\]\s+pub async fn (\w+)/g)].map(match => match[1]), expected);
  assert.deepEqual([...registered.matchAll(/commands::application_assistant::(\w+)/g)].map(match => match[1]), expected);
  assert.match(commands, /resolve_application\(&application_id\)/);
  assert.match(commands, /active::with_current\(Some\(expected_revision\)/);
});

test("sync scope reflects the supported environment schemes", () => {
  for (const protocol of ["http", "socks5", "mixed", "unknown"]) {
    const actual = ["http", "https", "all"].filter(variable => supportsProxyVariable(protocol, variable));
    assert.deepEqual(actual, { http: ["http", "https"], socks5: ["all"], mixed: ["http", "https", "all"], unknown: [] }[protocol]);
  }
});

test("one client groups IPv4, IPv6 and ports while retaining the chosen endpoint", () => {
  const endpoint = { iconKey: "v2rayn", clientName: "v2rayN", host: "127.0.0.1", port: 10808, protocol: "mixed", listening: true, pid: 42 };
  const candidates = [
    { ...endpoint, id: "v4" }, { ...endpoint, id: "v6", host: "::1" },
    { ...endpoint, id: "http", port: 10812, protocol: "http" },
    { ...endpoint, id: "clash", iconKey: "clash-verge-rev", clientName: "Clash Verge Rev" }
  ];
  const grouped = groupProxyCandidates(candidates, "http");
  assert.equal(grouped.length, 2);
  assert.equal(grouped[0].endpoints.length, 3);
  assert.equal(grouped[0].candidate.id, "http");
  assert.equal(groupProxyCandidates(candidates, null)[0].candidate.id, "v4");
});

test("unknown proxies are not conflated and unavailable selections remain explicit", () => {
  const endpoint = { iconKey: "generic-proxy", clientName: "Other", host: "127.0.0.1", port: 1, protocol: "http", listening: true };
  assert.equal(groupProxyCandidates([{ ...endpoint, id: "a", pid: 1 }, { ...endpoint, id: "b", pid: 2 }, { ...endpoint, id: "c" }, { ...endpoint, id: "d" }], null).length, 4);
  assert.equal(groupProxyCandidates([{ ...endpoint, id: "c", pid: null }, { ...endpoint, id: "d", pid: null }], null).length, 2, 'Missing serialized PIDs do not merge unrelated endpoints');
  const candidates = [{ ...endpoint, id: "off", pid: 1, listening: false }, { ...endpoint, id: "on", pid: 1 }];
  assert.equal(groupProxyCandidates(candidates, null)[0].candidate.id, "on");
  assert.equal(groupProxyCandidates(candidates, "off")[0].candidate.id, "off");
});
const deferred = () => { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; };
function context(id = "A", revision = 1) { return { selectedCandidateId: id, revision, available: true, candidate: { id, host: "127.0.0.1", port: id === "A" ? 7897 : 1080, protocol: "mixed" } }; }
function result(ctx = context(), successes = 3) { return { state: "reachable", endpoint: { host: ctx.candidate.host, port: ctx.candidate.port, protocol: "mixed" }, localListenerOk: true, protocolOk: true, testedAt: new Date().toISOString(), totalDurationMs: 30, targets: ["Google", "GitHub", "Cloudflare"].map((name, i) => ({ name, success: i < successes, statusCode: i < successes ? 200 : 403, errorKind: i < successes ? null : "remoteRejected", durationMs: 10 })) }; }
function session(clock = Date.now) {
  let ctx = context(); const tests = [], cache = [], errors = [];
  const state = { result: null, testing: false, cached: false, failed: false };
  const controller = new ConnectivitySession(state, () => ctx, {
    testCurrentProxyConnectivity: () => { const task = deferred(); tests.push(task); return task.promise; },
    lastProxyConnectivity: () => { const task = deferred(); cache.push(task); return task.promise; }
  }, error => errors.push(error), clock);
  return { state, controller, tests, cache, errors, change: next => { ctx = next; controller.invalidate(); } };
}

test("late A results never replace B or a new A selection; stale errors stay silent", async () => {
  const s = session(); const a = s.controller.test();
  s.change(context("B", 2)); const b = s.controller.test();
  s.tests[1].resolve(result(context("B", 2))); await b;
  s.tests[0].resolve(result()); await a;
  assert.equal(s.state.result.endpoint.port, 1080);
  s.change(context("A", 3)); const old = s.controller.test();
  s.change(context("B", 4)); s.change(context("A", 5));
  const fresh = s.controller.test(); s.tests[2].reject(Error("old")); await old;
  assert.equal(s.state.testing, true); assert.equal(s.errors.length, 0);
  s.tests[3].resolve(result()); await fresh; assert.equal(s.state.testing, false);
});
test("cache completion cannot replace an explicit test; duplicate clicks issue one test", async () => {
  const s = session(); const cache = s.controller.readCache(); const pending = s.controller.test();
  await s.controller.test(); assert.equal(s.tests.length, 1);
  s.tests[0].resolve(result(context(), 2)); await pending;
  s.cache[0].resolve(result()); await cache;
  assert.equal(s.state.result.targets.filter(t => t.success).length, 2);
  assert.equal(s.state.cached, false);
});
test("cache loading and unavailable selections never trigger external tests", async () => {
  const s = session(); const read = s.controller.readCache(); s.cache[0].resolve(result()); await read;
  assert.equal(s.state.cached, true); assert.equal(s.tests.length, 0);
  s.change({ ...context(), available: false }); await s.controller.test(); await s.controller.readCache();
  assert.equal(s.tests.length, 0); assert.equal(s.cache.length, 1); assert.equal(s.state.result, null);
});
test("operation errors remain distinct from measured connectivity failure", async () => {
  const s = session(); const pending = s.controller.test(); s.tests[0].reject(Error("revision changed")); await pending;
  assert.equal(s.state.result, null); assert.equal(s.state.failed, true); assert.equal(s.state.testing, false); assert.equal(s.errors.length, 1);
});
test("automatic checks run once per half hour, manual checks reset the interval", async () => {
  let now = Date.now();
  const s = session(() => now);
  const first = s.controller.testIfDue();
  await s.controller.testIfDue();
  assert.equal(s.tests.length, 1, 'No duplicate in-flight automatic test');
  s.tests[0].resolve({ ...result(), testedAt: new Date(now).toISOString() }); await first;
  now += connectivityTtlMs - 1;
  await s.controller.testIfDue(); assert.equal(s.tests.length, 1);
  const manual = s.controller.test();
  s.tests[1].resolve({ ...result(), testedAt: new Date(now).toISOString() }); await manual;
  now += 1; await s.controller.testIfDue(); assert.equal(s.tests.length, 2);
  now += connectivityTtlMs - 1;
  const next = s.controller.testIfDue(); assert.equal(s.tests.length, 3);
  s.tests[2].resolve({ ...result(), testedAt: new Date(now).toISOString() }); await next;
  now += 10 * connectivityTtlMs;
  const resumed = s.controller.testIfDue(); assert.equal(s.tests.length, 4, 'Sleep causes one check, not a backlog');
  s.tests[3].resolve({ ...result(), testedAt: new Date(now).toISOString() }); await resumed;
});

test("automatic failures stay quiet and wait for the next interval; unavailable proxies are skipped", async () => {
  let now = Date.now();
  const s = session(() => now);
  const first = s.controller.testIfDue(); s.tests[0].reject(new Error('offline')); await first;
  assert.equal(s.state.failed, true); assert.equal(s.errors.length, 0);
  now += connectivityTtlMs - 1; await s.controller.testIfDue(); assert.equal(s.tests.length, 1);
  s.change({ ...context(), available: false });
  now += 1; await s.controller.testIfDue(); assert.equal(s.tests.length, 1);
  s.change(context('B', 2));
  const recovered = s.controller.testIfDue(); assert.equal(s.tests.length, 2);
  s.tests[1].resolve(result(context('B', 2))); await recovered;
  assert.equal(s.state.result.endpoint.port, 1080);
});

test("fresh cached results defer automatic checks until expiry", async () => {
  let now = Date.now(); const s = session(() => now);
  const read = s.controller.readCache();
  s.cache[0].resolve({ ...result(), testedAt: new Date(now).toISOString() }); await read;
  await s.controller.testIfDue(); assert.equal(s.tests.length, 0);
  now += connectivityTtlMs;
  const check = s.controller.testIfDue(); assert.equal(s.tests.length, 1);
  s.tests[0].resolve({ ...result(), testedAt: new Date(now).toISOString() }); await check;
});

test("TTL, invalid times and partial success never show a healthy result", () => {
  const r = result(context(), 2); const now = Date.parse(r.testedAt);
  assert.equal(connectivityTtlMs, 30 * 60_000);
  assert.equal(connectivityExpired(r, now + connectivityTtlMs - 1), false);
  assert.equal(connectivityExpired(r, now + connectivityTtlMs), true);
  assert.equal(connectivityExpired(r, now - 1), true);
  assert.equal(connectivityExpired({ ...r, testedAt: "bad date" }, now), true);
  assert.equal(connectivityTone(r, false), "warning");
  assert.equal(connectivityTone(result(), false), "healthy");
  assert.equal(connectivityTone(result(), true), "idle");
  assert.equal(connectivityTone({ ...r, localListenerOk: false, targets: [] }, false), "failed");
});
test("new local copy has complete keys and interpolation in all four languages", () => {
  for (const messages of Object.values(localProxyMessages)) {
    assert.deepEqual(Object.keys(messages).sort(), Object.keys(localProxyMessages.en).sort());
    for (const value of Object.values(messages)) assert.ok(value.trim());
    assert.match(messages.localTestPassed, /\{passed\}.*\{total\}/);
  }
});

const shell = readFileSync("src/app/AppShell.vue", "utf8");
function shellHarness() {
  const values = { state: "enabled", entries: [], activeProxy: context(), candidates: [] };
  const reads = []; const writes = []; const saved = [];
  const state = {
    refreshPending: false, refreshAgain: false, refreshEpoch: new RefreshEpoch(), localNow: { value: 0 }, localStale: { value: false },
    toggling: { value: false }, loading: { value: false }, error: { value: "" }, candidates: { value: [] },
    environment: { value: values }, activeProxyContext: { value: values.activeProxy }, tun: { value: {} },
    copy: { value: { activeProxyUnavailable: "Unavailable" } }, reviewPreview: false, settingsSaveTimer: undefined,
    window: { clearTimeout() {} }, pendingSettings: undefined, settingsInFlight: undefined, settingsSaveError: undefined,
    saveInFlight: false, savedNotificationSettings: {}, restoringNotificationSettings: false,
    settingsError: { value: "" }, settingsLoadError: { value: "" }, view: { value: "local" },
    draftSettings: { value: { proxyVariables: ["http", "https"] } }, copySettings: settings => structuredClone(settings),
    backend: {
      environmentStatus: () => { const task = deferred(); reads.push(task); return task.promise; },
      tunObservation: async () => ({ state: "notDetected", evidence: [] }),
      saveAppSettings: async settings => { saved.push(settings); return settings; },
      syncProxyEnvironment: async () => { writes.push("sync"); return { ...values, state: "partial" }; },
      disableProxyEnvironment: async () => { writes.push("disable"); return { ...values, state: "disabled" }; },
      restoreProxyEnvironment: async () => { writes.push("restore"); return { ...values, state: "mismatch" }; }
    }, exports: {}
  };
  const source = shell.slice(shell.indexOf("async function refresh("), shell.indexOf("function openSettings()"))
    + shell.slice(shell.indexOf("async function flushSettings()"), shell.indexOf("async function loadDesktopAlerts()"))
    + "\nexports.refresh=refresh;exports.sync=applyDetectedProxy;exports.disable=disableEnvironment;exports.restore=restoreEnvironment;exports.persist=persistProxySettings;exports.flush=flushSettings;";
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText, state);
  return { state, reads, writes, saved, values };
}
for (const [operation, expected] of [["sync", "partial"], ["disable", "disabled"], ["restore", "mismatch"]]) {
  test(`slow same-revision refresh cannot undo ${operation}; a fresh observation is queued`, async () => {
    const h = shellHarness(); const oldRead = h.state.exports.refresh(true);
    await h.state.exports[operation](); assert.equal(h.state.environment.value.state, expected);
    h.reads[0].resolve(h.values); await oldRead;
    assert.equal(h.state.environment.value.state, expected); assert.equal(h.reads.length, 2);
    h.reads[1].resolve({ ...h.values, state: expected }); await new Promise(resolve => setImmediate(resolve));
    assert.equal(h.state.refreshPending, false);
  });
}
test("sync waits for the newest selected variable settings, including an in-flight save", async () => {
  const h = shellHarness(); const first = deferred(); let calls = 0;
  h.state.backend.saveAppSettings = async settings => { h.saved.push(settings); if (++calls === 1) await first.promise; return settings; };
  h.state.pendingSettings = { proxyVariables: ["http"] }; const prior = h.state.exports.flush();
  h.state.draftSettings.value.proxyVariables = ["http", "all"];
  const sync = h.state.exports.sync(); await Promise.resolve(); assert.equal(h.writes.length, 0);
  first.resolve(); await prior;
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(h.saved.at(-1).proxyVariables, ["http", "all"]); assert.equal(h.writes[0], "sync");
  h.reads[0].resolve(h.values); await sync;
});
test("settings failure blocks environment writes, refresh failures mark old observations stale", async () => {
  const h = shellHarness(); h.state.backend.saveAppSettings = async () => { throw Error("settings denied"); };
  const sync = h.state.exports.sync(); await new Promise(resolve => setImmediate(resolve));
  assert.equal(h.writes.length, 0); assert.match(h.state.error.value, /settings denied/);
  h.reads[0].reject(Error("probe failed")); await sync; assert.equal(h.state.localStale.value, true);
});
