import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";
import { copyModuleUrl } from "./remote-tool-test-modules.mjs";

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

test("core completion releases the dialog and launch controls without awaiting optional jobs", async () => {
  const context = {
    emit: (...args) => context.events.push(args), events: [], advancedView: ref(true), connectionPhase: ref("building"), busy: ref(true), authSubmitting: ref(true),
    authDialog: ref({ close: () => { context.closed = true; } }), authSession: ref({}), lastAuthPrompt: ref({}), authResponse: ref("secret-fixture"), submittedAuthMask: ref("masked"), submittedAuthPromptId: ref("prompt"), authRetryContext: ref({}),
    refreshNetworkChecks: () => new Promise(() => {}), nextTick: async () => {}, connectedHeading: ref({ focus: () => { context.focused = true; } }),
  };
  runInNewContext(section("async function finishConnectionUi(", "async function completeInteractiveAuth("), context);
  await context.finishConnectionUi(ready());
  assert.equal(context.events[0][0], "connected");
  assert.equal(context.connectionPhase.value, "idle");
  assert.equal(context.busy.value, false);
  assert.equal(context.closed, true);
  assert.equal(context.focused, true);
  assert.equal(context.advancedView.value, false);
  assert.equal(context.authResponse.value, "");
  assert.equal(context.authSession.value, undefined);
});

test("cancellation isolates both late finish and late password submission", async () => {
  let resolveFinish;
  const snapshot = { sessionId: "old" };
  const context = { authBeginRevision: 1, authSession: ref(snapshot), connectionPhase: ref("authenticating"), clearAuthPoll: () => {}, remoteBackend: { sshAuthFinish: () => new Promise(resolve => { resolveFinish = resolve; }) }, emit: () => { throw Error("stale emission"); } };
  runInNewContext(section("async function completeInteractiveAuth(", "async function pollInteractiveAuth("), context);
  const finish = context.completeInteractiveAuth(snapshot);
  context.authBeginRevision++;
  context.authSession.value = undefined;
  context.connectionPhase.value = "idle";
  resolveFinish({ operation: "check", ports: {} });
  await finish;
  assert.equal(context.connectionPhase.value, "idle");

  let resolveSubmit;
  Object.assign(context, { authSession: ref({ sessionId: "old", prompt: { id: "password" } }), authCanRespond: ref(true), authSubmitting: ref(false), authIsHostConfirmation: ref(false), authResponse: ref("secret-fixture"), submittedAuthMask: ref(""), submittedAuthPromptId: ref(), visibleAuthPrompt: ref({ secret: true }), error: ref(), remoteBackend: { sshAuthSubmit: () => new Promise(resolve => { resolveSubmit = resolve; }) } });
  runInNewContext(section("async function submitInteractiveAuth(", "async function retryInteractiveAuth("), context);
  const submit = context.submitInteractiveAuth();
  assert.equal(context.authResponse.value, "");
  context.authBeginRevision++;
  context.authSession.value = undefined;
  context.authSubmitting.value = false;
  resolveSubmit(snapshot);
  await submit;
  assert.equal(context.authSession.value, undefined);
  assert.equal(context.error.value, undefined);
});

test("one success notification per explicit connect; optional warnings stay in existing statuses", () => {
  const context = { watch: (_, callback) => { context.callback = callback; }, props: { summary: { ...ready(), reconnectState: "idle" }, copy: { rbReadinessReadyNotification: "ready" } }, readiness: ref({ state: "preparing" }), connectionPhase: ref("idle"), readinessAnnouncementTarget: ref("lab"), notify: (...args) => context.notices.push(args), notices: [] };
  const start = page.indexOf("watch([() => props.summary.status, () => props.summary.target?.id, () => readiness.value.state");
  runInNewContext(compile(page.slice(start, page.indexOf("const establishingBridge", start))), context);
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

test("cancelling any auth prompt releases controls; an old cancellation cannot reset a newer check", async () => {
  for (const status of ["waitingPrompt", "waitingUser", "promptUnavailable", "failed"]) {
    let finishCancel;
    const context = {
      clearAuthPoll: () => {}, authBeginRevision: 0, readinessAnnouncementTarget: ref("lab"), networkRefreshRevision: 0, networkCheckPending: ref(true), busy: ref(true), authSubmitting: ref(true),
      authSession: ref({ sessionId: "old", status }), lastAuthPrompt: ref({}), authResponse: ref("secret-fixture"), submittedAuthMask: ref("masked"), submittedAuthPromptId: ref("p"), showAuthDiagnostic: ref(true), authRetryContext: ref({}), connectionPhase: ref("authenticating"), authDialog: ref({ close: () => {} }), sshCheck: ref({ state: "checking" }),
      remoteBackend: { sshAuthCancel: () => new Promise(resolve => { finishCancel = resolve; }) }, nextTick: async () => {}, connectedHeading: ref(),
    };
    runInNewContext(section("async function cancelInteractiveAuth()", "async function load()"), context);
    const cancelled = context.cancelInteractiveAuth();
    assert.equal(context.busy.value, false);
    assert.equal(context.authSubmitting.value, false);
    assert.equal(context.networkCheckPending.value, false);
    assert.equal(context.authSession.value, undefined);
    assert.equal(context.authResponse.value, "");
    assert.equal(context.submittedAuthMask.value, "");
    assert.equal(context.readinessAnnouncementTarget.value, undefined);
    context.authBeginRevision++;
    context.connectionPhase.value = "checking";
    finishCancel();
    await cancelled;
    assert.equal(context.sshCheck.value.state, "checking");
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

test("check-to-connect authentication keeps the same dialog, prompt and password mask", async () => {
  const snapshot = { sessionId: "check", status: "succeeded" };
  const context = { authBeginRevision: 0, clearAuthPoll: () => {}, authSession: ref(snapshot), connectionPhase: ref("authenticating"), authDialog: ref({ open: true, showModal: () => { throw Error("dialog reopened"); } }), lastAuthPrompt: ref({ id: "password", message: "Password:" }), authResponse: ref(""), submittedAuthMask: ref("masked"), submittedAuthPromptId: ref("password"), error: ref(), showAuthDiagnostic: ref(false), authRetryContext: ref(), targetId: ref("lab"), checked: ref(false), sshCheck: ref({}), readinessAnnouncementTarget: ref("lab"), nextTick: async () => {},
    remoteBackend: { sshAuthFinish: async () => ({ operation: "check", ports: {} }), sshAuthBegin: async operation => ({ sessionId: operation, status: "starting" }) },
    usePorts: () => {}, request: () => ({}), connectPrepared: async () => { throw "sshAuth"; }, needsInteractiveSshAuth: error => error === "sshAuth", bridgeErrorCode: error => error,
    // Do not start real polling; this checks the handoff with the actual handlers.
    setTimeout: () => 1, authPollTimer: undefined, pollInteractiveAuth: () => {},
  };
  runInNewContext(section("async function beginInteractiveAuth(", "function updateAuthResponse("), context);
  runInNewContext(section("async function completeInteractiveAuth(", "async function pollInteractiveAuth("), context);
  await context.completeInteractiveAuth(snapshot);
  assert.equal(context.authSession.value.sessionId, "connect");
  assert.equal(context.lastAuthPrompt.value.message, "Password:");
  assert.equal(context.submittedAuthMask.value, "masked");
  assert.equal(context.authDialog.value.open, true);
  assert.equal(context.error.value, undefined);
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
    runInNewContext(section("async function perform(", "function clearAuthPoll()"), context);
    runInNewContext(section("function runDiagnostics()", "async function refreshSkills()"), context);
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
