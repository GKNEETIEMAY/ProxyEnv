import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { runInNewContext } from "node:vm";
import { build, transformSync } from "esbuild";
import { effectScope, nextTick, ref, watch } from "vue";

const page = readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue", "utf8");
const section = (start, end) => {
  const from = page.indexOf(start);
  const to = page.indexOf(end, from + start.length);
  assert.ok(from >= 0 && to > from, start);
  return page.slice(from, to);
};
const bundle = await build({
  entryPoints: ["src/features/remote-bridge/target-copies.ts"],
  bundle: true,
  platform: "node",
  format: "esm",
  write: false,
});
const helpers = await import("data:text/javascript;base64," + Buffer.from(bundle.outputFiles[0].text).toString("base64"));
const { code } = transformSync([
  section("const storedCopies =", "const checked ="),
  section("async function perform(", "function clearAuthPoll("),
  section("async function load()", "function usePorts("),
  section("function saveConnection()", "function openEditConnectionDialog("),
  section("function removeConnection()", "function cancelSetup("),
  section("watch(targetId,", "watch([() => props.ccDetection"),
  "Object.assign(actions, { targets, hiddenSourceTargets, sourceTargetNames, targetId, copyingSourceId, connectionToRemove, load, persistTargetCopies, saveConnection, removeConnection });",
].join("\n"), { loader: "ts" });

const copiesKey = "proxyenv.remoteBridge.targetCopies";
const lastKey = "proxyenv.remoteBridge.lastTargetId";
const plain = value => JSON.parse(JSON.stringify(value));
const target = (id, source = "openssh", available = true, overrides = {}) => ({
  id, source, available, displayName: "Lab", configPath: "", ...overrides,
});
const settle = () => new Promise(resolve => setImmediate(resolve));

function mount(t, { entries = {}, rows = [], summaryTarget, reviewPreview = false, failStorage, backend = {} } = {}) {
  const stored = new Map(Object.entries(entries));
  const events = [];
  const storage = {
    getItem: key => stored.get(key) ?? null,
    setItem: (key, value) => {
      events.push([key, value]);
      if (failStorage === "write") throw Error("fixture storage write denied");
      stored.set(key, value);
    },
  };
  const context = {
    ...helpers,
    actions: {},
    ref, watch, nextTick,
    props: { reviewPreview, summary: { target: summaryTarget } },
    remoteBackend: {
      targets: async () => { events.push(["targets"]); return rows; },
      clearSessionCredential: async () => { events.push(["clear"]); },
      addConnection: async () => { events.push(["add"]); return { id: "added" }; },
      updateConnection: async id => { events.push(["update", id]); return { id }; },
      removeConnection: async id => { events.push(["remove", id]); },
      ...backend,
    },
    bridgeErrorCode: cause => cause.code,
    emit: name => events.push([name]),
    pickingIdentity: ref(false),
    connectionValid: ref(true),
    identityErrorText: ref(""),
    newConnection: ref({ displayName: "Fixture", destination: "fixture.invalid", authentication: "password", identityFile: null }),
    connectionDialog: ref({ close: () => events.push(["close-form"]) }),
    removeConnectionDialog: ref({ close: () => events.push(["close-remove"]) }),
    networkRefreshRevision: 0,
  };
  for (const state of ["busy", "error", "vscodeSetupWarning", "feedback", "connectionTouched", "connectionSaveError",
    "identitySelectionError", "readinessAnnouncementTarget", "networkCheckPending", "checked", "proxyPort", "ccPort",
    "runtimeExpectedPort", "runtimePortConflict", "vscodeOpened", "mobaOpened", "sshCheck"]) context[state] = ref();
  Object.defineProperty(context, "localStorage", { get() {
    if (failStorage === "access") throw Error("fixture storage unavailable");
    return storage;
  } });
  const scope = effectScope();
  t.after(() => scope.stop());
  scope.run(() => runInNewContext(code, context));
  return { ...context, ...context.actions, events, stored, scope };
}

test("target copy storage tolerates empty, malformed, partial and denied reads", t => {
  for (const [input, failStorage] of [[null], ["{"], ["null"], ['{"names":17}'], [null, "access"]]) {
    const page = mount(t, { entries: input === null ? {} : { [copiesKey]: input }, failStorage });
    assert.deepEqual([...page.hiddenSourceTargets.value], []);
    assert.deepEqual([...page.sourceTargetNames.value], []);
  }
  const partial = mount(t, { entries: { [copiesKey]: '{"hidden":["one"]}' } });
  assert.deepEqual([...partial.hiddenSourceTargets.value], ["one"]);
  assert.deepEqual([...partial.sourceTargetNames.value], []);
});

test("target copy storage filters hidden entries and preserves the existing names and write shape", t => {
  const page = mount(t, { entries: { [copiesKey]: '{"hidden":["one",42,"two","one",null],"names":["first",null]}' } });
  assert.deepEqual([...page.hiddenSourceTargets.value], ["one", "two"]);
  assert.deepEqual(plain([...page.sourceTargetNames.value]), [["0", "first"], ["1", null]]);
  page.sourceTargetNames.value.set("ssh|fixture|lab", "Saved name");
  page.persistTargetCopies();
  assert.deepEqual(JSON.parse(page.stored.get(copiesKey)), {
    hidden: ["one", "two"], names: { 0: "first", 1: null, "ssh|fixture|lab": "Saved name" },
  });
  for (const failStorage of ["access", "write"]) {
    const denied = mount(t, { failStorage });
    denied.hiddenSourceTargets.value.add("in-memory");
    assert.doesNotThrow(denied.persistTargetCopies);
    assert.deepEqual([...denied.hiddenSourceTargets.value], ["in-memory"]);
  }
});

test("target list applies names by import ID and formats display paths without mutating backend rows", async t => {
  const rows = [
    target("openssh|fixture|one", "openssh", true, { configPath: "\\\\?\\C:\\fixture\\ssh.cfg" }),
    target("vscode|fixture|one", "vscode", true, { configPath: "\\\\?\\UNC\\fixture\\share\\ssh.cfg" }),
    target("manual|fixture|one", "manual"),
    target("openssh|fixture|hidden"),
  ];
  const before = plain(rows);
  const page = mount(t, { rows, entries: { [copiesKey]: JSON.stringify({
    hidden: ["openssh|fixture|hidden"],
    names: { "openssh|fixture|one": "Renamed", "manual|fixture|one": "Ignored" },
  }) } });
  await page.load();
  assert.deepEqual(plain(page.targets.value).map(({ id, displayName, configPath }) => [id, displayName, configPath]), [
    ["openssh|fixture|one", "Renamed", "C:\\fixture\\ssh.cfg"],
    ["vscode|fixture|one", "Lab", "\\\\fixture\\share\\ssh.cfg"],
    ["manual|fixture|one", "Lab", ""],
  ]);
  assert.deepEqual(rows, before);
});

test("target list keeps connected, remembered, current and fallback selection priorities", async t => {
  const rows = [target("unsupported", "openssh", false), target("first"), target("current")];
  const cases = [
    { preferred: rows[0], saved: "current", expected: "unsupported" },
    { saved: "current", current: "first", expected: "current" },
    { preferred: target("gone"), saved: "unsupported", current: "current", expected: "current" },
    { saved: "gone", current: "unsupported", expected: "first" },
    { failStorage: "access", current: "unsupported", expected: "first" },
    { rows: [rows[0]], expected: "unsupported" },
    { rows: [], current: "current", expected: "" },
  ];
  for (const option of cases) {
    const page = mount(t, { rows: option.rows ?? rows, summaryTarget: option.preferred, failStorage: option.failStorage });
    page.targetId.value = option.current ?? "";
    await nextTick();
    page.stored.set(lastKey, option.saved);
    await page.load();
    assert.equal(page.targetId.value, option.expected);
  }
});

test("failed target copy save keeps the in-memory copy and stored display data", async t => {
  const saved = '{"hidden":[],"names":{"source":"My Lab"}}';
  const failure = Error("synthetic add failed");
  const page = mount(t, { entries: { [copiesKey]: saved }, backend: { addConnection: async () => { throw failure; } } });
  page.copyingSourceId.value = "source";
  page.saveConnection();
  await settle();
  assert.equal(page.connectionSaveError.value, failure);
  assert.deepEqual([...page.hiddenSourceTargets.value], []);
  assert.equal(page.sourceTargetNames.value.get("source"), "My Lab");
  assert.equal(page.stored.get(copiesKey), saved);
  assert.deepEqual(page.events, [["refresh"]]);
});

test("successful target copy save hides the source and keeps write, credential and selection order", async t => {
  const page = mount(t, { rows: [target("source"), target("added", "manual")],
    entries: { [copiesKey]: '{"names":{"source":"My Lab"}}' } });
  page.copyingSourceId.value = "source";
  page.saveConnection();
  await settle();
  const copied = '{"hidden":["source"],"names":{}}';
  assert.deepEqual(page.events, [
    ["add"], [copiesKey, copied], [copiesKey, copied], ["clear"], ["targets"], [lastKey, "added"], ["close-form"], ["refresh"],
  ]);
  assert.deepEqual(plain(page.targets.value).map(({ id }) => id), ["added"]);
  assert.equal(page.targetId.value, "added");
  assert.equal(page.copyingSourceId.value, null);
});

test("removing an imported target hides it locally and clears the previous target after selecting a fallback", async t => {
  const imported = target("source");
  const fallback = target("fallback", "manual");
  const page = mount(t, { rows: [imported, fallback], entries: { [copiesKey]: '{"names":{"source":"My Lab"}}' } });
  await page.load();
  await settle();
  page.events.length = 0;
  page.connectionToRemove.value = imported;
  page.removeConnection();
  await settle();
  assert.deepEqual(page.events, [
    ["close-remove"], [copiesKey, '{"hidden":["source"],"names":{}}'], [lastKey, "fallback"], ["clear"],
  ]);
  assert.equal(page.targetId.value, "fallback");
});

test("removing a manual target uses backend success and preserves the list on failure", async t => {
  const manual = target("manual", "manual");
  const remaining = target("remaining");
  const fail = Error("synthetic remove failed");
  const denied = mount(t, { rows: [manual], backend: { removeConnection: async () => { throw fail; } } });
  await denied.load();
  await settle();
  denied.events.length = 0;
  denied.connectionToRemove.value = manual;
  denied.removeConnection();
  await settle();
  assert.equal(denied.error.value, fail);
  assert.equal(denied.targetId.value, "manual");
  assert.deepEqual(denied.events, [["close-remove"], ["refresh"]]);

  const page = mount(t, { rows: [remaining] });
  page.connectionToRemove.value = manual;
  page.removeConnection();
  await settle();
  assert.deepEqual(page.events, [["close-remove"], ["remove", "manual"], [copiesKey, '{"hidden":[],"names":{}}'],
    ["targets"], [lastKey, "remaining"], ["refresh"]]);
  assert.equal(page.targetId.value, "remaining");
});

test("target selection watcher persists before credential cleanup and stops with its Vue scope", async t => {
  const page = mount(t);
  page.targetId.value = "one";
  await nextTick();
  assert.deepEqual(page.events, [[lastKey, "one"]]);
  page.proxyPort.value = 1234;
  page.ccPort.value = 5678;
  page.checked.value = true;
  page.targetId.value = "two";
  await nextTick();
  assert.deepEqual(page.events, [[lastKey, "one"], [lastKey, "two"], ["clear"]]);
  assert.deepEqual([page.proxyPort.value, page.ccPort.value, page.checked.value], [0, 0, false]);
  page.targetId.value = "";
  await nextTick();
  assert.deepEqual(page.events.at(-1), ["clear"]);
  assert.equal(page.stored.get(lastKey), "two");
  page.scope.stop();
  page.targetId.value = "after-unmount";
  await nextTick();
  assert.equal(page.events.length, 4);
});
