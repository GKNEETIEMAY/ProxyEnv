import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { runInNewContext } from "node:vm";
import { transformSync } from "esbuild";
import * as vue from "vue";

const { computed, effectScope, reactive, ref, watch } = vue;

const page = readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue", "utf8");
function section(first, last) {
  const start = page.indexOf(first);
  const end = page.indexOf(last, start + first.length);
  assert.ok(start >= 0 && end > start, first);
  return page.slice(start, end);
}

const { code: moduleCode } = transformSync(
  readFileSync("src/features/remote-bridge/composables/useRemoteSkills.ts", "utf8"),
  { loader: "ts", format: "cjs" },
);

const { code } = transformSync([
  section("const live = computed", "const sshConnected = computed"),
  page.match(/^const sshConnected = computed.*$/m)?.[0],
  section("const { skillGroups, skillPending, refreshSkills", "const proxyAvailable ="),
  section('watch([() => props.summary.target?.id, () => props.summary.status, () => props.summary.sshAuth.authenticated]', 'watch(() => props.visible'),
  section("onMounted(() => {", "</script>"),
  "Object.assign(actions, { skillGroups, skillPending, toggleSkill });",
].join("\n"), { loader: "ts" });

const skill = (name, id = name) => ({ id, name, tool: "codex", enabled: true });
const settle = () => new Promise(resolve => setImmediate(resolve));

function deferred() {
  let resolve, reject;
  const pending = new Promise((accept, decline) => { resolve = accept; reject = decline; });
  return { pending, resolve, reject };
}

function mount(t, { reviewPreview = false, initialTarget = "first", initialSkills, disableSkill, enableSkill } = {}) {
  const props = reactive({ reviewPreview, summary: {} });
  const setConnection = (id, status = "connected") => {
    props.summary = { target: id ? { id } : null, status, sshAuth: { authenticated: status === "connected" } };
  };
  setConnection(initialTarget, initialTarget ? "connected" : "disconnected");
  const scope = effectScope();
  const actions = {};
  const timers = new Map();
  const reads = [];
  const responses = initialSkills ? [initialSkills] : [];
  const errors = [];
  let nextTimer = 0;
  let mounted, beforeUnmount;
  const busy = ref(false);
  const context = {
    actions, props, computed, watch, busy, URLSearchParams,
    establishingBridge: ref(false), needsAttention: ref(false), targetId: ref("first"),
    mobaOpened: ref(false), serverDirectState: ref("healthy"),
    onMounted(callback) { mounted = callback; },
    onBeforeUnmount(callback) { beforeUnmount = callback; },
    window: { location: { search: "" } },
    perform(action) {
      busy.value = true;
      return action().catch(error => { errors.push(error); }).finally(() => { busy.value = false; });
    },
    remoteBackend: {
      async skills() {
        const target = props.summary.target?.id;
        reads.push(target);
        return responses.shift() ?? [skill(target)];
      },
      disableSkill, enableSkill,
    },
    initializeCapabilities() {}, updateLocalProxyCheck() {}, load: async () => {},
    resetSessionCopyFeedback() {}, detachInteractiveAuth() {},
    sessionCommandGeneration: 0, sessionCopyRevision: 0,
    setTimeout(callback, delay) {
      const id = ++nextTimer;
      timers.set(id, { callback, delay });
      return id;
    },
    clearTimeout(id) { timers.delete(id); },
  };
  const module = { exports: {} };
  runInNewContext(moduleCode, {
    module,
    require(name) {
      assert.equal(name, "vue");
      return vue;
    },
    setTimeout: context.setTimeout,
    clearTimeout: context.clearTimeout,
  });
  context.useRemoteSkills = module.exports.useRemoteSkills;
  scope.run(() => runInNewContext(code, context));
  mounted();
  let disposed = false;
  function unmount() {
    if (disposed) return;
    disposed = true;
    beforeUnmount();
    scope.stop();
  }
  t.after(unmount);
  const items = () => Array.from(actions.skillGroups.value, group =>
    Array.from(group.agents, agent => agent.skill).filter(Boolean)).flat();
  return {
    ...actions, unmount, setConnection, reads, responses, busy, errors, items,
    names: () => items().map(item => item.name),
    timerIds: () => [...timers.keys()],
    delays: () => [...timers.values()].map(timer => timer.delay),
    tick() {
      assert.equal(timers.size, 1, "only one Skills timer may be pending");
      const [id, timer] = timers.entries().next().value;
      timers.delete(id);
      return timer.callback();
    },
  };
}

test("disconnect clears Skills and drops a late poll for the disconnected target", async t => {
  const page = mount(t);
  await settle();
  const old = deferred();
  page.responses.push(old.pending);
  const tick = page.tick();
  page.setConnection(null, "disconnected");
  await settle();
  assert.deepEqual(page.names(), []);
  old.resolve([skill("stale")]);
  await tick;
  assert.deepEqual(page.names(), []);
  assert.deepEqual(page.delays(), []);
});

test("unmount drops a late Skills result and does not restart the timer", async t => {
  const page = mount(t);
  await settle();
  const old = deferred();
  page.responses.push(old.pending);
  const tick = page.tick();
  page.unmount();
  old.resolve([skill("stale")]);
  await tick;
  assert.deepEqual(page.delays(), []);
  assert.deepEqual(page.names(), ["first"]);
});

test("target changes use the new Skills list and preserve the new poll timer", async t => {
  const page = mount(t);
  await settle();
  const old = deferred();
  page.responses.push(old.pending);
  const tick = page.tick();
  page.setConnection("second");
  await settle();
  assert.deepEqual(page.names(), ["second"]);
  const timers = page.timerIds();
  old.resolve([skill("stale")]);
  await tick;
  assert.deepEqual(page.names(), ["second"]);
  assert.equal(timers.length, 1);
  assert.deepEqual(page.timerIds(), timers);
});

test("reconnecting to the same target retires the old Skills poll", async t => {
  const page = mount(t);
  await settle();
  const old = deferred();
  page.responses.push(old.pending);
  const tick = page.tick();
  page.setConnection(null, "disconnected");
  await settle();
  page.setConnection("first");
  await settle();
  const timerIds = page.timerIds();
  old.resolve([skill("stale")]);
  await tick;
  assert.deepEqual(page.names(), ["first"]);
  assert.equal(timerIds.length, 1);
  assert.deepEqual(page.timerIds(), timerIds);
});

test("a late Skills operation cannot update or refresh a different target", async t => {
  const oldWrite = deferred();
  const page = mount(t, { disableSkill: () => oldWrite.pending });
  await settle();
  page.toggleSkill(page.items()[0]);
  page.responses.push(Promise.resolve([skill("current", "first")]));
  page.setConnection("second");
  await settle();
  assert.deepEqual(page.names(), ["current"]);
  const reads = [...page.reads];
  oldWrite.resolve(skill("stale", "first"));
  await settle();
  assert.deepEqual(page.names(), ["current"]);
  assert.deepEqual(page.reads, reads);
  assert.equal(page.skillPending.value, undefined);
});

test("current Skills polling keeps one 2200 ms timer and skips reads while busy", async t => {
  const page = mount(t);
  await settle();
  assert.deepEqual(page.reads, ["first"]);
  assert.deepEqual(page.delays(), [2200]);
  page.busy.value = true;
  await page.tick();
  assert.deepEqual(page.reads, ["first"]);
  assert.deepEqual(page.delays(), [2200]);
  page.busy.value = false;
  const failed = deferred();
  page.responses.push(failed.pending);
  const tick = page.tick();
  failed.reject(Error("synthetic list failure"));
  await tick;
  assert.deepEqual(page.reads, ["first", "first"]);
  assert.deepEqual(page.names(), ["first"]);
  assert.deepEqual(page.delays(), [2200]);
});

test("Skills review preview neither polls nor reads remote resources", async t => {
  const page = mount(t, { reviewPreview: true });
  await settle();
  assert.deepEqual(page.names(), ["frontend-audit", "release-notes", "release-notes", "rust-review"]);
  assert.deepEqual(page.items().map(({ id, hash, fileCount, totalSize, state, enabled }) =>
    [id, hash, fileCount, totalSize, state, enabled]), [
    ["claude|frontend-audit", "review-4", 4, 6800, "synced", true],
    ["codex|release-notes", "review-1", 3, 4100, "synced", true],
    ["claude|release-notes", "review-2", 3, 4100, "synced", true],
    ["codex|rust-review", "review-3", 5, 9200, "synced", true],
  ]);
  assert.deepEqual(page.reads, []);
  assert.deepEqual(page.delays(), []);
  page.setConnection(null, "disconnected");
  await settle();
  assert.deepEqual(page.names(), []);
  assert.deepEqual(page.reads, []);
  assert.deepEqual(page.delays(), []);
});

test("Skills groups sort by name and pair Codex before Claude Code", async t => {
  const page = mount(t, { initialSkills: [
    { ...skill("zeta", "claude|zeta"), tool: "claude" },
    { ...skill("alpha", "claude|alpha"), tool: "claude", enabled: false },
    skill("alpha", "codex|alpha"),
  ] });
  await settle();
  const groups = Array.from(page.skillGroups.value, group => [
    group.name,
    Array.from(group.agents, ({ tool, label, skill }) => [tool, label, skill?.id ?? null, skill?.enabled ?? null]),
  ]);
  assert.deepEqual(groups, [
    ["alpha", [["codex", "Codex", "codex|alpha", true], ["claude", "Claude Code", "claude|alpha", false]]],
    ["zeta", [["codex", "Codex", null, null], ["claude", "Claude Code", "claude|zeta", true]]],
  ]);
});

test("Skills keeps the existing mount and connection refresh triggers", async t => {
  const page = mount(t, { initialTarget: null });
  await settle();
  assert.deepEqual(page.names(), []);
  assert.deepEqual(page.reads, []);
  assert.deepEqual(page.delays(), []);
  page.setConnection("first");
  await settle();
  assert.deepEqual(page.reads, ["first", "first"]);
  assert.deepEqual(page.names(), ["first"]);
  assert.deepEqual(page.delays(), [2200]);
  page.setConnection(null, "disconnected");
  await settle();
  assert.deepEqual(page.reads, ["first", "first"]);
  assert.deepEqual(page.names(), []);
  assert.deepEqual(page.delays(), []);
});

test("Skills enable and disable replace the matching item before refreshing", async t => {
  const calls = [];
  let update;
  const page = mount(t, {
    disableSkill(id) { calls.push(["disable", id]); return update.pending; },
    enableSkill(id) { calls.push(["enable", id]); return update.pending; },
  });
  await settle();
  for (const enabled of [false, true]) {
    update = deferred();
    const list = deferred();
    page.responses.push(list.pending);
    page.toggleSkill(page.items()[0]);
    page.toggleSkill(page.items()[0]);
    assert.equal(page.skillPending.value, "first");
    assert.equal(page.busy.value, true);
    const updated = { ...skill("first"), enabled };
    update.resolve(updated);
    await settle();
    assert.equal(page.items()[0].enabled, enabled);
    assert.equal(page.skillPending.value, "first");
    list.resolve([updated]);
    await settle();
    assert.equal(page.skillPending.value, undefined);
    assert.equal(page.busy.value, false);
  }
  assert.deepEqual(calls, [["disable", "first"], ["enable", "first"]]);
  assert.deepEqual(page.reads, ["first", "first", "first"]);
});

test("failed Skills operations release the pending row through the existing error path", async t => {
  const cause = Error("synthetic Skills update failure");
  const page = mount(t, { disableSkill: async () => { throw cause; } });
  await settle();
  page.toggleSkill(page.items()[0]);
  await settle();
  assert.deepEqual(page.errors, [cause]);
  assert.equal(page.skillPending.value, undefined);
  assert.equal(page.busy.value, false);
  assert.equal(page.items()[0].enabled, true);
  assert.deepEqual(page.reads, ["first"]);
});

test("each Skills mount owns only its own timer", async t => {
  const first = mount(t);
  const second = mount(t);
  await settle();
  assert.deepEqual(first.delays(), [2200]);
  assert.deepEqual(second.delays(), [2200]);
  first.unmount();
  await second.tick();
  assert.deepEqual(first.delays(), []);
  assert.deepEqual(first.reads, ["first"]);
  assert.deepEqual(second.delays(), [2200]);
  assert.deepEqual(second.reads, ["first", "first"]);
  second.unmount();
  assert.deepEqual(second.delays(), []);
});
