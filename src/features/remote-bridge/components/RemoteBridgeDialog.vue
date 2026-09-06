<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import CompactSelect from "../../../shared/components/CompactSelect.vue";
import type { ActiveProxyContext } from "../../../shared/types";
import { bridgeError, type RemoteBridgeCopy } from "../../../shared/i18n/remote-bridge";
import { copyText } from "../../../shared/utils/clipboard";
import { remoteBackend, targetLabel, type BridgeRequest, type BridgeSummary, type ConfigPreview } from "../state";
const props=defineProps<{copy:RemoteBridgeCopy; activeProxy:ActiveProxyContext; summary:BridgeSummary}>();
const emit=defineEmits<{refresh:[]}>();
const dialog=ref<HTMLDialogElement>();
const confirmation=ref<HTMLDialogElement>();
const heading=ref<HTMLElement>();
const step=ref(1);
const aliases=ref<string[]>([]);
const alias=ref("");
const checked=ref(false);
const proxy=ref(true);
const cc=ref(false);
const proxyPort=ref(17897);
const ccPort=ref(25721);
const ccLocalPort=ref(15721);
const ccDetected=ref(false);
const busy=ref(false);
const error=ref<unknown>();
const feedback=ref<"copied"|"tested"|"applied"|"restored">();
const preview=ref<BridgeSummary>();
const reviewedRequest=ref<BridgeRequest>();
const config=ref<ConfigPreview>();
const launch=ref("");
const confirmationAction=ref<"disconnect"|"apply"|"restore">();
let previousFocus: HTMLElement | null=null;
const options=computed(() => aliases.value.map(value => ({value,label:targetLabel(value)})));
const live=computed(() => ["connected","stale","unavailable","connecting"].includes(props.summary.status) && !!props.summary.alias);
const proxyAvailable=computed(() => props.activeProxy.available && !!props.activeProxy.candidate && props.activeProxy.candidate.protocol!=="unknown");
const portValid=(value:number) => Number.isInteger(value) && value>=1024 && value<=65535;
const valid=computed(() => (proxy.value || cc.value) && (!proxy.value || proxyAvailable.value && portValid(proxyPort.value)) && (!cc.value || ccDetected.value && portValid(ccPort.value) && portValid(ccLocalPort.value)) && !(proxy.value && cc.value && proxyPort.value===ccPort.value));
const labels=computed(() => [props.copy.rbTarget,props.copy.rbCapabilities,props.copy.rbPreview,props.copy.rbStatus]);
const errorText=computed(() => error.value ? bridgeError(error.value,props.copy) : "");
const feedbackText=computed(() => feedback.value ? ({copied:props.copy.rbCopied,tested:props.copy.rbTested,applied:props.copy.rbApplied,restored:props.copy.rbRestored})[feedback.value] : "");
const endpoints=computed(() => { const source=step.value===3 ? preview.value : props.summary; return [{title:props.copy.rbProxy,value:source?.proxy},{title:props.copy.rbCc,value:source?.cc}].filter(row=>row.value); });
async function perform(action:()=>Promise<void>) {
  if (busy.value) return;
  busy.value=true; error.value=undefined; feedback.value=undefined;
  try { await action(); } catch(cause) { error.value=cause; } finally { busy.value=false; emit("refresh"); }
}
async function load() { aliases.value=await remoteBackend.targets(); if (!aliases.value.includes(alias.value)) alias.value=aliases.value[0] ?? ""; }
function open() {
  if(dialog.value?.open) return;
  previousFocus=document.activeElement instanceof HTMLElement ? document.activeElement : null;
  step.value=props.summary.alias ? 4 : 1;
  error.value=undefined; feedback.value=undefined;
  proxy.value=proxyAvailable.value;
  dialog.value?.showModal();
  void perform(load);
}
function close() { confirmation.value?.close(); dialog.value?.close(); previousFocus?.focus(); }
function go(value:number) { step.value=value; error.value=undefined; void nextTick(()=>heading.value?.focus()); }
function request():BridgeRequest { return {alias:alias.value,proxyPort:proxy.value?proxyPort.value:null,ccPort:cc.value?ccPort.value:null,ccLocalPort:ccLocalPort.value,expectedRevision:props.activeProxy.revision}; }
function review() { void perform(async()=>{ const selected=request(); preview.value=await remoteBackend.preview(selected); reviewedRequest.value=selected; go(3); }); }
function connect() { if(!reviewedRequest.value) return; const selected=reviewedRequest.value; void perform(async()=>{ await remoteBackend.connect(selected); go(4); }); }
function ask(action:typeof confirmationAction.value) { if (!dialog.value?.open) return; confirmationAction.value=action; confirmation.value?.showModal(); }
function configure(tool:string) { void perform(async()=>{ config.value=await remoteBackend.configPreview(tool); ask("apply"); }); }
function restore(tool:string, target=alias.value) { void perform(async()=>{ config.value=await remoteBackend.configRestorePreview(target,tool); ask("restore"); }); }
function confirm() {
  const action=confirmationAction.value;
  confirmation.value?.close();
  void perform(async()=>{
    if(action==="disconnect") { await remoteBackend.disconnect(); launch.value=""; go(4); }
    else if(action==="apply" && config.value) { await remoteBackend.configApply(config.value.id); launch.value=config.value.launch; feedback.value="applied"; config.value=undefined; }
    else if(action==="restore" && config.value) { await remoteBackend.configRestore(config.value.id); launch.value=""; config.value=undefined; feedback.value="restored"; }
  });
}
function copyValue(value:string) { void perform(async()=>{ await copyText(value); feedback.value="copied"; }); }
watch(alias,()=>{ checked.value=false; launch.value=""; });
watch(()=>props.summary.status,(value)=>{ if(value==="disconnected" || value==="error") launch.value=""; });
watch(ccLocalPort,()=>{ ccDetected.value=false; });
watch(()=>props.activeProxy.revision,()=>{ if(step.value===3 && reviewedRequest.value?.proxyPort) { reviewedRequest.value=undefined; go(2); error.value="activeChanged"; } });
defineExpose({open});
</script>
<template>
  <dialog ref="dialog" class="confirmation-dialog remote-bridge-dialog" aria-labelledby="remote-bridge-title" @cancel.prevent="close">
    <form @submit.prevent="step===1 ? perform(async()=>{ await remoteBackend.check(alias); checked=true; }) : step===2 ? review() : step===3 ? connect() : undefined">
      <div class="remote-heading"><h2 id="remote-bridge-title">{{ copy.rbTitle }}</h2><button class="secondary-action" type="button" @click="close">{{ copy.rbClose }}</button></div>
      <ol class="remote-steps" :aria-label="copy.rbTitle"><li v-for="(label,index) in labels" :key="label" :aria-current="step===index+1 ? 'step' : undefined">{{index+1}} · {{label}}</li></ol>
      <h3 ref="heading" tabindex="-1">{{labels[step-1]}}</h3>
      <fieldset :disabled="busy" class="remote-fields">
        <template v-if="step===1">
          <label>{{copy.rbAlias}}</label><CompactSelect v-model="alias" :options="options" :label="copy.rbAlias" :disabled="busy || !aliases.length" />
          <p v-if="!aliases.length" class="remote-hint">{{copy.rbEmpty}}</p><p class="remote-hint">{{copy.rbRequirements}}</p>
          <p class="remote-hint">{{copy.rbVscodeTargets}}</p>
          <div class="remote-actions"><button class="secondary-action" type="button" @click="perform(load)">{{copy.rbRefresh}}</button><button class="primary-action" type="submit" :disabled="!alias">{{copy.rbCheck}}</button></div>
          <p v-if="checked" role="status">{{copy.rbChecked}}</p>
          <div v-if="alias" class="remote-actions"><button class="secondary-action" type="button" @click="restore('codex')">{{copy.rbRestoreCodex}}</button><button class="secondary-action" type="button" @click="restore('claude')">{{copy.rbRestoreClaude}}</button></div>
        </template>
        <template v-else-if="step===2">
          <div class="remote-capability"><label class="remote-choice"><input v-model="proxy" type="checkbox" :disabled="!proxyAvailable" />{{copy.rbProxy}}</label>
            <p v-if="proxyAvailable"><span>{{activeProxy.candidate?.clientName}}</span> <code>{{activeProxy.candidate?.host}}:{{activeProxy.candidate?.port}} · {{activeProxy.candidate?.protocol}}</code></p><p v-else class="remote-hint">{{copy.rbNoProxy}}</p>
            <label v-if="proxy" class="remote-port">{{copy.rbRemotePort}}<input v-model.number="proxyPort" type="number" min="1024" max="65535" required /></label>
          </div>
          <div class="remote-capability"><label class="remote-choice"><input v-model="cc" type="checkbox" />{{copy.rbCc}}</label>
            <template v-if="cc"><p class="remote-hint">{{copy.rbCcHint}}</p><label class="remote-port">{{copy.rbLocalPort}}<input v-model.number="ccLocalPort" type="number" min="1024" max="65535" required /></label>
              <button class="secondary-action" type="button" :disabled="!portValid(ccLocalPort)" @click="perform(async()=>{ ccDetected=await remoteBackend.detectCc(ccLocalPort); if(!ccDetected) throw 'ccUnavailable'; })">{{copy.rbDetect}}</button>
              <p v-if="ccDetected" role="status">{{copy.rbDetected}}</p><label class="remote-port">{{copy.rbRemotePort}}<input v-model.number="ccPort" type="number" min="1024" max="65535" required /></label>
            </template>
          </div><p class="remote-hint">{{copy.rbPortHint}}</p>
        </template>
        <template v-else>
          <p><strong>{{targetLabel((step===3 ? preview?.alias : summary.alias) || '')}}</strong><span v-if="step===4" class="remote-state" :data-state="summary.status" role="status">{{copy.rbStates[summary.status]}}</span></p>
          <p v-if="step===4 && summary.status==='stale'" class="notice notice-warning">{{copy.rbStaleHint}}</p>
          <p v-if="step===4 && summary.status==='unavailable'" class="notice notice-warning">{{copy.rbUnavailableHint}}</p>
          <section v-for="row in endpoints" :key="row.title" class="remote-capability"><h4>{{row.title}}</h4><dl><dt>{{copy.rbLocal}}</dt><dd><code>{{row.value!.local.host}}:{{row.value!.local.port}} · {{row.value!.local.protocol}}</code></dd><dt>{{copy.rbRemote}}</dt><dd><code>127.0.0.1:{{row.value!.remotePort}}</code></dd></dl></section>
          <p class="remote-hint">{{copy.rbSafety}}</p>
          <template v-if="step===4">
            <section v-if="summary.alias && live" class="remote-capability"><h4>VS Code · Remote - SSH</h4><p class="remote-hint">{{copy.rbVscodeHint}}</p><button class="secondary-action" type="button" @click="perform(()=>remoteBackend.openVscode(summary.alias!))">{{copy.rbVscodeOpen}}</button></section>
            <div v-if="summary.proxy && live" class="remote-actions"><button class="secondary-action" type="button" @click="copyValue(summary.environment)">{{copy.rbCopy}}</button><button class="secondary-action" type="button" :disabled="summary.status!=='connected'" @click="perform(async()=>{await remoteBackend.test();feedback='tested';})">{{copy.rbTest}}</button></div>
            <p v-if="summary.proxy && live" class="remote-hint">{{copy.rbTestHint}}</p>
            <template v-if="summary.cc && live"><p class="remote-hint">{{copy.rbConfigHint}}</p><div class="remote-actions"><button class="secondary-action" type="button" @click="configure('codex')">{{copy.rbCodex}}</button><button class="secondary-action" type="button" @click="configure('claude')">{{copy.rbClaude}}</button></div></template>
            <div v-if="summary.alias" class="remote-actions"><button class="secondary-action" type="button" @click="restore('codex',summary.alias!)">{{copy.rbRestoreCodex}}</button><button class="secondary-action" type="button" @click="restore('claude',summary.alias!)">{{copy.rbRestoreClaude}}</button></div>
            <div v-if="launch" class="remote-launch"><label>{{copy.rbLaunch}}</label><pre>{{launch}}</pre><button class="secondary-action" type="button" @click="copyValue(launch)">{{copy.rbCopyLaunch}}</button></div>
          </template>
        </template>
      </fieldset>
      <p v-if="errorText" class="remote-error" role="alert">{{errorText}}</p><p class="remote-feedback" role="status">{{busy ? copy.rbBusy : feedbackText}}</p>
      <div class="confirmation-actions">
        <button v-if="step===2 || step===3" class="secondary-action" type="button" :disabled="busy" @click="go(step-1)">{{copy.rbBack}}</button>
        <button v-if="step===1" class="primary-action" type="button" :disabled="busy || !checked" @click="go(2)">{{copy.rbNext}}</button>
        <button v-if="step===2" class="primary-action" type="submit" :disabled="busy || !valid">{{copy.rbNext}}</button>
        <button v-if="step===3" class="primary-action" type="submit" :disabled="busy || !reviewedRequest">{{copy.rbConnect}}</button>
        <button v-if="step===4 && live" class="secondary-action remote-danger" type="button" :disabled="busy" @click="ask('disconnect')">{{copy.rbDisconnect}}</button>
        <button v-if="step===4 && !live" class="primary-action" type="button" :disabled="busy" @click="go(1)">{{copy.rbReconnect}}</button>
      </div>
    </form>
  </dialog>
  <dialog ref="confirmation" class="confirmation-dialog remote-bridge-dialog" aria-labelledby="remote-confirm-title" @cancel.prevent="confirmation?.close()">
    <form @submit.prevent="confirm">
      <h2 id="remote-confirm-title">{{confirmationAction==='apply' ? copy.rbApply : confirmationAction==='disconnect' ? copy.rbDisconnect : copy.rbConfirm}}</h2>
      <template v-if="confirmationAction!=='disconnect' && config"><p><strong>{{targetLabel(config.alias)}}</strong> · {{config.tool}} {{config.version}}</p><p>{{config.path}}</p><h3>{{copy.rbBefore}}</h3><pre>{{config.before || copy.rbAbsent}}</pre><h3>{{copy.rbAfter}}</h3><pre>{{config.after || copy.rbAbsent}}</pre><p class="remote-hint">{{config.restore ? copy.rbRestoreHint : copy.rbConfigHint}}</p><template v-if="config.launch"><h3>{{copy.rbLaunch}}</h3><pre>{{config.launch}}</pre></template></template>
      <p v-else>{{confirmationAction==='disconnect' ? copy.rbDisconnectHint : copy.rbRestoreHint}}</p>
      <div class="confirmation-actions"><button class="secondary-action" type="button" autofocus @click="confirmation?.close()">{{copy.rbCancel}}</button><button class="primary-action" type="submit" :disabled="busy">{{copy.rbConfirm}}</button></div>
    </form>
  </dialog>
</template>
<style>
.confirmation-dialog.remote-bridge-dialog { width: min(640px, calc(100vw - 40px)); max-height: calc(100vh - 40px); overflow-y: auto; }
.remote-heading { display:flex; align-items:center; justify-content:space-between; gap:16px; }
.remote-steps { display:flex; gap:10px; flex-wrap:wrap; padding:16px 0; margin:0; list-style:none; color:var(--muted); font-size:11px; border-bottom:1px solid var(--line); }
.remote-steps [aria-current] { color:var(--accent-strong); font-weight:650; }
.remote-bridge-dialog h3 { margin:20px 0 12px; font-size:13px; }
.remote-bridge-dialog h4 { margin:0 0 10px; font-size:13px; }
.remote-fields { border:0; padding:0; margin:0; min-width:0; }
.remote-fields > label { display:block; margin-bottom:8px; }
.remote-hint { color:var(--muted); font-size:12px; line-height:1.65; }
.remote-capability { padding:16px 0; border-bottom:1px solid var(--line); }
.remote-choice { display:flex; align-items:center; gap:10px; font-weight:650; }
.remote-choice input { accent-color:var(--accent-strong); }
.remote-port { display:flex; align-items:center; justify-content:space-between; gap:16px; margin:12px 0; }
.remote-port input { width:110px; padding:8px; border:1px solid var(--line-strong); border-radius:8px; background:var(--surface); color:var(--ink); font:inherit; }
.remote-actions { display:flex; gap:8px; flex-wrap:wrap; margin-top:12px; }
.remote-capability dl { display:grid; grid-template-columns:64px minmax(0,1fr); gap:8px; margin:0; font-size:12px; }
.remote-capability dt { color:var(--muted); }.remote-capability dd { margin:0; overflow-wrap:anywhere; }
.remote-state { display:inline-flex; align-items:center; gap:6px; margin-left:12px; font-size:12px; }
.remote-state::before { content:""; width:7px; height:7px; border-radius:50%; background:var(--muted); }
.remote-state[data-state=connected]::before { background:var(--success); }.remote-state[data-state=stale]::before { background:var(--warning); }
.remote-bridge-dialog pre { white-space:pre-wrap; overflow-wrap:anywhere; background:var(--surface); border:1px solid var(--line); padding:12px; border-radius:8px; font-size:11px; line-height:1.65; }
.remote-error,.remote-danger { color:var(--danger); }.remote-error { font-size:12px; line-height:1.65; }
.remote-feedback { min-height:18px; color:var(--muted); font-size:12px; }
.remote-launch { margin-top:16px; }.remote-fields:disabled { opacity:.7; }
</style>
