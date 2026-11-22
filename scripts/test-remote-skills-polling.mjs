import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { runInNewContext } from "node:vm";
import { transformSync } from "esbuild";
import { computed, effectScope, reactive, ref, watch } from "vue";

const page = readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue", "utf8");
function section(first, last) {
  const start = page.indexOf(first);
  const end = page.indexOf(last, start + first.length);
  assert.ok(start >= 0 && end > start, first);
  return page.slice(start, end);
}

const { code } = transformSync([
  section("let skillsPollTimer:", "const selectedTarget ="),
  section("const live = computed", "const sshConnected = computed"),
  page.match(/^const sshConnected = computed.*$/m)?.[0],
  section("async function refreshSkills()", "function copyValue("),
  section('watch([() => props.summary.target?.id, () => props.summary.status, () => props.summary.sshAuth.authenticated]', 'watch(() => props.visible'),
  section("onMounted(() => {", "</script>"),
  "Object.assign(actions, { skills, toggleSkill });",
].join("\n"), { loader: "ts" });

const skill = (name, id = name) => ({ id, name, tool: "codex", enabled: true });
const settle = () => new Promise(resolve => setImmediate(resolve));

function deferred() {
  let resolve, reject;
  const pending = new Promise((accept, decline) => { resolve = accept; reject = decline; });
  return { pending, resolve, reject };
}

function mount(t, { reviewPreview = false, disableSkill } = {}) {
  const props = reactive({ reviewPreview, summary: {} });
  const setConnection = (id, status = "connected") => {
    props.summary = { target: id ? { id } : null, status, sshAuth: { authenticated: status === "connected" } };
  };
  setConnection("first");
  const scope = effectScope();
  const actions = {};
  const timers = new Map();
  const reads = [];
  const responses = [];
  let nextTimer = 0;
  let mounted, beforeUnmount;
  const busy = ref(false);
  const skillPending = ref();
  const context = {
    actions, props, ref, computed, watch, busy, skillPending, URLSearchParams,
    establishingBridge: ref(false), needsAttention: ref(false), targetId: ref("first"),
    mobaOpened: ref(false), serverDirectState: ref("healthy"),
    onMounted(callback) { mounted = callback; },
    onBeforeUnmount(callback) { beforeUnmount = callback; },
    window: { location: { search: "" } },
    perform(action) {
      busy.value = true;
      return action().finally(() => { busy.value = false; });
    },
    remoteBackend: {
      async skills() {
        const target = props.summary.target?.id;
        reads.push(target);
        return responses.shift() ?? [skill(target)];
      },
      disableSkill,
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
  return {
    ...actions, unmount, setConnection, reads, responses, busy, skillPending,
    names: () => Array.from(actions.skills.value, item => item.name),
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
  page.toggleSkill(page.skills.value[0]);
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
  assert.deepEqual(page.names(), ["release-notes", "release-notes", "rust-review", "frontend-audit"]);
  assert.deepEqual(page.reads, []);
  assert.deepEqual(page.delays(), []);
  page.setConnection(null, "disconnected");
  await settle();
  assert.deepEqual(page.names(), []);
  assert.deepEqual(page.reads, []);
  assert.deepEqual(page.delays(), []);
});
