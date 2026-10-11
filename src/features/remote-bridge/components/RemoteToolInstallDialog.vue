<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { RemoteBridgeCopy } from "../../../shared/i18n/remote-bridge";
import { copyText } from "../../../shared/utils/clipboard";
import StatusIndicator from "../../../shared/components/StatusIndicator.vue";
import { remoteBackend, type BridgeSummary } from "../state";
import { getRemoteToolAdapter, type RemoteToolId } from "../tool-adapters";
import { installMethods, installMissingRequirements } from "../install-guidance";
import type { ToolInstallMethod } from "../install-guidance";
import { toolInstallationLabel } from "../overview-presentation";
import { useNotification } from "../../../shared/state/notifications";

const props=defineProps<{copy:RemoteBridgeCopy; summary:BridgeSummary}>();
const emit=defineEmits<{refresh:[]}>();
const dialog=ref<HTMLDialogElement>();
const heading=ref<HTMLElement>();
const tool=ref<RemoteToolId>("codex");
const targetId=ref("");
const pending=ref(false), terminalPending=ref(false), copied=ref("");
const error=ref<"recheck"|"terminal"|"copy"|"documentation"|null>(null);
let generation=0, timer:ReturnType<typeof setTimeout>|undefined;
let previousFocus:HTMLElement|null=null;
const adapter=computed(() => getRemoteToolAdapter(tool.value));
const inspection=computed(() => adapter.value.inspect(props.summary));
const connected=computed(() => props.summary.target?.id === targetId.value && props.summary.status === "connected");
const checking=computed(() => pending.value || inspection.value.installation === "checking");
const state=computed(() => checking.value ? "checking" : inspection.value.installation === "installed" ? "healthy" : ["unsupported","broken"].includes(inspection.value.installation) ? "warning" : "idle");
const proxyOn=computed(() => !!props.summary.proxy && props.summary.proxyStatus === "connected");
const errorText=computed(() => error.value === "recheck" ? props.copy.rbInstallRecheckError : error.value === "terminal" ? props.copy.rbInstallTerminalError : error.value === "documentation" ? props.copy.rbInstallOfficialError : props.copy.rbInstallCopyError);
useNotification(() => error.value ? errorText.value : "", "error", "remote-installation");
function open(selected:RemoteToolId) {
  if (!props.summary.target) return;
  generation++; previousFocus=document.activeElement instanceof HTMLElement ? document.activeElement : null;
  tool.value=selected; targetId.value=props.summary.target.id;
  pending.value=false; terminalPending.value=false; error.value=null; copied.value=""; clearTimeout(timer);
  dialog.value?.showModal(); void nextTick(() => heading.value?.focus());
}
function close() { generation++; dialog.value?.close(); previousFocus?.focus(); }
watch(() => props.summary.target?.id, id => { if (dialog.value?.open && id !== targetId.value) close(); });
async function copyCommand(command:string,id:string) {
  const revision=generation; error.value=null;
  try {
    await copyText(command);
    if(revision!==generation) return;
    copied.value=id; clearTimeout(timer); timer=setTimeout(() => {copied.value="";},2000);
  } catch { if(revision===generation) error.value="copy"; }
}
async function recheck() {
  if(checking.value || !connected.value) return;
  const revision=generation; pending.value=true; error.value=null;
  try { await remoteBackend.discoverTools(targetId.value); if(revision===generation) emit("refresh"); }
  catch { if(revision===generation) {error.value="recheck"; emit("refresh");} }
  finally { if(revision===generation) pending.value=false; }
}
async function terminal() {
  if(terminalPending.value || !connected.value) return;
  const revision=generation; terminalPending.value=true; error.value=null;
  try { await (proxyOn.value ? remoteBackend.launchProxyTerminal() : remoteBackend.launchManualTerminal()); }
  catch { if(revision===generation) error.value="terminal"; }
  finally { if(revision===generation) terminalPending.value=false; }
}
async function openOfficial(event:MouseEvent,method:ToolInstallMethod) {
  if (!isTauri()) return;
  event.preventDefault();
  // Only the two reviewed catalog URLs have opener permissions.
  try { await openUrl(method.officialUrl); }
  catch { error.value="documentation"; }
}
onBeforeUnmount(() => {generation++;clearTimeout(timer);});
defineExpose({open});
</script>

<template>
  <dialog ref="dialog" class="confirmation-dialog remote-install-dialog" aria-labelledby="remote-install-title" aria-describedby="remote-install-intro" @cancel.prevent="close">
    <header><h2 id="remote-install-title" ref="heading" tabindex="-1">{{ adapter.displayName }} · {{ copy.rbInstallTitle }}</h2><StatusIndicator :state="state" :label="toolInstallationLabel(copy,summary,tool)" /></header>
    <p id="remote-install-intro">{{ inspection.extensionInstallation === 'installed' ? copy.rbInstallExtensionOnly : copy.rbInstallIntro }}</p>
    <div class="install-methods">
      <section v-for="method in installMethods(tool)" :key="method.id" class="install-method">
        <h3>{{ method.label }}<span v-if="method.recommended" class="install-recommended">{{ copy.rbInstallRecommended }}</span><a :href="method.officialUrl" @click="openOfficial($event,method)" target="_blank" rel="noopener noreferrer">{{ copy.rbInstallOfficial }}</a></h3>
        <div class="install-command"><code>{{ method.command }}</code><button class="secondary-action install-copy" :class="{'is-copied':copied===method.id}" type="button" :aria-label="copied===method.id ? copy.rbCopied : `${copy.rbInstallCopy} · ${method.label}`" :title="copied===method.id ? copy.rbCopied : copy.rbInstallCopy" @click="copyCommand(method.command,method.id)"><svg viewBox="0 0 24 24" aria-hidden="true"><path v-if="copied===method.id" d="m5 12 4 4L19 6"/><template v-else><rect x="8" y="8" width="12" height="13" rx="2"/><path d="M15 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3"/></template></svg></button></div>
        <p v-for="requirement in installMissingRequirements(tool,method,summary.installCapabilities)" :key="requirement" class="install-requirement">{{ copy.rbInstallRequirement.replace('{requirement}',requirement==='node' ? copy.rbInstallNode : requirement) }}</p>
        <small v-if="method.note==='noSudo'">{{ copy.rbInstallNoSudo }}<template v-if="tool==='claude'"> {{ copy.rbInstallNodeRecommended }}</template></small>
      </section>
    </div>
    <div class="install-verification"><span>{{ copy.rbInstallVerify }}</span><code>{{ installMethods(tool)[0].verifyCommand }}</code></div>
    <p class="install-network">{{ copy.rbInstallNetwork }}<span v-if="!proxyOn"> {{ copy.rbInstallDirect }}</span></p>
    <footer><button class="secondary-action" type="button" :disabled="!connected || terminalPending" @click="terminal">{{ copy.rbOpenTerminal }}</button><button class="secondary-action" type="button" :disabled="!connected || checking" @click="recheck">{{ checking ? copy.rbInstallChecking : copy.rbInstallRecheck }}</button><button class="secondary-action" type="button" @click="close">{{ copy.rbClose }}</button></footer>
  </dialog>
</template>

<style scoped>
.remote-install-dialog { width:min(600px,calc(100vw - 32px)); max-width:600px; max-height:calc(100dvh - 32px); padding:24px; overflow-y:auto; color:var(--ink); user-select:text; scrollbar-width:thin; scrollbar-color:var(--line-strong) transparent; }
header { display:grid; gap:8px; }
header h2 { margin:0; font-size:19px; line-height:1.4; }
header h2:focus { outline:none; }
.remote-install-dialog p { margin:12px 0; color:var(--muted); font-size:12px; line-height:1.6; }
.install-methods { margin:16px 0; }
.install-method { padding:12px 0; border-top:1px solid var(--line); }
.install-method h3 { display:flex; flex-wrap:wrap; align-items:center; gap:8px; margin:0 0 8px; font-size:13px; }
.install-recommended { color:var(--success); font-size:11px; font-weight:500; }
.install-method a { margin-left:auto; color:var(--accent-strong); font-size:11px; font-weight:500; text-underline-offset:3px; }
.install-command { display:flex; align-items:center; min-width:0; gap:8px; padding:8px 10px; border-radius:8px; background:var(--surface); }
.install-command code { flex:1; min-width:0; font-size:11px; line-height:1.6; overflow-wrap:anywhere; }
.install-copy { display:grid; place-items:center; flex:none; width:32px; min-height:32px; padding:6px; }
.install-copy svg { width:16px; height:16px; fill:none; stroke:currentColor; stroke-width:1.8; stroke-linecap:round; stroke-linejoin:round; }
.install-copy.is-copied { color:var(--success); }
.install-method p.install-requirement { margin:6px 0 0; color:var(--warning); font-size:11px; }
.install-method small { color:var(--muted); font-size:11px; }
.install-verification { display:flex; flex-wrap:wrap; gap:8px; align-items:center; color:var(--muted); font-size:12px; }
.install-verification code { color:var(--ink); }
footer { display:flex; flex-wrap:wrap; justify-content:flex-end; gap:8px; margin-top:16px; }
footer button { min-height:34px; padding:7px 12px; font-size:12px; }
@media(max-width:540px) { .remote-install-dialog {padding:18px;} footer {justify-content:flex-start;} }
</style>
