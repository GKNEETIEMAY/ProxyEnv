<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import type { Copy } from "../../../shared/i18n";
import type { ProxyCandidate } from "../../../shared/types";
import { copyText } from "../../../shared/utils/clipboard";

const props = defineProps<{
  copy: Copy;
  candidate?: ProxyCandidate;
  busy: boolean;
  launchProcess: () => Promise<string>;
  restartProcess?: () => Promise<string>;
}>();

const dialog = ref<HTMLDialogElement>();
const copiedField = ref<"host" | "port" | "endpoint">();
const copyError = ref("");
const launchError = ref("");
const launchResult = ref("");
const launching = ref(false);
const confirmingRestart = ref(false);
let copyTimer: number | undefined;

const fullAddress = computed(() => {
  if (!props.candidate) return "";
  const host = props.candidate.host.includes(":") && !props.candidate.host.startsWith("[")
    ? `[${props.candidate.host}]`
    : props.candidate.host;
  return `${host}:${props.candidate.port}`;
});

function open() {
  copiedField.value = undefined;
  copyError.value = "";
  launchError.value = "";
  launchResult.value = "";
  launching.value = false;
  confirmingRestart.value = false;
  if (!dialog.value?.open) dialog.value?.showModal();
}

function close() {
  if (launching.value) return;
  confirmingRestart.value = false;
  dialog.value?.close();
}

function handleCancel(event: Event) {
  event.preventDefault();
  close();
}

async function copyField(field: "host" | "port" | "endpoint", value: string) {
  copyError.value = "";
  try {
    await copyText(value);
    copiedField.value = field;
    if (copyTimer !== undefined) window.clearTimeout(copyTimer);
    copyTimer = window.setTimeout(() => { copiedField.value = undefined; }, 1600);
  } catch (cause) {
    copyError.value = `${props.copy.copyFailed}: ${String(cause)}`;
  }
}

async function launchNewProcess() {
  if (props.busy || launching.value || !props.candidate) return;
  launchError.value = "";
  launching.value = true;
  try {
    launchResult.value = await props.launchProcess();
  } catch (cause) {
    launchError.value = String(cause);
  } finally {
    launching.value = false;
  }
}

function requestRestart() {
  launchError.value = "";
  confirmingRestart.value = true;
}

async function restartCurrentProcess() {
  if (props.busy || launching.value || !props.candidate || !props.restartProcess) return;
  launchError.value = "";
  launching.value = true;
  try {
    launchResult.value = await props.restartProcess();
    confirmingRestart.value = false;
  } catch (cause) {
    launchError.value = String(cause);
  } finally {
    launching.value = false;
  }
}

onBeforeUnmount(() => {
  if (copyTimer !== undefined) window.clearTimeout(copyTimer);
});

defineExpose({ open });
</script>

<template>
  <dialog ref="dialog" class="confirmation-dialog proxy-guide-dialog" @cancel="handleCancel">
    <form method="dialog" @submit.prevent>
      <span class="confirmation-icon verified" aria-hidden="true">
        <svg viewBox="0 0 24 24"><path d="M8.5 8.5 6.8 6.8a3 3 0 0 0-4.2 4.2l2.8 2.8a3 3 0 0 0 4.2 0l1.2-1.2m4.7 2.9 1.7 1.7a3 3 0 0 0 4.2-4.2l-2.8-2.8a3 3 0 0 0-4.2 0l-1.2 1.2M8.5 15.5l7-7" /></svg>
      </span>
      <h2>{{ copy.assistantProxyGuideTitle }}</h2>
      <p>{{ copy.assistantProxyGuideBody }}</p>

      <dl v-if="candidate" class="confirmation-endpoint proxy-guide-fields">
        <div>
          <dt>{{ copy.assistantHostAddress }}</dt>
          <dd><code>{{ candidate.host }}</code><button type="button" :class="{ 'is-copied': copiedField === 'host' }" :aria-label="`${copy.copyEndpoint}: ${copy.assistantHostAddress}`" :title="copiedField === 'host' ? copy.endpointCopied : copy.copyEndpoint" @click="copyField('host', candidate.host)"><svg v-if="copiedField !== 'host'" viewBox="0 0 20 20" aria-hidden="true"><rect x="6.5" y="6.5" width="9" height="9" rx="1.6"/><path d="M13.5 6.5V5A1.5 1.5 0 0 0 12 3.5H5A1.5 1.5 0 0 0 3.5 5v7A1.5 1.5 0 0 0 5 13.5h1.5"/></svg><svg v-else viewBox="0 0 20 20" aria-hidden="true"><path d="m4.5 10.2 3.2 3.2 7.8-7.8"/></svg></button></dd>
        </div>
        <div>
          <dt>{{ copy.port }}</dt>
          <dd><code>{{ candidate.port }}</code><button type="button" :class="{ 'is-copied': copiedField === 'port' }" :aria-label="`${copy.copyEndpoint}: ${copy.port}`" :title="copiedField === 'port' ? copy.endpointCopied : copy.copyEndpoint" @click="copyField('port', String(candidate.port))"><svg v-if="copiedField !== 'port'" viewBox="0 0 20 20" aria-hidden="true"><rect x="6.5" y="6.5" width="9" height="9" rx="1.6"/><path d="M13.5 6.5V5A1.5 1.5 0 0 0 12 3.5H5A1.5 1.5 0 0 0 3.5 5v7A1.5 1.5 0 0 0 5 13.5h1.5"/></svg><svg v-else viewBox="0 0 20 20" aria-hidden="true"><path d="m4.5 10.2 3.2 3.2 7.8-7.8"/></svg></button></dd>
        </div>
        <div>
          <dt>{{ copy.assistantFullAddress }}</dt>
          <dd><code>{{ fullAddress }}</code><button type="button" :class="{ 'is-copied': copiedField === 'endpoint' }" :aria-label="`${copy.copyEndpoint}: ${copy.assistantFullAddress}`" :title="copiedField === 'endpoint' ? copy.endpointCopied : copy.copyEndpoint" @click="copyField('endpoint', fullAddress)"><svg v-if="copiedField !== 'endpoint'" viewBox="0 0 20 20" aria-hidden="true"><rect x="6.5" y="6.5" width="9" height="9" rx="1.6"/><path d="M13.5 6.5V5A1.5 1.5 0 0 0 12 3.5H5A1.5 1.5 0 0 0 3.5 5v7A1.5 1.5 0 0 0 5 13.5h1.5"/></svg><svg v-else viewBox="0 0 20 20" aria-hidden="true"><path d="m4.5 10.2 3.2 3.2 7.8-7.8"/></svg></button></dd>
        </div>
      </dl>
      <span class="proxy-guide-copy-status" role="status" aria-live="polite">{{ copiedField ? copy.endpointCopied : "" }}</span>

      <p v-if="copyError" class="proxy-guide-copy-error" role="alert">{{ copyError }}</p>
      <div class="proxy-guide-feedback" aria-live="polite">
        <div v-if="launchResult" class="proxy-guide-launch-result" role="status"><strong>{{ copy.assistantManualProxyProcessStarted }}</strong><p>{{ launchResult }}</p></div>
        <p v-else-if="launchError" class="proxy-guide-copy-error" role="alert">{{ launchError }}</p>
        <div v-else-if="confirmingRestart" class="proxy-guide-restart-confirm" role="alert"><strong>{{ copy.assistantRestartProcessConfirmTitle }}</strong><p>{{ copy.assistantRestartProcessConfirmBody }}</p></div>
        <p v-else class="proxy-guide-credentials">{{ copy.assistantProxyCredentials }}</p>
      </div>
      <div class="confirmation-actions">
        <button v-if="launchResult" class="primary-action" type="button" @click="close">{{ copy.assistantClose }}</button>
        <template v-else-if="confirmingRestart">
          <button class="secondary-action" type="button" :disabled="busy || launching" @click="confirmingRestart = false">{{ copy.backToEdit }}</button>
          <button class="primary-action danger-confirm-action" type="button" :disabled="busy || launching || !candidate" :aria-busy="launching" @click="restartCurrentProcess">{{ copy.assistantConfirmRestartProcess }}</button>
        </template>
        <template v-else>
          <button class="secondary-action" type="button" :disabled="busy || launching" @click="close">{{ copy.cancel }}</button>
          <button v-if="restartProcess" class="secondary-action danger-action" type="button" :disabled="busy || launching || !candidate" @click="requestRestart">{{ copy.assistantRestartProcess }}</button>
          <button class="primary-action" type="button" :disabled="busy || launching || !candidate" :aria-busy="launching" @click="launchNewProcess">{{ copy.assistantLaunchNewProcess }}</button>
        </template>
      </div>
    </form>
  </dialog>
</template>
