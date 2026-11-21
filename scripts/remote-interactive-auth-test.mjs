import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";
import * as vue from "vue";

export const interactiveAuthPath = "src/features/remote-bridge/composables/useRemoteInteractiveAuth.ts";
const source = readFileSync(interactiveAuthPath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText;

export function mountInteractiveAuth(t, options = {}) {
  const timers = new Map();
  let timerId = 0;
  const exports = {};
  runInNewContext(compiled, {
    exports,
    require(name) {
      assert.equal(name, "vue");
      return vue;
    },
    setTimeout(callback, delay) {
      const id = ++timerId;
      timers.set(id, { callback, delay });
      return id;
    },
    clearTimeout(id) { timers.delete(id); },
  });
  const scope = vue.effectScope();
  t.after(() => scope.stop());
  const networkCheckPending = vue.ref(true);
  let dialogOpen = false;
  const settings = {
    targetId: vue.ref("lab"), connectionPhase: vue.ref("idle"), busy: vue.ref(false), checked: vue.ref(false),
    sshCheck: vue.ref({ state: "checking", checkedAt: null }), error: vue.ref(), readinessAnnouncementTarget: vue.ref("lab"),
    usePorts() {}, request: () => ({ targetId: "lab" }), isInteractiveAuthError: () => false,
    invalidateNetworkChecks() { networkCheckPending.value = false; }, refresh() {},
    ...options,
    backend: { sshAuthCancel: async () => {}, ...options.backend },
    dialog: {
      isOpen: () => dialogOpen,
      show() { dialogOpen = true; },
      close() { dialogOpen = false; },
      focusPrompt() {}, focusHeading() {},
      ...options.dialog,
    },
  };
  const instance = scope.run(() => exports.useRemoteInteractiveAuth(settings));
  return {
    ...instance,
    ...settings,
    networkCheckPending,
    scope,
    delays: () => [...timers.values()].map(timer => timer.delay),
    runTimer() {
      assert.equal(timers.size, 1, "only one auth timer may be pending");
      const [id, timer] = timers.entries().next().value;
      timers.delete(id);
      return timer.callback();
    },
  };
}
