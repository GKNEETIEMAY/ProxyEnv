import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";
import { copyModuleUrl } from "./remote-tool-test-modules.mjs";

const compile = source => ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }).outputText;
const { remoteNavigationState } = await import(`data:text/javascript;base64,${Buffer.from(compile(readFileSync("src/app/navigation.ts", "utf8"))).toString("base64")}`);
const { remoteBridgeMessages } = await import(copyModuleUrl);
const shell = readFileSync("src/app/AppShell.vue", "utf8");
const header = readFileSync("src/app/components/AppHeader.vue", "utf8");
const remote = readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue", "utf8");
const ref = value => ({ value });

test("remote navigation projects transport with attention > connecting > connected > idle priority", () => {
  for (const [summary, expected] of [
    [{ status: "disconnected" }, "idle"], [{ status: "connecting" }, "connecting"],
    [{ status: "connected", reconnectState: "idle" }, "connected"],
    ...["waiting", "retrying"].map(reconnectState => [{ status: "connected", reconnectState }, "connecting"]),
    ...["stale", "unavailable", "error"].map(status => [{ status, reconnectState: "retrying" }, "attention"]),
    [{ status: "connecting", reconnectState: "attentionRequired" }, "attention"],
  ]) assert.equal(remoteNavigationState(summary), expected);
});

test("optional preparation and diagnostic outcomes never change the healthy navigation state", () => {
  for (const postConnectStatus of ["idle", "preparing", "ready", "partial"]) {
    const summary = { status: "connected", reconnectState: "idle", postConnectStatus,
      codexState: "warning", claudeState: "warning", skillsState: "warning",
      toolInstallations: [{ tool: "codex", state: "missing" }],
      diagnostics: { serverInternet: "unreachable", generalProxyEgress: { state: "failed" }, aiRouteVerification: { state: "notTested" } } };
    const before = JSON.stringify(summary);
    assert.equal(remoteNavigationState(summary), "connected");
    assert.equal(JSON.stringify(summary), before);
  }
});

test("contextual Settings and Assistant return to the selected primary workspace", () => {
  const context = { view: ref("local"), primaryView: ref("local"), remoteBridgeSummary: ref({}) };
  runInNewContext(compile(shell.slice(shell.indexOf("function openSettings()"), shell.indexOf("async function copyEndpoint("))), context);
  for (const workspace of ["local", "remote"]) {
    context[workspace === "local" ? "openLocal" : "openRemote"]();
    context.openSettings();
    assert.equal(context.view.value, "settings");
    assert.equal(context.primaryView.value, workspace);
    context.closeSettings();
    assert.equal(context.view.value, workspace);
  }
  context.openLocal(); context.openAssistant(); context.closeSettings();
  assert.equal(context.view.value, "local");
  context.acceptRemoteBridgeSummary({ status: "connected" });
  assert.equal(context.view.value, "local", "A background bridge summary does not navigate");
});

test("keyboard navigation respects modal ownership and editable fields", () => {
  class Input {}
  const context = { view: ref("settings"), primaryView: ref("remote"), modal: false,
    document: { querySelector: () => context.modal ? {} : null },
    HTMLInputElement: Input, HTMLSelectElement: class {}, HTMLTextAreaElement: class {},
    closeSettings: () => { context.view.value = context.primaryView.value; }, openSettings: () => { context.view.value = "settings"; } };
  runInNewContext(compile(shell.slice(shell.indexOf("function onViewShortcut("), shell.indexOf("async function minimizeWindow("))), context);
  const key = (key, extra = {}) => ({ key, target: {}, defaultPrevented: false, preventDefault() { this.prevented = true; }, ...extra });
  context.modal = true; context.onViewShortcut(key("Escape"));
  assert.equal(context.view.value, "settings");
  context.modal = false; context.onViewShortcut(key("Escape", { target: new Input() }));
  assert.equal(context.view.value, "settings");
  context.onViewShortcut(key("Escape")); assert.equal(context.view.value, "remote");
  context.onViewShortcut(key(",", { ctrlKey: true })); assert.equal(context.view.value, "settings");
});

test("page hierarchy stays frozen without Router or remote component remounting", () => {
  assert.match(shell, /const primaryView = ref<"local" \| "remote">\("local"\)/);
  assert.match(shell, /const view = ref<"local" \| "remote" \| "assistant" \| "settings">\("local"\)/);
  assert.match(shell, /<div v-show="view === 'remote'"[^>]*>[\s\S]*?<RemoteBridgePage/);
  assert.match(shell, /:remote-nav-state="remoteNavState"/);
  assert.doesNotMatch(header, /BridgeSummary|remoteBackend/);
  const nav = header.slice(header.indexOf("<nav "), header.indexOf("</nav>"));
  assert.equal((nav.match(/<button /g) ?? []).length, 2);
  assert.match(nav, /aria-hidden="true"/);
  assert.match(nav, /:aria-label="`\$\{copy.navRemote\} · \$\{remoteNavStatusLabel\}`"/);
  assert.match(header, /\[data-state="idle"\] \{ visibility:hidden/);
  assert.doesNotMatch(header, /animation:|pulse|@keyframes/);
  assert.match(remote, /<div v-if="live" class="remote-workspace-heading">[\s\S]*?class="remote-view-switch"/);
  assert.doesNotMatch(readFileSync("package.json", "utf8"), /vue-router/);
});

test("four locales keep workspace names and localized transport descriptions", () => {
  const expected = { "zh-CN": ["本机环境", "远程桥接"], en: ["Local environment", "Remote bridge"], ja: ["ローカル環境", "リモートブリッジ"], ko: ["로컬 환경", "원격 브리지"] };
  for (const [locale, labels] of Object.entries(expected)) {
    const copy = remoteBridgeMessages[locale];
    assert.deepEqual([copy.navLocal, copy.navRemote], labels);
    for (const key of ["rbNavDisconnected", "rbNavConnecting", "rbNavConnected", "rbNavAttention"]) assert.ok(copy[key]);
    assert.equal(new Set([copy.rbNavDisconnected, copy.rbNavConnecting, copy.rbNavConnected, copy.rbNavAttention]).size, 4);
  }
});
