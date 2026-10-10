<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { CheckState } from "../../../shared/types";
import type { RemoteBridgeCopy } from "../../../shared/i18n/remote-bridge";
import { remoteBackend, type BridgeEvent, type BridgeSummary, type RemoteRuntimeState } from "../state";
import { diagnosticCheckState } from "../overview-presentation";
import { remoteToolAdapters } from "../tool-adapters";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
import StatusIndicator from "../../../shared/components/StatusIndicator.vue";
import AdvancedStatusRow from "./AdvancedStatusRow.vue";

// Operate: inherit the warm Overview vocabulary; separate preparation, proof and risk.
// Two columns at desktop widths; all actions use the page's existing command handlers.
const props = defineProps<{ copy:RemoteBridgeCopy; summary:BridgeSummary; busy:boolean; connected:boolean; active:boolean; diagnosing:boolean; sessionCommand:string; commandLoading:boolean; copying:boolean; copied:boolean }>();
const emit = defineEmits<{ copyEnvironment:[]; manualTerminal:[]; openConfig:[]; revealConfig:[]; vscodeSettings:[]; clearCredential:[]; runDiagnostics:[]; openReport:[]; openLogs:[] }>();
const timingDialog = ref<HTMLDialogElement>();
const errorLabels = computed(() => ({network:props.copy.rbAdvErrorNetwork, ssh:props.copy.rbAdvErrorSsh, authentication:props.copy.rbAdvErrorAuthentication, conflict:props.copy.rbAdvErrorConflict, unsupported:props.copy.rbAdvErrorUnsupported, unavailable:props.copy.rbAdvErrorUnavailable, invalidResponse:props.copy.rbAdvErrorResponse, unknown:props.copy.rbAdvErrorUnknown}));
const recentEvents = ref<BridgeEvent[]>([]);
const eventsLoaded = ref(false);
const logsAvailable = ref(false);
const clearLogsDialog = ref<HTMLDialogElement>();
const clearingLogs = ref(false);
const logsCleared = ref(false);
const clearLogsError = ref(false);
let clearedTimer:ReturnType<typeof setTimeout>|undefined;
const diagnosticHelp = computed(() => `${props.copy.rbAdvReportHelp}\n\n${logsAvailable.value ? props.copy.rbAdvLogsHelp : props.copy.rbAdvLogsUnavailable}`);
function confirmClearLogs() {
  if (props.busy || clearingLogs.value || !logsAvailable.value) return;
  clearLogsError.value = false;
  clearLogsDialog.value?.showModal();
}
async function clearLogs() {
  if (props.busy || clearingLogs.value) return;
  clearingLogs.value = true;
  clearLogsError.value = false;
  try {
    await remoteBackend.clearLogs();
    clearLogsDialog.value?.close();
    logsCleared.value = true;
    clearTimeout(clearedTimer);
    clearedTimer = setTimeout(() => { logsCleared.value = false; },3000);
  } catch { clearLogsError.value = true; }
  finally { clearingLogs.value = false; }
}
let eventsTimer:ReturnType<typeof setTimeout>|undefined;
let eventsRevision = 0;
watch([() => props.active, () => props.summary.target?.id], async ([active,targetId],[,previousTargetId]) => {
  const revision = ++eventsRevision;
  clearTimeout(eventsTimer);
  // Fresh summary objects arrive every two seconds. Only a different target
  // invalidates these snapshots; visibility changes merely pause/resume polling.
  if (targetId !== previousTargetId) {
    recentEvents.value = [];
    eventsLoaded.value = false;
    logsAvailable.value = false;
  }
  if (!active) return;
  async function poll() {
    try {
      const status = await remoteBackend.logStatus();
      if (revision === eventsRevision) logsAvailable.value = status.available;
    } catch { if (revision === eventsRevision) logsAvailable.value = false; }
    if (revision !== eventsRevision) return;
    try {
      const events = await remoteBackend.events();
      if (revision !== eventsRevision) return;
      recentEvents.value = events;
      eventsLoaded.value = true;
    } catch { /* Retain the last safe snapshot while an operation owns the store. */ }
    if (revision === eventsRevision) eventsTimer = setTimeout(poll, 2000);
  }
  await poll();
}, {immediate:true});
onBeforeUnmount(() => { eventsRevision++; clearTimeout(eventsTimer); clearTimeout(clearedTimer); });
const eventRows = computed(() => recentEvents.value.map(event => {
  const components = {ssh:"SSH", generalProxy:props.copy.rbLocalNetworkTitle, aiRoute:props.copy.rbCcUseTitle, sessionEnvironment:props.copy.rbAdvTerminal, vscode:"VS Code", codex:"Codex", claude:"Claude Code", skills:"Skills"};
  const actions = {connect:props.copy.rbAdvConnect, disconnect:props.copy.rbAdvDisconnect, transportLost:props.copy.rbAdvTransportLost, reconnect:props.copy.rbAdvReconnect, serverDirect:props.copy.rbAdvServerDirect, egressTest:props.copy.rbAdvEgressTest, toolVerify:props.copy.rbAdvToolVerify, environmentSetup:props.copy.rbAdvEnvironmentSetup, vscodeSetup:props.copy.rbAdvVscodeSetup, toolSetup:props.copy.rbAdvToolSetup, credentialClear:props.copy.rbAdvCredentialClear, enable:props.copy.rbAdvEnable, disable:props.copy.rbAdvDisable};
  const outcomes = {success:props.copy.rbAdvSuccess, warning:props.copy.rbAdvWarning, failed:props.copy.rbAdvFailed, started:props.copy.rbAdvStarted};
  const states = {success:"healthy", warning:"warning", failed:"failed", started:"idle"} as const;
  const outcome = event.action === "toolVerify" && event.outcome === "failed" ? props.copy.rbAdvFailed : outcomes[event.outcome];
  const status = event.errorCode ? `${outcome} · ${errorLabels.value[event.errorCode]}` : outcome;
  return {...event, time:new Date(event.timestamp).toLocaleTimeString(undefined,{hour12:false}), label:`${components[event.component]} · ${actions[event.action]}`, status, state:states[event.outcome]};
}));
const proxyOn = computed(() => !!props.summary.proxy && props.summary.proxyStatus === "connected");
const aiOn = computed(() => !!props.summary.cc && props.summary.ccStatus === "connected");
function runtime(state:RemoteRuntimeState|undefined) {
  if (state === "ready") return {state:"healthy" as const,label:props.copy.rbAdvReady};
  if (state === "warning") return {state:"warning" as const,label:props.copy.rbPostConnectPartial};
  if (state === "preparing" || state === "pending") return {state:"checking" as const,label:props.copy.rbCheckChecking};
  return {state:"idle" as const,label:props.copy.rbNotConfigured};
}
const terminal = computed(() => proxyOn.value ? runtime(props.summary.sessionEnvironmentState) : {state:"disabled" as const,label:props.copy.rbOverviewNotEnabled});
const vscode = computed(() => runtime(props.summary.vscodeState));
const port = computed(() => {
  const labels = {unknown:props.copy.rbAdvPortUnknown,matched:props.copy.rbAdvMatched,mismatch:props.copy.rbAdvMismatch,notSet:props.copy.rbAdvProxyNotSet,disabled:props.copy.rbAdvProxyDisabled,invalidSettings:props.copy.rbAdvProxyInvalid,readFailed:props.copy.rbAdvProxyReadFailed,unsupportedProxy:props.copy.rbAdvProxyUnsupported,notCompared:props.copy.rbAdvProxyNotCompared};
  const status = props.summary.runtimeProxyMatch ?? "unknown";
  const state:CheckState = status === "matched" ? "healthy" : ["mismatch","invalidSettings","readFailed","unsupportedProxy"].includes(status) ? "warning" : status === "disabled" ? "disabled" : "idle";
  const expected = props.summary.runtimeExpectedProxyPort;
  return {state,label:expected && ["matched","mismatch","notCompared"].includes(status) ? `${expected} · ${labels[status]}` : labels[status]};
});
const proofLabel = (state:CheckState) => state === "healthy" ? props.copy.rbAdvVerified : state === "checking" ? props.copy.rbCheckChecking : state === "failed" ? props.copy.rbAdvFailed : props.copy.rbAdvNotVerified;
const proxyProof = computed(() => {
  const state = diagnosticCheckState(props.summary.diagnostics?.generalProxyEgress.state);
  return proxyOn.value ? {state,label:proofLabel(state)} : {state:"disabled" as const,label:props.copy.rbOverviewNotEnabled};
});
const supportedTools = computed(() => remoteToolAdapters.map(adapter => adapter.inspect(props.summary)).filter(tool => tool.configured && tool.verificationSupported));
const aiProof = computed(() => {
  if (!aiOn.value) return {state:"disabled" as const,label:props.copy.rbOverviewNotEnabled};
  if (!supportedTools.value.length) return {state:"idle" as const,label:props.summary.codexConfigured ? props.copy.rbAdvNotSupported : props.copy.rbAdvNotVerified};
  const state = diagnosticCheckState(props.summary.diagnostics?.aiRouteVerification.state);
  return {state,label:proofLabel(state)};
});
const authLabel = computed(() => ({identityFile:props.copy.rbTargetAuthIdentity,agent:props.copy.rbTargetAuthAgent,password:props.copy.rbTargetAuthPassword,keyboardInteractive:"Keyboard Interactive",unknown:props.copy.rbCheckIdle})[props.summary.sshAuth.method]);
const authState = computed<CheckState>(() => props.connected && props.summary.sshAuth.authenticated ? "healthy" : "idle");
const recovery = computed(() => {
  const state = props.summary.reconnectState;
  return state === "attentionRequired" ? {state:"warning" as const,label:props.copy.rbReconnectAttention} : state === "waiting" || state === "retrying" ? {state:"checking" as const,label:props.copy.rbReconnectWaiting} : state === "idle" && props.connected && props.summary.status === "connected" ? {state:"healthy" as const,label:props.copy.rbAdvReady} : {state:"idle" as const,label:props.copy.rbCheckIdle};
});
const configActions = computed(() => !!props.summary.target && ["openssh","vscode"].includes(props.summary.target.source));
</script>

<template>
  <div class="bridge-advanced-view">
    <section class="advanced-card" aria-labelledby="advanced-environment-title">
      <h2 id="advanced-environment-title"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="m9 3 1-1h4l1 1 .5 2 2 1 2-.5 1.5 2.5-.5 2 1 2-1 2 .5 2-1.5 2.5-2-.5-2 1-.5 2-1 1h-4l-1-1-.5-2-2-1-2 .5L3 16l.5-2-1-2 1-2L3 8l1.5-2.5 2 .5 2-1 .5-2Z"/><circle cx="12" cy="12" r="3"/></svg>{{ copy.rbAdvEnvironment }}</h2>
      <div class="advanced-group"><h3>{{ copy.rbAdvTerminal }}<HelpTooltip pinnable :label="copy.rbAdvTerminal" :text="copy.rbManagedShellScope" /></h3>
        <AdvancedStatusRow :label="copy.rbAdvManagedTerminal" :state="terminal.state" :status="terminal.state === 'healthy' ? copy.rbAdvAutoLoad : terminal.label" />
        <AdvancedStatusRow :label="copy.rbAdvExternalTerminal" :state="proxyOn ? 'warning' : 'disabled'" :status="proxyOn ? copy.rbAdvManualLoad : copy.rbOverviewNotEnabled" :help="copy.rbAdvManualLoadHelp" />
        <div class="advanced-command"><code v-if="sessionCommand" :title="sessionCommand">{{ sessionCommand }}</code><span v-else role="status">{{ commandLoading ? copy.rbCheckChecking : proxyOn ? copy.rbAdvCommandUnavailable : copy.rbOverviewNotEnabled }}</span>
          <button class="secondary-action advanced-copy" type="button" :class="{'is-copied':copied}" :aria-busy="copying" :aria-label="copied ? copy.rbCopied : copy.rbCopySessionEnvironment" :title="copied ? copy.rbCopied : copy.rbCopySessionEnvironment" :disabled="busy || copying || !proxyOn || !connected" @click="emit('copyEnvironment')"><svg aria-hidden="true" viewBox="0 0 24 24"><path v-if="copied" d="m5 12 4 4L19 6"/><template v-else><rect x="8" y="8" width="12" height="13" rx="2"/><path d="M15 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3"/></template></svg></button>
        </div>
      </div>
      <div class="advanced-group"><h3><img src="/remote-apps/vscode.ico" alt="" />VS Code Remote</h3>
        <AdvancedStatusRow :label="copy.rbAdvVscodeTerminal" :state="vscode.state" :status="vscode.state === 'healthy' ? copy.rbAdvConfigured : vscode.label" />
        <AdvancedStatusRow :label="copy.rbAdvRuntimePort" :state="port.state" :status="port.label" :help="copy.rbAdvUserProxyHelp" />
      </div>
    </section>

    <section class="advanced-card advanced-links" aria-labelledby="advanced-links-title">
      <h2 id="advanced-links-title"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="M2 12h5l3-9 4 18 3-9h5"/></svg>{{ copy.rbAdvLinks }}<HelpTooltip pinnable :label="copy.rbAdvRun" :text="copy.rbAdvVerificationHelp" /></h2>
      <div class="advanced-group"><h3>{{ copy.rbLocalNetworkTitle }}</h3>
        <div class="advanced-proof-grid">
        <AdvancedStatusRow :label="copy.rbAdvLink" :state="proxyOn ? 'healthy' : 'disabled'" :status="proxyOn ? copy.rbAdvEstablished : copy.rbOverviewNotEnabled" />
        <AdvancedStatusRow :label="copy.rbAdvEgress" :state="proxyProof.state" :status="proxyProof.label" :detail="summary.diagnostics?.generalProxyEgress.errorCode && proxyOn ? errorLabels[summary.diagnostics.generalProxyEgress.errorCode] : undefined" />
        </div>
      </div>
      <div class="advanced-group"><h3>{{ copy.rbCcUseTitle }}</h3>
        <div class="advanced-proof-grid">
        <AdvancedStatusRow :label="copy.rbAdvLink" :state="aiOn ? 'healthy' : 'disabled'" :status="aiOn ? copy.rbAdvEstablished : copy.rbOverviewNotEnabled" />
        <AdvancedStatusRow :label="copy.rbAdvModelRequest" :state="aiProof.state" :status="aiProof.label" :help="copy.rbVerifyClaudeHint" />
        </div>
      </div>
      <div class="advanced-group"><h3>SSH Transport</h3>
        <div class="advanced-proof-grid">
        <AdvancedStatusRow :label="copy.rbAdvAuthentication" :state="authState" :status="authLabel" />
        <AdvancedStatusRow :label="copy.rbAdvRecovery" :state="recovery.state" :status="recovery.label" />
        </div>
      </div>
      <div class="advanced-card-actions"><button class="secondary-action" type="button" :disabled="busy || !connected" @click="emit('runDiagnostics')"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="M20 7v5h-5M20 12a8 8 0 1 1-2-6"/></svg>{{ diagnosing ? copy.rbCheckChecking : copy.rbAdvRun }}</button></div>
    </section>

    <section class="advanced-card" aria-labelledby="advanced-security-title">
      <h2 id="advanced-security-title"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6l8-3Z"/><path d="m8 12 3 3 5-6"/></svg>{{ copy.rbAdvSecurity }}</h2>
      <AdvancedStatusRow :label="copy.rbAdvBind" :state="proxyOn || aiOn ? summary.security?.remoteBindScope === 'loopback' ? 'healthy' : 'idle' : 'disabled'" :status="proxyOn || aiOn ? summary.security?.remoteBindScope === 'loopback' ? copy.rbAdvLoopback : copy.rbCheckIdle : copy.rbOverviewNotEnabled" :detail="summary.security?.remoteBindScope === 'loopback' ? '127.0.0.1' : undefined" />
      <AdvancedStatusRow :label="copy.rbAdvProxyAuth" :state="proxyOn ? 'warning' : 'disabled'" :status="proxyOn ? summary.security?.generalProxyAuth === 'none' ? copy.rbAdvNoProxyAuth : copy.rbCheckIdle : copy.rbOverviewNotEnabled" :help="copy.rbAdvProxyRisk" />
      <AdvancedStatusRow :label="copy.rbAdvAiAuth" :state="aiOn ? summary.security?.aiRouteAuth === 'session' ? 'healthy' : 'idle' : 'disabled'" :status="aiOn ? summary.security?.aiRouteAuth === 'session' ? copy.rbAdvSessionAuth : copy.rbCheckIdle : copy.rbOverviewNotEnabled" />
      <AdvancedStatusRow :label="copy.rbAdvShell" :state="summary.security?.shellScope === 'sessionOnly' ? 'healthy' : 'idle'" :status="summary.security?.shellScope === 'sessionOnly' ? copy.rbAdvSessionOnly : copy.rbCheckIdle" :help="copy.rbAdvShellHint" />
      <AdvancedStatusRow :label="copy.rbAdvCache" state="healthy" :status="summary.sshAuth.passwordStored ? copy.rbAdvCached : copy.rbAdvCacheEmpty" :help="copy.rbAdvCacheHelp" />
    </section>

    <section class="advanced-card advanced-events" aria-labelledby="advanced-events-title">
      <h2 id="advanced-events-title"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="M7 5h14M7 12h14M7 19h14M3 5h.1M3 12h.1M3 19h.1"/></svg>{{ copy.rbAdvEvents }}</h2>
      <ul v-if="eventRows.length" class="advanced-event-list" tabindex="0" :aria-label="copy.rbAdvEvents"><li v-for="(event,index) in eventRows" :key="`${event.timestamp}-${index}`">
        <time>{{ event.time }}</time><span class="advanced-event-label">{{ event.label }}</span><StatusIndicator :state="event.state" :label="event.status" />
      </li></ul>
      <p v-else class="advanced-note">{{ eventsLoaded ? copy.rbAdvNoEvents : copy.rbCheckChecking }}</p>
    </section>

    <section class="advanced-card advanced-manual" aria-labelledby="advanced-manual-title">
      <h2 id="advanced-manual-title"><svg aria-hidden="true" viewBox="0 0 24 24"><path d="m14 6 4 4 4-4a7 7 0 0 1-9 9l-6 6-4-4 6-6a7 7 0 0 1 9-9l-4 4Z"/></svg>{{ copy.rbAdvManual }}</h2>
      <div class="advanced-group"><h3>{{ copy.rbAdvEnvironmentActions }}</h3><div class="advanced-tool-actions">
        <button class="secondary-action" type="button" :class="{'is-copied':copied}" :aria-busy="copying" :disabled="busy || copying || !connected || !proxyOn" @click="emit('copyEnvironment')">{{ copied ? copy.rbCopied : copy.rbCopySessionEnvironment }}</button>
        <button class="secondary-action" type="button" :disabled="busy || !connected" @click="emit('manualTerminal')">{{ copy.rbLaunchManualTerminal }}</button>
      </div></div>
      <div class="advanced-group"><h3>{{ copy.rbAdvConfigActions }}</h3><div v-if="configActions" class="advanced-tool-actions">
        <button class="secondary-action" type="button" :disabled="busy" @click="emit('openConfig')">{{ copy.rbOpenSshConfig }}</button>
        <button class="secondary-action" type="button" :disabled="busy" @click="emit('revealConfig')">{{ copy.rbRevealConfig }}</button>
        <button v-if="summary.target?.source === 'vscode'" class="secondary-action" type="button" :disabled="busy" @click="emit('vscodeSettings')">{{ copy.rbOpenVscodeSettings }}</button>
      </div><p v-else class="advanced-note">{{ copy.rbAdvNoConfig }}</p></div>
      <div v-if="summary.sshAuth.passwordStored" class="advanced-group"><h3>{{ copy.rbAdvSecurityActions }}</h3><button class="secondary-action" type="button" :disabled="busy" @click="emit('clearCredential')">{{ copy.rbAdvClearCache }}</button></div>
    </section>

    <section class="advanced-card advanced-diagnostics" aria-labelledby="advanced-diagnostics-title">
      <h2 id="advanced-diagnostics-title"><svg aria-hidden="true" viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7v.1"/></svg>{{ copy.rbDiagnostics }}<HelpTooltip pinnable :label="copy.rbDiagnostics" :text="diagnosticHelp" /></h2>
      <div class="advanced-group"><h3>{{ copy.rbAdvDiagnosticActions }}</h3><div class="advanced-tool-actions">
        <button class="secondary-action" type="button" :disabled="busy" @click="emit('openReport')">{{ copy.rbAdvReport }}</button>
        <button class="secondary-action" type="button" @click="timingDialog?.showModal()">{{ copy.rbAdvTimings }}</button>
      </div></div>
      <div class="advanced-group"><h3>{{ copy.rbAdvLogActions }}</h3><div class="advanced-tool-actions">
        <button class="secondary-action" type="button" :disabled="busy || clearingLogs || !logsAvailable" @click="emit('openLogs')">{{ copy.rbAdvOpenLogs }}</button>
        <button class="secondary-action advanced-clear-logs" type="button" :class="{'is-cleared':logsCleared}" :disabled="busy || clearingLogs || !logsAvailable" @click="confirmClearLogs">{{ logsCleared ? copy.rbAdvLogsCleared : copy.rbAdvClearLogs }}</button>
      </div></div>
    </section>
  <dialog ref="clearLogsDialog" class="confirmation-dialog advanced-clear-logs-dialog" aria-labelledby="advanced-clear-logs-title" aria-describedby="advanced-clear-logs-description" @cancel.prevent="!clearingLogs && clearLogsDialog?.close()">
    <h2 id="advanced-clear-logs-title">{{ copy.rbAdvClearLogsTitle }}</h2>
    <p id="advanced-clear-logs-description">{{ copy.rbAdvClearLogsBody }}</p>
    <p v-if="clearLogsError" class="advanced-clear-error" role="alert">{{ copy.rbAdvClearLogsError }}</p>
    <div class="confirmation-actions"><button class="secondary-action" type="button" :disabled="clearingLogs" @click="clearLogsDialog?.close()">{{ copy.rbCancel }}</button><button class="secondary-action danger-action" type="button" :disabled="busy || clearingLogs" @click="clearLogs">{{ clearingLogs ? copy.rbAdvLogsClearing : copy.rbAdvClearLogs }}</button></div>
  </dialog>
  <dialog ref="timingDialog" class="confirmation-dialog advanced-timing-dialog" aria-labelledby="advanced-timing-title" @cancel.prevent="timingDialog?.close()">
    <h2 id="advanced-timing-title">{{ copy.rbAdvTimings }}</h2>
    <table v-if="summary.timings?.length"><thead><tr><th>{{ copy.rbAdvPhase }}</th><th>{{ copy.rbAdvDuration }}</th><th>{{ copy.rbAdvOutcome }}</th></tr></thead><tbody><tr v-for="(timing,index) in summary.timings" :key="index"><td>{{ timing.phase }}</td><td>{{ timing.durationMs }} ms</td><td>{{ timing.outcome }}</td></tr></tbody></table>
    <p v-else class="advanced-note">{{ copy.rbAdvNoTimings }}</p>
    <form method="dialog"><button class="secondary-action" type="submit">{{ copy.rbClose }}</button></form>
  </dialog>
  </div>
</template>

<style scoped>
.bridge-advanced-view { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); align-items:stretch; gap:10px; min-width:0; }
.advanced-card { min-width:0; padding:12px; border:1px solid var(--line); border-radius:12px; background:var(--surface-strong); }
.advanced-card h2 { display:flex; align-items:center; gap:8px; margin:0 0 9px; font-size:16px; font-weight:680; }
.advanced-card h2 > svg { width:23px; height:23px; flex:none; color:var(--accent); }
.advanced-card svg { fill:none; stroke:currentColor; stroke-width:1.7; stroke-linecap:round; stroke-linejoin:round; }
.advanced-group + .advanced-group { border-top:1px solid var(--line); padding-top:9px; margin-top:8px; }
.advanced-group h3 { display:flex; align-items:center; gap:7px; margin:0 0 5px; font-size:12px; font-weight:650; }
.advanced-group h3 img { width:17px; height:17px; }
.advanced-card-actions { display:flex; justify-content:flex-end; margin-top:10px; }
.advanced-links { container-type:inline-size; }
.advanced-proof-grid { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:10px; }
.advanced-proof-grid :deep(.advanced-status-row) { display:grid; grid-template-columns:auto minmax(0,1fr); min-width:0; gap:5px 6px; }
.advanced-proof-grid :deep(.advanced-status-row + .advanced-status-row) { border-left:1px solid var(--line); padding-left:10px; }
.advanced-proof-grid :deep(.advanced-row-label) { min-width:0; font-size:11px; }
.advanced-proof-grid :deep(.check-status) { max-width:100%; justify-self:end; }
.advanced-proof-grid :deep(small) { grid-column:1 / -1; }
@container (max-width:270px) {
  .advanced-proof-grid { grid-template-columns:minmax(0,1fr); gap:5px; }
  .advanced-proof-grid :deep(.advanced-status-row + .advanced-status-row) { border-left:0; padding-left:0; }
}
.advanced-card button { display:inline-flex; align-items:center; justify-content:center; gap:7px; font-size:11px; min-height:32px; padding:6px 9px; line-height:1.45; }
.advanced-card button svg { width:16px; height:16px; flex:none; }
.advanced-command { display:grid; grid-template-columns:minmax(0,1fr) 32px; align-items:center; gap:6px; width:100%; height:32px; min-width:0; margin-top:5px; }
.advanced-command code,.advanced-command > span { display:block; min-width:0; height:32px; padding:6px 8px; border:1px solid var(--line); border-radius:8px; background:var(--canvas); font-size:11px; line-height:18px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; user-select:text; }
.advanced-command > span { color:var(--muted); }
.advanced-command .advanced-copy { width:32px; height:32px; padding:6px; }
.advanced-command .advanced-copy.is-copied { color:var(--success); }
.advanced-tool-actions { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:6px; }
.advanced-tool-actions .is-copied { color:var(--success); }
.advanced-note { margin:5px 0 9px; font-size:11px; line-height:1.6; color:var(--muted); }
.advanced-event-list { list-style:none; margin:0; padding:0; max-height:150px; overflow:auto; scrollbar-gutter:stable; }
.advanced-event-list li { display:flex; align-items:center; gap:8px; min-height:32px; font-size:11px; padding:4px 0; }
.advanced-event-list li + li { border-top:1px solid var(--line); }
.advanced-event-list time { color:var(--muted); flex:none; font-variant-numeric:tabular-nums; }
.advanced-event-label { flex:1; min-width:0; overflow-wrap:anywhere; }
.advanced-event-label small { display:block; color:var(--muted); }
.advanced-event-list :deep(.check-status) { flex:none; font-size:11px; }
.advanced-clear-logs.is-cleared { color:var(--success); }
.advanced-clear-logs-dialog { width:min(400px,calc(100vw - 40px)); padding:20px; }
.advanced-clear-logs-dialog h2 { margin:0 0 12px; font-size:17px; }
.advanced-clear-logs-dialog p { font-size:13px; line-height:1.65; }
.advanced-clear-logs-dialog .confirmation-actions { margin-top:18px; }
.advanced-clear-error,.advanced-clear-logs-dialog .danger-action { color:var(--danger); }
.advanced-timing-dialog { width:min(500px,calc(100vw - 40px)); max-height:calc(100dvh - 40px); padding:20px; overflow:auto; }
.advanced-timing-dialog h2 { margin:0 0 14px; font-size:16px; }
.advanced-timing-dialog table { width:100%; border-collapse:collapse; font-size:12px; }
.advanced-timing-dialog th,.advanced-timing-dialog td { padding:8px 6px; border-bottom:1px solid var(--line); text-align:left; overflow-wrap:anywhere; }
.advanced-timing-dialog td:nth-child(2) { font-variant-numeric:tabular-nums; white-space:nowrap; }
.advanced-timing-dialog form { display:flex; justify-content:flex-end; margin-top:16px; padding:0; }
@media (max-width:760px) { .bridge-advanced-view { grid-template-columns:minmax(0,1fr); } }
@media (max-width:420px) { .advanced-tool-actions { grid-template-columns:minmax(0,1fr); } }
</style>
