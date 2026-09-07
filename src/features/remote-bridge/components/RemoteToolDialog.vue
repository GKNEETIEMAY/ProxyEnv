<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { RemoteBridgeCopy } from '../../../shared/i18n/remote-bridge';
import { bridgeError } from '../../../shared/i18n/remote-bridge';
import { copyText } from '../../../shared/utils/clipboard';
import { remoteBackend, targetLabel, type ConfigPreview, type ExtensionInspection, type ExtensionPreview } from '../state';

const props = defineProps<{ copy: RemoteBridgeCopy; sessionAlias: string | null; sessionStatus: string }>();
const emit = defineEmits<{ refresh: [] }>();
const dialog = ref<HTMLDialogElement>();
const heading = ref<HTMLElement>();
const tool = ref('codex'), alias = ref('');
const restoring = ref(false), cli = ref(true), extension = ref(false), locationConfirmed = ref(false), busy = ref(false);
const phase = ref<'select' | 'preview' | 'result'>('select');
const inspection = ref<ExtensionInspection>();
const cliPreview = ref<ConfigPreview>();
const extensionPreview = ref<ExtensionPreview>();
const error = ref<unknown>();
const cliResult = ref<'success' | 'failed' | 'waiting'>('waiting');
const extensionResult = ref<'success' | 'failed' | 'waiting'>('waiting');
const copied = ref(false);
const operationSurface = ref<'cli' | 'extension'>('cli');
let previousFocus: HTMLElement | null = null;
let generation = 0;
const capability = computed(() => inspection.value?.extensions.find(e => e.tool === tool.value));
const path = computed(() => tool.value === 'codex' ? '~/.codex/config.toml' : '~/.vscode-server/data/Machine/settings.json');
const canPreview = computed(() => (cli.value || extension.value) && (!extension.value || inspection.value && locationConfirmed.value && (restoring.value || capability.value?.supported)));
const outcome = (result: string) => result === 'success' ? (restoring.value ? props.copy.rbRestored : props.copy.rbExtApplied) : result === 'failed' ? props.copy.rbExtFailed : props.copy.rbExtWaiting;
const title = computed(() => `${tool.value === 'codex' ? 'Codex' : 'Claude Code'} · ${restoring.value ? props.copy.rbExtRestoreTitle : props.copy.rbExtTitle}`);
const impact = computed(() => restoring.value ? props.copy.rbExtRestoreImpact : tool.value === 'codex' ? props.copy.rbExtCodexImpact : props.copy.rbExtClaudeImpact);
const errorText = computed(() => operationSurface.value === 'extension' && ['configConflict','unsafePath','noBackup','rollbackConflict','rollbackFailed','writeRolledBack','verifyFailed'].includes(String(error.value)) ? props.copy.rbExtError : bridgeError(error.value, props.copy));
const after = computed(() => {
  const p = extensionPreview.value;
  if (!p) return '';
  if (p.restore) return p.originalExists ? props.copy.rbExtRestoreOpaque : props.copy.rbExtRestoreAbsent;
  return tool.value === 'codex'
    ? `model_provider = "proxyenv_bridge"\n[model_providers.proxyenv_bridge]\nname = "ProxyEnv CC Switch"\nbase_url = "http://127.0.0.1:${p.port}/v1"\nwire_api = "responses"\nrequires_openai_auth = false`
    : JSON.stringify({ 'claudeCode.environmentVariables': [{ name: 'ANTHROPIC_BASE_URL', value: `http://127.0.0.1:${p.port}` }, { name: 'ANTHROPIC_AUTH_TOKEN', value: 'PROXY_MANAGED' }] }, null, 2);
});
async function perform(action: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true; error.value = undefined;
  try { await action(); } catch(cause) { error.value = cause; } finally { busy.value = false; emit('refresh'); }
}
function open(selected: string, target: string, restore = false) {
  if (busy.value) return;
  generation++;
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  tool.value = selected; alias.value = target; restoring.value = restore;
  cli.value = true; extension.value = false; inspection.value = undefined; locationConfirmed.value = false;
  phase.value = 'select'; cliPreview.value = undefined; extensionPreview.value = undefined;
  cliResult.value = 'waiting'; extensionResult.value = 'waiting'; error.value = undefined; copied.value = false;
  dialog.value?.showModal();
}
function close() { generation++; dialog.value?.close(); previousFocus?.focus(); }
function escape(event: KeyboardEvent) {
  if (event.key !== 'Escape' || !dialog.value?.open) return;
  // Replacing the focused phase button can temporarily focus body. Own Escape
  // at capture phase so a single key never dismisses the underlying dialog too.
  event.preventDefault(); event.stopImmediatePropagation();
  if (!busy.value) close();
}
onMounted(() => window.addEventListener('keydown', escape, true));
onBeforeUnmount(() => window.removeEventListener('keydown', escape, true));
watch(phase, () => { void nextTick(() => { if (dialog.value?.open) heading.value?.focus(); }); });
function inspect() {
  const current = generation;
  locationConfirmed.value = false; inspection.value = undefined;
  operationSurface.value = 'extension';
  void perform(async () => { const result = await remoteBackend.extensionInspect(alias.value); if (current === generation) inspection.value = result; });
}
function review() {
  if (!canPreview.value) return;
  const current = generation;
  void perform(async () => {
    cliPreview.value = undefined; extensionPreview.value = undefined;
    if (cli.value) {
      operationSurface.value = 'cli';
      const result = restoring.value ? await remoteBackend.configRestorePreview(alias.value, tool.value) : await remoteBackend.configPreview(tool.value);
      if (current !== generation) return;
      cliPreview.value = result;
    }
    if (extension.value && inspection.value) {
      operationSurface.value = 'extension';
      const result = await remoteBackend.extensionPreview({ alias: alias.value, tool: tool.value, contextHash: inspection.value.contextHash, remoteConfirmed: locationConfirmed.value, restore: restoring.value });
      if (current !== generation) return;
      extensionPreview.value = result;
    }
    if (current === generation) phase.value = 'preview';
  });
}
function apply() {
  if (phase.value !== 'preview') return;
  void perform(async () => {
    // Each reviewed file commits independently. Stop on failure and report the exact partial result.
    phase.value = 'result';
    if (cliPreview.value) {
      operationSurface.value = 'cli';
      try {
        if (restoring.value) await remoteBackend.configRestore(cliPreview.value.id);
        else await remoteBackend.configApply(cliPreview.value.id);
        cliResult.value = 'success';
      } catch(cause) { cliResult.value = 'failed'; throw cause; }
    }
    if (extensionPreview.value) {
      operationSurface.value = 'extension';
      try { await remoteBackend.extensionApply(extensionPreview.value.id); extensionResult.value = 'success'; }
      catch(cause) { extensionResult.value = 'failed'; throw cause; }
    }
  });
}
watch([() => props.sessionAlias, () => props.sessionStatus], () => {
  locationConfirmed.value = false;
  if (cliPreview.value && (props.sessionAlias !== alias.value || ['disconnected', 'error'].includes(props.sessionStatus))) cliPreview.value.launch = '';
  if (phase.value === 'preview' && !restoring.value) { generation++; phase.value = 'select'; cliPreview.value = undefined; extensionPreview.value = undefined; }
});
watch(extension, () => { locationConfirmed.value = false; });
defineExpose({ open, close });
</script>
<template>
  <dialog ref="dialog" class="confirmation-dialog remote-bridge-dialog" aria-labelledby="remote-tool-title" @keydown.esc.stop.prevent="busy ? undefined : close()" @cancel.prevent="busy ? undefined : close()">
    <form @submit.prevent="phase === 'select' ? review() : phase === 'preview' ? apply() : undefined">
      <div class="remote-heading"><h2 id="remote-tool-title" ref="heading" tabindex="-1">{{ title }}</h2><button type="button" class="secondary-action" :disabled="busy" @click="close">{{ copy.rbClose }}</button></div>
      <p><strong>{{ targetLabel(alias) }}</strong></p>
      <p class="remote-hint">{{ restoring ? copy.rbExtRestoreScope : copy.rbExtScope }}</p>
      <fieldset class="remote-fields" :disabled="busy">
        <template v-if="phase === 'select'">
          <label class="remote-choice"><input v-model="cli" type="checkbox">{{ copy.rbExtCli }}</label>
          <p v-if="cli" class="remote-hint">{{ restoring ? copy.rbExtRestoreImpact : copy.rbConfigHint }}</p>
          <label class="remote-choice"><input v-model="extension" type="checkbox">{{ copy.rbExtGui }}</label>
          <section v-if="extension" class="remote-capability">
            <p class="remote-hint">{{ copy.rbExtLocation }}</p>
            <p><code>{{ path }}</code></p>
            <button type="button" class="secondary-action" @click="inspect">{{ copy.rbExtInspect }}</button>
            <p role="status">{{ !inspection ? copy.rbExtUnknown : capability?.supported ? copy.rbExtDetected : copy.rbExtUnsupported }}</p>
            <p v-if="capability" role="status">{{ capability.configuration === 'configured' ? copy.rbExtPending : capability.configuration === 'conflict' ? copy.rbConfigError : copy.rbExtNotConfigured }}</p>
            <p v-if="inspection"><strong>{{ inspection.user }}</strong> · {{ capability?.version }}<span v-if="capability?.runtimeVersion"> · Codex {{ capability.runtimeVersion }}</span></p>
            <label class="remote-choice"><input v-model="locationConfirmed" type="checkbox" :disabled="!inspection || (!restoring && !capability?.supported)">{{ copy.rbExtConfirmLocation }}</label>
            <p class="notice notice-warning">{{ impact }}</p>
          </section>
        </template>
        <template v-else-if="phase === 'preview'">
          <section v-if="cliPreview" class="remote-capability"><h3>{{ copy.rbExtCli }}</h3><p><code>{{ cliPreview.path }}</code></p><h4>{{ copy.rbBefore }}</h4><pre>{{ cliPreview.before || copy.rbAbsent }}</pre><h4>{{ copy.rbAfter }}</h4><pre>{{ cliPreview.after || copy.rbAbsent }}</pre></section>
          <section v-if="extensionPreview" class="remote-capability"><h3>{{ copy.rbExtGui }}</h3><p><code>{{ extensionPreview.path }}</code></p><h4>{{ copy.rbBefore }}</h4><p>{{ copy.rbExtOpaque }}</p><p v-if="extensionPreview.previousPort"><code>127.0.0.1:{{ extensionPreview.previousPort }}</code></p><h4>{{ copy.rbAfter }}</h4><pre>{{ after }}</pre><p class="notice notice-warning">{{ impact }}</p></section>
          <p v-if="restoring" class="remote-hint">{{ copy.rbExtRestoreScope }}</p>
        </template>
        <template v-else>
          <p v-if="cliPreview">{{ copy.rbExtCli }} · {{ outcome(cliResult) }}</p>
          <p v-if="extensionPreview">{{ copy.rbExtGui }} · {{ outcome(extensionResult) }}</p>
          <p v-if="error && (cliResult === 'success' || extensionResult === 'success')" class="notice notice-warning">{{ copy.rbExtPartial }}</p>
          <p v-if="extensionResult === 'success'" class="remote-hint">{{ restoring ? copy.rbExtRestored : copy.rbExtRestart }}</p>
          <template v-if="cliResult === 'success' && cliPreview?.launch && !restoring"><pre>{{ cliPreview.launch }}</pre><button class="secondary-action" type="button" @click="perform(async () => { await copyText(cliPreview!.launch); copied = true; })">{{ copied ? copy.rbCopied : copy.rbCopyLaunch }}</button></template>
        </template>
      </fieldset>
      <p v-if="error" role="alert" class="remote-error">{{ errorText }}<br v-if="operationSurface === 'extension'"><code v-if="operationSurface === 'extension'">{{ path }}</code></p>
      <p role="status" class="remote-feedback">{{ busy ? copy.rbBusy : '' }}</p>
      <div class="confirmation-actions">
        <button v-if="phase !== 'select'" type="button" class="secondary-action" :disabled="busy" @click="phase = 'select'; locationConfirmed = false; cliResult = 'waiting'; extensionResult = 'waiting'">{{ copy.rbBack }}</button>
        <button v-if="phase !== 'result'" type="submit" class="primary-action" :disabled="busy || (phase === 'select' && !canPreview)">{{ phase === 'select' ? copy.rbExtReview : restoring ? copy.rbExtConfirmRestore : copy.rbConfirm }}</button>
      </div>
    </form>
  </dialog>
</template>
