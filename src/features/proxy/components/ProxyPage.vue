<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import type { Copy } from "../../../shared/i18n";
import type { EnvironmentStatus, ManagedProxyVariable, ProxyCandidate, ProxyEndpoint, ProxyEndpointInspection, ProxyProtocol, TunObservation } from "../../../shared/types";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
import NetworkObservationPanel from "../../network-observation/components/NetworkObservationPanel.vue";

const props = defineProps<{ copy: Copy; environment: EnvironmentStatus; candidates: ProxyCandidate[]; detected?: ProxyCandidate; systemProxy?: ProxyCandidate; tun: TunObservation; endpoint: string; error: string; loading: boolean; toggling: boolean; copiedEndpoint: boolean; selectedVariables: ManagedProxyVariable[]; inspectManualEndpoint: (endpoint: ProxyEndpoint) => Promise<ProxyEndpointInspection> }>();
const emit = defineEmits<{ refresh: []; applyDetected: []; applyManual: [endpoint: ProxyEndpoint]; disable: []; restore: []; copyEndpoint: []; toggleVariable: [name: string]; openAssistant: [] }>();

const clientIcons: Record<string, string> = { "clash-verge-rev": "/proxy-clients/clash-verge-rev.png", v2rayn: "/proxy-clients/v2rayn.png", flclash: "/proxy-clients/flclash.ico", hiddify: "/proxy-clients/hiddify.ico", "clash-nyanpasu": "/proxy-clients/clash-nyanpasu.png", "generic-proxy": "/proxy-clients/generic-proxy.svg" };
const sourceMode = ref<"automatic" | "manual">("automatic");
const manualHost = ref("127.0.0.1");
const manualPort = ref("7897");
const manualProtocol = ref<Exclude<ProxyProtocol, "unknown">>("mixed");
const pendingManualEndpoint = ref<ProxyEndpoint>();
const confirmationDialog = ref<HTMLDialogElement>();
const manualAttempted = ref(false);
const inspectingManual = ref(false);
const inspectionWarning = ref<"notListening" | "unknownProtocol" | "protocolMismatch" | "inspectionFailed">();
const activeCount = computed(() => props.environment.entries.filter((entry) => entry.exists).length);
const genericProxyIcon = clientIcons["generic-proxy"];
const detectedIcon = computed(() => candidateIcon(props.detected));
const tunSuspected = computed(() => !props.detected?.listening && (props.tun.state === "possible" || props.tun.state === "detected"));
const secondaryCandidates = computed(() => tunSuspected.value
  ? props.candidates
  : props.candidates.filter((candidate) => candidate.id !== props.detected?.id));
const activeCandidateCount = computed(() => props.candidates.filter((candidate) => candidate.listening).length);
const observedCandidateCount = computed(() => props.candidates.length + (tunSuspected.value ? 1 : 0));
const automaticDetectionLabel = computed(() => {
  if (props.loading && observedCandidateCount.value === 0) return props.copy.autoDetected;
  return props.copy.autoDetectedCount
    .replace("{active}", String(activeCandidateCount.value))
    .replace("{total}", String(observedCandidateCount.value));
});
const stateLabel = computed(() => ({ disabled: props.copy.environmentDisabled, partial: props.copy.environmentPartial, enabled: props.copy.environmentEnabled, mismatch: props.copy.environmentMismatch })[props.environment.state]);
const stateHint = computed(() => ({ disabled: props.copy.environmentOffHint, partial: props.copy.partialHint, enabled: props.copy.environmentOnHint, mismatch: props.copy.mismatchHint })[props.environment.state]);
const canApplyAutomatic = computed(() => Boolean(props.detected?.listening));
const manualHostValid = computed(() => {
  const host = manualHost.value.trim();
  return host.length > 0 && !/\s|\/|\\/.test(host) && !host.includes("://");
});
const manualPortValid = computed(() => { const port = Number(manualPort.value); return Number.isInteger(port) && port >= 1 && port <= 65535; });
const manualValid = computed(() => manualHostValid.value && manualPortValid.value);
const inspectionWarningText = computed(() => inspectionWarning.value ? ({
  notListening: props.copy.manualNotListening,
  unknownProtocol: props.copy.manualProtocolUnknown,
  protocolMismatch: props.copy.manualProtocolMismatch,
  inspectionFailed: props.copy.manualInspectionFailed
})[inspectionWarning.value] : "");

function candidateIcon(candidate?: ProxyCandidate): string {
  return candidate ? clientIcons[candidate.iconKey ?? ""] ?? genericProxyIcon : genericProxyIcon;
}

function candidateEndpoint(candidate: ProxyCandidate): string {
  return `${candidate.host}:${candidate.port}`;
}

function protocolsCompatible(selected: ProxyProtocol, detected: ProxyProtocol): boolean {
  return selected === detected
    || detected === "mixed" && (selected === "http" || selected === "socks5");
}

function matchesDetectedEndpoint(endpoint: ProxyEndpoint): boolean {
  if (!props.detected?.listening) return false;
  const host = endpoint.host.toLowerCase();
  const detectedHost = props.detected.host.toLowerCase();
  const sameHost = host === detectedHost
    || [host, detectedHost].every((value) => value === "localhost" || value === "127.0.0.1" || value === "::1");
  return sameHost && endpoint.port === props.detected.port;
}

async function requestManualApply() {
  manualAttempted.value = true;
  if (!manualValid.value) return;
  pendingManualEndpoint.value = { host: manualHost.value.trim(), port: Number(manualPort.value), protocol: manualProtocol.value };
  inspectionWarning.value = undefined;
  inspectingManual.value = true;
  try {
    if (matchesDetectedEndpoint(pendingManualEndpoint.value)) {
      if (!protocolsCompatible(pendingManualEndpoint.value.protocol, props.detected!.protocol)) inspectionWarning.value = "protocolMismatch";
    } else {
      const inspection = await props.inspectManualEndpoint(pendingManualEndpoint.value);
      if (!inspection.listening) inspectionWarning.value = "notListening";
      else if (inspection.detectedProtocol === "unknown") inspectionWarning.value = "unknownProtocol";
      else if (!inspection.protocolMatches) inspectionWarning.value = "protocolMismatch";
    }
  } catch {
    inspectionWarning.value = "inspectionFailed";
  } finally {
    inspectingManual.value = false;
  }
  await nextTick();
  confirmationDialog.value?.showModal();
}
function cancelManualApply() { confirmationDialog.value?.close(); pendingManualEndpoint.value = undefined; inspectionWarning.value = undefined; }
function confirmManualApply() { if (pendingManualEndpoint.value) emit("applyManual", pendingManualEndpoint.value); cancelManualApply(); }
function managedVariableKey(name: string): ManagedProxyVariable | undefined { const normalized = name.toUpperCase(); if (normalized === "HTTP_PROXY") return "http"; if (normalized === "HTTPS_PROXY") return "https"; if (normalized === "ALL_PROXY") return "all"; return undefined; }
function variableDescription(name: string): string { const normalized = name.toUpperCase(); if (normalized === "HTTP_PROXY") return props.copy.httpProxyDescription; if (normalized === "HTTPS_PROXY") return props.copy.httpsProxyDescription; if (normalized === "ALL_PROXY") return props.copy.allProxyDescription; return props.copy.noProxyDescription; }
function variableActionLabel(name: string, action: "write" | "about"): string { return (action === "write" ? props.copy.writeVariable : props.copy.aboutVariable).replace("{name}", name); }
function isManagedVariableSelected(name: string): boolean { const key = managedVariableKey(name); return key !== undefined && props.selectedVariables.includes(key); }
function isLastManagedVariable(name: string): boolean { return isManagedVariableSelected(name) && props.selectedVariables.length === 1; }
</script>

<template>
  <main class="page home-page">
    <div v-if="error" class="notice notice-error" role="alert"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 8v5m0 3.5v.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg><p><strong>{{ copy.operationFailed }}</strong><span>{{ error }} · {{ copy.retryHint }}</span></p></div>

    <section class="proxy-console">
      <div class="layer-row client-layer">
        <div class="layer-heading"><h2>{{ copy.proxyClient }}</h2><span>{{ automaticDetectionLabel }}</span></div>
        <div v-if="detected && !tunSuspected" class="client-heading">
          <span class="client-art"><img :src="detectedIcon" alt="" /></span>
          <div><h1>{{ detected.clientName || copy.localProxy }}</h1><p><span class="status-dot" :class="{ quiet: !detected.listening }"></span>{{ detected.listening ? copy.listening : copy.notListening }}</p></div>
          <div class="endpoint-line"><div class="endpoint-address"><code>{{ endpoint }}</code><button type="button" :aria-label="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" :title="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" @click="emit('copyEndpoint')"><svg v-if="!copiedEndpoint" viewBox="0 0 20 20" aria-hidden="true"><rect x="6.5" y="6.5" width="9" height="9" rx="1.6"/><path d="M13.5 6.5V5A1.5 1.5 0 0 0 12 3.5H5A1.5 1.5 0 0 0 3.5 5v7A1.5 1.5 0 0 0 5 13.5h1.5"/></svg><svg v-else viewBox="0 0 20 20" aria-hidden="true"><path d="m4.5 10.2 3.2 3.2 7.8-7.8"/></svg></button></div><span>{{ detected.protocol }} · {{ detected.confidence }} {{ copy.autoConfidence }}</span></div>
        </div>
        <div v-else-if="tunSuspected" class="client-heading suspected-client">
          <span class="client-art generic"><img :src="genericProxyIcon" alt="" /></span>
          <div><h1>{{ copy.suspectedTunProxy }}</h1><p><span class="status-dot warning"></span>{{ copy.suspectedTunProxyState }}</p></div>
          <div class="endpoint-line suspected-endpoint"><code>{{ tun.interfaceName || copy.assistantTun }}</code><span>{{ copy.suspectedTunProxyHint }}</span></div>
        </div>
        <div v-else class="empty-state"><span class="client-art generic"><img :src="detectedIcon" alt="" /></span><div><h1>{{ loading ? copy.detecting : copy.noProxy }}</h1><p>{{ copy.noProxyHint }}</p></div></div>

        <div v-if="secondaryCandidates.length" class="proxy-candidate-results">
          <div class="candidate-results-heading"><strong>{{ copy.otherProxyCandidates }}</strong><span>{{ secondaryCandidates.length }}</span></div>
          <div class="candidate-results-list">
            <div v-for="candidate in secondaryCandidates" :key="candidate.id" class="proxy-candidate-row">
              <span class="candidate-icon"><img :src="candidateIcon(candidate)" alt="" /></span>
              <div class="candidate-identity"><strong>{{ candidate.clientName || copy.localProxy }}</strong><small>{{ candidate.processName || candidate.protocol }}</small></div>
              <code>{{ candidateEndpoint(candidate) }}</code>
              <span class="candidate-state" :class="{ active: candidate.listening }">{{ candidate.listening ? copy.listening : copy.notListening }}</span>
            </div>
          </div>
        </div>
      </div>

      <NetworkObservationPanel :copy="copy" :system-proxy="systemProxy" :tun="tun" :loading="loading" context="console" />

      <div class="environment-layer" :class="`state-${environment.state}`">
        <div class="environment-summary"><div><h2>{{ copy.proxyEnvironment }}</h2><p>{{ copy.environmentLayerHint }}</p></div><div class="environment-state"><span>{{ stateLabel }}</span><p>{{ stateHint }}</p></div></div>
        <div class="source-selector" role="group" :aria-label="copy.proxySource">
          <button type="button" :class="{ active: sourceMode === 'automatic' }" @click="sourceMode = 'automatic'"><span></span><strong>{{ copy.autoDetect }}</strong><small>{{ detected ? endpoint : copy.noProxy }}</small></button>
          <button type="button" :class="{ active: sourceMode === 'manual' }" @click="sourceMode = 'manual'"><span></span><strong>{{ copy.manualProxy }}</strong><small>{{ copy.manualProxyHint }}</small></button>
        </div>
        <form v-if="sourceMode === 'manual'" class="manual-endpoint" @submit.prevent="requestManualApply">
          <label><span class="field-label"><span>{{ copy.host }}</span><HelpTooltip :label="copy.aboutHost" :text="copy.hostDescription" /></span><input v-model="manualHost" autocomplete="off" spellcheck="false" placeholder="127.0.0.1" :aria-invalid="manualAttempted && !manualHostValid" aria-describedby="manual-host-error" /><small v-if="manualAttempted && !manualHostValid" id="manual-host-error" class="field-error">{{ copy.invalidHost }}</small></label>
          <label><span class="field-label"><span>{{ copy.port }}</span><HelpTooltip :label="copy.aboutPort" :text="copy.portDescription" /></span><input v-model="manualPort" inputmode="numeric" autocomplete="off" placeholder="7897" :aria-invalid="manualAttempted && !manualPortValid" aria-describedby="manual-port-error" /><small v-if="manualAttempted && !manualPortValid" id="manual-port-error" class="field-error">{{ copy.invalidPort }}</small></label>
          <label><span class="field-label"><span>{{ copy.protocol }}</span><HelpTooltip :label="copy.aboutProtocol" :text="copy.protocolDescription" /></span><select v-model="manualProtocol"><option value="http">HTTP</option><option value="socks5">SOCKS5</option><option value="mixed">Mixed</option></select></label>
          <button class="primary-action" type="submit" :disabled="toggling || inspectingManual">{{ inspectingManual ? copy.checkingEndpoint : toggling ? copy.enabling : copy.applyManualProxy }}</button>
        </form>
        <div v-else class="environment-actions">
          <button v-if="environment.state === 'disabled' || environment.state === 'mismatch' || !environment.matchesActiveProxy" class="primary-action" type="button" :disabled="!canApplyAutomatic || toggling" @click="emit('applyDetected')">{{ toggling ? copy.enabling : environment.state === 'mismatch' ? copy.syncToActive : copy.applyDetectedProxy }}</button>
          <button v-if="environment.state !== 'disabled'" class="secondary-action danger-action" type="button" :disabled="toggling" @click="emit('disable')">{{ copy.disableProxyEnvironment }}</button>
          <button v-if="environment.snapshotAvailable" class="secondary-action" type="button" :disabled="toggling" @click="emit('restore')">{{ copy.restorePrevious }}</button>
        </div>
      </div>
    </section>

    <section class="assistant-entry">
      <span class="assistant-entry-icon" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M4 6.5h16v10H4zM8 20h8M12 16.5V20M8 10l2.2 2.2L14.8 8" /></svg></span>
      <div><h2>{{ copy.assistantEntryTitle }}</h2><p>{{ copy.assistantEntryHint }}</p></div>
      <button class="primary-action" type="button" @click="emit('openAssistant')">{{ copy.assistantEntryAction }}</button>
    </section>

    <section class="variables-section">
      <div class="section-header"><div><h2>{{ copy.variables }}</h2><p>{{ copy.variablesHint }}</p></div><button class="refresh-button" type="button" :disabled="loading" @click="emit('refresh')"><svg :class="{ spinning: loading }" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.34 5.66M20 5v6h-6" /></svg>{{ copy.refresh }}</button></div>
      <div v-if="environment.entries.length" class="variable-table">
        <div v-for="entry in environment.entries" :key="entry.name" class="variable-row">
          <span class="variable-indicator" :class="{ active: entry.exists }"></span>
          <div class="variable-label"><code class="variable-name">{{ entry.name }}</code><HelpTooltip :label="variableActionLabel(entry.name, 'about')" :text="variableDescription(entry.name)" /></div>
          <code class="variable-value" :class="{ unset: !entry.exists }">{{ entry.value ?? copy.unset }}</code>
          <label v-if="managedVariableKey(entry.name)" class="variable-choice" :class="{ locked: isLastManagedVariable(entry.name) }"><input type="checkbox" :checked="isManagedVariableSelected(entry.name)" :disabled="isLastManagedVariable(entry.name)" :aria-label="variableActionLabel(entry.name, 'write')" @change="emit('toggleVariable', entry.name)" /><span aria-hidden="true"><svg viewBox="0 0 16 16"><path d="m4 8.2 2.5 2.5L12 5.5"/></svg></span></label><span v-else class="variable-choice-spacer"></span>
        </div>
      </div>
      <div v-else class="skeleton-list" :aria-label="copy.detecting"><span v-for="item in 4" :key="item"></span></div>
      <p class="table-caption">{{ activeCount }} / {{ environment.entries.length }} · {{ copy.localOnly }}</p>
    </section>

    <dialog ref="confirmationDialog" class="confirmation-dialog" @cancel="cancelManualApply">
      <form method="dialog" @submit.prevent>
        <div class="confirmation-icon" :class="{ verified: !inspectionWarning }" aria-hidden="true"><svg v-if="inspectionWarning" viewBox="0 0 24 24"><path d="M12 8v5m0 3.5v.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg><svg v-else viewBox="0 0 24 24"><path d="m7 12.3 3.2 3.2L17.5 8M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg></div>
        <h2>{{ inspectionWarning ? copy.manualWarningTitle : copy.manualConfirmTitle }}</h2>
        <p>{{ inspectionWarning ? inspectionWarningText : copy.manualConfirmBody }}</p>
        <dl v-if="pendingManualEndpoint" class="confirmation-endpoint">
          <div><dt>{{ copy.host }}</dt><dd><code>{{ pendingManualEndpoint.host }}</code></dd></div>
          <div><dt>{{ copy.port }}</dt><dd><code>{{ pendingManualEndpoint.port }}</code></dd></div>
          <div><dt>{{ copy.protocol }}</dt><dd><code>{{ pendingManualEndpoint.protocol }}</code></dd></div>
          <div><dt>{{ copy.variables }}</dt><dd><code>{{ selectedVariables.join(', ') }}</code></dd></div>
        </dl>
        <p class="confirmation-consequence" :class="{ warning: inspectionWarning }">{{ inspectionWarning ? copy.manualOverrideConsequence : copy.manualConfirmConsequence }}</p>
        <div class="confirmation-actions">
          <button class="secondary-action" type="button" autofocus @click="cancelManualApply">{{ inspectionWarning ? copy.backToEdit : copy.cancel }}</button>
          <button class="primary-action" :class="{ 'warning-action': inspectionWarning }" type="button" @click="confirmManualApply">{{ inspectionWarning ? copy.applyAnyway : copy.confirmApply }}</button>
        </div>
      </form>
    </dialog>
  </main>
</template>
