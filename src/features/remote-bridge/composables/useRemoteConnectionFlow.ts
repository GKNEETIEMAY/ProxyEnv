import { computed, nextTick, ref, type Ref } from "vue";
import type { CheckState } from "../../../shared/types";
import { bridgeErrorCode } from "../../../shared/i18n/remote-bridge";
import { needsInteractiveSshAuth } from "../ssh-presentation";
import type { BridgeRequest, BridgeSummary, PortAllocation, remoteBackend } from "../state";
import type { ConnectionPhase } from "./useRemoteInteractiveAuth";

interface RemoteConnectionFlowOptions {
  remoteBackend: Pick<typeof remoteBackend, "check" | "preview" | "connect" | "allocatePorts" | "retryReconnect">;
  targetId: Readonly<Ref<string>>;
  ports: {
    proxy: Readonly<Ref<boolean>>;
    cc: Readonly<Ref<boolean>>;
    proxyPort: Ref<number>;
    ccPort: Ref<number>;
    ccLocalPort: Readonly<Ref<number>>;
    runtimeExpectedPort: Ref<number | null>;
    runtimePortConflict: Ref<boolean>;
    proxyRevision: () => number;
  };
  busy: Ref<boolean>;
  error: Ref<unknown>;
  checked: Ref<boolean>;
  sshCheck: Ref<{ state: CheckState; checkedAt: number | null }>;
  advancedView: Ref<boolean>;
  canStartConnection: () => boolean;
  reviewPreview: () => boolean | undefined;
  currentSummary: () => BridgeSummary;
  auth: {
    advanceAuthRevision: () => number;
    isCurrentAuthRevision: (revision: number) => boolean;
    beginInteractiveCheck: () => Promise<void>;
    dismissAuth: () => void;
  };
  emit: {
    (event: "connected", summary: BridgeSummary): void;
    (event: "refresh"): void;
  };
  showAllocatedPorts: () => void;
  refreshNetworkChecks: () => Promise<void>;
  focusConnectedHeading: () => void;
}

export function useRemoteConnectionFlow({
  remoteBackend, targetId, ports, busy, error, checked, sshCheck, advancedView,
  canStartConnection, reviewPreview, currentSummary, auth, emit, showAllocatedPorts,
  refreshNetworkChecks, focusConnectedHeading,
}: RemoteConnectionFlowOptions) {
  const { proxy, cc, proxyPort, ccPort, ccLocalPort, runtimeExpectedPort, runtimePortConflict, proxyRevision } = ports;
  const { advanceAuthRevision, isCurrentAuthRevision, beginInteractiveCheck, dismissAuth } = auth;
  const connectionPhase = ref<ConnectionPhase>("idle");
  const readinessAnnouncementTarget = ref<string>();
  const establishingBridge = computed(() => ["checking", "authenticating", "building"].includes(connectionPhase.value));

  function request(): BridgeRequest {
    return {
      targetId: targetId.value,
      proxyPort: proxy.value ? proxyPort.value : null,
      ccPort: cc.value ? ccPort.value : null,
      ccLocalPort: ccLocalPort.value,
      expectedRevision: proxyRevision(),
    };
  }

  function usePorts(ports: PortAllocation) {
    proxyPort.value = ports.proxyPort;
    ccPort.value = ports.ccPort;
    runtimeExpectedPort.value = ports.runtimeExpectedProxyPort ?? null;
    runtimePortConflict.value = ports.runtimePortConflict ?? false;
    showAllocatedPorts();
  }

  async function finishConnectionUi(summary: BridgeSummary) {
    emit("connected", summary);
    advancedView.value = false;
    connectionPhase.value = "idle";
    busy.value = false;
    dismissAuth();
    void refreshNetworkChecks();
    await nextTick();
    focusConnectedHeading();
  }

  async function connectPrepared(selected: BridgeRequest, isCurrent: () => boolean): Promise<BridgeSummary | undefined> {
    if (!isCurrent()) return;
    try {
      await remoteBackend.preview(selected);
      if (!isCurrent()) return;
      return await remoteBackend.connect(selected);
    } catch (cause) {
      if (!isCurrent()) return;
      if (bridgeErrorCode(cause) !== "portInUse") throw cause;
      const ports = await remoteBackend.allocatePorts(selected.targetId, false);
      if (!isCurrent()) return;
      usePorts(ports);
      const retry: BridgeRequest = {
        ...selected,
        proxyPort: selected.proxyPort === null ? null : ports.proxyPort,
        ccPort: selected.ccPort === null ? null : ports.ccPort,
      };
      await remoteBackend.preview(retry);
      if (!isCurrent()) return;
      return remoteBackend.connect(retry);
    }
  }

  function startConnection() {
    if (!canStartConnection() || busy.value) return;
    const revision = advanceAuthRevision();
    const target = targetId.value;
    const isCurrent = () => isCurrentAuthRevision(revision) && targetId.value === target;
    readinessAnnouncementTarget.value = target;
    checked.value = false;
    proxyPort.value = 0;
    ccPort.value = 0;
    runtimeExpectedPort.value = null;
    runtimePortConflict.value = false;
    sshCheck.value = { ...sshCheck.value, state: "checking" };
    busy.value = true;
    error.value = undefined;
    connectionPhase.value = "checking";
    void (async () => {
      try {
        const ports = reviewPreview()
          ? { proxyPort: 23841, ccPort: 31472 }
          : await remoteBackend.check(target);
        if (!isCurrent()) return;
        usePorts(ports);
        checked.value = true;
        sshCheck.value = { state: "healthy", checkedAt: Date.now() };
        const summary = reviewPreview()
          ? currentSummary()
          : await connectPrepared(request(), isCurrent);
        if (!isCurrent() || !summary) return;
        await finishConnectionUi(summary);
      } catch (cause) {
        if (!isCurrent()) return;
        if (needsInteractiveSshAuth(bridgeErrorCode(cause))) {
          busy.value = false;
          try {
            await beginInteractiveCheck();
          } catch { /* The current auth attempt owns its error and revision. */ }
          return;
        }
        sshCheck.value = { state: "failed", checkedAt: Date.now() };
        readinessAnnouncementTarget.value = undefined;
        connectionPhase.value = "failed";
        error.value = cause;
      } finally {
        if (isCurrent()) {
          busy.value = false;
          emit("refresh");
        }
      }
    })();
  }

  async function reconnectManually() {
    if (busy.value) return;
    busy.value = true;
    error.value = undefined;
    try {
      emit("connected", await remoteBackend.retryReconnect());
    } catch (cause) {
      error.value = cause;
    } finally {
      busy.value = false;
      emit("refresh");
    }
  }

  return {
    connectionPhase, readinessAnnouncementTarget, establishingBridge, request,
    usePorts, connectPrepared, finishConnectionUi, startConnection, reconnectManually,
  };
}
