<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { ActiveProxyContext, CheckState } from "../../../shared/types";
import { bridgeError, bridgeErrorCode, type RemoteBridgeCopy } from "../../../shared/i18n/remote-bridge";
import { copyText } from "../../../shared/utils/clipboard";
import { withoutWindowsExtendedPathPrefix } from "../../../shared/utils/path";
import CheckRow from "../../../shared/components/CheckRow.vue";
import LastChecked from "../../../shared/components/LastChecked.vue";
import StatusIndicator from "../../../shared/components/StatusIndicator.vue";
import RemoteToolDialog from "./RemoteToolDialog.vue";
import {
  remoteBackend,
  targetLabel,
  type BridgeRequest,
  type BridgeSummary,
  type CcDetection,
  type PortAllocation,
  type RemoteTarget,
  type SshAuthOperation,
  type SshAuthSnapshot,
} from "../state";

const props = defineProps<{ copy: RemoteBridgeCopy; activeProxy: ActiveProxyContext; summary: BridgeSummary; reviewPreview?: boolean }>();
const emit = defineEmits<{ refresh: [] }>();
const toolDialog = ref<InstanceType<typeof RemoteToolDialog>>();
const confirmation = ref<HTMLDialogElement>();
const authDialog = ref<HTMLDialogElement>();
const authInput = ref<HTMLInputElement>();
const heading = ref<HTMLElement>();
const step = ref(1);
const targets = ref<RemoteTarget[]>([]);
const targetId = ref("");
const checked = ref(false);
const proxy = ref(true);
const cc = ref(false);
const proxyPort = ref(0);
const ccPort = ref(0);
const ccLocalPort = ref(15721);
const ccDetection = ref<CcDetection>({ state: "notDetected", localPort: 15721 });
type CheckSnapshot = { state: CheckState; checkedAt: number | null };
const sshCheck = ref<CheckSnapshot>({ state: "idle", checkedAt: null });
const serverInternetCheck = ref<CheckSnapshot>({ state: "idle", checkedAt: null });
const localProxyCheck = ref<CheckSnapshot>({ state: "idle", checkedAt: null });
const ccCheck = ref<CheckSnapshot>({ state: "idle", checkedAt: null });
let networkRefreshRevision = 0;
const busy = ref(false);
const error = ref<unknown>();
const feedback = ref<"copied" | "tested" | "ports">();
const preview = ref<BridgeSummary>();
const reviewedRequest = ref<BridgeRequest>();
const vscodeOpened = ref(false);
const authSession = ref<SshAuthSnapshot>();
const authResponse = ref("");
const authSubmitting = ref(false);
let authPollTimer: ReturnType<typeof setTimeout> | undefined;
const codexLaunch = "codex --profile proxyenv_bridge";
const claudeLaunch = 'claude --settings "$HOME/.claude/proxyenv-bridge.json"';

const labels = computed(() => [props.copy.rbTarget, props.copy.rbCapabilities, props.copy.rbPreview, props.copy.rbStatus]);
const selectedTarget = computed(() => targets.value.find((target) => target.id === targetId.value));
const live = computed(() => ["connected", "stale", "unavailable", "connecting"].includes(props.summary.status) && !!props.summary.target);
const proxyAvailable = computed(() => props.activeProxy.available && !!props.activeProxy.candidate && props.activeProxy.candidate.protocol !== "unknown");
const ccUsable = computed(() => ccDetection.value.state !== "notDetected");
const portsReady = computed(() => proxyPort.value >= 20000 && ccPort.value >= 20000 && proxyPort.value !== ccPort.value);
const valid = computed(() => (proxy.value || cc.value)
  && (!proxy.value || proxyAvailable.value && portsReady.value)
  && (!cc.value || ccUsable.value && portsReady.value && ccLocalPort.value >= 1024 && ccLocalPort.value <= 65535));
const errorText = computed(() => error.value ? bridgeError(error.value, props.copy) : "");
const feedbackText = computed(() => feedback.value ? ({ copied: props.copy.rbCopied, tested: props.copy.rbTested, ports: props.copy.rbPortsGenerated })[feedback.value] : "");
const endpoints = computed(() => {
  const source = step.value === 3 ? preview.value : props.summary;
  return [
    { title: props.copy.rbProxyUseTitle, value: source?.proxy },
    { title: props.copy.rbCcUseTitle, value: source?.cc },
  ].filter((row) => row.value);
});
const activeTarget = computed(() => (step.value === 3 ? preview.value?.target : props.summary.target) ?? selectedTarget.value);
const sourceLabel = (target: RemoteTarget) => ({ openssh: props.copy.rbSourceOpenSsh, vscode: props.copy.rbSourceVscode, mobaxterm: props.copy.rbSourceMoba })[target.source];
const helpHeadings = computed(() => ({ check: props.copy.rbHelpCheck, success: props.copy.rbHelpSuccess, failure: props.copy.rbHelpFailure, next: props.copy.rbHelpNext }));
const helpContent = computed(() => ({
  ssh: { check: props.copy.rbSshHelpCheck, success: props.copy.rbSshHelpSuccess, failure: props.copy.rbSshHelpFailure, next: props.copy.rbSshHelpNext },
  internet: { check: props.copy.rbInternetHelpCheck, success: props.copy.rbInternetHelpSuccess, failure: props.copy.rbInternetHelpFailure, next: props.copy.rbInternetHelpNext },
  proxy: { check: props.copy.rbProxyHelpCheck, success: props.copy.rbProxyHelpSuccess, failure: props.copy.rbProxyHelpFailure, next: props.copy.rbProxyHelpNext },
  cc: { check: props.copy.rbCcHelpCheck, success: props.copy.rbCcHelpSuccess, failure: props.copy.rbCcHelpFailure, next: props.copy.rbCcHelpNext },
}));
const genericStateLabel = (state: CheckState) => ({ idle: props.copy.rbCheckIdle, checking: props.copy.rbCheckChecking, healthy: props.copy.rbCheckHealthy, warning: props.copy.rbCheckWarning, failed: props.copy.rbCheckFailed, disabled: props.copy.rbCheckDisabled })[state];
const serverInternetLabel = computed(() => serverInternetCheck.value.state === "healthy" ? props.copy.rbServerInternetReachable : serverInternetCheck.value.state === "failed" ? props.copy.rbServerInternetUnreachable : serverInternetCheck.value.state === "warning" ? props.copy.rbServerInternetUnknown : genericStateLabel(serverInternetCheck.value.state));
const localProxyLabel = computed(() => localProxyCheck.value.state === "healthy" ? props.copy.rbLocalProxyReady : localProxyCheck.value.state === "failed" ? props.copy.rbLocalProxyUnavailable : genericStateLabel(localProxyCheck.value.state));
const ccStateLabel = computed(() => ccCheck.value.state === "healthy" ? props.copy.rbCcConfirmed : ccCheck.value.state === "warning" ? props.copy.rbCcUnknown : ccCheck.value.state === "failed" ? props.copy.rbCcMissing : ccCheck.value.state === "disabled" ? props.copy.rbCcDisabled : genericStateLabel(ccCheck.value.state));
const lastNetworkChecked = computed(() => Math.max(0, sshCheck.value.checkedAt ?? 0, serverInternetCheck.value.checkedAt ?? 0, localProxyCheck.value.checkedAt ?? 0, ccCheck.value.checkedAt ?? 0) || null);
const networkChecking = computed(() => serverInternetCheck.value.state === "checking" || ccCheck.value.state === "checking");
const sshRuntimeState = computed<CheckState>(() => live.value ? (props.summary.status === "connecting" ? "checking" : "healthy") : bridgeCheckState(props.summary.status));
const sshRuntimeLabel = computed(() => sshRuntimeState.value === "healthy" ? props.copy.rbHealthy : genericStateLabel(sshRuntimeState.value));
const authPrompt = computed(() => authSession.value?.prompt);
const authPromptType = computed(() => authPrompt.value?.type ?? "unknown");
const authStatusState = computed<CheckState>(() => {
  if (authSession.value?.status === "failed") return "failed";
  if (["authenticated", "succeeded"].includes(authSession.value?.status ?? "")) return "healthy";
  if (authSession.value?.status === "waitingUser") return "warning";
  return "checking";
});
const authStatusLabel = computed(() => {
  if (authSession.value?.status === "failed") return props.copy.rbAuthFailure;
  if (["authenticated", "succeeded"].includes(authSession.value?.status ?? "")) return props.copy.rbAuthSuccess;
  if (authSession.value?.status === "waitingUser") return props.copy.rbAuthWaitingUser;
  if (["submitting", "waitingServer"].includes(authSession.value?.status ?? "")) return props.copy.rbAuthVerifying;
  return props.copy.rbAuthWaitingPrompt;
});
const authPromptCopy = computed(() => ({
  password: { title: props.copy.rbAuthPasswordTitle, description: props.copy.rbAuthPasswordDescription, label: props.copy.rbAuthPasswordLabel, action: props.copy.rbAuthConnect, notice: props.copy.rbAuthSecretNotice },
  keyPassphrase: { title: props.copy.rbAuthPassphraseTitle, description: props.copy.rbAuthPassphraseDescription, label: props.copy.rbAuthPassphraseLabel, action: props.copy.rbAuthContinue, notice: props.copy.rbAuthPassphraseNotice },
  verificationCode: { title: props.copy.rbAuthOtpTitle, description: props.copy.rbAuthOtpDescription, label: props.copy.rbAuthOtpLabel, action: props.copy.rbAuthVerify, notice: props.copy.rbAuthSecretNotice },
  hostKeyConfirmation: { title: props.copy.rbAuthHostKeyTitle, description: props.copy.rbAuthHostKeyDescription, label: "", action: props.copy.rbAuthConfirmHost, notice: props.copy.rbAuthGenericNotice },
  keyboardInteractive: { title: props.copy.rbAuthKeyboardTitle, description: props.copy.rbAuthKeyboardDescription, label: props.copy.rbAuthResponseLabel, action: props.copy.rbAuthSubmit, notice: props.copy.rbAuthSecretNotice },
  unknown: { title: props.copy.rbAuthUnknownTitle, description: props.copy.rbAuthUnknownDescription, label: props.copy.rbAuthResponseLabel, action: props.copy.rbAuthSubmit, notice: props.copy.rbAuthGenericNotice },
})[authPromptType.value]);
const authCanRespond = computed(() => authSession.value?.status === "waitingUser" && !!authPrompt.value);
const authIsHostConfirmation = computed(() => authPromptType.value === "hostKeyConfirmation");

function bridgeCheckState(status: BridgeSummary["status"] | null, enabled = true): CheckState {
  if (!enabled) return "disabled";
  if (status === "connected") return "healthy";
  if (status === "connecting") return "checking";
  if (status === "stale") return "warning";
  if (status === "unavailable" || status === "error") return "failed";
  return "idle";
}

function updateLocalProxyCheck() {
  localProxyCheck.value = { state: proxyAvailable.value ? "healthy" : "failed", checkedAt: Date.now() };
}

async function refreshNetworkChecks() {
  const currentTarget = selectedTarget.value ?? props.summary.target;
  if (!currentTarget?.available) return;
  if (props.reviewPreview) {
    const checkedAt = Date.now();
    sshCheck.value = { state: "healthy", checkedAt };
    serverInternetCheck.value = { state: "healthy", checkedAt };
    updateLocalProxyCheck();
    ccDetection.value = { state: "confirmed", localPort: ccLocalPort.value };
    ccCheck.value = cc.value || props.summary.cc ? { state: "healthy", checkedAt } : { state: "disabled", checkedAt };
    return;
  }
  const revision = ++networkRefreshRevision;
  updateLocalProxyCheck();
  serverInternetCheck.value = { ...serverInternetCheck.value, state: "checking" };
  ccCheck.value = cc.value ? { ...ccCheck.value, state: "checking" } : { state: "disabled", checkedAt: Date.now() };
  const serverTask = remoteBackend.checkNetwork(currentTarget.id).then((result) => {
    if (revision !== networkRefreshRevision) return;
    serverInternetCheck.value = {
      state: result.serverInternet === "reachable" ? "healthy" : result.serverInternet === "unreachable" ? "failed" : "warning",
      checkedAt: Date.now(),
    };
  }).catch(() => {
    if (revision === networkRefreshRevision) serverInternetCheck.value = { state: "warning", checkedAt: Date.now() };
  });
  const ccTask = cc.value ? remoteBackend.detectCc(ccLocalPort.value).then((result) => {
    if (revision !== networkRefreshRevision) return;
    ccDetection.value = result;
    ccCheck.value = { state: result.state === "confirmed" ? "healthy" : result.state === "listeningUnknown" ? "warning" : "failed", checkedAt: Date.now() };
  }).catch(() => {
    if (revision === networkRefreshRevision) ccCheck.value = { state: "failed", checkedAt: Date.now() };
  }) : Promise.resolve();
  await Promise.allSettled([serverTask, ccTask]);
}

async function perform(action: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true;
  error.value = undefined;
  feedback.value = undefined;
  try {
    await action();
  } catch (cause) {
    error.value = cause;
  } finally {
    busy.value = false;
    emit("refresh");
  }
}

function clearAuthPoll() {
  clearTimeout(authPollTimer);
  authPollTimer = undefined;
}

async function completeInteractiveAuth(snapshot: SshAuthSnapshot) {
  authSession.value = snapshot;
  await new Promise((resolve) => setTimeout(resolve, 420));
  if (authSession.value?.sessionId !== snapshot.sessionId) return;
  const outcome = await remoteBackend.sshAuthFinish(snapshot.sessionId);
  clearAuthPoll();
  authSession.value = undefined;
  authResponse.value = "";
  authDialog.value?.close();
  if (outcome.operation === "check" && outcome.ports) {
    usePorts(outcome.ports);
    checked.value = true;
    sshCheck.value = { state: "healthy", checkedAt: Date.now() };
  } else if (outcome.operation === "connect" && outcome.summary) {
    go(4);
  }
  emit("refresh");
}

async function pollInteractiveAuth() {
  const session = authSession.value;
  if (!session) return;
  try {
    const next = await remoteBackend.sshAuthState(session.sessionId);
    authSession.value = next;
    if (next.status === "succeeded") {
      await completeInteractiveAuth(next);
      return;
    }
  } catch (cause) {
    error.value = cause;
    clearAuthPoll();
    return;
  }
  if (authSession.value?.status !== "failed") {
    authPollTimer = setTimeout(pollInteractiveAuth, 180);
  }
}

async function beginInteractiveAuth(operation: SshAuthOperation, request: BridgeRequest | null = null) {
  clearAuthPoll();
  error.value = undefined;
  authResponse.value = "";
  authSession.value = await remoteBackend.sshAuthBegin(operation, targetId.value, request);
  await nextTick();
  authDialog.value?.showModal();
  authPollTimer = setTimeout(pollInteractiveAuth, 120);
}

async function submitInteractiveAuth() {
  const session = authSession.value;
  if (!session || !authCanRespond.value || authSubmitting.value) return;
  if (!authIsHostConfirmation.value && !authResponse.value) return;
  authSubmitting.value = true;
  const response = authResponse.value;
  authResponse.value = "";
  try {
    authSession.value = authIsHostConfirmation.value
      ? await remoteBackend.sshAuthConfirmHost(session.sessionId)
      : await remoteBackend.sshAuthSubmit(session.sessionId, response);
  } catch (cause) {
    error.value = cause;
  } finally {
    authSubmitting.value = false;
  }
}

watch(
  () => [authSession.value?.prompt?.type, authSession.value?.prompt?.attempt, authSession.value?.status],
  async () => {
    if (!authCanRespond.value || authIsHostConfirmation.value) return;
    await nextTick();
    authInput.value?.focus();
  },
);

async function cancelInteractiveAuth() {
  clearAuthPoll();
  const sessionId = authSession.value?.sessionId;
  authSession.value = undefined;
  authResponse.value = "";
  authDialog.value?.close();
  if (sessionId) {
    try { await remoteBackend.sshAuthCancel(sessionId); } catch { /* Session may already be closed. */ }
  }
  if (sshCheck.value.state === "checking") {
    sshCheck.value = { state: "idle", checkedAt: null };
  }
}

async function load() {
  if (props.reviewPreview) {
    targets.value = [
      { id: "openssh|preview|aliyun-dev", displayName: "aliyun-dev", source: "openssh", sourceLabel: "OpenSSH", configPath: "~\\.ssh\\config", sshAlias: "aliyun-dev", host: null, user: null, port: null, identityFile: null, available: true, compatibility: "compatible", unavailableReason: null, canOpenVscode: true },
      { id: "vscode|preview|gpu-server", displayName: "gpu-server", source: "vscode", sourceLabel: "VS Code Remote", configPath: "D:\\SSH\\vscode-config", sshAlias: "gpu-server", host: null, user: null, port: null, identityFile: null, available: true, compatibility: "compatible", unavailableReason: null, canOpenVscode: true },
      { id: "mobaxterm|preview|GPU Server", displayName: "GPU Server", source: "mobaxterm", sourceLabel: "MobaXterm", configPath: "~\\Documents\\MobaXterm\\MobaXterm.ini", sshAlias: null, host: "gpu.example.test", user: "dev", port: 22, identityFile: null, available: false, compatibility: "unsupported", unavailableReason: "mobaSessionUnsupported", canOpenVscode: false },
    ];
    targetId.value = targets.value[0].id;
    return;
  }
  const next = await remoteBackend.targets();
  targets.value = next;
  const preferred = props.summary.target?.id;
  if (preferred && next.some((target) => target.id === preferred)) targetId.value = preferred;
  else if (!next.some((target) => target.id === targetId.value && target.available)) targetId.value = next.find((target) => target.available)?.id ?? next[0]?.id ?? "";
}

function go(value: number) {
  step.value = value;
  error.value = undefined;
  void nextTick(() => heading.value?.focus());
  if (value === 2 || value === 4) void refreshNetworkChecks();
}

function usePorts(ports: PortAllocation) {
  proxyPort.value = ports.proxyPort;
  ccPort.value = ports.ccPort;
  feedback.value = "ports";
}

function checkTarget() {
  checked.value = false;
  proxyPort.value = 0;
  ccPort.value = 0;
  reviewedRequest.value = undefined;
  sshCheck.value = { ...sshCheck.value, state: "checking" };
  if (props.reviewPreview) {
    usePorts({ proxyPort: 23841, ccPort: 31472 });
    checked.value = true;
    sshCheck.value = { state: "healthy", checkedAt: Date.now() };
    return;
  }
  void perform(async () => {
    try {
      const ports = await remoteBackend.check(targetId.value);
      usePorts(ports);
      checked.value = true;
      sshCheck.value = { state: "healthy", checkedAt: Date.now() };
    } catch (cause) {
      if (bridgeErrorCode(cause) === "sshAuth") {
        await beginInteractiveAuth("check");
        return;
      }
      sshCheck.value = { state: "failed", checkedAt: Date.now() };
      throw cause;
    }
  });
}

function refreshTargets() {
  checked.value = false;
  proxyPort.value = 0;
  ccPort.value = 0;
  reviewedRequest.value = undefined;
  void perform(load);
}

function regeneratePorts() {
  void perform(async () => usePorts(await remoteBackend.allocatePorts(targetId.value)));
}

function request(): BridgeRequest {
  return {
    targetId: targetId.value,
    proxyPort: proxy.value ? proxyPort.value : null,
    ccPort: cc.value ? ccPort.value : null,
    ccLocalPort: ccLocalPort.value,
    expectedRevision: props.activeProxy.revision,
  };
}

function review() {
  void perform(async () => {
    const selected = request();
    preview.value = await remoteBackend.preview(selected);
    reviewedRequest.value = selected;
    go(3);
  });
}

function connect() {
  if (!reviewedRequest.value || busy.value) return;
  busy.value = true;
  error.value = undefined;
  const selected = reviewedRequest.value;
  void remoteBackend.connect(selected).then(() => {
    go(4);
  }).catch(async (cause) => {
    if (bridgeErrorCode(cause) === "portInUse") {
      try {
        usePorts(await remoteBackend.allocatePorts(selected.targetId));
        reviewedRequest.value = undefined;
        go(2);
        error.value = "portRace";
      } catch (allocationError) {
        error.value = allocationError;
      }
    } else {
      if (bridgeErrorCode(cause) === "sshAuth") {
        try {
          await beginInteractiveAuth("connect", selected);
        } catch (authError) {
          error.value = authError;
        }
      } else {
        error.value = cause;
      }
    }
  }).finally(() => {
    busy.value = false;
    emit("refresh");
  });
}

function detectCc() {
  ccCheck.value = { ...ccCheck.value, state: "checking" };
  void remoteBackend.detectCc(ccLocalPort.value).then((result) => {
    ccDetection.value = result;
    ccCheck.value = { state: result.state === "confirmed" ? "healthy" : result.state === "listeningUnknown" ? "warning" : "failed", checkedAt: Date.now() };
  }).catch((cause) => {
    ccCheck.value = { state: "failed", checkedAt: Date.now() };
    error.value = cause;
  });
}

function configure(tool: string) {
  const target = props.summary.target ?? selectedTarget.value;
  if (target) toolDialog.value?.open(tool, target.id, false, targetLabel(target));
}

function restore(tool: string, id = targetId.value) {
  const target = targets.value.find((candidate) => candidate.id === id) ?? props.summary.target;
  toolDialog.value?.open(tool, id, true, targetLabel(target));
}

function copyValue(value: string) {
  void perform(async () => {
    await copyText(value);
    feedback.value = "copied";
  });
}

function confirmDisconnect() {
  confirmation.value?.close();
  void perform(async () => {
    await remoteBackend.disconnect();
    go(1);
    await load();
  });
}

watch(targetId, () => {
  networkRefreshRevision += 1;
  checked.value = false;
  proxyPort.value = 0;
  ccPort.value = 0;
  reviewedRequest.value = undefined;
  vscodeOpened.value = false;
  sshCheck.value = { state: "idle", checkedAt: null };
  serverInternetCheck.value = { state: "idle", checkedAt: null };
  ccCheck.value = { state: "idle", checkedAt: null };
});
watch(ccLocalPort, () => {
  ccDetection.value = { state: "notDetected", localPort: ccLocalPort.value };
  ccCheck.value = cc.value ? { state: "idle", checkedAt: null } : { state: "disabled", checkedAt: null };
});
watch(proxyAvailable, updateLocalProxyCheck);
watch(cc, (enabled) => {
  ccCheck.value = enabled ? { state: "idle", checkedAt: null } : { state: "disabled", checkedAt: Date.now() };
  if (enabled && (step.value === 2 || step.value === 4)) void refreshNetworkChecks();
});
watch(() => props.activeProxy.revision, () => {
  if (step.value === 3 && reviewedRequest.value?.proxyPort) {
    reviewedRequest.value = undefined;
    go(2);
    error.value = "activeChanged";
  }
});
watch(() => props.summary.target?.id, (id) => {
  if (id && live.value) {
    targetId.value = id;
    step.value = 4;
    void refreshNetworkChecks();
  }
});

onMounted(() => {
  proxy.value = proxyAvailable.value;
  updateLocalProxyCheck();
  step.value = props.summary.target ? 4 : 1;
  void perform(load);
  if (props.reviewPreview && new URLSearchParams(window.location.search).get("impeccable-review") === "remote-auth") {
    authSession.value = {
      sessionId: "review-session",
      operation: "check",
      status: "waitingUser",
      auth: { mode: "interactive", method: "password", authenticated: false, passwordStored: false },
      prompt: { type: "password", message: "dev@remote's password:", secret: true, attempt: 1, target: "remote", fingerprint: null },
      error: null,
    };
    void nextTick(() => authDialog.value?.showModal());
  }
});
onBeforeUnmount(() => {
  const sessionId = authSession.value?.sessionId;
  clearAuthPoll();
  if (sessionId) void remoteBackend.sshAuthCancel(sessionId).catch(() => undefined);
});
</script>

<template>
  <main class="page remote-bridge-page">
    <header class="remote-page-intro">
      <div><h1>{{ copy.rbTitle }}</h1><p>{{ copy.rbPageIntro }}</p></div>
      <StatusIndicator :state="bridgeCheckState(summary.status)" :label="copy.rbStates[summary.status]" />
    </header>

    <ol class="remote-steps" :aria-label="copy.rbTitle">
      <li v-for="(label, index) in labels" :key="label" :aria-current="step === index + 1 ? 'step' : undefined">
        <button type="button" :disabled="index + 1 >= step || busy" @click="go(index + 1)"><span>{{ index + 1 }}</span>{{ label }}</button>
      </li>
    </ol>

    <section class="remote-workspace">
      <div class="remote-workspace-heading">
        <h2 ref="heading" tabindex="-1">{{ labels[step - 1] }}</h2>
        <div v-if="step === 2 || step === 4" class="remote-status-toolbar">
          <LastChecked :label="copy.rbLastChecked" :checked-at="lastNetworkChecked" />
          <button class="secondary-action" type="button" :disabled="busy || networkChecking" @click="refreshNetworkChecks">{{ copy.rbRefreshStatus }}</button>
        </div>
      </div>
      <fieldset :disabled="busy" class="remote-fields">
        <template v-if="step === 1">
          <div v-if="targets.length" class="remote-target-list" role="radiogroup" :aria-label="copy.rbTarget">
            <label v-for="target in targets" :key="target.id" class="remote-target" :class="{ selected: targetId === target.id, unavailable: !target.available }" :title="!target.available ? bridgeError(target.unavailableReason, copy) : undefined">
              <input v-model="targetId" type="radio" name="remote-target" :value="target.id" :disabled="!target.available" />
              <span class="remote-target-mark" aria-hidden="true"></span>
              <span class="remote-target-copy">
                <strong>{{ target.displayName }}</strong>
                <small>{{ sourceLabel(target) }}</small>
                <code>{{ withoutWindowsExtendedPathPrefix(target.configPath) }}</code>
                <em v-if="!target.available">{{ copy.rbTargetUnsupported }}</em>
              </span>
            </label>
          </div>
          <p v-else class="remote-empty">{{ copy.rbEmpty }}</p>
          <p v-if="selectedTarget && !selectedTarget.available" class="notice notice-warning">{{ copy.rbMobaUnsupported }}</p>
          <p class="remote-hint">{{ copy.rbRequirements }}</p>
          <div class="remote-actions">
            <button class="secondary-action" type="button" @click="refreshTargets">{{ copy.rbRefresh }}</button>
            <button class="primary-action" type="button" :disabled="!selectedTarget?.available" @click="checkTarget">{{ copy.rbCheck }}</button>
          </div>
          <p v-if="checked" class="remote-success" role="status">{{ copy.rbChecked }} · {{ copy.rbPortsGenerated }}</p>
          <div v-if="targetId" class="remote-actions remote-restore-actions">
            <button class="secondary-action" type="button" @click="restore('codex')">{{ copy.rbRestoreCodex }}</button>
            <button class="secondary-action" type="button" @click="restore('claude')">{{ copy.rbRestoreClaude }}</button>
          </div>
        </template>

        <template v-else-if="step === 2">
          <section class="remote-check-group">
            <header><h3>{{ copy.rbNetworkSection }}</h3><p>{{ copy.rbNetworkSectionHint }}</p></header>
            <CheckRow :label="copy.rbServerInternet" :state="serverInternetCheck.state" :state-label="serverInternetLabel" :checked-at="serverInternetCheck.checkedAt" :last-checked-label="copy.rbLastChecked" :help-label="copy.rbHelpLabel" :help-headings="helpHeadings" :help-content="helpContent.internet">
              <template #detail><p class="check-row-detail">{{ selectedTarget?.displayName }}</p></template>
            </CheckRow>
            <CheckRow :label="copy.rbProxy" :state="localProxyCheck.state" :state-label="localProxyLabel" :checked-at="localProxyCheck.checkedAt" :last-checked-label="copy.rbLastChecked" :help-label="copy.rbHelpLabel" :help-headings="helpHeadings" :help-content="helpContent.proxy">
              <template #detail><p v-if="proxyAvailable" class="check-row-detail"><span>{{ activeProxy.candidate?.clientName }}</span> · <code>{{ activeProxy.candidate?.host }}:{{ activeProxy.candidate?.port }} · {{ activeProxy.candidate?.protocol }}</code></p><p v-else class="check-row-detail">{{ copy.rbNoProxy }}</p></template>
              <template #actions><label class="remote-choice"><input v-model="proxy" type="checkbox" :disabled="!proxyAvailable" />{{ copy.rbUseProxyBridge }}</label></template>
            </CheckRow>
            <dl v-if="proxy" class="remote-port-pair"><dt>{{ copy.rbRemotePort }}</dt><dd><code>127.0.0.1:{{ proxyPort }}</code></dd></dl>
          </section>
          <section class="remote-check-group">
            <header><h3>{{ copy.rbAiRouteSection }}</h3><p>{{ copy.rbAiRouteSectionHint }}</p></header>
            <CheckRow :label="copy.rbCc" :state="ccCheck.state" :state-label="ccStateLabel" :checked-at="ccCheck.checkedAt" :last-checked-label="copy.rbLastChecked" :help-label="copy.rbHelpLabel" :help-headings="helpHeadings" :help-content="helpContent.cc">
              <template #detail><p class="check-row-detail">{{ copy.rbCcHint }}</p></template>
              <template #actions><label class="remote-choice"><input v-model="cc" type="checkbox" />{{ copy.rbUseCcBridge }}</label></template>
            </CheckRow>
            <template v-if="cc">
              <label class="remote-port">{{ copy.rbLocalPort }}<input v-model.number="ccLocalPort" type="number" min="1024" max="65535" required /></label>
              <div class="remote-actions"><button class="secondary-action" type="button" @click="detectCc">{{ copy.rbDetect }}</button></div>
              <p v-if="ccDetection.state === 'notDetected'" class="remote-hint">{{ copy.rbCcOpenHint }}</p>
              <dl class="remote-port-pair"><dt>{{ copy.rbRemotePort }}</dt><dd><code>127.0.0.1:{{ ccPort }}</code></dd></dl>
            </template>
          </section>
          <div class="remote-port-footer"><p class="remote-hint">{{ copy.rbRemotePortAuto }}</p><button class="secondary-action" type="button" :disabled="!targetId" @click="regeneratePorts">{{ copy.rbRegeneratePorts }}</button></div>
        </template>

        <template v-else>
          <div v-if="activeTarget" class="remote-selected-target">
            <div><strong>{{ activeTarget.displayName }}</strong><span>{{ sourceLabel(activeTarget) }}</span><code>{{ withoutWindowsExtendedPathPrefix(activeTarget.configPath) }}</code></div>
            <StatusIndicator v-if="step === 4" :state="bridgeCheckState(summary.status)" :label="copy.rbStates[summary.status]" />
          </div>
          <p v-if="step === 4 && summary.status === 'stale'" class="notice notice-warning">{{ copy.rbStaleHint }}</p>
          <p v-if="step === 4 && summary.status === 'unavailable'" class="notice notice-warning">{{ copy.rbUnavailableHint }}</p>
          <section v-for="row in endpoints" :key="row.title" class="remote-capability">
            <h3>{{ row.title }}</h3>
            <dl><dt>{{ copy.rbLocal }}</dt><dd><code>{{ row.value!.local.host }}:{{ row.value!.local.port }} · {{ row.value!.local.protocol }}</code></dd><dt>{{ copy.rbRemote }}</dt><dd><code>127.0.0.1:{{ row.value!.remotePort }}</code></dd></dl>
          </section>
          <p class="remote-hint">{{ copy.rbSafety }}</p>

          <template v-if="step === 4">
            <section v-if="live" class="remote-health">
              <h3>{{ copy.rbStatus }}</h3>
              <CheckRow :label="copy.rbSshHealth" :state="sshRuntimeState" :state-label="sshRuntimeLabel" :checked-at="sshCheck.checkedAt" :last-checked-label="copy.rbLastChecked" :help-label="copy.rbHelpLabel" :help-headings="helpHeadings" :help-content="helpContent.ssh">
                <template #detail><p class="check-row-detail">{{ summary.target?.displayName }}</p></template>
              </CheckRow>
              <CheckRow :label="copy.rbServerInternet" :state="serverInternetCheck.state" :state-label="serverInternetLabel" :checked-at="serverInternetCheck.checkedAt" :last-checked-label="copy.rbLastChecked" :help-label="copy.rbHelpLabel" :help-headings="helpHeadings" :help-content="helpContent.internet" />
              <CheckRow v-if="summary.proxy" :label="copy.rbLocalProxyHealth" :state="bridgeCheckState(summary.proxyStatus)" :state-label="summary.proxyStatus ? copy.rbStates[summary.proxyStatus] : copy.rbCheckIdle" :checked-at="localProxyCheck.checkedAt" :last-checked-label="copy.rbLastChecked" :help-label="copy.rbHelpLabel" :help-headings="helpHeadings" :help-content="helpContent.proxy">
                <template #detail><p class="check-row-detail"><code>{{ summary.proxy.local.host }}:{{ summary.proxy.local.port }}</code> → <code>127.0.0.1:{{ summary.proxy.remotePort }}</code></p></template>
              </CheckRow>
              <CheckRow v-if="summary.cc" :label="copy.rbLocalCcHealth" :state="bridgeCheckState(summary.ccStatus)" :state-label="summary.ccStatus ? copy.rbStates[summary.ccStatus] : copy.rbCheckIdle" :checked-at="ccCheck.checkedAt" :last-checked-label="copy.rbLastChecked" :help-label="copy.rbHelpLabel" :help-headings="helpHeadings" :help-content="helpContent.cc">
                <template #detail><p class="check-row-detail"><code>{{ summary.cc.local.host }}:{{ summary.cc.local.port }}</code> → <code>127.0.0.1:{{ summary.cc.remotePort }}</code></p></template>
              </CheckRow>
              <div v-if="summary.cc" class="remote-tool-status"><span>Codex</span><span>{{ summary.codexConfigured ? copy.rbExtPending : copy.rbNotConfigured }}</span></div>
              <div v-if="summary.cc" class="remote-tool-status"><span>Claude Code</span><span>{{ summary.claudeConfigured ? copy.rbExtPending : copy.rbNotConfigured }}</span></div>
            </section>

            <header v-if="live" class="remote-next-heading">
              <strong>{{ copy.rbConnectedTitle }}</strong>
              <h3>{{ copy.rbNextSteps }}</h3>
            </header>

            <section v-if="summary.proxy && live" class="remote-next-section">
              <h3>{{ copy.rbProxyUseTitle }}</h3>
              <ol><li>{{ copy.rbStepTerminal }}</li><li>{{ copy.rbStepCopy }}</li><li>{{ copy.rbStepPaste }}</li><li>{{ copy.rbStepTest }}</li><li>{{ copy.rbStepRun }}</li></ol>
              <pre>{{ summary.environment }}</pre>
              <div class="remote-actions"><button class="primary-action" type="button" @click="copyValue(summary.environment)">{{ copy.rbCopy }}</button><button class="secondary-action" type="button" :disabled="summary.status !== 'connected'" @click="perform(async () => { await remoteBackend.test(); feedback = 'tested'; })">{{ copy.rbTest }}</button></div>
              <p class="remote-hint">{{ copy.rbShellScope }}</p>
            </section>

            <section v-if="summary.cc && live" class="remote-next-section">
              <h3>{{ copy.rbCcUseTitle }}</h3>
              <div class="remote-actions"><button class="primary-action" type="button" @click="configure('codex')">{{ copy.rbCodex }}</button><button class="secondary-action" type="button" @click="configure('claude')">{{ copy.rbClaude }}</button></div>
              <div class="remote-command"><span>Codex</span><template v-if="summary.codexConfigured"><code>{{ codexLaunch }}</code><button type="button" :aria-label="copy.rbCopyLaunch" @click="copyValue(codexLaunch)">{{ copy.rbCopyLaunch }}</button></template><em v-else>{{ copy.rbConfigureBeforeLaunch }}</em></div>
              <div class="remote-command"><span>Claude Code</span><template v-if="summary.claudeConfigured"><code>{{ claudeLaunch }}</code><button type="button" :aria-label="copy.rbCopyLaunch" @click="copyValue(claudeLaunch)">{{ copy.rbCopyLaunch }}</button></template><em v-else>{{ copy.rbConfigureBeforeLaunch }}</em></div>
              <div class="remote-actions"><button class="secondary-action" type="button" @click="restore('codex', summary.target!.id)">{{ copy.rbRestoreCodex }}</button><button class="secondary-action" type="button" @click="restore('claude', summary.target!.id)">{{ copy.rbRestoreClaude }}</button></div>
            </section>

            <section v-if="summary.target?.canOpenVscode && live" class="remote-vscode">
              <p class="remote-hint">{{ copy.rbVscodeHint }}</p>
              <button class="secondary-action" type="button" @click="perform(async () => { await remoteBackend.openVscode(summary.target!.id); vscodeOpened = true; })">{{ copy.rbVscodeOpen }}</button>
              <p v-if="vscodeOpened" role="status">{{ copy.rbExtOpened }}</p>
            </section>
          </template>
        </template>
      </fieldset>

      <p v-if="errorText" class="remote-error" role="alert">{{ errorText }}</p>
      <p class="remote-feedback" role="status">{{ busy ? copy.rbBusy : feedbackText }}</p>
      <div class="confirmation-actions">
        <button v-if="step === 2 || step === 3" class="secondary-action" type="button" :disabled="busy" @click="go(step - 1)">{{ copy.rbBack }}</button>
        <button v-if="step === 1" class="primary-action" type="button" :disabled="busy || !checked" @click="go(2)">{{ copy.rbNext }}</button>
        <button v-if="step === 2" class="primary-action" type="button" :disabled="busy || !valid" @click="review">{{ copy.rbNext }}</button>
        <button v-if="step === 3" class="primary-action" type="button" :disabled="busy || !reviewedRequest" @click="connect">{{ copy.rbConnect }}</button>
        <button v-if="step === 4 && live" class="secondary-action remote-danger" type="button" :disabled="busy" @click="confirmation?.showModal()">{{ copy.rbDisconnect }}</button>
        <button v-if="step === 4 && !live" class="primary-action" type="button" :disabled="busy" @click="go(1)">{{ copy.rbReconnect }}</button>
      </div>
    </section>
  </main>

  <dialog ref="confirmation" class="confirmation-dialog remote-disconnect-dialog" aria-labelledby="remote-confirm-title" @cancel.prevent="confirmation?.close()">
    <form @submit.prevent="confirmDisconnect">
      <h2 id="remote-confirm-title">{{ copy.rbDisconnect }}</h2>
      <p>{{ copy.rbDisconnectHint }}</p>
      <p class="notice notice-warning">{{ copy.rbDisconnectDetails }}</p>
      <div class="confirmation-actions"><button class="secondary-action" type="button" autofocus @click="confirmation?.close()">{{ copy.rbCancel }}</button><button class="primary-action" type="submit" :disabled="busy">{{ copy.rbConfirm }}</button></div>
    </form>
  </dialog>
  <dialog ref="authDialog" class="confirmation-dialog remote-auth-dialog" aria-labelledby="remote-auth-title" @cancel.prevent="cancelInteractiveAuth">
    <form @submit.prevent="submitInteractiveAuth">
      <header class="remote-auth-heading">
        <h2 id="remote-auth-title">{{ copy.rbAuthTitle }}</h2>
        <StatusIndicator :state="authStatusState" :label="authStatusLabel" />
      </header>

      <section class="remote-auth-context" aria-live="polite">
        <h3 v-if="authCanRespond">{{ authPromptCopy.title }}</h3>
        <h3 v-else-if="authSession?.status === 'failed'">{{ copy.rbAuthFailure }}</h3>
        <h3 v-else-if="authSession?.auth.authenticated">{{ copy.rbAuthSuccess }}</h3>
        <h3 v-else>{{ copy.rbAuthWaitingPrompt }}</h3>
        <p v-if="authCanRespond">{{ authPromptCopy.description }}</p>
      </section>

      <section class="remote-auth-terminal" :aria-label="copy.rbAuthPrompt" aria-live="polite">
        <span>{{ copy.rbAuthPrompt }}</span>
        <pre>{{ authPrompt ? (authPrompt.message || copy.rbAuthPromptUnavailable) : copy.rbAuthWaitingPromptMessage }}</pre>
      </section>

      <dl v-if="authIsHostConfirmation && authPrompt?.fingerprint" class="remote-auth-fingerprint">
        <dt>{{ copy.rbAuthFingerprint }}</dt>
        <dd><code>{{ authPrompt.fingerprint }}</code></dd>
      </dl>

      <label v-if="authCanRespond && !authIsHostConfirmation" class="remote-auth-field">
        <span>{{ authPromptCopy.label }}</span>
        <input ref="authInput" v-model="authResponse" :type="authPrompt?.secret ? 'password' : 'text'" autocomplete="off" maxlength="4096" :disabled="authSubmitting" />
      </label>

      <p v-if="authCanRespond && !authIsHostConfirmation" class="notice notice-warning remote-auth-privacy">{{ authPromptCopy.notice }}</p>
      <p v-if="authSession?.error" class="remote-error remote-auth-error" role="alert">{{ bridgeError(authSession.error, copy) }}</p>
      <div class="confirmation-actions">
        <button v-if="authSession?.status !== 'succeeded'" class="secondary-action" type="button" :disabled="authSubmitting" @click="cancelInteractiveAuth">{{ authSession?.status === 'failed' ? copy.rbClose : copy.rbAuthCancel }}</button>
        <button v-if="authCanRespond" class="primary-action" type="submit" :disabled="(!authIsHostConfirmation && !authResponse) || authSubmitting">{{ authPromptCopy.action }}</button>
      </div>
    </form>
  </dialog>
  <RemoteToolDialog ref="toolDialog" :copy="copy" :session-alias="summary.target?.id ?? targetId" :session-status="summary.status" @refresh="emit('refresh')" />
</template>

<style>
.remote-bridge-page { display:grid; gap:24px; }
.confirmation-dialog.remote-bridge-dialog { width:min(640px,calc(100vw - 40px)); max-height:calc(100vh - 40px); overflow-y:auto; }
.remote-heading { display:flex; align-items:center; justify-content:space-between; gap:16px; }
.remote-page-intro { display:flex; align-items:flex-end; justify-content:space-between; gap:24px; }
.remote-page-intro h1 { margin:0; font-family:"Newsreader","Noto Serif SC",serif; font-size:clamp(24px,4vw,32px); font-weight:590; letter-spacing:-.03em; }
.remote-page-intro p { max-width:62ch; margin:8px 0 0; color:var(--muted); font-size:12px; line-height:1.6; }
.remote-steps { display:grid; grid-template-columns:repeat(4,minmax(0,1fr)); padding:0; margin:0; list-style:none; border-block:1px solid var(--line); }
.remote-steps li { position:relative; min-width:0; }
.remote-steps li:not(:last-child)::after { position:absolute; top:15px; right:0; width:1px; height:20px; content:""; background:var(--line); }
.remote-steps button { display:flex; width:100%; min-height:50px; padding:8px 12px; align-items:center; gap:8px; color:var(--muted); background:none; text-align:left; cursor:pointer; }
.remote-steps button:disabled { cursor:default; }
.remote-steps [aria-current] button { color:var(--accent-strong); font-weight:650; }
.remote-steps span { display:grid; width:23px; height:23px; flex:none; place-items:center; border:1px solid var(--line-strong); border-radius:50%; font-size:10px; }
.remote-steps [aria-current] span { border-color:var(--accent); background:var(--accent-soft); }
.remote-workspace { padding:26px 28px; border:1px solid var(--line); border-radius:16px; background:var(--surface); box-shadow:var(--shadow); }
.remote-workspace-heading { display:flex; margin-bottom:18px; align-items:center; justify-content:space-between; gap:18px; }
.remote-workspace-heading h2 { margin:0; font-family:"Newsreader","Noto Serif SC",serif; font-size:19px; letter-spacing:-.025em; }
.remote-status-toolbar { display:flex; align-items:center; justify-content:flex-end; gap:10px; }
.remote-status-toolbar .secondary-action { min-height:30px; padding:6px 10px; font-size:10px; }
.remote-fields { min-width:0; padding:0; margin:0; border:0; }
.remote-target-list { display:grid; gap:8px; }
.remote-target { display:grid; min-width:0; padding:14px 15px; align-items:center; grid-template-columns:18px minmax(0,1fr); gap:12px; border:1px solid var(--line); border-radius:12px; background:var(--surface-strong); cursor:pointer; }
.remote-target.selected { border-color:var(--accent); background:color-mix(in srgb,var(--accent-soft) 58%,var(--surface-strong)); }
.remote-target.unavailable { opacity:.72; cursor:not-allowed; }
.remote-target input { position:absolute; opacity:0; pointer-events:none; }
.remote-target-mark { width:14px; height:14px; border:1px solid var(--line-strong); border-radius:50%; box-shadow:inset 0 0 0 3px var(--surface-strong); }
.remote-target.selected .remote-target-mark { border-color:var(--accent); background:var(--accent); }
.remote-target-copy { display:grid; min-width:0; grid-template-columns:minmax(0,1fr) auto; gap:3px 12px; }
.remote-target-copy strong { min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.remote-target-copy small { color:var(--accent-strong); }
.remote-target-copy code,.remote-target-copy em { grid-column:1/-1; overflow-wrap:anywhere; color:var(--muted); font-size:10px; font-style:normal; }
.remote-target-copy em { color:var(--warning); }
.remote-empty { padding:24px; border-block:1px solid var(--line); color:var(--muted); text-align:center; }
.remote-hint { color:var(--muted); font-size:12px; line-height:1.65; overflow-wrap:anywhere; }
.remote-success { color:var(--success); font-size:12px; }
.remote-actions { display:flex; margin-top:12px; flex-wrap:wrap; gap:8px; }
.remote-restore-actions { padding-top:12px; border-top:1px solid var(--line); }
.remote-capability { padding:18px 0; border-bottom:1px solid var(--line); }
.remote-capability h3 { margin:0 0 12px; font-size:16px; }
.remote-choice { display:flex; align-items:center; gap:10px; font-weight:650; }
.remote-choice input { accent-color:var(--accent-strong); }
.remote-capability > p { display:flex; align-items:baseline; flex-wrap:wrap; gap:8px; }
.remote-port { display:flex; margin:14px 0; align-items:center; justify-content:space-between; gap:16px; }
.remote-port input { width:110px; padding:8px; border:1px solid var(--line-strong); border-radius:8px; background:var(--surface-strong); }
.remote-port-pair { display:flex; margin:14px 0 0; align-items:center; justify-content:space-between; gap:16px; }
.remote-port-pair dt { color:var(--muted); font-size:12px; }.remote-port-pair dd { margin:0; }
.remote-port-footer { display:flex; align-items:center; justify-content:space-between; gap:18px; }
.cc-detection { color:var(--warning); font-size:12px; }.cc-detection[data-state=confirmed] { color:var(--success); }.cc-detection[data-state=notDetected] { color:var(--danger); }
.remote-check-group { padding:18px 0 4px; border-bottom:1px solid var(--line); }
.remote-check-group > header { margin-bottom:8px; }
.remote-check-group > header h3 { margin:0; font-size:16px; }
.remote-check-group > header p { max-width:68ch; margin:5px 0 0; color:var(--muted); font-size:11px; line-height:1.55; }
.remote-check-group .remote-port-pair { padding:10px 0 12px; margin:0; border-top:1px solid var(--line); }
.remote-check-group .remote-choice { color:var(--muted); font-size:10px; font-weight:600; white-space:nowrap; }
.remote-selected-target { display:flex; padding-bottom:18px; align-items:flex-start; justify-content:space-between; gap:20px; border-bottom:1px solid var(--line); }
.remote-selected-target > div { display:grid; min-width:0; gap:4px; }.remote-selected-target span,.remote-selected-target code { color:var(--muted); font-size:11px; overflow-wrap:anywhere; }
.remote-capability dl { display:grid; grid-template-columns:64px minmax(0,1fr); gap:8px; margin:0; font-size:12px; }.remote-capability dt { color:var(--muted); }.remote-capability dd { margin:0; overflow-wrap:anywhere; }
.remote-health,.remote-next-section,.remote-vscode { padding:20px 0; border-top:1px solid var(--line); }
.remote-next-heading { padding:26px 0 4px; border-top:1px solid var(--line); }.remote-next-heading strong { color:var(--success); font-size:12px; }.remote-next-heading h3 { margin:5px 0 0; font-family:"Newsreader","Noto Serif SC",serif; font-size:19px; letter-spacing:-.025em; }
.remote-health h3,.remote-next-section h3 { margin:0 0 14px; font-size:16px; }
.remote-tool-status { display:grid; min-height:36px; padding:7px 0; align-items:center; grid-template-columns:minmax(150px,.8fr) minmax(0,1fr); gap:16px; border-top:1px solid var(--line); font-size:11px; }
.remote-tool-status span:first-child { color:var(--muted); }
.remote-next-section ol { padding-left:22px; color:var(--muted); font-size:12px; line-height:1.8; }
.remote-next-section pre { padding:12px; overflow-wrap:anywhere; white-space:pre-wrap; border:1px solid var(--line); border-radius:8px; background:var(--surface-strong); font-size:11px; line-height:1.65; }
.remote-bridge-dialog pre { padding:12px; overflow-wrap:anywhere; white-space:pre-wrap; border:1px solid var(--line); border-radius:8px; background:var(--surface); font-size:11px; line-height:1.65; }
.remote-command { display:grid; min-width:0; padding:10px 0; align-items:center; grid-template-columns:90px minmax(0,1fr) auto; gap:12px; border-top:1px solid var(--line); }.remote-command span { color:var(--muted); font-size:11px; }.remote-command code { min-width:0; overflow-wrap:anywhere; }.remote-command button { padding:6px 8px; border-radius:8px; color:var(--accent-strong); background:var(--accent-soft); cursor:pointer; font-size:10px; }
.remote-command em { color:var(--muted); font-size:11px; font-style:normal; }
.remote-error,.remote-danger { color:var(--danger); }.remote-error { font-size:12px; line-height:1.65; }.remote-feedback { min-height:18px; color:var(--muted); font-size:12px; }.remote-fields:disabled { opacity:.7; }
.remote-disconnect-dialog { width:min(520px,calc(100vw - 40px)); }
.remote-auth-dialog { width:min(540px,calc(100vw - 40px)); }
.remote-auth-heading { display:flex; align-items:center; justify-content:space-between; gap:18px; }
.remote-auth-heading h2 { margin:0; font-family:"Newsreader","Noto Serif SC",serif; font-size:22px; letter-spacing:-.025em; }
.remote-auth-heading .check-status { flex:none; }
.remote-auth-context { margin-top:22px; }
.remote-auth-context h3 { margin:0; font-size:16px; letter-spacing:-.01em; }
.remote-auth-context p { max-width:62ch; margin:6px 0 0; color:var(--muted); font-size:12px; line-height:1.65; }
.remote-auth-terminal { margin:16px 0; overflow:hidden; border-radius:12px; background:#1a1a19; color:#f6f3ec; box-shadow:0 10px 28px rgba(20,20,19,.12); }
.remote-auth-terminal > span { display:block; padding:8px 12px; border-bottom:1px solid #343431; color:#aaa69e; font-size:10px; font-weight:650; }
.remote-auth-terminal pre { min-height:62px; max-height:132px; padding:13px 14px; margin:0; overflow:auto; white-space:pre-wrap; overflow-wrap:anywhere; color:inherit; background:transparent; font:11px/1.65 ui-monospace,SFMono-Regular,Consolas,monospace; }
.remote-auth-fingerprint { display:grid; padding:11px 0; margin:0 0 16px; grid-template-columns:100px minmax(0,1fr); gap:12px; border-block:1px solid var(--line); font-size:11px; }
.remote-auth-fingerprint dt { color:var(--muted); }.remote-auth-fingerprint dd { min-width:0; margin:0; overflow-wrap:anywhere; }
.remote-auth-field { display:grid; gap:7px; color:var(--muted); font-size:11px; font-weight:650; }
.remote-auth-field input { width:100%; min-height:38px; padding:8px 10px; border:1px solid var(--line-strong); border-radius:9px; color:var(--text); background:var(--surface-strong); outline:none; }
.remote-auth-field input:focus { border-color:var(--accent); box-shadow:0 0 0 3px var(--accent-soft); }
.remote-auth-privacy { margin:14px 0 0; font-size:11px; line-height:1.6; }
.remote-auth-error { margin:14px 0 0; }
@media (max-width:680px) {
  .remote-page-intro,.remote-port-footer,.remote-workspace-heading { align-items:flex-start; flex-direction:column; }
  .remote-status-toolbar { width:100%; justify-content:space-between; }
  .remote-steps { grid-template-columns:1fr 1fr; }.remote-steps li:nth-child(2)::after { display:none; }
  .remote-workspace { padding:22px 20px; }
  .remote-tool-status { grid-template-columns:1fr; gap:4px; }
  .remote-command { grid-template-columns:1fr; gap:6px; }.remote-command button { justify-self:start; }
}
</style>
