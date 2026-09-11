<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { RemoteBridgeCopy } from '../../../shared/i18n/remote-bridge';
import { bridgeError, bridgeErrorCode } from '../../../shared/i18n/remote-bridge';
import { copyText } from '../../../shared/utils/clipboard';
import type { CheckState } from '../../../shared/types';
import StatusIndicator from '../../../shared/components/StatusIndicator.vue';
import { remoteBackend, type ConfigPreview, type ExtensionInspection, type ExtensionPreview } from '../state';
import { getRemoteToolAdapter, type RemoteToolId } from '../tool-adapters';

const props = defineProps<{ copy: RemoteBridgeCopy; sessionAlias: string | null; sessionStatus: string }>();
const emit = defineEmits<{ refresh: [] }>();
const dialog = ref<HTMLDialogElement>();
const heading = ref<HTMLElement>();
const tool = ref<RemoteToolId>('codex'), alias = ref('');
const targetName = ref('');
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
const adapter = computed(() => getRemoteToolAdapter(tool.value));
const path = computed(() => adapter.value.usesVscodeRemoteSettings
  ? inspection.value?.vscode.remoteSettingsPath || props.copy.rbVscodeRemoteSettings
  : adapter.value.extensionPath);
const extensionContextReady = computed(() => inspection.value?.vscode.status === 'detected');
const canPreview = computed(() => (cli.value || extension.value) && (!extension.value || Boolean(
  inspection.value
  && (restoring.value ? !adapter.value.restoreRequiresVscodeContext || extensionContextReady.value : extensionContextReady.value && locationConfirmed.value && capability.value?.detected && capability.value.supported)
)));
const outcome = (result: string) => result === 'success' ? (restoring.value ? props.copy.rbRestored : cliPreview.value?.permissionHardening ? props.copy.rbPermissionsHardened : cliPreview.value?.routeUpdate ? props.copy.rbRouteUpdated : props.copy.rbExtApplied) : result === 'failed' ? props.copy.rbExtFailed : props.copy.rbExtWaiting;
const resultState = (result: string): CheckState => result === 'success' ? 'healthy' : result === 'failed' ? 'failed' : 'idle';
const contextState = computed<CheckState>(() => !inspection.value ? 'idle' : extensionContextReady.value ? 'healthy' : inspection.value.vscode.status === 'ambiguous' ? 'warning' : 'idle');
const contextLabel = computed(() => !inspection.value ? props.copy.rbExtUnknown : ({ detected: props.copy.rbVscodeContextDetected, ambiguous: props.copy.rbVscodeContextAmbiguous, unsupported: props.copy.rbVscodeContextMissing })[inspection.value.vscode.status]);
const extensionDetectionState = computed<CheckState>(() => !inspection.value || !capability.value?.detected ? 'idle' : capability.value.candidateCount > 1 || !capability.value.supported ? 'warning' : 'healthy');
const extensionDetectionLabel = computed(() => {
  if (!inspection.value) return props.copy.rbExtUnknown;
  if (!capability.value?.detected) return props.copy.rbExtMissing;
  if (capability.value.candidateCount > 1) return props.copy.rbExtMultipleVersions;
  return capability.value.supported ? props.copy.rbExtDetected : props.copy.rbExtUnsupported;
});
const extensionLocationState = computed<CheckState>(() => locationConfirmed.value ? 'healthy' : capability.value?.detected ? 'warning' : 'idle');
const extensionLocationLabel = computed(() => locationConfirmed.value ? props.copy.rbExtRemoteConfirmed : capability.value?.location === 'activeUnknown' ? props.copy.rbExtActiveUnknown : props.copy.rbExtLocationUnknown);
const extensionConfigurationState = computed<CheckState>(() => capability.value?.configuration === 'configured' ? 'warning' : capability.value?.configuration === 'conflict' ? 'failed' : 'idle');
const extensionConfigurationLabel = computed(() => capability.value?.configuration === 'configured' ? props.copy.rbExtPending : capability.value?.configuration === 'conflict' ? props.copy.rbConfigError : capability.value?.configuration === 'unknown' ? props.copy.rbExtConfigUnknown : props.copy.rbExtNotConfigured);
const installedVersions = computed(() => capability.value?.versions.join(' · ') || '');
const serverVersions = computed(() => inspection.value?.vscode.serverVersions.join(' · ') || '');
const title = computed(() => `${adapter.value.displayName} · ${phase.value === 'select' ? props.copy.rbExtTitle : restoring.value ? props.copy.rbExtRestoreTitle : props.copy.rbExtEnableTitle}`);
const impact = computed(() => adapter.value.impact(props.copy, restoring.value));
const errorText = computed(() => operationSurface.value === 'extension' && ['configConflict','unsafePath','noBackup','rollbackConflict','rollbackFailed','writeRolledBack','verifyFailed'].includes(bridgeErrorCode(error.value)) ? props.copy.rbExtError : bridgeError(error.value, props.copy));
const after = computed(() => {
  const p = extensionPreview.value;
  if (!p) return '';
  if (p.restore) return p.originalExists ? props.copy.rbExtRestoreOpaque : props.copy.rbExtRestoreAbsent;
  return adapter.value.renderExtensionPreview(p.port);
});
async function perform(action: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true; error.value = undefined;
  try { await action(); } catch(cause) { error.value = cause; } finally { busy.value = false; emit('refresh'); }
}
function open(selected: RemoteToolId, target: string, restore = false, label = '', directCli = false) {
  if (busy.value) return;
  generation++;
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  tool.value = selected; alias.value = target; targetName.value = label; restoring.value = restore;
  cli.value = true; extension.value = false; inspection.value = undefined; locationConfirmed.value = false;
  phase.value = 'select'; cliPreview.value = undefined; extensionPreview.value = undefined;
  cliResult.value = 'waiting'; extensionResult.value = 'waiting'; error.value = undefined; copied.value = false;
  dialog.value?.showModal();
  if (directCli) void nextTick(review);
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
      const result = await adapter.value.preview(alias.value, restoring.value);
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
        if (restoring.value) await adapter.value.restore(cliPreview.value.id);
        else await adapter.value.apply(cliPreview.value.id);
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
  inspection.value = undefined;
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
      <p><strong>{{ targetName }}</strong></p>
      <p class="remote-hint">{{ restoring ? copy.rbExtRestoreScope : copy.rbExtScope }}</p>
      <fieldset class="remote-fields" :disabled="busy">
        <template v-if="phase === 'select'">
          <label class="remote-choice"><input v-model="cli" type="checkbox">{{ copy.rbExtCli }}</label>
          <p v-if="cli" class="remote-hint">{{ restoring ? copy.rbExtRestoreImpact : adapter.configHint(copy) }}</p>
          <label class="remote-choice"><input v-model="extension" type="checkbox">{{ copy.rbExtGui }}</label>
          <section v-if="extension" class="remote-capability">
            <p class="remote-hint">{{ copy.rbExtLocation }}</p>
            <button type="button" class="secondary-action" @click="inspect">{{ inspection ? copy.rbExtReinspect : copy.rbExtInspect }}</button>
            <div class="remote-context-status">
              <StatusIndicator :state="contextState" :label="contextLabel" />
              <StatusIndicator :state="extensionDetectionState" :label="extensionDetectionLabel" />
              <StatusIndicator v-if="capability" :state="extensionLocationState" :label="extensionLocationLabel" />
              <StatusIndicator v-if="capability" :state="extensionConfigurationState" :label="extensionConfigurationLabel" />
            </div>
            <dl v-if="inspection" class="remote-context-facts">
              <div><dt>{{ copy.rbVscodeEdition }}</dt><dd>{{ inspection.vscode.edition === 'unknown' ? copy.rbExtUnknown : inspection.vscode.edition }}</dd></div>
              <div v-if="inspection.vscode.serverRoot"><dt>{{ copy.rbVscodeServerRoot }}</dt><dd><code>{{ inspection.vscode.serverRoot }}</code></dd></div>
              <div v-if="serverVersions"><dt>{{ copy.rbVscodeServerVersions }}</dt><dd><code>{{ serverVersions }}</code></dd></div>
              <div v-if="inspection.vscode.remoteSettingsPath"><dt>{{ copy.rbVscodeRemoteSettings }}</dt><dd><code>{{ inspection.vscode.remoteSettingsPath }}</code></dd></div>
              <div><dt>{{ copy.rbExtRemoteAccount }}</dt><dd><strong>{{ inspection.user }}</strong></dd></div>
              <div v-if="installedVersions"><dt>{{ copy.rbExtInstalledVersions }}</dt><dd><code>{{ installedVersions }}</code><span v-if="capability?.runtimeVersions.length"> · Codex <code>{{ capability.runtimeVersions.join(' · ') }}</code></span></dd></div>
            </dl>
            <p v-if="inspection?.vscode.status === 'ambiguous'" class="notice notice-warning">{{ copy.rbVscodeContextAmbiguousHint }}</p>
            <p v-else-if="inspection?.vscode.status === 'unsupported'" class="notice notice-warning">{{ copy.rbVscodeContextMissingHint }}</p>
            <p v-if="capability && capability.candidateCount > 1" class="notice notice-warning">{{ copy.rbExtMultipleVersionsHint }}</p>
            <label v-if="inspection && capability?.detected && extensionContextReady && !restoring" class="remote-choice"><input v-model="locationConfirmed" type="checkbox" :disabled="!capability.supported">{{ copy.rbExtConfirmLocation }}</label>
            <p v-if="inspection && capability?.detected && !locationConfirmed && !restoring" class="remote-hint">{{ copy.rbExtConfirmationEvidence }}</p>
            <p class="notice notice-warning">{{ impact }}</p>
          </section>
        </template>
        <template v-else-if="phase === 'preview'">
          <section v-if="cliPreview" class="remote-capability"><h3>{{ copy.rbExtCli }}</h3><p><code>{{ cliPreview.path }}</code></p><p v-if="cliPreview.permissionHardening" class="notice notice-warning">{{ copy.rbPermissionHardeningHint }}</p><p v-if="cliPreview.routeUpdate" class="notice notice-warning">{{ copy.rbRouteUpdateHint }}</p><h4>{{ copy.rbBefore }}</h4><pre>{{ cliPreview.before || (cliPreview.existingConfig ? copy.rbExistingConfigProtected : copy.rbAbsent) }}</pre><h4>{{ copy.rbAfter }}</h4><pre>{{ cliPreview.after || (cliPreview.existingConfig ? copy.rbExistingConfigProtected : copy.rbAbsent) }}</pre><p class="notice notice-warning">{{ restoring ? copy.rbRestoreHint : copy.rbConfigBackupHint }}</p></section>
          <section v-if="extensionPreview" class="remote-capability"><h3>{{ copy.rbExtGui }}</h3><p><code>{{ extensionPreview.path }}</code></p><h4>{{ copy.rbBefore }}</h4><p>{{ copy.rbExtOpaque }}</p><p v-if="extensionPreview.previousPort"><code>127.0.0.1:{{ extensionPreview.previousPort }}</code></p><p v-if="extensionPreview.loginPromptChange === 'overrideFalse'" class="notice notice-warning">{{ copy.rbExtLoginPromptConflict }}</p><h4>{{ copy.rbAfter }}</h4><pre>{{ after }}</pre><p class="notice notice-warning">{{ impact }}</p></section>
          <p v-if="restoring" class="remote-hint">{{ copy.rbExtRestoreScope }}</p>
        </template>
        <template v-else>
          <StatusIndicator v-if="cliPreview" :state="resultState(cliResult)" :label="`${copy.rbExtCli} · ${outcome(cliResult)}`" />
          <StatusIndicator v-if="extensionPreview" :state="resultState(extensionResult)" :label="`${copy.rbExtGui} · ${outcome(extensionResult)}`" />
          <p v-if="error && (cliResult === 'success' || extensionResult === 'success')" class="notice notice-warning">{{ copy.rbExtPartial }}</p>
          <p v-if="extensionResult === 'success'" class="remote-hint">{{ restoring ? copy.rbExtRestored : copy.rbExtRestart }}</p>
          <template v-if="cliResult === 'success' && cliPreview?.launch && !restoring"><p class="remote-success">{{ copy.rbCliOverlayReady }}</p><p><code>{{ cliPreview.path }}</code></p><pre>{{ cliPreview.launch }}</pre><button class="secondary-action" type="button" @click="perform(async () => { await copyText(cliPreview!.launch); copied = true; })">{{ copied ? copy.rbCopied : copy.rbCopyLaunch }}</button></template>
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

<style scoped>
.remote-context-status {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 14px;
  margin: 14px 0 10px;
}
.remote-context-facts {
  margin: 0;
  border-block: 1px solid var(--line);
}
.remote-context-facts > div {
  display: grid;
  grid-template-columns: minmax(118px, .42fr) minmax(0, 1fr);
  gap: 12px;
  padding: 8px 0;
}
.remote-context-facts > div + div { border-top: 1px solid var(--line); }
.remote-context-facts dt { color: var(--muted); }
.remote-context-facts dd {
  min-width: 0;
  margin: 0;
  overflow-wrap: anywhere;
}
@media (max-width: 560px) {
  .remote-context-facts > div { grid-template-columns: 1fr; gap: 3px; }
}
</style>
