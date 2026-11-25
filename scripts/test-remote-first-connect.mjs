import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";
import { copyModuleUrl } from "./remote-tool-test-modules.mjs";
import { mountInteractiveAuth } from "./remote-interactive-auth-test.mjs";
import { mountConnection } from "./remote-connection-flow-test.mjs";

const compile = source => ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }).outputText;
const { remoteReadiness } = await import(`data:text/javascript;base64,${Buffer.from(compile(readFileSync("src/features/remote-bridge/readiness-presentation.ts", "utf8"))).toString("base64")}`);
const page = readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue", "utf8");
const section = (start, end) => compile(page.slice(page.indexOf(start), page.indexOf(end, page.indexOf(start))));
const ref = value => ({ value });
const ready = () => ({ status: "connected", target: { id: "lab", canOpenVscode: true }, proxy: {}, cc: {}, postConnectStatus: "ready", sessionEnvironmentState: "ready", vscodeState: "ready", codexState: "ready", claudeState: "ready", diagnostics: { generalProxyEgress: { state: "passed" }, serverDirect: { state: "failed" }, aiRouteVerification: { state: "notTested" } } });

test("readiness projects applicable capabilities without changing the snapshot", () => {
  const summary = ready();
  const before = JSON.stringify(summary);
  assert.equal(remoteReadiness(summary).state, "ready");
  assert.deepEqual(remoteReadiness(summary).items.map(item => item.id), ["terminal", "vscode", "tools", "egress"]);
  assert.equal(JSON.stringify(summary), before);
  assert.deepEqual(remoteReadiness({ ...summary, proxy: null }).items.map(item => item.id), ["vscode", "tools"]);
  assert.deepEqual(remoteReadiness({ ...summary, cc: null }).items.map(item => item.id), ["terminal", "vscode", "egress"]);
  assert.deepEqual(remoteReadiness({ ...summary, proxy: null, cc: null, target: { canOpenVscode: false } }).items, []);
});

test("missing CLI, offline direct internet and unrequested model verification are neutral", () => {
  const summary = { ...ready(), toolInstallations: [{ tool: "codex", state: "missing", extensionState: "installed" }, { tool: "claude", state: "missing", extensionState: "missing" }] };
  assert.equal(remoteReadiness(summary).state, "ready");
  assert.equal(remoteReadiness({ ...summary, toolInstallations: [{ state: "installed", extensionState: "checking" }] }).state, "preparing");
});

test("preparing takes priority, then actionable warnings; proxy egress is independent", () => {
  for (const field of ["sessionEnvironmentState", "vscodeState", "codexState", "claudeState"]) {
    assert.equal(remoteReadiness({ ...ready(), [field]: "preparing" }).state, "preparing");
    assert.equal(remoteReadiness({ ...ready(), [field]: "warning" }).state, "attention");
  }
  for (const state of ["notTested", "failed"]) assert.equal(remoteReadiness({ ...ready(), diagnostics: { generalProxyEgress: { state } } }).state, "attention");
  assert.equal(remoteReadiness({ ...ready(), diagnostics: { generalProxyEgress: { state: "testing" } }, vscodeState: "warning" }).state, "preparing");
  assert.equal(remoteReadiness({ ...ready(), skillsState: "warning" }).state, "attention");
  assert.equal(remoteReadiness({ ...ready(), postConnectStatus: "preparing", diagnostics: undefined, sessionEnvironmentState: undefined }).state, "preparing");
});

test("Overview and Advanced do not introduce an aggregate optional-preparation warning", () => {
  assert.doesNotMatch(page, /RemoteBridgeReadinessNotice|readinessVisible/);
});

test("core completion releases the dialog and launch controls without awaiting optional jobs", async t => {
  const auth = mountConnection(t, {});
  auth.connectionPhase.value = "building";
  auth.busy.value = true;
  auth.authSubmitting.value = true;
  auth.prepareAuthPreview({ sessionId: "old" });
  auth.authResponse.value = "secret-fixture";
  const summary = ready();
  await auth.finishConnectionUi(summary);
  assert.deepEqual(auth.calls, [["connected", summary], ["close"], ["focus"]]);
  assert.equal(auth.connectionPhase.value, "idle");
  assert.equal(auth.busy.value, false);
  assert.equal(auth.advancedView.value, false);
  assert.equal(auth.authResponse.value, "");
  assert.equal(auth.authSession.value, undefined);
});

test("cancellation isolates both late finish and late password submission", async t => {
  const snapshot = { sessionId: "old", status: "succeeded" };
  let resolveFinish;
  let markFinishing;
  const finishing = new Promise(resolve => { markFinishing = resolve; });
  const auth = mountInteractiveAuth(t, {
    backend: {
      sshAuthBegin: async () => ({ sessionId: "old", status: "waitingPrompt" }),
      sshAuthState: async () => snapshot,
      sshAuthFinish: () => new Promise(resolve => {
        resolveFinish = resolve;
        markFinishing();
      }),
    },
    finishConnectionUi() { assert.fail("stale connection"); },
    refresh() { assert.fail("stale refresh"); },
  });
  await auth.beginInteractiveAuth("check");
  const finish = auth.runTimer();
  await finishing;
  await auth.cancelInteractiveAuth();
  resolveFinish({ operation: "check", ports: {} });
  await finish;
  assert.equal(auth.connectionPhase.value, "idle");
  assert.deepEqual(auth.delays(), []);

  let resolveSubmit;
  const submitAuth = mountInteractiveAuth(t, {
    backend: { sshAuthSubmit: () => new Promise(resolve => { resolveSubmit = resolve; }) },
  });
  submitAuth.authSession.value = { sessionId: "old", status: "waitingUser", prompt: { id: "password", type: "password", secret: true } };
  submitAuth.authResponse.value = "secret-fixture";
  const submit = submitAuth.submitInteractiveAuth();
  assert.equal(submitAuth.authResponse.value, "");
  await submitAuth.cancelInteractiveAuth();
  resolveSubmit(snapshot);
  await submit;
  assert.equal(submitAuth.authSession.value, undefined);
  assert.equal(submitAuth.error.value, undefined);
});

test("one success notification per explicit connect; optional warnings stay in existing statuses", () => {
  const context = { watch: (_, callback) => { context.callback = callback; }, props: { summary: { ...ready(), reconnectState: "idle" }, copy: { rbReadinessReadyNotification: "ready" } }, readiness: ref({ state: "preparing" }), connectionPhase: ref("idle"), readinessAnnouncementTarget: ref("lab"), notify: (...args) => context.notices.push(args), notices: [] };
  const start = page.indexOf("watch([() => props.summary.status, () => props.summary.target?.id, () => readiness.value.state");
  runInNewContext(compile(page.slice(start, page.indexOf("const newConnection", start))), context);
  context.callback();
  assert.equal(context.notices.length, 0);
  context.readiness.value.state = "ready";
  context.callback(); context.callback();
  assert.equal(context.notices.length, 1);
  assert.equal(context.notices[0][1], "success");
  context.readinessAnnouncementTarget.value = "lab";
  context.props.summary.reconnectState = "retrying";
  context.callback();
  assert.equal(context.notices.length, 1);
  context.props.summary.reconnectState = "idle";
  context.readiness.value.state = "attention";
  context.callback(); context.callback();
  assert.equal(context.notices.length, 1);
  assert.equal(context.readinessAnnouncementTarget.value, undefined);
});

test("localized readiness copy and launch safety use existing boundaries", async () => {
  const { remoteBridgeMessages } = await import(copyModuleUrl);
  for (const copy of Object.values(remoteBridgeMessages)) assert.ok(copy.rbReadinessReadyNotification);
  const launch = readFileSync("src/features/remote-bridge/components/BridgeLaunchBar.vue", "utf8");
  assert.doesNotMatch(launch, /postConnectStatus|readiness/);
  assert.doesNotMatch(page, /function connect\(\)/);
  assert.doesNotMatch(page, /RemoteBridgeReadinessNotice/);
});

test("cancelling any auth prompt releases controls; an old cancellation cannot reset a newer check", async t => {
  for (const status of ["waitingPrompt", "waitingUser", "promptUnavailable", "failed"]) {
    let finishCancel;
    const auth = mountInteractiveAuth(t, {
      connectionPhase: ref("authenticating"), busy: ref(true),
      backend: { sshAuthCancel: () => new Promise(resolve => { finishCancel = resolve; }) },
    });
    auth.prepareAuthPreview({ sessionId: "old", status });
    auth.authSubmitting.value = true;
    auth.authResponse.value = "secret-fixture";
    auth.showAuthDiagnostic.value = true;
    const cancelled = auth.cancelInteractiveAuth();
    assert.equal(auth.busy.value, false);
    assert.equal(auth.authSubmitting.value, false);
    assert.equal(auth.networkCheckPending.value, false);
    assert.equal(auth.authSession.value, undefined);
    assert.equal(auth.authResponse.value, "");
    assert.equal(auth.authInputValue.value, "");
    assert.equal(auth.readinessAnnouncementTarget.value, undefined);
    auth.advanceAuthRevision();
    auth.connectionPhase.value = "checking";
    finishCancel();
    await cancelled;
    assert.equal(auth.sshCheck.value.state, "checking");
  }
});

test("connected CC detection refreshes its observation without changing selected capabilities", () => {
  const context = { props: { ccDetection: { state: "confirmed", localPort: 15722 }, reviewPreview: false }, proxyAvailable: ref(true), proxy: ref(true), proxyPreferred: ref(true), live: ref(true), establishingBridge: ref(false), ccCheck: ref({ state: "checking" }), cc: ref(false), ccLocalPort: ref(15721), ccPreferred: ref(false), Date };
  runInNewContext(section("function initializeCapabilities(", "function rememberCcChoice("), context);
  context.initializeCapabilities(false);
  assert.equal(context.ccCheck.value.state, "healthy");
  assert.equal(context.cc.value, false);
  assert.equal(context.ccLocalPort.value, 15721);
  assert.match(page, /:disabled="busy \|\| diagnosing \|\| !sshConnected" :aria-busy="diagnosing" @click="runDiagnostics"/);
  assert.doesNotMatch(page, /networkChecking/);
});

test("check-to-connect authentication keeps the same dialog, prompt and password mask", async t => {
  const snapshot = { sessionId: "check", status: "waitingUser", prompt: { id: "password", type: "password", message: "Password:", secret: true } };
  const ports = { proxyPort: 7897, ccPort: 15721 };
  const selected = { targetId: "lab" };
  const calls = [];
  const auth = mountInteractiveAuth(t, {
    dialog: { isOpen: () => true, show() { assert.fail("dialog reopened"); } },
    backend: {
      sshAuthFinish: async () => ({ operation: "check", ports }),
      sshAuthBegin: async (operation, targetId, request) => {
        calls.push(["begin", operation, targetId, request]);
        return operation === "check" ? snapshot : { sessionId: "connect", status: "starting" };
      },
      sshAuthSubmit: async () => snapshot,
      sshAuthState: async () => ({ sessionId: "check", status: "succeeded", prompt: null }),
    },
    usePorts: actual => calls.push(["ports", actual]),
    request: () => selected,
    connectPrepared: async request => { calls.push(["connect", request]); throw "sshAuth"; },
    isInteractiveAuthError: error => error === "sshAuth",
  });
  await auth.beginInteractiveAuth("check");
  auth.authResponse.value = "secret-fixture";
  await auth.submitInteractiveAuth();
  await auth.runTimer();
  assert.equal(auth.authSession.value.sessionId, "connect");
  assert.equal(auth.visibleAuthPrompt.value.message, "Password:");
  assert.equal(auth.authInputValue.value, "••••••••");
  assert.equal(auth.dialog.isOpen(), true);
  assert.equal(auth.error.value, undefined);
  assert.deepEqual(calls, [["begin", "check", "lab", null], ["ports", ports], ["connect", selected], ["begin", "connect", "lab", selected]]);
  assert.deepEqual(auth.delays(), [120]);
});

function deferred() {
  let resolve, reject;
  const pending = new Promise((onResolve, onReject) => { resolve = onResolve; reject = onReject; });
  return { pending, resolve, reject };
}

const initialPorts = { proxyPort: 7897, ccPort: 15721 };
const fallbackPorts = { proxyPort: 23841, ccPort: 31472 };
const flushConnection = () => new Promise(resolve => setImmediate(resolve));

test("Escape during authenticated preview prevents dispatching connect", async t => {
  const previewStarted = deferred();
  const preview = deferred();
  const calls = [];
  const state = mountConnection(t, {
    preview(request) {
      calls.push(["preview", request.targetId]);
      previewStarted.resolve();
      return preview.pending;
    },
    async connect(request) { calls.push(["connect", request.targetId]); return ready(); },
  }, {
    authBackend: {
      async sshAuthBegin() { return { sessionId: "fixture", status: "waitingPrompt" }; },
      async sshAuthState(sessionId) { return { sessionId, status: "succeeded" }; },
      async sshAuthFinish() { return { operation: "check", ports: initialPorts }; },
    },
  });
  await state.beginInteractiveAuth("check");
  const attempt = state.runTimer();
  await previewStarted.pending;
  await state.cancelInteractiveAuth();
  preview.resolve();
  await attempt;
  assert.deepEqual(calls, [["preview", "lab"]]);
  assert.equal(state.connectionPhase.value, "idle");
  assert.equal(state.busy.value, false);
  assert.deepEqual(state.calls, []);
});

test("changing targets during fallback cannot use the old allocation even after selecting the original target again", async t => {
  for (const switchBack of [false, true]) {
    const allocated = deferred();
    const allocationStarted = deferred();
    const calls = [];
    const state = mountConnection(t, {
      async check(target) { calls.push(["check", target]); return initialPorts; },
      async preview(request) { calls.push(["preview", request.targetId]); throw "portInUse"; },
      allocatePorts(target, preferDefaults) {
        calls.push(["allocate", target, preferDefaults]);
        allocationStarted.resolve();
        return allocated.pending;
      },
      async connect(request) { calls.push(["connect", request.targetId]); return ready(); },
    });
    state.startConnection();
    await allocationStarted.pending;
    state.targetId.value = "replacement";
    if (switchBack) state.targetId.value = "lab";
    allocated.resolve(fallbackPorts);
    await flushConnection();
    assert.deepEqual(calls, [["check", "lab"], ["preview", "lab"], ["allocate", "lab", false]]);
    assert.equal(state.proxyPort.value, initialPorts.proxyPort);
    assert.equal(state.connectionPhase.value, "idle");
    assert.equal(state.busy.value, false);
  }
});

test("first connect and one port fallback preserve the selected target, capability and proxy revision", async t => {
  for (const [proxy, cc, conflict] of [[true, false, false], [false, true, true], [true, true, true]]) {
    const calls = [];
    const summary = ready();
    const state = mountConnection(t, {
      async check(id) { calls.push(["check", id]); return initialPorts; },
      async preview(request) { calls.push(["preview", { ...request }]); },
      async connect(request) {
        calls.push(["connect", { ...request }]);
        if (conflict && calls.length === 3) throw "portInUse";
        return summary;
      },
      async allocatePorts(id, defaults) { calls.push(["allocate", id, defaults]); return fallbackPorts; },
    }, { proxy, cc });
    state.startConnection();
    state.startConnection();
    await flushConnection();
    const request = ports => ({
      targetId: "lab", proxyPort: proxy ? ports.proxyPort : null, ccPort: cc ? ports.ccPort : null,
      ccLocalPort: 15721, expectedRevision: 17,
    });
    const initial = request(initialPorts);
    const retry = request(fallbackPorts);
    assert.deepEqual(calls, [
      ["check", "lab"], ["preview", initial], ["connect", initial],
      ...(conflict ? [["allocate", "lab", false], ["preview", retry], ["connect", retry]] : []),
    ]);
    assert.deepEqual(state.calls, [["connected", summary], ["close"], ["focus"], ["refresh"]]);
    assert.equal(state.busy.value, false);
  }
});

test("stale preview errors cannot retry and ordinary failures propagate without allocation", async t => {
  for (const cancel of [false, true]) {
    const preview = deferred();
    const previewStarted = deferred();
    const state = mountConnection(t, {
      async check() { return initialPorts; },
      preview() { previewStarted.resolve(); return preview.pending; },
      connect() { assert.fail("failed preview cannot connect"); },
      allocatePorts() { assert.fail("failed or stale preview cannot allocate"); },
    });
    state.startConnection();
    await previewStarted.pending;
    if (cancel) await state.cancelInteractiveAuth();
    preview.reject(cancel ? "portInUse" : "sshFailed");
    await flushConnection();
    assert.equal(state.error.value, cancel ? undefined : "sshFailed");
    assert.equal(state.connectionPhase.value, cancel ? "idle" : "failed");
    assert.equal(state.busy.value, false);
  }
});

test("interactive check uses the real prepared connection before handing off to connect authentication", async t => {
  const calls = [];
  const state = mountConnection(t, {
    async check(id) { calls.push(["check", id]); throw "sshAuth"; },
    async preview(request) { calls.push(["preview", request.targetId]); },
    async connect(request) { calls.push(["connect", request.targetId]); throw "sshAuth"; },
  }, {
    authBackend: {
      async sshAuthBegin(operation, id, request) {
        calls.push(["auth", operation, id, request?.proxyPort ?? null]);
        return { sessionId: operation, status: "waitingPrompt" };
      },
      async sshAuthState(sessionId) { return { sessionId, status: "succeeded" }; },
      async sshAuthFinish() { return { operation: "check", ports: initialPorts }; },
    },
  });
  state.startConnection();
  await flushConnection();
  await state.runTimer();
  assert.deepEqual(calls, [
    ["check", "lab"], ["auth", "check", "lab", null],
    ["preview", "lab"], ["connect", "lab"], ["auth", "connect", "lab", initialPorts.proxyPort],
  ]);
  assert.equal(state.authSession.value.sessionId, "connect");
  assert.deepEqual(state.delays(), [120]);
  assert.deepEqual(state.calls, []);
});

test("fallback preview failure preserves the error and cancelling it prevents the retry connect", async t => {
  for (const cancelled of [false, true]) {
    const fallback = deferred();
    const fallbackStarted = deferred();
    const calls = [];
    const state = mountConnection(t, {
      async check() { return initialPorts; },
      async preview(request) {
        calls.push(["preview", request.proxyPort]);
        if (request.proxyPort === initialPorts.proxyPort) return;
        fallbackStarted.resolve();
        await fallback.pending;
      },
      async connect(request) { calls.push(["connect", request.proxyPort]); throw "portInUse"; },
      async allocatePorts() { return fallbackPorts; },
    });
    state.startConnection();
    await fallbackStarted.pending;
    if (cancelled) {
      await state.cancelInteractiveAuth();
      fallback.resolve();
    } else {
      fallback.reject("portInUse");
    }
    await flushConnection();
    assert.deepEqual(calls, [["preview", 7897], ["connect", 7897], ["preview", 23841]]);
    assert.equal(state.error.value, cancelled ? undefined : "portInUse");
    assert.equal(state.busy.value, false);
  }
});

test("disabled and review-only starts avoid IPC; manual reconnect shares the same busy state", async t => {
  const deniedBackend = new Proxy({}, { get() { assert.fail("no IPC for disabled or review-only starts"); } });
  const summary = ready();
  const review = mountConnection(t, deniedBackend, { props: { reviewPreview: true, summary }, canStart: false });
  review.startConnection();
  await flushConnection();
  assert.deepEqual(review.calls, []);
  review.canStartConnection.value = true;
  review.startConnection();
  await flushConnection();
  assert.deepEqual(review.calls, [["connected", summary], ["close"], ["focus"], ["refresh"]]);
  assert.equal(review.proxyPort.value, 23841);

  const reconnect = deferred();
  let requests = 0;
  const state = mountConnection(t, { retryReconnect() { requests++; return reconnect.pending; } });
  const pending = state.reconnectManually();
  state.startConnection();
  await state.reconnectManually();
  assert.equal(state.busy.value, true);
  reconnect.resolve(summary);
  await pending;
  assert.equal(requests, 1);
  assert.equal(state.busy.value, false);
  assert.deepEqual(state.calls, [["connected", summary], ["refresh"]]);
});

test("late begin cancels only the old SSH session and leaves the current timer in place", async t => {
  const oldBegin = deferred();
  const started = deferred();
  const cancelled = [];
  const selected = ref("first");
  const auth = mountInteractiveAuth(t, {
    targetId: selected,
    backend: {
      sshAuthBegin: async (_operation, target) => {
        if (target !== "first") return { sessionId: "current", status: "waitingPrompt" };
        started.resolve();
        return oldBegin.pending;
      },
      sshAuthCancel: async id => { cancelled.push(id); },
    },
  });
  const stale = auth.beginInteractiveAuth("check");
  await started.pending;
  selected.value = "second";
  await auth.beginInteractiveAuth("check");
  oldBegin.resolve({ sessionId: "obsolete", status: "waitingPrompt" });
  await stale;
  assert.deepEqual(cancelled, ["obsolete"]);
  assert.equal(auth.authSession.value.sessionId, "current");
  assert.deepEqual(auth.delays(), [120]);
});

test("begin failure closes only its own attempt and releases no polling timer", async t => {
  const oldBegin = deferred();
  const started = deferred();
  let attempt = 0;
  let closes = 0;
  const auth = mountInteractiveAuth(t, {
    dialog: { close() { closes++; } },
    backend: {
      sshAuthBegin: async () => {
        attempt++;
        if (attempt === 1) {
          started.resolve();
          return oldBegin.pending;
        }
        if (attempt === 2) return { sessionId: "current", status: "waitingPrompt" };
        throw Error("current begin failed");
      },
    },
  });
  const stale = auth.beginInteractiveAuth("check");
  const rejected = assert.rejects(stale, /old begin failed/);
  await started.pending;
  await auth.beginInteractiveAuth("check");
  oldBegin.reject(Error("old begin failed"));
  await rejected;
  assert.equal(closes, 0);
  assert.equal(auth.error.value, undefined);
  assert.equal(auth.authSession.value.sessionId, "current");
  await assert.rejects(auth.beginInteractiveAuth("check"), /current begin failed/);
  assert.equal(closes, 1);
  assert.equal(auth.connectionPhase.value, "failed");
  assert.match(auth.error.value.message, /current begin failed/);
  assert.deepEqual(auth.delays(), []);
});

test("SSH polling uses the original delays and stops for missing prompts or terminal failures", async t => {
  for (const terminal of ["promptUnavailable", "failed"]) {
    let refreshes = 0;
    const states = ["waitingServer", "waitingUser", terminal];
    const auth = mountInteractiveAuth(t, {
      busy: ref(true),
      refresh() { refreshes++; },
      backend: {
        sshAuthBegin: async () => ({ sessionId: "pty", status: "waitingPrompt" }),
        sshAuthState: async id => ({ sessionId: id, status: states.shift(), prompt: null }),
      },
    });
    await auth.beginInteractiveAuth("check");
    assert.deepEqual(auth.delays(), [120]);
    await auth.runTimer();
    assert.equal(auth.authSession.value.status, "waitingServer");
    assert.deepEqual(auth.delays(), [180]);
    await auth.runTimer();
    assert.equal(auth.authSession.value.status, "waitingUser");
    assert.deepEqual(auth.delays(), [180]);
    await auth.runTimer();
    assert.equal(auth.authSession.value.status, terminal);
    assert.deepEqual(auth.delays(), []);
    assert.equal(refreshes, terminal === "failed" ? 1 : 0);
    if (terminal === "failed") {
      assert.equal(auth.connectionPhase.value, "failed");
      assert.equal(auth.busy.value, false);
    }
  }
});

test("host confirmation is explicit and keyboard replies are removed before IPC", async t => {
  const host = { sessionId: "pty", status: "waitingUser", prompt: { id: "host", type: "hostKeyConfirmation", secret: false } };
  const keyboard = { sessionId: "pty", status: "waitingUser", prompt: { id: "keyboard", type: "keyboardInteractive", secret: true } };
  const received = [];
  const auth = mountInteractiveAuth(t, {
    backend: {
      sshAuthBegin: async () => host,
      sshAuthState: async () => host,
      sshAuthConfirmHost: async (...args) => { received.push(["confirm", ...args]); return keyboard; },
      sshAuthSubmit: async (...args) => {
        assert.equal(auth.authResponse.value, "");
        assert.equal(auth.authInputValue.value, "••••••••");
        received.push(["submit", ...args]);
        return { sessionId: "pty", status: "submitting", prompt: null };
      },
    },
  });
  await auth.beginInteractiveAuth("check");
  await auth.runTimer();
  assert.deepEqual(received, [], "discovery of a host prompt may not confirm it");
  await auth.submitInteractiveAuth();
  await auth.submitInteractiveAuth();
  assert.deepEqual(received, [["confirm", "pty", "host"]], "a keyboard reply cannot be sent empty");
  auth.authResponse.value = "fixture-keyboard-response";
  await auth.submitInteractiveAuth();
  assert.deepEqual(received, [["confirm", "pty", "host"], ["submit", "pty", "keyboard", "fixture-keyboard-response"]]);
  assert.equal(auth.authResponse.value, "");
});

test("an SSH response with the same revision still cannot overwrite a different session", async t => {
  const response = deferred();
  const auth = mountInteractiveAuth(t, { backend: { sshAuthSubmit: () => response.pending } });
  auth.authSession.value = { sessionId: "old", status: "waitingUser", prompt: { id: "pass", secret: true } };
  auth.authResponse.value = "fixture-password";
  const pending = auth.submitInteractiveAuth();
  auth.prepareAuthPreview({ sessionId: "replacement", status: "waitingPrompt" });
  response.resolve({ sessionId: "old", status: "failed" });
  await pending;
  assert.equal(auth.authSession.value.sessionId, "replacement");
  assert.equal(auth.authResponse.value, "");
});

test("an old SSH retry cannot reopen the dialog after cancellation and a new start", async t => {
  const cancellations = [];
  const beginnings = [];
  const auth = mountInteractiveAuth(t, {
    backend: {
      sshAuthBegin: async operation => {
        beginnings.push(operation);
        return { sessionId: operation, status: "waitingPrompt" };
      },
      sshAuthCancel: id => {
        const call = deferred();
        cancellations.push([id, call]);
        return call.pending;
      },
    },
  });
  await auth.beginInteractiveAuth("check");
  const retry = auth.retryInteractiveAuth();
  assert.deepEqual(auth.delays(), []);
  const cancel = auth.cancelInteractiveAuth();
  assert.deepEqual(cancellations.map(([id]) => id), ["check", "check"]);
  cancellations[1][1].resolve();
  await cancel;
  await auth.beginInteractiveAuth("connect");
  cancellations[0][1].resolve();
  await retry;
  assert.deepEqual(beginnings, ["check", "connect"]);
  assert.equal(auth.authSession.value.sessionId, "connect");
  assert.equal(auth.authSubmitting.value, false);
  assert.deepEqual(auth.delays(), [120]);
});

test("detaching one SSH auth instance retires its pending poll and clears input without touching its peer", async t => {
  const state = deferred();
  const auth = mountInteractiveAuth(t, {
    backend: {
      sshAuthBegin: async () => ({ sessionId: "old", status: "waitingUser" }),
      sshAuthState: () => state.pending,
      sshAuthFinish() { assert.fail("a detached poll cannot finish authentication"); },
    },
  });
  const peer = mountInteractiveAuth(t, {
    backend: { sshAuthBegin: async () => ({ sessionId: "peer", status: "waitingPrompt" }) },
  });
  await auth.beginInteractiveAuth("check");
  const poll = auth.runTimer();
  await peer.beginInteractiveAuth("check");
  auth.authResponse.value = "fixture-password";
  assert.equal(auth.detachInteractiveAuth(), "old");
  auth.scope.stop();
  state.resolve({ sessionId: "old", status: "succeeded" });
  await poll;
  assert.equal(auth.authResponse.value, "");
  assert.equal(auth.authSession.value, undefined);
  assert.equal(auth.visibleAuthPrompt.value, null);
  assert.deepEqual(auth.delays(), []);
  assert.equal(peer.authSession.value.sessionId, "peer");
  assert.deepEqual(peer.delays(), [120]);
});

test("successful direct interactive connect hands off exactly once and stops polling", async t => {
  const calls = [];
  const summary = ready();
  const selected = { targetId: "lab" };
  const auth = mountInteractiveAuth(t, {
    backend: {
      sshAuthBegin: async (...args) => { calls.push(args); return { sessionId: "connect", status: "waitingPrompt" }; },
      sshAuthState: async id => ({ sessionId: id, status: "succeeded" }),
      sshAuthFinish: async id => { calls.push(["finish", id]); return { operation: "connect", summary }; },
    },
    finishConnectionUi: async result => { calls.push(["completed", result]); },
    refresh() { calls.push(["refresh"]); },
    connectPrepared() { assert.fail("direct authentication must not start a second connection"); },
  });
  await auth.beginInteractiveAuth("connect", selected);
  await auth.runTimer();
  assert.deepEqual(calls, [["connect", "lab", selected], ["finish", "connect"], ["completed", summary], ["refresh"]]);
  assert.deepEqual(auth.delays(), []);
});


test("reconnection and external disconnect disarm first-connect completion feedback", () => {
  const context = { watch: (_, callback) => { context.callback = callback; }, props: { summary: ready() }, readinessAnnouncementTarget: ref("lab") };
  runInNewContext(section("watch([() => props.summary.status, () => props.summary.reconnectState]", "const live = computed"), context);
  for (const reconnect of ["waiting", "retrying", "attentionRequired"]) {
    context.readinessAnnouncementTarget.value = "lab";
    context.callback(["connected", reconnect], ["connected", "idle"]);
    assert.equal(context.readinessAnnouncementTarget.value, undefined);
  }
  context.readinessAnnouncementTarget.value = "lab";
  context.callback(["disconnected", "idle"], ["connected", "idle"]);
  assert.equal(context.readinessAnnouncementTarget.value, undefined);
});

test("manual diagnostics executes once and releases the button on success or failure", async () => {
  for (const fail of [false, true]) {
    let finish;
    const context = { busy: ref(false), error: ref(), vscodeSetupWarning: ref(), feedback: ref(), diagnosing: ref(false), calls: 0, refreshes: 0,
      remoteBackend: { runDiagnostics: () => { context.calls++; return new Promise((resolve, reject) => { finish = () => fail ? reject("network") : resolve({}); }); } },
      emit: () => { context.refreshes++; },
    };
    runInNewContext(section("async function perform(", "function updateAuthResponse("), context);
    runInNewContext(section("function runDiagnostics()", "function copyValue("), context);
    context.runDiagnostics(); context.runDiagnostics();
    assert.equal(context.calls, 1);
    assert.equal(context.diagnosing.value, true);
    finish();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(context.busy.value, false);
    assert.equal(context.diagnosing.value, false);
    assert.equal(context.error.value, fail ? "network" : undefined);
    assert.equal(context.refreshes, 1);
  }
});
