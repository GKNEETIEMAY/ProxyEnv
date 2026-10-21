<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import type { Copy } from "../../../shared/i18n";
import type { ManagedProxyVariable, ProxyEndpoint, ProxyEndpointInspection, ProxyProtocol } from "../../../shared/types";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
const props = defineProps<{ copy: Copy; toggling: boolean; selectedVariables: ManagedProxyVariable[]; inspectManualEndpoint: (endpoint: ProxyEndpoint) => Promise<ProxyEndpointInspection> }>();
const emit = defineEmits<{ applyManual: [endpoint: ProxyEndpoint] }>();
const manualHost = ref("127.0.0.1");
const manualPort = ref("7897");
const manualProtocol = ref<Exclude<ProxyProtocol, "unknown">>("mixed");
const pendingManualEndpoint = ref<ProxyEndpoint>();
const confirmationDialog = ref<HTMLDialogElement>();
const manualAttempted = ref(false);
const inspectingManual = ref(false);
const inspectionWarning = ref<"notListening" | "unknownProtocol" | "protocolMismatch" | "inspectionFailed">();
const manualHostValid = computed(() => {
  const host = manualHost.value.trim().toLowerCase();
  if (host === "localhost" || host === "::1" || host === "[::1]") return true;
  const octets = host.split(".").map(Number);
  return octets.length === 4
    && octets[0] === 127
    && octets.every((octet) => Number.isInteger(octet) && octet >= 0 && octet <= 255);
});
const manualPortValid = computed(() => { const port = Number(manualPort.value); return Number.isInteger(port) && port >= 1 && port <= 65535; });
const manualValid = computed(() => manualHostValid.value && manualPortValid.value);
const inspectionWarningText = computed(() => inspectionWarning.value ? ({
  notListening: props.copy.manualNotListening,
  unknownProtocol: props.copy.manualProtocolUnknown,
  protocolMismatch: props.copy.manualProtocolMismatch,
  inspectionFailed: props.copy.manualInspectionFailed
})[inspectionWarning.value] : "");

async function requestManualApply() {
  if (inspectingManual.value || props.toggling) return;
  manualAttempted.value = true;
  if (!manualValid.value) return;
  const endpoint = { host: manualHost.value.trim(), port: Number(manualPort.value), protocol: manualProtocol.value };
  pendingManualEndpoint.value = endpoint;
  inspectionWarning.value = undefined;
  inspectingManual.value = true;
  try {
    const inspection = await props.inspectManualEndpoint(endpoint);
    if (!inspection.listening) inspectionWarning.value = "notListening";
    else if (inspection.detectedProtocol === "unknown") inspectionWarning.value = "unknownProtocol";
    else if (!inspection.protocolMatches) inspectionWarning.value = "protocolMismatch";
  } catch { inspectionWarning.value = "inspectionFailed"; }
  finally { inspectingManual.value = false; }
  await nextTick();
  confirmationDialog.value?.showModal();
}
function cancelManualApply() { confirmationDialog.value?.close(); pendingManualEndpoint.value = undefined; inspectionWarning.value = undefined; }
function confirmManualApply() { if (!props.toggling && pendingManualEndpoint.value) emit("applyManual", pendingManualEndpoint.value); cancelManualApply(); }
</script>
<template>
  <section class="local-section local-manual">
    <h2>{{ copy.manualProxy }}</h2>
    <p class="local-caption">{{ copy.localManualEffect }}</p>
        <form class="manual-endpoint" @submit.prevent="requestManualApply">
          <label><span class="field-label"><span>{{ copy.host }}</span><HelpTooltip :label="copy.aboutHost" :text="copy.hostDescription" /></span><input v-model="manualHost" autocomplete="off" spellcheck="false" placeholder="127.0.0.1" :aria-invalid="manualAttempted && !manualHostValid" aria-describedby="manual-host-error" /><small v-if="manualAttempted && !manualHostValid" id="manual-host-error" class="field-error">{{ copy.invalidHost }}</small></label>
          <label><span class="field-label"><span>{{ copy.port }}</span><HelpTooltip :label="copy.aboutPort" :text="copy.portDescription" /></span><input v-model="manualPort" inputmode="numeric" autocomplete="off" placeholder="7897" :aria-invalid="manualAttempted && !manualPortValid" aria-describedby="manual-port-error" /><small v-if="manualAttempted && !manualPortValid" id="manual-port-error" class="field-error">{{ copy.invalidPort }}</small></label>
          <label><span class="field-label"><span>{{ copy.protocol }}</span><HelpTooltip :label="copy.aboutProtocol" :text="copy.protocolDescription" /></span><select v-model="manualProtocol"><option value="http">HTTP</option><option value="socks5">SOCKS5</option><option value="mixed">Mixed</option></select></label>
          <button class="primary-action" type="submit" :disabled="toggling || inspectingManual">{{ inspectingManual ? copy.checkingEndpoint : toggling ? copy.enabling : copy.applyManualProxy }}</button>
        </form>
    <dialog ref="confirmationDialog" class="confirmation-dialog" aria-labelledby="local-manual-confirm-title" @cancel="cancelManualApply">
      <form method="dialog" @submit.prevent>
        <div class="confirmation-icon" :class="{ verified: !inspectionWarning }" aria-hidden="true"><svg v-if="inspectionWarning" viewBox="0 0 24 24"><path d="M12 8v5m0 3.5v.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg><svg v-else viewBox="0 0 24 24"><path d="m7 12.3 3.2 3.2L17.5 8M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg></div>
        <h2 id="local-manual-confirm-title">{{ inspectionWarning ? copy.manualWarningTitle : copy.manualConfirmTitle }}</h2>
        <p>{{ inspectionWarning ? inspectionWarningText : copy.localManualEffect }}</p>
        <dl v-if="pendingManualEndpoint" class="confirmation-endpoint">
          <div><dt>{{ copy.host }}</dt><dd><code>{{ pendingManualEndpoint.host }}</code></dd></div>
          <div><dt>{{ copy.port }}</dt><dd><code>{{ pendingManualEndpoint.port }}</code></dd></div>
          <div><dt>{{ copy.protocol }}</dt><dd><code>{{ pendingManualEndpoint.protocol }}</code></dd></div>
          <div><dt>{{ copy.variables }}</dt><dd><code>{{ selectedVariables.join(', ') }}</code></dd></div>
        </dl>
        <p v-if="inspectionWarning" class="local-caption">{{ copy.localManualEffect }}</p>
        <p class="confirmation-consequence" :class="{ warning: inspectionWarning }">{{ inspectionWarning ? copy.manualOverrideConsequence : copy.manualConfirmConsequence }}</p>
        <div class="confirmation-actions">
          <button class="secondary-action" type="button" autofocus @click="cancelManualApply">{{ inspectionWarning ? copy.backToEdit : copy.cancel }}</button>
          <button class="primary-action" :class="{ 'warning-action': inspectionWarning }" type="button" @click="confirmManualApply">{{ inspectionWarning ? copy.applyAnyway : copy.confirmApply }}</button>
        </div>
      </form>
    </dialog>
  </section>
</template>
