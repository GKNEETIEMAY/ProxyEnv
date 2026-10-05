<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { ActiveProxyContext, CheckState } from "../../../shared/types";
import { bridgeError, bridgeErrorCode, type RemoteBridgeCopy } from "../../../shared/i18n/remote-bridge";
import { copyText } from "../../../shared/utils/clipboard";
import { withoutWindowsExtendedPathPrefix } from "../../../shared/utils/path";
import CheckRow from "../../../shared/components/CheckRow.vue";
import LastChecked from "../../../shared/components/LastChecked.vue";
import StatusIndicator from "../../../shared/components/StatusIndicator.vue";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
import FieldValidation from "../../../shared/components/FieldValidation.vue";
import RemoteToolDialog from "./RemoteToolDialog.vue";
import RemoteTargetGroups from "./RemoteTargetGroups.vue";
import RemoteEmptyMascot from "./RemoteEmptyMascot.vue";
import { remoteToolAdapters, type RemoteToolAdapter, type RemoteToolId } from "../tool-adapters";
import {
  remoteBackend,
  targetLabel,
  type BridgeRequest,
  type BridgeSummary,
  type CcDetection,
  type ManualConnectionInput,
  type PortAllocation,
  type RemoteTarget,
  type RemoteSkill,
  type RemoteToolVerification,
  type ProfileSyncState,
  type RemoteRuntimeState,
  type SshAuthOperation,
  type SshAuthSnapshot,
} from "../state";

const props = defineProps<{ copy: RemoteBridgeCopy; activeProxy: ActiveProxyContext; summary: BridgeSummary; ccDetection: CcDetection | null; reviewPreview?: boolean; visible?: boolean }>();
const emit = defineEmits<{ refresh: []; connected: [summary: BridgeSummary] }>();
const toolDialog = ref<InstanceType<typeof RemoteToolDialog>>();
const confirmation = ref<HTMLDialogElement>();
const authDialog = ref<HTMLDialogElement>();
const authInput = ref<HTMLInputElement>();
const connectionDialog = ref<HTMLDialogElement>();
const removeConnectionDialog = ref<HTMLDialogElement>();
const targetCopiesStorageKey = "proxyenv.remoteBridge.targetCopies";
function storedTargetCopies(): { hidden: string[]; names: Record<string, string> } {
  try {
    const parsed = JSON.parse(localStorage.getItem(targetCopiesStorageKey) ?? "null");
    return {
      hidden: Array.isArray(parsed?.hidden) ? parsed.hidden.filter((value: unknown): value is string => typeof value === "string") : [],
      names: parsed?.names && typeof parsed.names === "object" ? parsed.names : {},
    };
  } catch {
    return { hidden: [], names: {} };
  }
}
const storedCopies = storedTargetCopies();
const targets = ref<RemoteTarget[]>([]);
const hiddenSourceTargets = ref(new Set(storedCopies.hidden));
const sourceTargetNames = ref(new Map(Object.entries(storedCopies.names)));
const targetId = ref("");
const editingManualId = ref<string | null>(null);
const copyingSourceId = ref<string | null>(null);
const connectionToRemove = ref<RemoteTarget | null>(null);
const lastTargetStorageKey = "proxyenv.remoteBridge.lastTargetId";

function persistTargetCopies() {
  try {
    localStorage.setItem(targetCopiesStorageKey, JSON.stringify({
      hidden: [...hiddenSourceTargets.value],
      names: Object.fromEntries(sourceTargetNames.value),
    }));
  } catch {
    // Display copies remain usable for the current session when storage is unavailable.
  }
}
const checked = ref(false);
const proxy = ref(true);
const cc = ref(false);
const ccPreferred = ref(true);
const proxyPort = ref(0);
const ccPort = ref(0);
const runtimeExpectedPort = ref<number | null>(null);
const runtimePortConflict = ref(false);
const ccLocalPort = ref(props.summary.cc?.local.port ?? props.ccDetection?.localPort ?? 15721);
const ccDetection = ref<CcDetection>(props.ccDetection ?? { state: "notDetected", localPort: ccLocalPort.value });
type CheckSnapshot = { state: CheckState; checkedAt: number | null };
const sshCheck = ref<CheckSnapshot>({ state: "idle", checkedAt: null });
const serverInternetCheck = ref<CheckSnapshot>({ state: "idle", checkedAt: null });
const localProxyCheck = ref<CheckSnapshot>({ state: "idle", checkedAt: null });
const ccCheck = ref<CheckSnapshot>({ state: "idle", checkedAt: null });
let networkRefreshRevision = 0;
const busy = ref(false);
const error = ref<unknown>();
const feedback = ref<"copied" | "tested" | "ports" | "terminal">();
const vscodeOpened = ref(false);
const authSession = ref<SshAuthSnapshot>();
const lastAuthPrompt = ref<SshAuthSnapshot["prompt"]>(null);
const authResponse = ref("");
const submittedAuthMask = ref("");
const submittedAuthPromptId = ref<string>();
const authSubmitting = ref(false);
const authCopyState = ref<"idle" | "copied" | "failed">("idle");
const showAuthDiagnostic = ref(false);
const advancedView = ref(false);
type ConnectionPhase = "idle" | "checking" | "authenticating" | "building" | "succeeded" | "failed";
const connectionPhase = ref<ConnectionPhase>("idle");
const establishingBridge = computed(() => ["checking", "authenticating", "building"].includes(connectionPhase.value));
const authRetryContext = ref<{ operation: SshAuthOperation; request: BridgeRequest | null }>();
const newConnection = ref<ManualConnectionInput>({ displayName: "", destination: "", port: 22, authentication: "password", identityFile: null });
const pickingIdentity = ref(false);
const identitySelectionError = ref<unknown>();
const identityErrorText = computed(() => identitySelectionError.value ? bridgeError(identitySelectionError.value, props.copy) : "");
const connectionSaveError = ref<unknown>();
const connectionSaveErrorText = computed(() => connectionSaveError.value ? bridgeError(connectionSaveError.value, props.copy) : "");
const connectionNameLimit = 32;
type ConnectionField = "displayName" | "destination" | "port" | "identityFile";
const connectionTouched = ref<Record<ConnectionField, boolean>>({ displayName: false, destination: false, port: false, identityFile: false });
let authPollTimer: ReturnType<typeof setTimeout> | undefined;
let skillsPollTimer: ReturnType<typeof setTimeout> | undefined;
let authBeginRevision = 0;
const remoteTools = computed(() => remoteToolAdapters.map((adapter) => ({
  adapter,
  inspection: adapter.inspect(props.summary),
  launch: adapter.launch(props.summary),
})));
const skills = ref<RemoteSkill[]>([]);
type RemoteSkillGroup = {
  name: string;
  agents: Array<{ tool: RemoteToolId; label: string; skill?: RemoteSkill }>;
};
const skillGroups = computed<RemoteSkillGroup[]>(() => {
  const grouped = new Map<string, Partial<Record<RemoteToolId, RemoteSkill>>>();
  for (const skill of skills.value) {
    const group = grouped.get(skill.name) ?? {};
    group[skill.tool] = skill;
    grouped.set(skill.name, group);
  }
  return [...grouped.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, group]) => ({
      name,
      agents: [
        { tool: "codex" as const, label: "Codex", skill: group.codex },
        { tool: "claude" as const, label: "Claude Code", skill: group.claude },
      ],
    }));
});
const synchronizedSkillCount = computed(() => skills.value.filter((skill) => skill.state === "synced").length);
const skillsSummaryState = computed<CheckState>(() => {
  if (skills.value.length === 0) return "disabled";
  if (synchronizedSkillCount.value === skills.value.length) return "healthy";
  return "warning";
});
const skillsSummaryLabel = computed(() => props.copy.rbSkillsSummary
  .replace("{synced}", String(synchronizedSkillCount.value))
  .replace("{total}", String(skills.value.length)));
const postConnectRows = computed(() => [
  { label: props.copy.rbPostEnvironment, state: props.summary.sessionEnvironmentState },
  { label: props.copy.rbPostVscode, state: props.summary.vscodeState },
  { label: props.copy.rbPostCodex, state: props.summary.codexState },
  { label: props.copy.rbPostClaude, state: props.summary.claudeState },
  { label: props.copy.rbPostSkills, state: props.summary.skillsState },
].filter((row): row is { label:string; state:RemoteRuntimeState } => !!row.state));
const postConnectLabel = computed(() => ({
  preparing: props.copy.rbPostConnectPreparing,
  ready: props.copy.rbPostConnectReady,
  partial: props.copy.rbPostConnectPartial,
  idle: "",
})[props.summary.postConnectStatus ?? "idle"]);

const selectedTarget = computed(() => targets.value.find((target) => target.id === targetId.value));
const reconnecting = computed(() => ["waiting", "retrying"].includes(props.summary.reconnectState ?? ""));
const needsAttention = computed(() => props.summary.reconnectState === "attentionRequired");
const live = computed(() => !establishingBridge.value && (["connected", "stale", "unavailable", "connecting"].includes(props.summary.status) || needsAttention.value) && !!props.summary.target);
const sshConnected = computed(() => live.value && props.summary.status !== "connecting" && !needsAttention.value);
const proxyAvailable = computed(() => props.activeProxy.available && !!props.activeProxy.candidate && props.activeProxy.candidate.protocol !== "unknown");
const ccUsable = computed(() => ccDetection.value.state === "confirmed");
const portsReady = computed(() => (!proxy.value || proxyPort.value >= 1024 && proxyPort.value <= 65535)
  && (!cc.value || ccPort.value >= 1024 && ccPort.value <= 65535)
  && (!(proxy.value && cc.value) || proxyPort.value !== ccPort.value));
const valid = computed(() => (proxy.value || cc.value)
  && (!proxy.value || proxyAvailable.value && portsReady.value)
  && (!cc.value || ccUsable.value && portsReady.value && ccLocalPort.value >= 1024 && ccLocalPort.value <= 65535));
const canStartConnection = computed(() => !!selectedTarget.value?.available
  && (proxy.value || cc.value)
  && (!proxy.value || proxyAvailable.value)
  && (!cc.value || ccUsable.value)
  && ccLocalPort.value >= 1024 && ccLocalPort.value <= 65535);
const errorText = computed(() => error.value ? bridgeError(error.value, props.copy) : "");
const feedbackText = computed(() => feedback.value ? ({ copied: props.copy.rbCopied, tested: props.copy.rbTested, ports: props.copy.rbPortsGenerated, terminal: props.copy.rbTerminalLaunched })[feedback.value] : "");
const activeTarget = computed(() => props.summary.target ?? selectedTarget.value);
const sourceLabel = (target: RemoteTarget) => ({ openssh: props.copy.rbSourceOpenSsh, vscode: props.copy.rbSourceVscode, mobaxterm: props.copy.rbSourceMoba, manual: props.copy.rbSourceManual })[target.source];
const removingImportedConnection = computed(() => connectionToRemove.value?.source !== "manual");
function targetAddress(target: RemoteTarget): string {
  if (!target.host) return target.sshAlias ?? target.displayName;
  const host = target.host.includes(":") ? `[${target.host}]` : target.host;
  return `${target.user ? `${target.user}@` : ""}${host}${target.port ? `:${target.port}` : ""}`;
}
function targetAuthenticationLabel(target: RemoteTarget): string {
  return ({
    identityFile: props.copy.rbTargetAuthIdentity,
    agent: props.copy.rbTargetAuthAgent,
    password: props.copy.rbTargetAuthPassword,
    keyboardInteractive: props.copy.rbTargetAuthPassword,
    unknown: props.copy.rbTargetAuthAutomatic,
  })[target.authenticationMethod];
}
function ipv4InputValid(value: string): boolean {
  const parts = value.split(".");
  return parts.length === 4 && parts.every((part) => /^(?:0|[1-9]\d{0,2})$/.test(part) && Number(part) <= 255);
}
function ipv6InputValid(value: string): boolean {
  if (!value.includes(":")) return false;
  try {
    return new URL(`http://[${value}]/`).hostname.length > 2;
  } catch {
    return false;
  }
}
function hostInputValid(value: string): boolean {
  if (!value || /[\s\u0000-\u001f]/.test(value)) return false;
  if (ipv4InputValid(value) || ipv6InputValid(value)) return true;
  if (value.includes(":") || /^[\d.]+$/.test(value)) return false;
  const hostname = value.endsWith(".") ? value.slice(0, -1) : value;
  return hostname.length > 0 && hostname.length <= 253 && hostname.split(".").every((label) => (
    label.length > 0
    && label.length <= 63
    && /^[A-Za-z0-9](?:[A-Za-z0-9_-]*[A-Za-z0-9])?$/.test(label)
  ));
}
function destinationInputValid(value: string): boolean {
  const destination = value.trim();
  if (!destination || destination.length > 320) return false;
  const parts = destination.split("@");
  if (parts.length > 2) return false;
  const hostInput = parts.length === 2 ? parts[1] : parts[0];
  if (parts.length === 2 && !/^[A-Za-z0-9][A-Za-z0-9._-]{0,252}$/.test(parts[0])) return false;
  const bracketed = hostInput.startsWith("[") || hostInput.endsWith("]");
  if (bracketed) {
    return hostInput.startsWith("[") && hostInput.endsWith("]") && ipv6InputValid(hostInput.slice(1, -1));
  }
  return hostInputValid(hostInput);
}
function identityPathInputValid(value: string | null): boolean {
  const path = value?.trim() ?? "";
  if (!path) return false;
  if (/^(?:\\\\[?.]\\|\/\/[?.]\/)/.test(path) || /[<>"|?*\u0000-\u001f]/.test(path)) return false;
  return ![...path].some((character, index) => character === ":" && index !== 1);
}
function displayNameInputValid(value: string): boolean {
  const name = value.trim();
  return name.length > 0 && [...name].length <= connectionNameLimit && !/[\u0000-\u001f\u007f-\u009f]/.test(name);
}
function limitConnectionName(event: Event) {
  if ((event as InputEvent).isComposing) return;
  const input = event.target as HTMLInputElement;
  const value = [...input.value].slice(0, connectionNameLimit).join("");
  if (input.value !== value) input.value = value;
  newConnection.value.displayName = value;
}
const connectionNameCount = computed(() => [...newConnection.value.displayName.trim()].length);
const connectionValid = computed(() => displayNameInputValid(newConnection.value.displayName)
  && destinationInputValid(newConnection.value.destination)
  && Number.isInteger(newConnection.value.port) && newConnection.value.port > 0
  && newConnection.value.port <= 65535
  && (newConnection.value.authentication !== "identityFile" || identityPathInputValid(newConnection.value.identityFile)));
const connectionFieldErrors = computed<Record<ConnectionField, string>>(() => ({
  displayName: (connectionTouched.value.displayName || !!newConnection.value.displayName) && !displayNameInputValid(newConnection.value.displayName) ? props.copy.rbConnectionNameInvalid : "",
  destination: (connectionTouched.value.destination || !!newConnection.value.destination) && !destinationInputValid(newConnection.value.destination) ? props.copy.rbConnectionDestinationInvalid : "",
  port: !Number.isInteger(newConnection.value.port) || newConnection.value.port < 1 || newConnection.value.port > 65535 ? props.copy.rbConnectionPortInvalid : "",
  identityFile: newConnection.value.authentication !== "identityFile" ? "" : identityErrorText.value || (connectionTouched.value.identityFile && !identityPathInputValid(newConnection.value.identityFile) ? props.copy.rbConnectionIdentityRequired : ""),
}));
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
const sshRuntimeState = computed<CheckState>(() => needsAttention.value ? "failed" : live.value ? (props.summary.status === "connecting" ? "checking" : "healthy") : bridgeCheckState(props.summary.status));
const sshRuntimeLabel = computed(() => sshRuntimeState.value === "healthy" ? props.copy.rbHealthy : genericStateLabel(sshRuntimeState.value));
const authPrompt = computed(() => authSession.value?.prompt);
const visibleAuthPrompt = computed(() => authPrompt.value ?? lastAuthPrompt.value);
const authPromptType = computed(() => visibleAuthPrompt.value?.type ?? "unknown");
const authPromptUnavailable = computed(() => authSession.value?.status === "promptUnavailable");
const authCompleting = computed(() => authSession.value?.status === "authenticated");
const authRemoteCheckFailed = computed(() => authSession.value?.status === "failed" && authSession.value.auth.authenticated);
const authDiagnosticAvailable = computed(() => ["failed", "promptUnavailable"].includes(authSession.value?.status ?? ""));
const authStatusState = computed<CheckState>(() => {
  if (connectionPhase.value === "failed") return "failed";
  if (connectionPhase.value === "succeeded") return "healthy";
  if (["failed", "promptUnavailable"].includes(authSession.value?.status ?? "")) return "failed";
  if (authSession.value?.status === "succeeded") return "healthy";
  if (authSession.value?.status === "waitingUser") return "warning";
  return "checking";
});
const authStatusLabel = computed(() => {
  if (connectionPhase.value === "building") return props.copy.rbAuthBuildingBridge;
  if (connectionPhase.value === "succeeded") return props.copy.rbAuthBridgeReady;
  if (connectionPhase.value === "failed") return props.copy.rbAuthBridgeFailed;
  if (authRemoteCheckFailed.value) return props.copy.rbAuthRemoteCheckFailure;
  if (authSession.value?.status === "failed") return props.copy.rbAuthFailure;
  if (authPromptUnavailable.value) return props.copy.rbAuthInteractionError;
  if (authSession.value?.status === "succeeded") return props.copy.rbAuthSuccess;
  if (authCompleting.value) return props.copy.rbAuthCompleting;
  if (authSession.value?.status === "waitingUser") return props.copy.rbAuthWaitingUser;
  if (authSession.value?.status === "submitting") return props.copy.rbAuthVerifying;
  if (authSession.value?.status === "waitingServer") return props.copy.rbAuthWaitingPrompt;
  return props.copy.rbAuthWaitingPrompt;
});
const authPromptCopy = computed(() => ({
  password: { title: props.copy.rbAuthPasswordTitle, description: props.copy.rbAuthPasswordDescription, label: props.copy.rbAuthPasswordLabel, action: props.copy.rbAuthConnect },
  keyPassphrase: { title: props.copy.rbAuthPassphraseTitle, description: props.copy.rbAuthPassphraseDescription, label: props.copy.rbAuthPassphraseLabel, action: props.copy.rbAuthContinue },
  verificationCode: { title: props.copy.rbAuthOtpTitle, description: props.copy.rbAuthOtpDescription, label: props.copy.rbAuthOtpLabel, action: props.copy.rbAuthVerify },
  hostKeyConfirmation: { title: props.copy.rbAuthHostKeyTitle, description: props.copy.rbAuthHostKeyDescription, label: "", action: props.copy.rbAuthConfirmHost },
  keyboardInteractive: { title: props.copy.rbAuthKeyboardTitle, description: props.copy.rbAuthKeyboardDescription, label: props.copy.rbAuthResponseLabel, action: props.copy.rbAuthSubmit },
  unknown: { title: props.copy.rbAuthUnknownTitle, description: props.copy.rbAuthUnknownDescription, label: props.copy.rbAuthResponseLabel, action: props.copy.rbAuthSubmit },
})[authPromptType.value]);
const authCanRespond = computed(() => authSession.value?.status === "waitingUser" && !!authPrompt.value);
const authIsHostConfirmation = computed(() => authPromptType.value === "hostKeyConfirmation");
const authInteractionVisible = computed(() => connectionPhase.value === "authenticating" && !authPromptUnavailable.value);
const authSurfaceVisible = computed(() => ["authenticating", "building", "succeeded"].includes(connectionPhase.value) && !authPromptUnavailable.value);
const authInputValue = computed(() => authSubmitting.value || !authCanRespond.value ? submittedAuthMask.value : authResponse.value);
const authErrorMessage = computed(() => {
  const cause = authSession.value?.error ?? error.value;
  return cause && connectionPhase.value !== "succeeded" ? bridgeError(cause, props.copy) : "";
});
const authCopyLabel = computed(() => authCopyState.value === "copied" ? props.copy.rbCopied : authCopyState.value === "failed" ? props.copy.rbAuthCopyFailed : props.copy.rbAuthCopyPrompt);

async function copyAuthPrompt() {
  // Copy only displayed OpenSSH text and a localized error, never the response or password mask.
  const prompt = visibleAuthPrompt.value?.message ?? props.copy.rbAuthWaitingPromptMessage;
  try {
    await copyText([prompt, authErrorMessage.value].filter(Boolean).join("\n\n"));
    authCopyState.value = "copied";
  } catch {
    authCopyState.value = "failed";
  }
}

watch([() => visibleAuthPrompt.value?.message, authErrorMessage], () => { authCopyState.value = "idle"; });

function toolVerificationState(verification: RemoteToolVerification): CheckState {
  if (verification === "verified") return "healthy";
  if (["authenticationRequired", "routeUnavailable", "timedOut", "failed"].includes(verification)) return "warning";
  return verification === "verifyPending" ? "warning" : "idle";
}

function runtimeCheckState(state: RemoteRuntimeState): CheckState {
  if (state === "ready") return "healthy";
  if (state === "warning") return "warning";
  if (state === "preparing") return "checking";
  return "idle";
}

function toolVerificationLabel(verification: RemoteToolVerification): string {
  return ({
    notConfigured: props.copy.rbNotConfigured,
    verifyPending: props.copy.rbToolVerifyPending,
    verified: props.copy.rbToolVerified,
    authenticationRequired: props.copy.rbToolAuthRequired,
    routeUnavailable: props.copy.rbToolRouteUnavailable,
    timedOut: props.copy.rbToolVerifyTimedOut,
    failed: props.copy.rbToolVerifyFailed,
  })[verification];
}

function claudeProfileLabel(state: ProfileSyncState | undefined): string {
  return ({
    disabled: props.copy.rbProfileDisabled,
    notStarted: props.copy.rbProfileNotStarted,
    synced: props.copy.rbProfileSynced,
    localChanged: props.copy.rbProfileLocalChanged,
    remoteChanged: props.copy.rbProfileRemoteChanged,
    conflict: props.copy.rbProfileConflict,
    invalidLocalProfile: props.copy.rbProfileInvalid,
    remoteUnavailable: props.copy.rbProfileUnavailable,
    restartRequired: props.copy.rbExtPending,
  })[state ?? "notStarted"];
}

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

const vscodeSetupWarning = ref<string>();
async function openVscode() {
  const warning = await remoteBackend.openVscode(props.summary.target!.id);
  vscodeOpened.value = true;
  vscodeSetupWarning.value = warning ?? undefined;
}
async function perform(action: () => Promise<void>, onError?: (cause: unknown) => void) {
  if (busy.value) return;
  busy.value = true;
  error.value = undefined;
  vscodeSetupWarning.value = undefined;
  feedback.value = undefined;
  try {
    await action();
  } catch (cause) {
    if (onError) onError(cause);
    else error.value = cause;
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
  const outcome = await remoteBackend.sshAuthFinish(snapshot.sessionId);
  clearAuthPoll();
  if (outcome.operation === "check" && outcome.ports) {
    usePorts(outcome.ports);
    checked.value = true;
    sshCheck.value = { state: "healthy", checkedAt: Date.now() };
    connectionPhase.value = "building";
    try {
      const summary = await connectPrepared(request());
      emit("connected", summary);
      void refreshNetworkChecks();
      connectionPhase.value = "succeeded";
      await new Promise((resolve) => setTimeout(resolve, 180));
    } catch (cause) {
      if (bridgeErrorCode(cause) === "sshAuth") {
        await beginInteractiveAuth("connect", request());
        return;
      }
      connectionPhase.value = "failed";
      error.value = cause;
      return;
    }
  } else if (outcome.operation === "connect" && outcome.summary) {
    emit("connected", outcome.summary);
    connectionPhase.value = "succeeded";
    await new Promise((resolve) => setTimeout(resolve, 180));
  }
  authDialog.value?.close();
  authSession.value = undefined;
  lastAuthPrompt.value = null;
  authResponse.value = "";
  submittedAuthMask.value = "";
  submittedAuthPromptId.value = undefined;
  connectionPhase.value = "idle";
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
  if (!["failed", "promptUnavailable"].includes(authSession.value?.status ?? "")) {
    authPollTimer = setTimeout(pollInteractiveAuth, 180);
  }
}

async function beginInteractiveAuth(operation: SshAuthOperation, request: BridgeRequest | null = null) {
  clearAuthPoll();
  const revision = ++authBeginRevision;
  error.value = undefined;
  authSession.value = undefined;
  lastAuthPrompt.value = null;
  authResponse.value = "";
  submittedAuthMask.value = "";
  submittedAuthPromptId.value = undefined;
  showAuthDiagnostic.value = false;
  authRetryContext.value = { operation, request };
  connectionPhase.value = "authenticating";
  await nextTick();
  if (!authDialog.value?.open) authDialog.value?.showModal();
  let session: SshAuthSnapshot;
  try {
    session = await remoteBackend.sshAuthBegin(operation, targetId.value, request);
  } catch (cause) {
    if (revision === authBeginRevision) {
      connectionPhase.value = "failed";
      error.value = cause;
    }
    throw cause;
  }
  if (revision !== authBeginRevision || connectionPhase.value !== "authenticating") {
    await remoteBackend.sshAuthCancel(session.sessionId).catch(() => undefined);
    return;
  }
  authSession.value = session;
  authPollTimer = setTimeout(pollInteractiveAuth, 120);
}

function updateAuthResponse(event: Event) {
  if (!authCanRespond.value || authIsHostConfirmation.value) return;
  authResponse.value = (event.target as HTMLInputElement).value;
}

async function submitInteractiveAuth() {
  const session = authSession.value;
  const promptId = session?.prompt?.id;
  if (!session || !promptId || !authCanRespond.value || authSubmitting.value) return;
  if (!authIsHostConfirmation.value && !authResponse.value) return;
  error.value = undefined;
  authSubmitting.value = true;
  const response = authResponse.value;
  submittedAuthMask.value = visibleAuthPrompt.value?.secret ? "••••••••" : "";
  submittedAuthPromptId.value = promptId;
  authResponse.value = "";
  try {
    authSession.value = authIsHostConfirmation.value
      ? await remoteBackend.sshAuthConfirmHost(session.sessionId, promptId)
      : await remoteBackend.sshAuthSubmit(session.sessionId, promptId, response);
  } catch (cause) {
    error.value = cause;
  } finally {
    authSubmitting.value = false;
  }
}

async function retryInteractiveAuth() {
  const retry = authRetryContext.value;
  const sessionId = authSession.value?.sessionId;
  if (!retry || authSubmitting.value) return;
  authSubmitting.value = true;
  clearAuthPoll();
  try {
    if (sessionId) {
      try { await remoteBackend.sshAuthCancel(sessionId); } catch { /* The timed-out PTY may already be closed. */ }
    }
    await beginInteractiveAuth(retry.operation, retry.request);
  } catch (cause) {
    error.value = cause;
  } finally {
    authSubmitting.value = false;
  }
}

watch(
  // Watch primitive values separately: identical polling snapshots must not steal text selection.
  [
    () => authSession.value?.prompt?.id,
    () => authSession.value?.prompt?.type,
    () => authSession.value?.prompt?.attempt,
    () => authSession.value?.status,
  ],
  async () => {
    if (authPrompt.value) {
      if (authPrompt.value.id !== submittedAuthPromptId.value) {
        submittedAuthMask.value = "";
        submittedAuthPromptId.value = undefined;
      }
      lastAuthPrompt.value = authPrompt.value;
    }
    if (!authCanRespond.value || authIsHostConfirmation.value) return;
    await nextTick();
    authInput.value?.focus({ preventScroll: true });
  },
);

async function cancelInteractiveAuth() {
  clearAuthPoll();
  authBeginRevision += 1;
  const sessionId = authSession.value?.sessionId;
  authSession.value = undefined;
  lastAuthPrompt.value = null;
  authResponse.value = "";
  submittedAuthMask.value = "";
  submittedAuthPromptId.value = undefined;
  showAuthDiagnostic.value = false;
  authRetryContext.value = undefined;
  connectionPhase.value = "idle";
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
      { id: "openssh|preview|aliyun-dev", displayName: "aliyun-dev", source: "openssh", sourceLabel: "OpenSSH", configPath: "~\\.ssh\\config", sshAlias: "aliyun-dev", host: "8.138.152.49", user: "lxl", port: 22, identityFile: null, authenticationMethod: "password", available: true, compatibility: "compatible", unavailableReason: null, canOpenVscode: true, canOpenMobaxterm: false },
      { id: "vscode|preview|gpu-server", displayName: "gpu-server", source: "vscode", sourceLabel: "VS Code Remote", configPath: "D:\\SSH\\vscode-config", sshAlias: "gpu-server", host: "gpu.example.test", user: "dev", port: 2222, identityFile: "C:\\Users\\example\\.ssh\\id_ed25519", authenticationMethod: "identityFile", available: true, compatibility: "compatible", unavailableReason: null, canOpenVscode: true, canOpenMobaxterm: false },
      { id: "mobaxterm|preview|GPU Server", displayName: "GPU Server", source: "mobaxterm", sourceLabel: "MobaXterm", configPath: "~\\Documents\\MobaXterm\\MobaXterm.ini", sshAlias: null, host: "gpu.example.test", user: "dev", port: 22, identityFile: null, authenticationMethod: "unknown", available: false, compatibility: "unsupported", unavailableReason: "mobaSessionUnsupported", canOpenVscode: false, canOpenMobaxterm: false },
      { id: "manual|preview|campus-lab", displayName: "campus-lab", source: "manual", sourceLabel: "ProxyEnv", configPath: "ProxyEnv", sshAlias: null, host: "10.24.8.16", user: "student", port: 22, identityFile: null, authenticationMethod: "password", available: true, compatibility: "compatible", unavailableReason: null, canOpenVscode: true, canOpenMobaxterm: false },
    ];
    targets.value.push(...["lab-backup", "research-node"].map((displayName, index) => ({
      ...targets.value[3], id: `manual|preview|${displayName}`, displayName, host: `10.24.8.${17 + index}`,
    })));
    targetId.value = targets.value[0].id;
    return;
  }
  const next = (await remoteBackend.targets())
    .filter((target) => !hiddenSourceTargets.value.has(target.id))
    .map((target) => ({
      ...target,
      configPath: withoutWindowsExtendedPathPrefix(target.configPath),
      displayName: target.source === "manual" ? target.displayName : sourceTargetNames.value.get(target.id) ?? target.displayName,
    }));
  targets.value = next;
  const preferred = props.summary.target?.id;
  let stored = "";
  try {
    stored = localStorage.getItem(lastTargetStorageKey) ?? "";
  } catch {
    // Web storage can be disabled without blocking target discovery.
  }
  if (preferred && next.some((target) => target.id === preferred)) targetId.value = preferred;
  else if (stored && next.some((target) => target.id === stored && target.available)) targetId.value = stored;
  else if (!next.some((target) => target.id === targetId.value && target.available)) targetId.value = next.find((target) => target.available)?.id ?? next[0]?.id ?? "";
}

function usePorts(ports: PortAllocation) {
  proxyPort.value = ports.proxyPort;
  ccPort.value = ports.ccPort;
  runtimeExpectedPort.value = ports.runtimeExpectedProxyPort ?? null;
  runtimePortConflict.value = ports.runtimePortConflict ?? false;
  feedback.value = "ports";
}

function initializeCapabilities(autoEnable = true) {
  if (autoEnable || !proxyAvailable.value) proxy.value = proxyAvailable.value;
  if (live.value || establishingBridge.value) return;
  const result = props.reviewPreview ? { state: "confirmed" as const, localPort: ccLocalPort.value } : props.ccDetection;
  if (!result) {
    ccCheck.value = { state: "checking", checkedAt: null };
    return;
  }
  // Apply the cached observation in one render; never clear it during a port refresh.
  if (result.state === "confirmed") ccLocalPort.value = result.localPort;
  ccDetection.value = result;
  cc.value = result.state === "confirmed" && ccPreferred.value;
  ccCheck.value = { state: result.state === "confirmed" ? "healthy" : result.state === "listeningUnknown" ? "warning" : "failed", checkedAt: Date.now() };
}

function rememberCcChoice(event: Event) {
  ccPreferred.value = (event.target as HTMLInputElement).checked;
}

async function connectPrepared(selected: BridgeRequest): Promise<BridgeSummary> {
  try {
    await remoteBackend.preview(selected);
    return await remoteBackend.connect(selected);
  } catch (cause) {
    if (bridgeErrorCode(cause) !== "portInUse") throw cause;
    usePorts(await remoteBackend.allocatePorts(selected.targetId, false));
    const retry = request();
    await remoteBackend.preview(retry);
    return remoteBackend.connect(retry);
  }
}

function startConnection() {
  if (!canStartConnection.value || busy.value) return;
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
      const ports = props.reviewPreview
        ? { proxyPort: 23841, ccPort: 31472 }
        : await remoteBackend.check(targetId.value);
      usePorts(ports);
      checked.value = true;
      sshCheck.value = { state: "healthy", checkedAt: Date.now() };
      const summary = props.reviewPreview
        ? props.summary
        : await connectPrepared(request());
      emit("connected", summary);
      connectionPhase.value = "succeeded";
      void refreshNetworkChecks();
    } catch (cause) {
      if (bridgeErrorCode(cause) === "sshAuth") {
        busy.value = false;
        await beginInteractiveAuth("check");
        return;
      }
      sshCheck.value = { state: "failed", checkedAt: Date.now() };
      connectionPhase.value = "failed";
      error.value = cause;
    } finally {
      busy.value = false;
      emit("refresh");
    }
  })();
}

function refreshTargets() {
  checked.value = false;
  proxyPort.value = 0;
  ccPort.value = 0;
  runtimeExpectedPort.value = null;
  runtimePortConflict.value = false;
  void perform(async () => {
    await Promise.all([load(), initializeCapabilities(false)]);
  });
}

function openConnectionDialog() {
  error.value = undefined;
  identitySelectionError.value = undefined;
  connectionSaveError.value = undefined;
  connectionTouched.value = { displayName: false, destination: false, port: false, identityFile: false };
  editingManualId.value = null;
  copyingSourceId.value = null;
  newConnection.value = { displayName: "", destination: "", port: 22, authentication: "password", identityFile: null };
  connectionDialog.value?.showModal();
}

async function pickIdentityFile() {
  if (pickingIdentity.value) return;
  pickingIdentity.value = true;
  identitySelectionError.value = undefined;
  connectionSaveError.value = undefined;
  try {
    const selected = await remoteBackend.pickIdentityFile(newConnection.value.identityFile);
    if (selected && connectionDialog.value?.open && newConnection.value.authentication === "identityFile") {
      newConnection.value.identityFile = selected;
      connectionTouched.value.identityFile = true;
    }
  } catch (cause) {
    identitySelectionError.value = cause;
  } finally {
    pickingIdentity.value = false;
  }
}

function manualDestination(target: RemoteTarget): string {
  const host = target.host?.includes(":") ? `[${target.host}]` : target.host ?? "";
  return target.user ? `${target.user}@${host}` : host;
}

function saveConnection() {
  if (pickingIdentity.value) return;
  connectionTouched.value = { displayName: true, destination: true, port: true, identityFile: true };
  connectionSaveError.value = undefined;
  if (!connectionValid.value || identityErrorText.value && newConnection.value.authentication === "identityFile") {
    void nextTick(() => connectionDialog.value?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus());
    return;
  }
  void perform(async () => {
    const input = {
      ...newConnection.value,
      identityFile: newConnection.value.authentication === "identityFile" ? newConnection.value.identityFile?.trim() || null : null,
    };
    const sourceCopyId = copyingSourceId.value;
    const saved = editingManualId.value
      ? await remoteBackend.updateConnection(editingManualId.value, input)
      : await remoteBackend.addConnection(input);
    if (sourceCopyId) {
      hiddenSourceTargets.value = new Set(hiddenSourceTargets.value).add(sourceCopyId);
      sourceTargetNames.value.delete(sourceCopyId);
      persistTargetCopies();
    }
    if (editingManualId.value || sourceCopyId) {
      if (editingManualId.value) sourceTargetNames.value.delete(editingManualId.value);
      persistTargetCopies();
      await remoteBackend.clearSessionCredential().catch(() => undefined);
    }
    await load();
    targetId.value = saved.id;
    editingManualId.value = null;
    copyingSourceId.value = null;
    connectionDialog.value?.close();
  }, (cause) => {
    if (["identityFileInvalid", "identityFileUnsupported"].includes(bridgeErrorCode(cause))) identitySelectionError.value = cause;
    else connectionSaveError.value = cause;
  });
}

function openEditConnectionDialog(target: RemoteTarget | undefined = selectedTarget.value) {
  if (!target) return;
  error.value = undefined;
  identitySelectionError.value = undefined;
  connectionSaveError.value = undefined;
  connectionTouched.value = { displayName: false, destination: false, port: false, identityFile: false };
  targetId.value = target.id;
  editingManualId.value = target.source === "manual" ? target.id : null;
  copyingSourceId.value = target.source === "manual" ? null : target.id;
  newConnection.value = {
    displayName: target.displayName,
    destination: manualDestination(target),
    port: target.port ?? 22,
    authentication: target.authenticationMethod === "password" || target.authenticationMethod === "keyboardInteractive"
      ? "password"
      : target.authenticationMethod === "identityFile" && target.identityFile
        ? "identityFile"
        : "automatic",
    identityFile: target.identityFile,
  };
  connectionDialog.value?.showModal();
}

function openRemoveConnectionDialog(target: RemoteTarget) {
  connectionToRemove.value = target;
  removeConnectionDialog.value?.showModal();
}

function closeRemoveConnectionDialog() {
  removeConnectionDialog.value?.close();
  connectionToRemove.value = null;
}

function removeConnection() {
  const target = connectionToRemove.value;
  if (!target) return;
  removeConnectionDialog.value?.close();
  connectionToRemove.value = null;
  if (target.source !== "manual") {
    hiddenSourceTargets.value = new Set(hiddenSourceTargets.value).add(target.id);
    sourceTargetNames.value.delete(target.id);
    targets.value = targets.value.filter((item) => item.id !== target.id);
    if (targetId.value === target.id) {
      targetId.value = targets.value.find((item) => item.available)?.id ?? targets.value[0]?.id ?? "";
    }
    persistTargetCopies();
    return;
  }
  void perform(async () => {
    await remoteBackend.removeConnection(target.id);
    sourceTargetNames.value.delete(target.id);
    persistTargetCopies();
    if (targetId.value === target.id) targetId.value = "";
    await load();
  });
}

function cancelSetup() {
  targetId.value = "";
  checked.value = false;
  error.value = undefined;
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

function connect() {
  if (!valid.value || busy.value) return;
  busy.value = true;
  error.value = undefined;
  const selected = request();
  void remoteBackend.preview(selected).then(() => remoteBackend.connect(selected)).then((summary) => {
    emit("connected", summary);
    void refreshNetworkChecks();
  }).catch(async (cause) => {
    if (bridgeErrorCode(cause) === "portInUse") {
      try {
        usePorts(await remoteBackend.allocatePorts(selected.targetId));
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

function configure(tool: RemoteToolId) {
  const target = props.summary.target ?? selectedTarget.value;
  if (target) toolDialog.value?.open(tool, target.id, false, targetLabel(target), true);
}

function restore(tool: RemoteToolId, id = targetId.value) {
  const target = targets.value.find((candidate) => candidate.id === id) ?? props.summary.target;
  toolDialog.value?.open(tool, id, true, targetLabel(target), true);
}

function toggleTool(adapter: RemoteToolAdapter, configured: boolean) {
  const target = props.summary.target ?? selectedTarget.value;
  if (!target) return;
  if (!adapter.directToggle) {
    if (configured) restore(adapter.id, target.id);
    else configure(adapter.id);
    return;
  }
  void perform(async () => {
    const preview = await adapter.preview(target.id, configured);
    if (configured) await adapter.restore(preview.id);
    else await adapter.apply(preview.id);
  });
}

function verifyTool(adapter: RemoteToolAdapter) {
  void perform(async () => {
    await adapter.verify();
  });
}

async function refreshSkills() {
  if (props.reviewPreview) {
    skills.value = sshConnected.value ? [
      { id: "codex|release-notes", tool: "codex", name: "release-notes", hash: "review-1", fileCount: 3, totalSize: 4100, state: "synced", enabled: true },
      { id: "claude|release-notes", tool: "claude", name: "release-notes", hash: "review-2", fileCount: 3, totalSize: 4100, state: "synced", enabled: true },
      { id: "codex|rust-review", tool: "codex", name: "rust-review", hash: "review-3", fileCount: 5, totalSize: 9200, state: "synced", enabled: true },
      { id: "claude|frontend-audit", tool: "claude", name: "frontend-audit", hash: "review-4", fileCount: 4, totalSize: 6800, state: "synced", enabled: true },
    ] : [];
    return;
  }
  if (!sshConnected.value) {
    skills.value = [];
    return;
  }
  skills.value = await remoteBackend.skills();
}

function stopSkillsPolling() {
  clearTimeout(skillsPollTimer);
  skillsPollTimer = undefined;
}

function scheduleSkillsPolling() {
  stopSkillsPolling();
  if (props.reviewPreview || !sshConnected.value) return;
  skillsPollTimer = setTimeout(async () => {
    if (!busy.value) await refreshSkills().catch(() => undefined);
    scheduleSkillsPolling();
  }, 2200);
}

function skillStateLabel(skill: RemoteSkill): string {
  return ({
    notSynced: props.copy.rbSkillNotSynced,
    synced: props.copy.rbSkillSynced,
    localChanged: props.copy.rbSkillChanged,
    conflict: props.copy.rbSkillConflict,
    unavailable: props.copy.rbSkillUnavailable,
  })[skill.state];
}

function toggleSkill(skill: RemoteSkill) {
  void perform(async () => {
    if (skill.enabled) await remoteBackend.disableSkill(skill.id);
    else await remoteBackend.enableSkill(skill.id);
    await refreshSkills();
  });
}

function copyValue(value: string) {
  void perform(async () => {
    await copyText(value);
    feedback.value = "copied";
  });
}

function launchProxyTerminal() {
  void perform(async () => {
    await remoteBackend.launchProxyTerminal();
    feedback.value = "terminal";
  });
}

function launchTerminal() {
  if (props.summary.proxy) {
    launchProxyTerminal();
    return;
  }
  void perform(() => remoteBackend.launchManualTerminal());
}

function copySessionEnvironment() {
  void perform(async () => {
    await copyText(await remoteBackend.sessionEnvironmentCommand());
    feedback.value = "copied";
  });
}

function confirmDisconnect() {
  confirmation.value?.close();
  void perform(async () => {
    await remoteBackend.disconnect();
    checked.value = false;
    await load();
  });
}

watch(targetId, (nextTarget, previousTarget) => {
  if (nextTarget && !props.reviewPreview) {
    try {
      localStorage.setItem(lastTargetStorageKey, nextTarget);
    } catch {
      // Selection still works for this session when storage is unavailable.
    }
  }
  if (previousTarget && nextTarget !== previousTarget) {
    void remoteBackend.clearSessionCredential().catch(() => undefined);
  }
  networkRefreshRevision += 1;
  checked.value = false;
  proxyPort.value = 0;
  ccPort.value = 0;
  runtimeExpectedPort.value = null;
  runtimePortConflict.value = false;
  vscodeOpened.value = false;
  sshCheck.value = { state: "idle", checkedAt: null };
  serverInternetCheck.value = { state: "idle", checkedAt: null };
});
watch([() => props.ccDetection, live, establishingBridge], () => initializeCapabilities(false), { immediate: true });
watch(proxyAvailable, updateLocalProxyCheck);
watch(cc, (enabled) => {
  if (enabled && (checked.value || live.value)) void refreshNetworkChecks();
});
watch(() => props.summary.target?.id, (id) => {
  if (id && live.value) {
    targetId.value = id;
    void refreshNetworkChecks();
    void refreshSkills().catch(() => undefined);
  }
});
watch(sshConnected, (connected) => {
  if (connected) {
    void refreshSkills().catch(() => undefined);
    scheduleSkillsPolling();
  } else {
    stopSkillsPolling();
    skills.value = [];
  }
});
watch(() => props.visible, (visible) => {
  if (visible && !live.value && !busy.value) void initializeCapabilities(false);
});

onMounted(() => {
  void initializeCapabilities();
  updateLocalProxyCheck();
  void perform(load);
  if (props.summary.target) {
    void refreshNetworkChecks();
    void refreshSkills().catch(() => undefined);
  }
  scheduleSkillsPolling();
  const authReview = new URLSearchParams(window.location.search).get("impeccable-review");
  if (props.reviewPreview && authReview === "remote-connected-advanced") advancedView.value = true;
  if (props.reviewPreview && ["remote-auth", "remote-auth-rejected", "remote-auth-completing", "remote-auth-timeout", "remote-auth-unavailable"].includes(authReview ?? "")) {
    const rejected = authReview === "remote-auth-rejected";
    const unavailable = authReview === "remote-auth-unavailable";
    const completing = authReview === "remote-auth-completing";
    const completionTimeout = authReview === "remote-auth-timeout";
    connectionPhase.value = "authenticating";
    authSession.value = {
      sessionId: "review-session",
      operation: "check",
      status: unavailable ? "promptUnavailable" : completionTimeout ? "failed" : completing ? "authenticated" : "waitingUser",
      auth: { mode: "interactive", method: "password", authenticated: completing || completionTimeout, passwordStored: false },
      prompt: unavailable || completing || completionTimeout ? null : { id: "review-session:1", type: "password", message: "dev@remote's password:", secret: true, attempt: 1, target: "remote", fingerprint: null },
      diagnostic: { bytesReceived: 34, printableBytes: 18, cprRequests: 1, promptDetected: false, authMarkerDetected: completing || completionTimeout, remoteResultDetected: false, outputClosed: false },
      error: rejected ? "sshAuthRejected" : unavailable ? "sshAuthPromptUnavailable" : completionTimeout ? "sshAuthCompletionTimeout" : null,
    };
    authRetryContext.value = { operation: "check", request: null };
    void nextTick(() => authDialog.value?.showModal());
  }
});
onBeforeUnmount(() => {
  authBeginRevision += 1;
  const sessionId = authSession.value?.sessionId;
  clearAuthPoll();
  stopSkillsPolling();
  if (sessionId) void remoteBackend.sshAuthCancel(sessionId).catch(() => undefined);
});
</script>

<template>
  <main class="page remote-bridge-page" :class="{ 'remote-setup-page': !live }">
    <section class="remote-workspace" :class="{ 'remote-workspace-configure': !live, 'remote-workspace-advanced': live && advancedView, 'remote-workspace-simple': live && !advancedView }">
      <div v-if="live" class="remote-workspace-heading">
        <h1>{{ copy.rbStatus }}</h1>
        <div class="remote-status-toolbar">
          <div class="remote-view-switch" role="group" :aria-label="copy.rbViewMode">
            <button type="button" :class="{ active: !advancedView }" :aria-pressed="!advancedView" @click="advancedView = false">{{ copy.rbSimpleView }}</button>
            <button type="button" :class="{ active: advancedView }" :aria-pressed="advancedView" @click="advancedView = true">{{ copy.rbAdvancedView }}</button>
          </div>
          <LastChecked v-if="advancedView" :label="copy.rbLastChecked" :checked-at="lastNetworkChecked" />
          <button class="secondary-action" type="button" :disabled="busy || networkChecking" @click="refreshNetworkChecks">{{ copy.rbRefreshStatus }}</button>
        </div>
      </div>
      <header v-else class="remote-page-intro">
        <h1>{{ copy.rbWorkspaceTitle }}</h1>
      </header>
      <fieldset :disabled="busy" class="remote-fields">
        <div v-if="!live" class="remote-setup-grid">
          <section class="remote-connections-panel">
            <header class="remote-panel-heading">
              <h2>{{ copy.rbConnectionList }} <span>({{ targets.length }})</span></h2>
              <div class="remote-panel-actions">
                <button class="secondary-action remote-panel-action" type="button" :disabled="busy" @click="openConnectionDialog">
                  <svg aria-hidden="true" viewBox="0 0 20 20"><path d="M10 4v12M4 10h12" /></svg>
                  <span>{{ copy.rbAddConnection }}</span>
                </button>
                <button class="secondary-action remote-panel-action" type="button" :disabled="busy" @click="refreshTargets">
                  <svg aria-hidden="true" viewBox="0 0 20 20"><path d="M15.5 7A6 6 0 1 0 16 12M15.5 7V3.5M15.5 7H12" /></svg>
                  <span>{{ copy.rbRefreshShort }}</span>
                </button>
              </div>
            </header>
            <div class="remote-connection-list-scroll" @click.self="!busy && cancelSetup()">
              <RemoteTargetGroups
                v-if="targets.length"
                :targets="targets"
                :selected-id="targetId"
                :copy="copy"
                :disabled="busy"
                @select="targetId = $event"
                @edit="openEditConnectionDialog"
                @remove="openRemoveConnectionDialog"
              />
              <p v-else class="remote-empty">{{ copy.rbEmpty }}</p>
            </div>
          </section>

          <section class="remote-current-panel">
            <header class="remote-panel-heading remote-current-heading">
              <h2>{{ copy.rbCurrentConnection }}</h2>
              <span v-if="selectedTarget" class="remote-current-state" :class="{ unavailable: !selectedTarget.available }">
                <span aria-hidden="true"></span>{{ selectedTarget.available ? copy.rbTargetAvailable : copy.rbTargetUnsupported }}
              </span>
            </header>

            <div class="remote-current-content">
            <template v-if="selectedTarget">
              <div class="remote-current-summary">
                <span class="remote-server-icon" aria-hidden="true">
                  <svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="6" rx="2"/><rect x="4" y="14" width="16" height="6" rx="2"/><circle cx="8" cy="7" r="1"/><circle cx="8" cy="17" r="1"/></svg>
                </span>
                <span><strong>{{ selectedTarget.displayName }}</strong><code>{{ targetAddress(selectedTarget) }}</code><small>{{ sourceLabel(selectedTarget) }} · {{ targetAuthenticationLabel(selectedTarget) }}</small></span>
                <button class="text-action" type="button" @click="openEditConnectionDialog(selectedTarget)">{{ copy.rbConnectionEdit }}</button>
              </div>
              <p v-if="!selectedTarget.available" class="notice notice-warning">{{ copy.rbMobaUnsupported }}</p>

              <section class="remote-setup-panel">
                <header><h2>{{ copy.rbCapabilitySelection }}</h2></header>
                <div class="remote-capability-options">
                <div class="remote-capability-choice" :class="{ unavailable: !proxyAvailable }">
                  <span class="remote-capability-icon" aria-hidden="true">
                    <svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M4 12h16M12 4c2.2 2.3 3.2 5 3.2 8S14.2 17.7 12 20c-2.2-2.3-3.2-5-3.2-8S9.8 6.3 12 4Z"/></svg>
                  </span>
                  <span class="remote-capability-copy"><span class="remote-capability-title"><label for="bridge-local-network">{{ copy.rbLocalNetworkTitle }}</label><HelpTooltip :label="copy.rbLocalNetworkTitle" :text="copy.rbLocalNetworkHelp" /></span><small v-if="proxyAvailable" :title="`${activeProxy.candidate?.clientName} · ${activeProxy.candidate?.host}:${activeProxy.candidate?.port}`">{{ activeProxy.candidate?.clientName }} · {{ activeProxy.candidate?.host }}:{{ activeProxy.candidate?.port }}<template v-if="proxyPort"> → 127.0.0.1:{{ proxyPort }}</template></small><small v-else>{{ copy.rbNoProxy }}</small></span>
                  <input id="bridge-local-network" v-model="proxy" class="switch-input" type="checkbox" role="switch" :disabled="!proxyAvailable" />
                </div>
                <div class="remote-capability-choice" :class="{ unavailable: !ccUsable }">
                  <span class="remote-capability-icon" aria-hidden="true">
                    <svg viewBox="0 0 24 24"><path d="M8.5 7.5 5 12l3.5 4.5M15.5 7.5 19 12l-3.5 4.5M13.5 5l-3 14"/></svg>
                  </span>
                  <span class="remote-capability-copy"><span class="remote-capability-title"><label for="bridge-ai-route">{{ copy.rbCcUseTitle }}</label><HelpTooltip :label="copy.rbCcUseTitle" :text="copy.rbCcOpenHint" /></span><small :title="ccUsable ? `${copy.rbCcRouteDetected} · http://127.0.0.1:${ccDetection.localPort}` : ccStateLabel"><template v-if="ccUsable">{{ copy.rbCcRouteDetected }} · http://127.0.0.1:{{ ccDetection.localPort }}</template><template v-else>{{ ccStateLabel }}</template></small><small v-if="ccUsable" class="remote-capability-reminder">{{ copy.rbCcRouteReminder }}</small></span>
                  <input id="bridge-ai-route" v-model="cc" class="switch-input" type="checkbox" role="switch" :disabled="!ccUsable" @change="rememberCcChoice" />
                </div>
                </div>
              </section>
            </template>
            <div v-else class="remote-current-empty">
              <span class="remote-server-icon" aria-hidden="true"><svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="6" rx="2"/><rect x="4" y="14" width="16" height="6" rx="2"/><circle cx="8" cy="7" r="1"/><circle cx="8" cy="17" r="1"/></svg></span>
              <strong>{{ copy.rbNoSelectionTitle }}</strong>
              <p>{{ copy.rbNoSelectionHint }}</p>
              <RemoteEmptyMascot :paused="visible === false" />
            </div>

            </div>
            <footer class="remote-setup-actions">
            <button class="secondary-action" type="button" :disabled="busy || !selectedTarget" @click="cancelSetup">{{ copy.rbCancel }}</button>
            <button class="primary-action remote-connect-action" :class="{ 'is-loading': establishingBridge }" type="button" :aria-busy="establishingBridge" :disabled="busy || establishingBridge || !canStartConnection" @click="startConnection"><span>{{ establishingBridge ? copy.rbEstablishingBridge : copy.rbConnect }}</span><svg v-if="establishingBridge" class="remote-button-spinner" aria-hidden="true" viewBox="0 0 20 20"><circle cx="10" cy="10" r="7"/><path d="M10 3a7 7 0 0 1 7 7"/></svg><svg v-else aria-hidden="true" viewBox="0 0 20 20"><path d="M4 10h11M11 6l4 4-4 4" /></svg></button>
            </footer>
          </section>
        </div>

        <template v-else>
          <div v-if="activeTarget" class="remote-selected-target">
            <div><strong>{{ activeTarget.displayName }}</strong><template v-if="advancedView"><span>{{ sourceLabel(activeTarget) }}</span><code>{{ withoutWindowsExtendedPathPrefix(activeTarget.configPath) }}</code></template></div>
            <div class="remote-target-runtime">
              <StatusIndicator :state="bridgeCheckState(summary.status)" :label="copy.rbStates[summary.status]" />
              <span v-if="reconnecting" class="remote-reconnect-inline" role="status">{{ copy.rbReconnectWaiting }}</span>
            </div>
          </div>
          <p v-if="summary.status === 'stale'" class="notice notice-warning">{{ copy.rbStaleHint }}</p>
          <section v-if="summary.postConnectStatus && summary.postConnectStatus !== 'idle'" class="remote-post-connect" :class="`state-${summary.postConnectStatus}`">
            <div class="remote-section-heading">
              <div><h3>{{ copy.rbPostConnectTitle }}</h3><p class="remote-hint">{{ postConnectLabel }}</p></div>
              <StatusIndicator :state="summary.postConnectStatus === 'ready' ? 'healthy' : summary.postConnectStatus === 'partial' ? 'warning' : 'checking'" :label="postConnectLabel" />
            </div>
            <div v-if="advancedView || summary.postConnectStatus !== 'ready'" class="remote-post-connect-grid">
              <div v-for="row in postConnectRows" :key="row.label"><span>{{ row.label }}</span><StatusIndicator :state="runtimeCheckState(row.state)" :label="genericStateLabel(runtimeCheckState(row.state))" /></div>
            </div>
          </section>
          <div v-if="needsAttention" class="notice notice-warning" role="alert">
            <p>{{ copy.rbReconnectAttention }} {{ summary.error ? bridgeError(summary.error, copy) : '' }}</p>
            <button class="primary-action" type="button" :disabled="busy" @click="reconnectManually">{{ copy.rbReconnectAction }}</button>
          </div>
          <p v-if="summary.status === 'unavailable'" class="notice notice-warning">{{ copy.rbUnavailableHint }}</p>
          <section v-show="!advancedView" class="remote-overview-section">
            <h3>{{ copy.rbCapabilitySelection }}</h3>
            <div class="remote-capability-list">
              <div v-if="summary.proxy" class="remote-capability-summary">
                <span><strong>{{ copy.rbProxyUseTitle }}</strong><small>{{ summary.proxy.local.protocol }} · {{ activeProxy.candidate?.clientName ?? copy.rbHealthy }}</small></span>
                <StatusIndicator :state="bridgeCheckState(summary.proxyStatus)" :label="summary.proxyStatus ? copy.rbStates[summary.proxyStatus] : copy.rbCheckIdle" />
              </div>
              <div v-if="summary.cc" class="remote-capability-summary">
                <span><strong>{{ copy.rbCcUseTitle }}</strong><small>CC Switch</small></span>
                <StatusIndicator :state="bridgeCheckState(summary.ccStatus)" :label="summary.ccStatus ? copy.rbStates[summary.ccStatus] : copy.rbCheckIdle" />
              </div>
            </div>
          </section>

          <section v-show="advancedView" class="remote-health remote-advanced-group">
              <h3>{{ copy.rbSshHealth }}</h3>
              <CheckRow :label="copy.rbSshHealth" :state="sshRuntimeState" :state-label="sshRuntimeLabel" :checked-at="sshCheck.checkedAt" :last-checked-label="copy.rbLastChecked" :help-label="copy.rbHelpLabel" :help-headings="helpHeadings" :help-content="helpContent.ssh">
                <template v-if="advancedView" #detail><p class="check-row-detail">{{ summary.target?.displayName }}</p></template>
              </CheckRow>
              <CheckRow v-show="advancedView" :label="copy.rbServerInternet" :state="serverInternetCheck.state" :state-label="serverInternetLabel" :checked-at="serverInternetCheck.checkedAt" :last-checked-label="copy.rbLastChecked" :help-label="copy.rbHelpLabel" :help-headings="helpHeadings" :help-content="helpContent.internet" />
              <CheckRow v-if="summary.proxy" :label="copy.rbLocalProxyHealth" :state="bridgeCheckState(summary.proxyStatus)" :state-label="summary.proxyStatus ? copy.rbStates[summary.proxyStatus] : copy.rbCheckIdle" :checked-at="localProxyCheck.checkedAt" :last-checked-label="copy.rbLastChecked" :help-label="copy.rbHelpLabel" :help-headings="helpHeadings" :help-content="helpContent.proxy">
                <template v-if="advancedView" #detail><p class="check-row-detail"><code>{{ summary.proxy.local.host }}:{{ summary.proxy.local.port }}</code> → <code>127.0.0.1:{{ summary.proxy.remotePort }}</code></p></template>
              </CheckRow>
              <div v-if="summary.proxy && summary.target?.canOpenVscode" v-show="advancedView" class="remote-runtime-observation">
                <p v-if="summary.runtimeExpectedProxyPort" class="remote-hint">{{ copy.rbRuntimeSetting }} · <code>127.0.0.1:{{ summary.runtimeExpectedProxyPort }}</code></p>
                <p class="remote-hint" :class="{ 'notice notice-warning': summary.runtimeProxyMatch === 'mismatch' }">{{ summary.runtimeProxyMatch === 'mismatch' ? copy.rbRuntimeMismatch : summary.runtimeProxyMatch === 'matched' ? copy.rbRuntimeMatched : copy.rbRuntimeUnknown }}</p>
              </div>
              <CheckRow v-if="summary.cc" :label="copy.rbLocalCcHealth" :state="bridgeCheckState(summary.ccStatus)" :state-label="summary.ccStatus ? copy.rbStates[summary.ccStatus] : copy.rbCheckIdle" :checked-at="ccCheck.checkedAt" :last-checked-label="copy.rbLastChecked" :help-label="copy.rbHelpLabel" :help-headings="helpHeadings" :help-content="helpContent.cc">
                <template v-if="advancedView" #detail><p class="check-row-detail"><code>{{ summary.cc.local.host }}:{{ summary.cc.local.port }}</code> → <code>127.0.0.1:{{ summary.cc.remotePort }}</code></p></template>
              </CheckRow>
          </section>

          <header v-show="!advancedView" class="remote-next-heading"><h3>{{ copy.rbNextSteps }}</h3></header>
          <div v-show="!advancedView" class="remote-primary-actions">
            <button v-if="summary.target?.canOpenVscode" class="secondary-action" type="button" :disabled="busy || !sshConnected" @click="perform(openVscode)">{{ copy.rbVscodeOpen }}</button>
            <button v-if="summary.target?.canOpenMobaxterm" class="secondary-action" type="button" :disabled="busy || !sshConnected" @click="perform(() => remoteBackend.launchMobaxterm(summary.target!.id))">{{ copy.rbLaunchMobaxterm }}</button>
            <button class="primary-action" type="button" :disabled="busy || !sshConnected" @click="launchTerminal">{{ copy.rbOpenTerminal }}</button>
          </div>
          <p v-if="!advancedView && summary.proxy && summary.target?.canOpenVscode" class="remote-hint">{{ copy.rbVscodeAutoAuth }}</p>
          <p v-if="vscodeOpened" class="remote-success" role="status">{{ copy.rbExtOpened }}</p>

          <section v-if="summary.proxy" v-show="advancedView" class="remote-next-section remote-advanced-group">
              <h3>{{ copy.rbProxyUseTitle }}</h3>
              <p class="remote-terminal-lead">{{ copy.rbTerminalLaunchHint }}</p>
              <div class="remote-actions"><button class="secondary-action" type="button" :disabled="busy || summary.proxyStatus !== 'connected'" @click="perform(async () => { await remoteBackend.test(); feedback = 'tested'; })">{{ copy.rbTest }}</button></div>
              <p class="remote-hint">{{ copy.rbTerminalAuthHint }}</p>
              <p class="remote-hint">{{ copy.rbManagedShellScope }}</p>
              <div class="remote-actions">
                <button class="secondary-action" type="button" :disabled="busy || summary.proxyStatus !== 'connected'" @click="copySessionEnvironment">{{ copy.rbCopySessionEnvironment }}</button>
              </div>
              <p class="remote-hint">{{ copy.rbVscodeEnvironmentScope }}</p>
              <div class="remote-advanced-panel">
                <h4>{{ copy.rbAdvancedCopy }}</h4>
                <div class="remote-actions">
                  <button class="secondary-action" type="button" :disabled="busy || summary.proxyStatus !== 'connected'" @click="perform(() => remoteBackend.launchManualTerminal())">{{ copy.rbLaunchManualTerminal }}</button>
                </div>
              </div>
          </section>

          <section v-if="summary.cc" class="remote-next-section" :class="{ 'remote-advanced-group': advancedView }">
              <h3>{{ copy.rbToolsTitle }}</h3>
              <div class="remote-tool-access-list">
                <label v-for="tool in remoteTools" :key="tool.adapter.id" class="remote-tool-access">
                  <span class="remote-tool-access-copy"><strong>{{ tool.adapter.displayName }}</strong><small>{{ toolVerificationLabel(tool.inspection.verification) }}</small><small v-if="tool.adapter.id === 'claude' && tool.inspection.configured">{{ claudeProfileLabel(summary.claudeProfileState) }}</small></span>
                  <span class="remote-tool-access-control">
                    <span>{{ tool.inspection.configured ? copy.rbAccessEnabled : copy.rbAccessDisabled }}</span>
                    <input class="switch-input" type="checkbox" role="switch" :checked="tool.inspection.configured" :disabled="busy" :aria-label="`${tool.adapter.displayName} · ${tool.inspection.configured ? copy.rbAccessEnabled : copy.rbAccessDisabled}`" @click.prevent="toggleTool(tool.adapter, tool.inspection.configured)" />
                  </span>
                </label>
              </div>
              <div v-for="tool in remoteTools" v-show="advancedView" :key="tool.adapter.id" class="remote-command"><span>{{ tool.adapter.displayName }}</span><template v-if="tool.launch"><code>{{ tool.launch }}</code><button type="button" :aria-label="copy.rbCopyLaunch" @click="copyValue(tool.launch)">{{ copy.rbCopyLaunch }}</button></template><em v-else>{{ copy.rbCliOverlayMissing }}</em></div>
              <template v-if="advancedView">
                <template v-for="tool in remoteTools" :key="`${tool.adapter.id}-verify`">
                  <div v-if="tool.inspection.configured && tool.inspection.verificationSupported" class="remote-tool-verification">
                    <p class="remote-hint">{{ copy.rbVerifyClaudeHint }}</p>
                    <button class="secondary-action" type="button" :disabled="busy || summary.ccStatus !== 'connected'" @click="verifyTool(tool.adapter)">{{ tool.adapter.verifyLabel(copy) }}</button>
                  </div>
                </template>
              </template>
          </section>

          <section v-if="advancedView || skills.length > 0" class="remote-next-section remote-skills">
              <div class="remote-section-heading"><div><h3>{{ copy.rbSkillsTitle }}</h3><p v-if="advancedView" class="remote-hint">{{ copy.rbSkillsHint }}</p></div><StatusIndicator v-if="!advancedView" :state="skillsSummaryState" :label="skillsSummaryLabel" /></div>
              <p v-if="advancedView && skills.length === 0" class="remote-hint">{{ copy.rbSkillsEmpty }}</p>
              <div v-if="skillGroups.length" v-show="advancedView" class="remote-tool-access-list remote-skill-list">
                <div v-for="group in skillGroups" :key="group.name" class="remote-tool-access remote-skill-row">
                  <span class="remote-tool-access-copy"><strong>{{ group.name }}</strong></span>
                  <span class="remote-skill-agents" role="group" :aria-label="group.name">
                    <label v-for="agent in group.agents" :key="agent.tool" class="remote-skill-agent" :class="{ unavailable: !agent.skill }">
                      <input class="remote-skill-checkbox" type="checkbox" :checked="!!agent.skill?.enabled" :disabled="!agent.skill || busy || !sshConnected" :aria-label="`${group.name} · ${agent.label} · ${agent.skill?.enabled ? copy.rbSkillDisable : copy.rbSkillEnable}`" @click.prevent="agent.skill && toggleSkill(agent.skill)" />
                      <span><strong>{{ agent.label }}</strong><small>{{ agent.skill ? skillStateLabel(agent.skill) : copy.rbSkillNotLinked }}</small></span>
                    </label>
                  </span>
                </div>
              </div>
              <p v-if="skills.some((skill) => skill.enabled)" v-show="advancedView" class="remote-hint">{{ copy.rbSkillRestart }}</p>
          </section>

          <section v-show="advancedView" class="remote-vscode remote-advanced-group">
              <h3>{{ copy.rbDiagnostics }}</h3>
              <p class="remote-hint">{{ copy.rbVscodeHint }}</p>
              <div class="remote-actions">
              <button v-if="summary.target && summary.target.source !== 'mobaxterm'" class="secondary-action" type="button" @click="perform(() => remoteBackend.openTargetConfig(summary.target!.id))">{{ copy.rbOpenSshConfig }}</button>
              <button v-if="summary.target && summary.target.source !== 'manual'" class="secondary-action" type="button" @click="perform(async () => remoteBackend.revealTargetConfig(summary.target!.id))">{{ copy.rbRevealConfig }}</button>
              <button v-if="summary.target?.source === 'vscode'" class="secondary-action" type="button" @click="perform(() => remoteBackend.openVscodeSettings())">{{ copy.rbOpenVscodeSettings }}</button>
              </div>
          </section>
          <div class="confirmation-actions"><button class="secondary-action remote-danger" type="button" :disabled="busy" @click="confirmation?.showModal()">{{ copy.rbDisconnect }}</button></div>
        </template>
      </fieldset>

      <p v-if="vscodeSetupWarning" class="remote-error" role="alert">{{ copy.rbVscodeSetupWarning }} {{ bridgeError(vscodeSetupWarning, copy) }}</p>
      <p v-if="errorText" class="remote-error" role="alert">{{ errorText }}</p>
      <p v-if="feedbackText && !establishingBridge && !busy" class="remote-feedback" role="status">{{ feedbackText }}</p>
    </section>
  </main>

  <dialog ref="connectionDialog" class="confirmation-dialog remote-connection-dialog" aria-labelledby="remote-connection-title" @cancel.prevent="connectionDialog?.close()">
    <form novalidate @submit.prevent="saveConnection" @input="connectionSaveError = undefined">
      <div class="remote-connection-heading"><h2 id="remote-connection-title">{{ editingManualId ? copy.rbConnectionEditTitle : copyingSourceId ? copy.rbConnectionCopyTitle : copy.rbConnectionTitle }}</h2><HelpTooltip v-if="editingManualId || copyingSourceId" :label="editingManualId ? copy.rbConnectionEditTitle : copy.rbConnectionCopyTitle" :text="editingManualId ? copy.rbConnectionEditHint : copy.rbConnectionCopyEditHint" /></div>
      <div class="remote-connection-fields">
        <label><span class="remote-connection-label">{{ copy.rbConnectionDisplayName }}<span class="remote-name-count" :class="{ invalid: connectionFieldErrors.displayName }">{{ connectionNameCount }}/{{ connectionNameLimit }}</span><FieldValidation id="remote-name-error" :message="connectionFieldErrors.displayName" /></span><input v-model="newConnection.displayName" type="text" :maxlength="connectionNameLimit * 2" autocomplete="off" required :aria-invalid="!!connectionFieldErrors.displayName" :aria-describedby="connectionFieldErrors.displayName ? 'remote-name-error' : undefined" @input="limitConnectionName" @compositionend="limitConnectionName" @blur="connectionTouched.displayName = true" /></label>
        <label><span class="remote-connection-label">{{ copy.rbConnectionDestination }}<HelpTooltip :label="copy.rbConnectionDestination" :text="copy.rbConnectionDestinationHint" /><FieldValidation id="remote-destination-error" :message="connectionFieldErrors.destination" /></span><input v-model="newConnection.destination" type="text" maxlength="320" autocomplete="off" placeholder="student@lab.example.edu" required :aria-describedby="connectionFieldErrors.destination ? 'remote-destination-error' : undefined" :aria-invalid="!!connectionFieldErrors.destination" @blur="connectionTouched.destination = true" /></label>
        <label><span class="remote-connection-label">{{ copy.rbConnectionPort }}<HelpTooltip :label="copy.rbConnectionPort" :text="copy.rbConnectionPortHint" /><FieldValidation id="remote-port-error" :message="connectionFieldErrors.port" /></span><input v-model.number="newConnection.port" type="number" min="1" max="65535" step="1" required :aria-invalid="!!connectionFieldErrors.port" :aria-describedby="connectionFieldErrors.port ? 'remote-port-error' : undefined" /></label>
        <fieldset class="remote-auth-choice">
          <legend>{{ copy.rbConnectionAuthentication }}</legend>
          <label><input v-model="newConnection.authentication" type="radio" value="password" /><span class="remote-auth-option-label"><strong>{{ copy.rbConnectionAuthPassword }}</strong><HelpTooltip :label="copy.rbConnectionAuthPassword" :text="copy.rbConnectionAuthPasswordHint" /></span></label>
          <label><input v-model="newConnection.authentication" type="radio" value="automatic" /><span class="remote-auth-option-label"><strong>{{ copy.rbConnectionAuthAutomatic }}</strong><HelpTooltip :label="copy.rbConnectionAuthAutomatic" :text="copy.rbConnectionAuthAutomaticHint" /></span></label>
          <div class="remote-identity-option" :class="{ invalid: connectionFieldErrors.identityFile }">
            <label><input v-model="newConnection.authentication" type="radio" value="identityFile" /><span><span class="remote-connection-label"><strong>{{ copy.rbConnectionAuthIdentity }}</strong><HelpTooltip :label="copy.rbConnectionIdentityFile" :text="copy.rbConnectionIdentityHint" /><FieldValidation id="remote-identity-error" :message="connectionFieldErrors.identityFile" /></span><span class="remote-identity-name" :class="{ inactive: newConnection.authentication !== 'identityFile' }" :title="newConnection.identityFile ?? undefined">{{ newConnection.identityFile ? withoutWindowsExtendedPathPrefix(newConnection.identityFile) : copy.rbConnectionIdentityEmpty }}</span></span></label>
            <button v-if="newConnection.authentication === 'identityFile'" class="secondary-action" type="button" :disabled="busy || pickingIdentity" :aria-invalid="!!connectionFieldErrors.identityFile" :aria-describedby="connectionFieldErrors.identityFile ? 'remote-identity-error' : undefined" @click="pickIdentityFile">{{ pickingIdentity ? copy.rbConnectionIdentityChecking : copy.rbConnectionIdentityBrowse }}</button>
          </div>
        </fieldset>
      </div>
      <p v-if="connectionSaveErrorText" class="remote-connection-error" role="alert">{{ connectionSaveErrorText }}</p>
      <div class="confirmation-actions"><button class="secondary-action" type="button" :disabled="busy || pickingIdentity" @click="connectionDialog?.close()">{{ copy.rbCancel }}</button><button class="primary-action" type="submit" :disabled="busy || pickingIdentity">{{ editingManualId ? copy.rbConnectionUpdate : copyingSourceId ? copy.rbConnectionCopySave : copy.rbConnectionSave }}</button></div>
    </form>
  </dialog>

  <dialog ref="removeConnectionDialog" class="confirmation-dialog remote-disconnect-dialog" aria-labelledby="remote-remove-connection-title" @cancel.prevent="closeRemoveConnectionDialog">
    <form @submit.prevent="removeConnection">
      <h2 id="remote-remove-connection-title">{{ removingImportedConnection ? copy.rbConnectionHideTitle : copy.rbConnectionRemoveTitle }}</h2>
      <p v-if="connectionToRemove"><strong>{{ connectionToRemove.displayName }}</strong></p>
      <p>{{ removingImportedConnection ? copy.rbConnectionHideHint : copy.rbConnectionRemoveHint }}</p>
      <div class="confirmation-actions"><button class="secondary-action" type="button" autofocus @click="closeRemoveConnectionDialog">{{ copy.rbCancel }}</button><button class="primary-action" :class="{ 'danger-action': !removingImportedConnection }" type="submit" :disabled="busy">{{ removingImportedConnection ? copy.rbConnectionHide : copy.rbConnectionRemove }}</button></div>
    </form>
  </dialog>

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

      <section v-if="authSurfaceVisible || authDiagnosticAvailable || connectionPhase === 'failed'" class="remote-auth-terminal" :aria-label="copy.rbAuthPrompt">
        <header><span>{{ copy.rbAuthPrompt }}</span><button class="remote-auth-copy" type="button" :title="authCopyLabel" :aria-label="authCopyLabel" @click="copyAuthPrompt"><svg v-if="authCopyState === 'copied'" aria-hidden="true" viewBox="0 0 20 20"><path d="m4 10 4 4 8-8"/></svg><svg v-else aria-hidden="true" viewBox="0 0 20 20"><rect x="7" y="7" width="9" height="10" rx="2"/><path d="M12 7V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h2"/></svg><span>{{ authCopyLabel }}</span></button></header>
        <pre>{{ visibleAuthPrompt ? visibleAuthPrompt.message : copy.rbAuthWaitingPromptMessage }}</pre>
      </section>

      <section v-if="authDiagnosticAvailable && showAuthDiagnostic && authSession" class="remote-auth-diagnostic" aria-live="polite">
        <dl>
          <div><dt>{{ copy.rbAuthDiagnosticBytes }}</dt><dd>{{ authSession.diagnostic.bytesReceived }}</dd></div>
          <div><dt>{{ copy.rbAuthDiagnosticPrintable }}</dt><dd>{{ authSession.diagnostic.printableBytes }}</dd></div>
          <div><dt>{{ copy.rbAuthDiagnosticCpr }}</dt><dd>{{ authSession.diagnostic.cprRequests }}</dd></div>
          <div><dt>{{ copy.rbAuthDiagnosticPrompt }}</dt><dd>{{ authSession.diagnostic.promptDetected ? copy.rbAuthDiagnosticYes : copy.rbAuthDiagnosticNo }}</dd></div>
          <div><dt>{{ copy.rbAuthDiagnosticMarker }}</dt><dd>{{ authSession.diagnostic.authMarkerDetected ? copy.rbAuthDiagnosticYes : copy.rbAuthDiagnosticNo }}</dd></div>
          <div><dt>{{ copy.rbAuthDiagnosticResult }}</dt><dd>{{ authSession.diagnostic.remoteResultDetected ? copy.rbAuthDiagnosticYes : copy.rbAuthDiagnosticNo }}</dd></div>
          <div><dt>{{ copy.rbAuthDiagnosticClosed }}</dt><dd>{{ authSession.diagnostic.outputClosed ? copy.rbAuthDiagnosticYes : copy.rbAuthDiagnosticNo }}</dd></div>
        </dl>
      </section>

      <dl v-if="authIsHostConfirmation && visibleAuthPrompt?.fingerprint" class="remote-auth-fingerprint">
        <dt>{{ copy.rbAuthFingerprint }}</dt>
        <dd><code>{{ visibleAuthPrompt.fingerprint }}</code></dd>
      </dl>

      <label v-if="authSurfaceVisible" class="remote-auth-field">
        <span :class="{ 'remote-auth-field-placeholder': !visibleAuthPrompt }">{{ visibleAuthPrompt ? (authPromptCopy.label || copy.rbAuthResponseLabel) : copy.rbAuthResponseLabel }}</span>
        <input ref="authInput" :value="authInputValue" :type="visibleAuthPrompt?.secret !== false ? 'password' : 'text'" autocomplete="off" maxlength="4096" :aria-invalid="authSession?.error === 'sshAuthRejected'" :aria-describedby="authErrorMessage ? 'remote-auth-error' : undefined" :disabled="authIsHostConfirmation || !authCanRespond || authSubmitting" @input="updateAuthResponse" />
      </label>

      <div id="remote-auth-error" class="remote-auth-error-slot" :class="{ 'has-error': !!authErrorMessage }" role="alert" aria-atomic="true"><svg v-if="authErrorMessage" aria-hidden="true" viewBox="0 0 20 20"><circle cx="10" cy="10" r="7"/><path d="M10 6v5m0 3h.01"/></svg><p v-if="authErrorMessage">{{ authErrorMessage }}</p></div>
      <div class="confirmation-actions">
        <button v-if="connectionPhase === 'failed' || authSession?.status !== 'succeeded'" class="secondary-action" type="button" :disabled="authSubmitting" @click="cancelInteractiveAuth">{{ connectionPhase === 'failed' || authSession?.status === 'failed' ? copy.rbClose : copy.rbAuthCancel }}</button>
        <button v-if="authDiagnosticAvailable" class="secondary-action" type="button" :aria-expanded="showAuthDiagnostic" :disabled="authSubmitting" @click="showAuthDiagnostic = !showAuthDiagnostic">{{ showAuthDiagnostic ? copy.rbAuthHideDiagnostic : copy.rbAuthOpenDiagnostic }}</button>
        <button v-if="authPromptUnavailable" class="primary-action" type="button" :disabled="authSubmitting" @click="retryInteractiveAuth">{{ copy.rbAuthRetry }}</button>
        <button v-if="authInteractionVisible || authSurfaceVisible" class="primary-action remote-auth-connect-action" :class="{ 'is-loading': connectionPhase === 'building' || authSubmitting || authCompleting }" type="submit" :aria-busy="connectionPhase === 'building' || authSubmitting || authCompleting" :disabled="!authCanRespond || (!authIsHostConfirmation && !authResponse) || authSubmitting"><svg v-if="connectionPhase === 'building' || authSubmitting || authCompleting" class="remote-button-spinner" aria-hidden="true" viewBox="0 0 20 20"><circle cx="10" cy="10" r="7"/><path d="M10 3a7 7 0 0 1 7 7"/></svg><span>{{ connectionPhase === 'building' ? copy.rbEstablishingBridge : connectionPhase === 'succeeded' ? copy.rbAuthBridgeReady : authSubmitting || authCompleting ? copy.rbAuthVerifying : authPromptCopy.action }}</span></button>
      </div>
    </form>
  </dialog>
  <RemoteToolDialog ref="toolDialog" :copy="copy" :session-alias="summary.target?.id ?? targetId" :session-status="summary.status" @refresh="emit('refresh')" />
</template>

<style>
.remote-bridge-page { display:grid; gap:24px; }
.remote-bridge-page.remote-setup-page { height:100%; min-height:0; padding:20px 24px; gap:0; overflow:hidden; }
.remote-setup-page .remote-workspace-configure { display:flex; min-height:0; flex-direction:column; }
.remote-setup-page .remote-fields { display:flex; min-height:0; flex:1; }
.remote-setup-page .remote-setup-grid { width:100%; min-height:0; flex:1; }
.confirmation-dialog.remote-bridge-dialog { width:min(640px,calc(100vw - 40px)); max-height:calc(100vh - 40px); overflow-y:auto; }
.remote-heading { display:flex; align-items:center; justify-content:space-between; gap:16px; }
.remote-workspace { padding:22px 28px 26px; border:1px solid var(--line); border-radius:16px; background:var(--surface); box-shadow:var(--shadow); }
.remote-workspace-configure { padding:18px 20px 14px; border-color:color-mix(in srgb,var(--line) 88%,transparent); background:color-mix(in srgb,var(--surface) 97%,var(--surface-strong)); box-shadow:0 10px 34px rgba(45,39,33,.055); }
.remote-page-intro { margin:0 6px 14px; }
.remote-page-intro h1 { margin:0; font-family:"Newsreader","Noto Serif SC",serif; font-size:23px; letter-spacing:-.028em; }
.remote-page-intro p { margin:5px 0 0; color:var(--muted); font-size:12px; line-height:1.55; }
.remote-setup-grid { display:grid; min-width:0; align-items:stretch; grid-template-columns:minmax(0,.9fr) minmax(0,1.1fr); gap:14px 12px; }
.remote-connections-panel,.remote-current-panel { min-width:0; border:1px solid var(--line); border-radius:13px; background:var(--surface); box-shadow:0 4px 16px rgba(45,39,33,.035); }
.remote-connections-panel { display:flex; min-height:0; padding:16px; flex-direction:column; }
.remote-current-panel { display:flex; min-height:0; padding:16px 18px; flex-direction:column; }
.remote-current-content { min-height:0; flex:1; overflow-y:auto; scrollbar-width:thin; scrollbar-color:color-mix(in srgb,var(--muted) 16%,transparent) transparent; }
.remote-current-content:hover { scrollbar-color:color-mix(in srgb,var(--muted) 52%,transparent) transparent; }
.remote-panel-heading { display:flex; min-height:32px; margin-bottom:14px; align-items:center; justify-content:space-between; gap:14px; }
.remote-panel-heading h2 { margin:0; font-size:14px; font-weight:720; letter-spacing:-.01em; }
.remote-panel-heading h2 span { color:var(--muted); font-weight:560; }
.remote-panel-actions { display:flex; flex-wrap:wrap; justify-content:flex-end; gap:7px; }
.remote-panel-actions .secondary-action { min-height:30px; padding:5px 9px; font-size:10px; }
.remote-panel-action { display:inline-flex; align-items:center; gap:5px; }
.remote-panel-action svg { width:14px; height:14px; fill:none; stroke:currentColor; stroke-width:1.7; stroke-linecap:round; stroke-linejoin:round; }
.remote-connection-list-scroll { min-height:0; max-height:calc(var(--ssh-row-height,72px) * 5 + 24px); flex:1; overflow-y:auto; scrollbar-width:thin; scrollbar-color:color-mix(in srgb,var(--muted) 16%,transparent) transparent; scrollbar-gutter:stable; }
.remote-connection-list-scroll:hover,.remote-connection-list-scroll:focus-within { scrollbar-color:color-mix(in srgb,var(--muted) 52%,transparent) transparent; }
.remote-connection-list-scroll::-webkit-scrollbar { width:6px; }
.remote-connection-list-scroll::-webkit-scrollbar-track { background:transparent; }
.remote-connection-list-scroll::-webkit-scrollbar-thumb { border-radius:6px; background:color-mix(in srgb,var(--muted) 16%,transparent); }
.remote-connection-list-scroll:hover::-webkit-scrollbar-thumb { background:color-mix(in srgb,var(--muted) 52%,transparent); }
.remote-connection-list-scroll::-webkit-scrollbar-thumb:hover { background:var(--muted); }
.remote-current-heading { margin-bottom:10px; }
.remote-current-state { display:inline-flex; min-height:24px; padding:3px 9px; align-items:center; gap:6px; border-radius:999px; color:var(--success); font-size:10px; font-weight:680; background:color-mix(in srgb,var(--success) 10%,transparent); }
.remote-current-state > span { width:7px; height:7px; border-radius:50%; background:currentColor; }
.remote-current-state.unavailable { color:var(--danger); background:color-mix(in srgb,var(--danger) 9%,transparent); }
.remote-current-summary { display:grid; min-width:0; padding:14px; align-items:center; grid-template-columns:46px minmax(0,1fr) auto; gap:13px; border:1px solid var(--line); border-radius:11px; background:color-mix(in srgb,var(--surface) 92%,var(--surface-strong)); }
.remote-current-summary > span:not(.remote-server-icon) { display:grid; min-width:0; gap:3px; }
.remote-current-summary strong,.remote-current-summary code,.remote-current-summary small { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.remote-current-summary strong { font-size:14px; }
.remote-current-summary code { color:var(--text); font:11px/1.45 ui-monospace,SFMono-Regular,Consolas,monospace; }
.remote-current-summary small { color:var(--muted); font-size:10px; }
.remote-server-icon { display:grid; width:46px; height:46px; place-items:center; border:1px solid color-mix(in srgb,var(--accent) 18%,var(--line)); border-radius:11px; color:var(--accent-strong); background:var(--accent-soft); }
.remote-server-icon svg { width:21px; height:21px; fill:none; stroke:currentColor; stroke-width:1.7; }
.text-action { min-height:28px; padding:4px 7px; border:0; border-radius:7px; color:var(--accent-strong); background:transparent; cursor:pointer; font-size:10px; font-weight:680; }
.text-action:hover { background:var(--accent-soft); }
.text-action:focus-visible { outline:2px solid var(--focus); outline-offset:2px; }
.remote-current-empty { display:grid; min-height:255px; place-items:center; align-content:center; gap:8px; color:var(--muted); text-align:center; }
.remote-current-empty strong { color:var(--text); font-size:14px; }
.remote-current-empty p { max-width:34ch; margin:0; font-size:11px; line-height:1.55; }
.remote-workspace-heading { display:flex; margin-bottom:18px; align-items:center; justify-content:space-between; gap:18px; }
.remote-workspace-heading h1 { margin:0; font-family:"Newsreader","Noto Serif SC",serif; font-size:19px; letter-spacing:-.025em; }
.remote-status-toolbar { display:flex; align-items:center; justify-content:flex-end; gap:10px; }
.remote-status-toolbar .secondary-action { min-height:30px; padding:6px 10px; font-size:10px; }
.remote-view-switch { display:flex; padding:2px; border:1px solid var(--line); border-radius:10px; background:var(--surface-strong); }
.remote-view-switch button { min-height:26px; padding:4px 9px; border:0; border-radius:8px; color:var(--muted); background:transparent; cursor:pointer; font-size:10px; font-weight:650; }
.remote-view-switch button:hover { color:var(--ink); }
.remote-view-switch button.active { color:var(--accent-strong); background:var(--accent-soft); }
.remote-view-switch button:focus-visible { outline:2px solid var(--focus); outline-offset:2px; }
.remote-fields { min-width:0; padding:0; margin:0; border:0; }
.remote-empty { padding:24px; border-block:1px solid var(--line); color:var(--muted); text-align:center; }
.remote-hint { color:var(--muted); font-size:12px; line-height:1.65; overflow-wrap:anywhere; }
.remote-success { color:var(--success); font-size:12px; }
.remote-actions { display:flex; margin-top:12px; flex-wrap:wrap; gap:8px; }
.remote-setup-panel { padding:16px 0 2px; margin-top:16px; border-top:1px solid var(--line); }
.remote-setup-panel > header { margin-bottom:8px; }
.remote-setup-panel > header h2 { margin:0; font-size:16px; }
.remote-setup-panel > header p { max-width:68ch; margin:5px 0 0; color:var(--muted); font-size:11px; line-height:1.55; }
.remote-capability-options { display:grid; margin-top:8px; grid-auto-rows:1fr; gap:7px; }
.remote-capability-choice { display:grid; min-height:62px; padding:10px 12px; margin-top:8px; align-items:center; grid-template-columns:34px minmax(0,1fr) auto; gap:11px; border:1px solid var(--line); border-radius:10px; background:color-mix(in srgb,var(--surface) 94%,var(--surface-strong)); cursor:pointer; }
.remote-capability-choice + .remote-capability-choice { margin-top:7px; }
.remote-capability-options .remote-capability-choice { margin-top:0; }
.remote-capability-copy { display:grid; min-width:0; gap:3px; }
.remote-capability-title { display:flex; align-items:center; gap:6px; font-size:12px; font-weight:650; }
.remote-capability-title label { cursor:pointer; }
.remote-capability-choice.unavailable .help-tooltip { opacity:1; }
.remote-capability-choice strong { font-size:12px; }
.remote-capability-choice small { overflow:hidden; color:var(--muted); font-size:10px; text-overflow:ellipsis; white-space:nowrap; }
.remote-capability-choice .remote-capability-reminder { white-space:normal; line-height:1.4; }
.remote-capability-icon { display:grid; width:34px; height:34px; place-items:center; border-radius:9px; color:var(--accent-strong); background:var(--accent-soft); }
.remote-capability-icon svg { width:19px; height:19px; fill:none; stroke:currentColor; stroke-width:1.65; stroke-linecap:round; stroke-linejoin:round; }
.remote-capability-choice.unavailable { cursor:default; }
.remote-capability-choice.unavailable .remote-capability-icon,.remote-capability-choice.unavailable .switch-input { opacity:.58; }
.remote-capability-choice .switch-input { flex:none; }
.remote-requirements { padding:12px 0 2px; margin:0; }
.remote-setup-actions { display:flex; flex:none; padding-top:16px; margin-top:auto; align-items:center; justify-content:flex-end; flex-wrap:wrap; gap:9px; }
.remote-setup-actions .remote-actions { margin:0; }
.remote-connect-action { display:inline-flex; min-width:144px; align-items:center; justify-content:center; gap:7px; }
.remote-connect-action svg { width:15px; height:15px; fill:none; stroke:currentColor; stroke-width:1.7; stroke-linecap:round; stroke-linejoin:round; }
.primary-action.is-loading:disabled { opacity:.85; cursor:wait; }
.remote-button-spinner { width:15px; height:15px; flex:none; fill:none; stroke:currentColor; stroke-width:1.7; animation:remote-button-spin .85s linear infinite; }
.remote-button-spinner circle { opacity:.25; }
.remote-button-spinner path { stroke-linecap:round; }
@keyframes remote-button-spin { to { transform:rotate(360deg); } }
@media (prefers-reduced-motion:reduce) { .remote-button-spinner { animation:none; } }
.confirmation-actions .primary-action.danger-action { border-color:var(--danger); color:#fff; background:var(--danger); }
.confirmation-actions .primary-action.danger-action:hover:not(:disabled) { filter:brightness(.94); }
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
.remote-selected-target > .remote-target-runtime { display:flex; align-items:center; justify-content:flex-end; gap:9px; }
.remote-reconnect-inline { padding-left:14px; position:relative; white-space:nowrap; }
.remote-reconnect-inline::before { position:absolute; left:0; top:50%; width:7px; height:7px; border:1px solid var(--accent); border-top-color:transparent; border-radius:50%; content:""; transform:translateY(-50%); animation:remote-inline-spin .8s linear infinite; }
@keyframes remote-inline-spin { to { transform:translateY(-50%) rotate(360deg); } }
.remote-post-connect { padding:14px 0; border-bottom:1px solid var(--line); }
.remote-post-connect .remote-section-heading h3 { margin:0; font-size:13px; }
.remote-post-connect .remote-section-heading p { margin:3px 0 0; }
.remote-post-connect-grid { display:grid; margin-top:10px; grid-template-columns:repeat(2,minmax(0,1fr)); gap:7px 18px; }
.remote-post-connect-grid > div { display:flex; min-width:0; align-items:center; justify-content:space-between; gap:10px; }
.remote-post-connect-grid > div > span { color:var(--muted); font-size:10px; }
.remote-setup-target { padding:14px 0 18px; }
.remote-setup-target { flex-wrap:wrap; }
.remote-setup-target > .remote-actions { display:flex; flex-wrap:wrap; gap:8px; margin-top:0; }
.remote-setup-target .secondary-action { min-height:32px; padding:6px 10px; font-size:10px; }
.remote-connect-actions { padding-top:18px; border-top:1px solid var(--line); }
.remote-capability dl { display:grid; grid-template-columns:64px minmax(0,1fr); gap:8px; margin:0; font-size:12px; }.remote-capability dt { color:var(--muted); }.remote-capability dd { margin:0; overflow-wrap:anywhere; }
.remote-health,.remote-next-section,.remote-vscode,.remote-overview-section { padding:20px 0; border-top:1px solid var(--line); }
.remote-overview-section h3 { margin:0 0 12px; font-size:16px; }
.remote-capability-list { display:grid; border-block:1px solid var(--line); }
.remote-capability-summary { display:flex; min-height:62px; padding:10px 0; align-items:center; justify-content:space-between; gap:18px; }
.remote-capability-summary + .remote-capability-summary { border-top:1px solid var(--line); }
.remote-capability-summary > span { display:grid; min-width:0; gap:3px; }
.remote-capability-summary strong { font-size:12px; }
.remote-capability-summary small { color:var(--muted); font-size:10px; }
.remote-advanced-group > h3 { margin:0 0 14px; font-size:16px; }
.remote-next-heading { padding:26px 0 4px; border-top:1px solid var(--line); }.remote-next-heading h3 { margin:0; font-family:"Newsreader","Noto Serif SC",serif; font-size:19px; letter-spacing:-.025em; }
.remote-health h3,.remote-next-section h3 { margin:0 0 14px; font-size:16px; }
.remote-workspace-simple .remote-health .last-checked { display:none; }
.remote-primary-actions { display:flex; padding:14px 0 4px; flex-wrap:wrap; gap:8px; }
.remote-section-heading { display:flex; align-items:flex-start; justify-content:space-between; gap:18px; }
.remote-section-heading > div { min-width:0; }
.remote-section-heading h3 { margin-bottom:4px; }
.remote-section-heading .remote-hint { margin:0 0 12px; }
.remote-tool-access-list { display:grid; border-block:1px solid var(--line); }
.remote-tool-access { display:flex; min-height:58px; padding:10px 0; align-items:center; justify-content:space-between; gap:20px; cursor:pointer; }
.remote-tool-access + .remote-tool-access { border-top:1px solid var(--line); }
.remote-tool-access-copy { display:grid; min-width:0; gap:3px; }
.remote-tool-access-copy strong { font-size:12px; }
.remote-tool-access-copy small { color:var(--muted); font-size:10px; }
.remote-tool-access-control { display:flex; flex:none; align-items:center; gap:10px; color:var(--muted); font-size:10px; font-weight:650; }
.remote-tool-access .switch-input { margin:0; }
.remote-skill-row { cursor:default; }
.remote-skill-agents { display:grid; flex:none; grid-template-columns:repeat(2,minmax(118px,1fr)); gap:8px; }
.remote-skill-agent { display:grid; min-height:42px; padding:7px 9px; align-items:center; grid-template-columns:16px minmax(0,1fr); gap:8px; border:1px solid var(--line); border-radius:10px; background:var(--surface-strong); cursor:pointer; }
.remote-skill-agent:hover:not(.unavailable) { border-color:var(--line-strong); }
.remote-skill-agent:focus-within { outline:2px solid var(--focus); outline-offset:2px; }
.remote-skill-agent.unavailable { opacity:.56; cursor:default; }
.remote-skill-agent > span { display:grid; min-width:0; gap:1px; }
.remote-skill-agent strong { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:10px; }
.remote-skill-agent small { overflow:hidden; color:var(--muted); text-overflow:ellipsis; white-space:nowrap; font-size:10px; }
.remote-skill-checkbox { width:14px; height:14px; margin:0; accent-color:var(--accent-strong); cursor:pointer; }
.remote-skill-checkbox:focus-visible { outline:none; }
.remote-skill-checkbox:disabled { cursor:default; }
.remote-next-section ol { padding-left:22px; color:var(--muted); font-size:12px; line-height:1.8; }
.remote-terminal-lead { max-width:68ch; margin:0; color:var(--ink); font-size:12px; line-height:1.65; overflow-wrap:anywhere; }
.remote-advanced-panel { padding-top:14px; margin-top:14px; border-top:1px solid var(--line); }
.remote-advanced-panel h4 { margin:0 0 10px; color:var(--muted); font-size:11px; }
.remote-next-section pre { padding:12px; overflow-wrap:anywhere; white-space:pre-wrap; border:1px solid var(--line); border-radius:8px; background:var(--surface-strong); font-size:11px; line-height:1.65; }
.remote-bridge-dialog pre { padding:12px; overflow-wrap:anywhere; white-space:pre-wrap; border:1px solid var(--line); border-radius:8px; background:var(--surface); font-size:11px; line-height:1.65; }
.remote-command { display:grid; min-width:0; padding:10px 0; align-items:center; grid-template-columns:90px minmax(0,1fr) auto; gap:12px; border-top:1px solid var(--line); }.remote-command span { color:var(--muted); font-size:11px; }.remote-command code { min-width:0; overflow-wrap:anywhere; }.remote-command button { padding:6px 8px; border-radius:8px; color:var(--accent-strong); background:var(--accent-soft); cursor:pointer; font-size:10px; }
.remote-command em { color:var(--muted); font-size:11px; font-style:normal; }
.remote-tool-verification { display:flex; margin:8px 0 14px; align-items:center; justify-content:space-between; gap:16px; }
.remote-tool-verification .remote-hint { max-width:68ch; margin:0; }
.remote-error,.remote-danger { color:var(--danger); }.remote-error { font-size:12px; line-height:1.65; }.remote-feedback { min-height:18px; color:var(--muted); font-size:12px; }.remote-fields:disabled { opacity:.7; }
.remote-disconnect-dialog { width:min(520px,calc(100vw - 40px)); }
.remote-connection-dialog { width:min(520px,calc(100vw - 40px)); max-height:calc(100vh - 40px); overflow-y:auto; }
.remote-connection-heading,.remote-connection-label { display:flex; align-items:center; gap:6px; }
.remote-connection-fields { display:grid; margin-top:14px; gap:12px; }
.remote-connection-fields > label { display:grid; min-width:0; gap:7px; color:var(--muted); font-size:11px; font-weight:650; }
.remote-identity-option { display:grid; min-width:0; min-height:36px; grid-template-columns:minmax(0,1fr) auto; align-items:center; gap:8px; }
.remote-identity-option label,.remote-identity-option label > span { min-width:0; }
.remote-identity-option .remote-identity-name { display:block; overflow:hidden; color:var(--muted); font-size:11px; font-weight:400; line-height:14px; text-overflow:ellipsis; white-space:nowrap; user-select:text; }
.remote-identity-name.inactive { visibility:hidden; }
.remote-identity-option .secondary-action { min-height:30px; padding:5px 10px; margin:0; border-radius:8px; white-space:nowrap; font-size:11px; }
.remote-identity-option.invalid .secondary-action { border-color:var(--danger); }
.remote-name-count { margin-inline-start:auto; color:var(--muted); font-size:11px; font-weight:400; font-variant-numeric:tabular-nums; }
.remote-name-count.invalid { color:var(--danger); }
.remote-connection-error { margin:12px 0 0; color:var(--danger); font-size:12px; line-height:1.5; user-select:text; }
.remote-connection-fields input[type=text],.remote-connection-fields input[type=number] { width:100%; min-height:38px; padding:8px 10px; border:1px solid var(--line-strong); border-radius:8px; color:var(--text); background:var(--surface-strong); outline:none; }
.remote-connection-fields input:focus { border-color:var(--accent); box-shadow:0 0 0 3px var(--accent-soft); }
.remote-connection-fields small { color:var(--muted); font-size:10px; font-weight:400; line-height:1.55; }
.remote-connection-fields input[aria-invalid="true"] { border-color:var(--danger); }
.remote-auth-choice { display:grid; padding:12px 0; margin:0; gap:10px; border:0; border-block:1px solid var(--line); }
.remote-auth-choice legend { padding:0 0 8px; color:var(--muted); font-size:11px; font-weight:650; }
.remote-auth-choice label { display:grid; align-items:start; grid-template-columns:16px minmax(0,1fr); gap:10px; cursor:pointer; }
.remote-auth-choice label > span { display:grid; gap:3px; }.remote-auth-choice strong { color:var(--text); font-size:12px; }.remote-auth-choice input { margin-top:2px; accent-color:var(--accent-strong); }
.remote-auth-choice label > .remote-auth-option-label { display:flex; align-items:center; gap:6px; }
.confirmation-dialog.remote-auth-dialog { width:min(440px,calc(100vw - 40px)); max-height:calc(100vh - 40px); overflow:hidden; }
.confirmation-dialog.remote-auth-dialog > form { padding:22px; }
.remote-auth-dialog > form { display:flex; max-height:calc(100vh - 40px); min-height:0; overflow-y:auto; flex-direction:column; }
.remote-auth-dialog .confirmation-actions { flex:none; margin-top:18px; }
.remote-auth-heading { display:flex; align-items:center; justify-content:space-between; gap:18px; }
.remote-auth-heading h2 { margin:0; font-family:"Newsreader","Noto Serif SC",serif; font-size:19px; letter-spacing:-.025em; }
.remote-auth-heading .check-status { flex:none; }
.remote-auth-dialog { -webkit-user-select:text; user-select:text; }
.remote-auth-terminal { flex:none; margin:24px 0 16px; overflow:hidden; border-radius:12px; background:#1a1a19; color:#f1ece4; box-shadow:0 10px 28px rgba(20,20,19,.12); user-select:text; }
.remote-auth-terminal > header { display:flex; align-items:center; justify-content:space-between; gap:12px; min-height:33px; padding:3px 10px 3px 12px; border-bottom:1px solid #403b35; color:#b5ada2; font-size:10px; font-weight:650; }
.remote-auth-copy { display:inline-flex; align-items:center; gap:5px; min-height:26px; padding:3px 6px; border:0; border-radius:5px; color:#ded7cd; background:transparent; font:inherit; cursor:pointer; }
.remote-auth-copy:hover { background:#35332f; color:#fff; }
.remote-auth-copy:focus-visible { outline:2px solid var(--accent); outline-offset:2px; }
.remote-auth-copy svg { width:14px; height:14px; fill:none; stroke:currentColor; stroke-width:1.5; stroke-linecap:round; stroke-linejoin:round; }
.remote-auth-terminal pre { min-height:48px; max-height:112px; padding:12px 14px; margin:0; overflow:auto; white-space:pre-wrap; overflow-wrap:anywhere; color:inherit; background:transparent; font:11px/1.6 ui-monospace,SFMono-Regular,Consolas,monospace; }
.remote-auth-fingerprint { display:grid; padding:11px 0; margin:0 0 16px; grid-template-columns:100px minmax(0,1fr); gap:12px; border-block:1px solid var(--line); font-size:11px; }
.remote-auth-fingerprint dt { color:var(--muted); }.remote-auth-fingerprint dd { min-width:0; margin:0; overflow-wrap:anywhere; }
.remote-auth-field { display:grid; gap:7px; color:var(--muted); font-size:11px; font-weight:650; }
.remote-auth-field input { width:100%; height:40px; padding:8px 12px; border:1px solid var(--line-strong); border-radius:9px; color:var(--text); background:var(--surface); outline:none; font-family:inherit; font-size:13px; line-height:1.4; }
.remote-auth-field input:focus { border-color:var(--accent); box-shadow:0 0 0 3px var(--accent-soft); }
.remote-auth-connect-action { display:inline-flex; min-width:126px; align-items:center; justify-content:center; gap:7px; }
.remote-auth-field-placeholder { visibility:hidden; }
.remote-auth-error-slot { display:flex; flex:none; gap:7px; height:44px; margin-top:8px; padding:6px 9px; border-radius:7px; overflow:hidden; color:var(--danger); }
.remote-auth-error-slot.has-error { background:color-mix(in srgb,var(--danger) 7%,var(--surface)); }
.remote-auth-error-slot svg { flex:none; width:15px; height:15px; margin-top:1px; fill:none; stroke:currentColor; stroke-width:1.5; stroke-linecap:round; }
.remote-auth-error-slot p { min-width:0; margin:0; overflow:auto; overflow-wrap:anywhere; font-size:12px; line-height:16px; }
.remote-auth-dialog pre,.remote-auth-error-slot p { -webkit-user-select:text; user-select:text; cursor:text; }
.remote-auth-heading h2,.remote-auth-heading :deep(.check-status span),.remote-auth-field > span,.remote-auth-terminal > header > span,.remote-auth-fingerprint dt,.remote-auth-fingerprint dd,.remote-auth-diagnostic dt,.remote-auth-diagnostic dd { -webkit-user-select:text; user-select:text; cursor:text; }
.remote-auth-diagnostic { padding:12px 14px; margin:16px 0; border:1px solid var(--line); border-radius:12px; background:var(--surface-strong); }
.remote-auth-diagnostic dl { display:grid; margin:0; gap:7px; }
.remote-auth-diagnostic dl > div { display:flex; align-items:center; justify-content:space-between; gap:20px; }
.remote-auth-diagnostic dt { color:var(--muted); font-size:11px; }
.remote-auth-diagnostic dd { margin:0; color:var(--text); font:600 11px/1.4 ui-monospace,SFMono-Regular,Consolas,monospace; }
@media (max-width:760px) {
  .remote-bridge-page.remote-setup-page { --ssh-row-height:60px; padding:14px 20px; }
  /* Reserve two complete SSH rows, their gap, and the panel heading/padding. */
  .remote-setup-grid { grid-template-columns:1fr; grid-template-rows:minmax(calc(var(--ssh-row-height) * 2 + 66px),1fr) minmax(0,2fr); }
  .remote-connections-panel,.remote-current-panel { padding:10px 14px; }
  .remote-panel-heading { min-height:26px; margin-bottom:8px; }
  .remote-current-summary { padding:6px 10px; grid-template-columns:32px minmax(0,1fr) auto; gap:9px; }
  .remote-server-icon { width:32px; height:32px; }
  .remote-current-empty { min-height:0; flex:1; }
  .remote-setup-panel { padding-top:8px; margin-top:8px; }
  .remote-setup-panel > header h2 { font-size:14px; }
  .remote-setup-actions { padding-top:8px; }
  .remote-capability-choice { min-height:48px; padding:6px 10px; margin-top:6px; }
}
@media (max-height:680px) and (min-width:761px) {
  .remote-bridge-page.remote-setup-page { --ssh-row-height:60px; padding:14px 24px; }
  .remote-workspace-configure { padding:14px 16px; }
  .remote-current-summary { padding:8px 10px; grid-template-columns:36px minmax(0,1fr) auto; gap:9px; }
  .remote-server-icon { width:36px; height:36px; }
  .remote-setup-panel { padding-top:10px; margin-top:10px; }
  .remote-capability-choice { padding:8px 10px; }
}
@media (max-width:760px) and (max-height:680px) {
  .remote-bridge-page.remote-setup-page { padding-block:8px; }
  .remote-setup-page .remote-workspace-configure { padding:8px; }
  .remote-page-intro { margin-bottom:8px; }
  .remote-page-intro h1 { line-height:1.3; }
  .remote-connections-panel,.remote-current-panel { padding:8px 12px; }
  .remote-panel-heading { margin-bottom:4px; }
  .remote-current-summary { line-height:1.35; }
  .remote-capability-title,.remote-capability-choice small { line-height:1.4; }
}
@media (max-width:680px) {
  .remote-port-footer,.remote-workspace-heading { align-items:flex-start; flex-direction:column; }
  .remote-status-toolbar { width:100%; flex-wrap:wrap; justify-content:space-between; }
  .remote-view-switch { order:-1; }
  .remote-workspace { padding:22px 20px; }
  .remote-workspace-configure { padding:10px; }
  .remote-page-intro h1 { font-size:21px; }
  .remote-connections-panel,.remote-current-panel { padding-inline:14px; }
  .remote-panel-heading { gap:8px; }
  .remote-current-heading { align-items:center; flex-direction:row; }
  .remote-section-heading { align-items:flex-start; }
  .remote-tool-access { align-items:flex-start; }
  .remote-skill-row { display:grid; }
  .remote-skill-agents { width:100%; grid-template-columns:1fr; }
  .remote-command { grid-template-columns:1fr; gap:6px; }.remote-command button { justify-self:start; }
  .remote-tool-verification { align-items:flex-start; flex-direction:column; }
}
</style>
