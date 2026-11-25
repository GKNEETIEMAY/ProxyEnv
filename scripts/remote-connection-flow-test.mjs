import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";
import * as vue from "vue";
import { mountInteractiveAuth } from "./remote-interactive-auth-test.mjs";

export const connectionFlowPath = "src/features/remote-bridge/composables/useRemoteConnectionFlow.ts";

function loadModule(path, dependencies = {}) {
  const source = readFileSync(path, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const exports = {};
  runInNewContext(compiled, {
    exports,
    require(name) {
      assert.ok(Object.hasOwn(dependencies, name), `unexpected dependency: ${name}`);
      return dependencies[name];
    },
  });
  return exports;
}

const sshPresentation = loadModule("src/features/remote-bridge/ssh-presentation.ts");
const { useRemoteConnectionFlow } = loadModule(connectionFlowPath, {
  vue,
  "../../../shared/i18n/remote-bridge": loadModule("src/shared/i18n/remote-bridge.ts"),
  "../ssh-presentation": sshPresentation,
});

export function mountConnection(t, remoteBackend, options = {}) {
  const { ref } = vue;
  const calls = [];
  const targetId = ref("lab");
  const busy = ref(false);
  const error = ref();
  const checked = ref(false);
  const sshCheck = ref({ state: "idle", checkedAt: null });
  const advancedView = ref(true);
  const canStartConnection = ref(options.canStart ?? true);
  const ports = {
    proxy: ref(options.proxy ?? true), cc: ref(options.cc ?? true),
    proxyPort: ref(0), ccPort: ref(0), ccLocalPort: ref(15721),
    runtimeExpectedPort: ref(null), runtimePortConflict: ref(false),
    proxyRevision: () => 17,
  };
  const flow = useRemoteConnectionFlow({
    remoteBackend, targetId, busy, error, checked, sshCheck, advancedView, ports,
    canStartConnection: () => canStartConnection.value,
    reviewPreview: () => options.props?.reviewPreview,
    currentSummary: () => options.props?.summary,
    auth: {
      advanceAuthRevision: () => auth.advanceAuthRevision(),
      isCurrentAuthRevision: revision => auth.isCurrentAuthRevision(revision),
      beginInteractiveCheck: () => auth.beginInteractiveAuth("check"),
      dismissAuth() {
        auth.authSubmitting.value = false;
        calls.push(["close"]);
        auth.clearAuthSession();
      },
    },
    emit: (...args) => calls.push(args),
    showAllocatedPorts() {},
    refreshNetworkChecks: () => new Promise(() => {}),
    focusConnectedHeading: () => calls.push(["focus"]),
  });
  const auth = mountInteractiveAuth(t, {
    backend: options.authBackend, targetId, busy, error, checked, sshCheck,
    connectionPhase: flow.connectionPhase, readinessAnnouncementTarget: flow.readinessAnnouncementTarget,
    request: flow.request, usePorts: flow.usePorts, connectPrepared: flow.connectPrepared,
    finishConnectionUi: flow.finishConnectionUi,
    isInteractiveAuthError: sshPresentation.needsInteractiveSshAuth,
    refresh: () => calls.push(["refresh"]),
  });
  return { ...auth, ...flow, ...ports, advancedView, canStartConnection, calls };
}
