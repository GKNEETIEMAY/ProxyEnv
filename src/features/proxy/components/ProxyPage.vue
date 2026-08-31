<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { Copy } from "../../../shared/i18n";
import type { EnvironmentStatus, ManagedProxyVariable, ProxyCandidate, ProxyEndpoint, ProxyEndpointInspection, ProxyProtocol, TunObservation } from "../../../shared/types";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
import NetworkObservationPanel from "../../network-observation/components/NetworkObservationPanel.vue";

const props = defineProps<{ copy: Copy; environment: EnvironmentStatus; candidates: ProxyCandidate[]; detected?: ProxyCandidate; systemProxy?: ProxyCandidate; tun: TunObservation; error: string; loading: boolean; toggling: boolean; copiedEndpoint: boolean; selectedVariables: ManagedProxyVariable[]; inspectManualEndpoint: (endpoint: ProxyEndpoint) => Promise<ProxyEndpointInspection> }>();
const emit = defineEmits<{ refresh: []; applyDetected: [candidate: ProxyCandidate]; applyManual: [endpoint: ProxyEndpoint]; disable: []; restore: []; copyEndpoint: [candidate: ProxyCandidate]; toggleVariable: [name: string]; openAssistant: [] }>();

const clientIcons: Record<string, string> = {
  "clash-verge-rev": "/proxy-clients/clash-verge-rev.png",
  v2rayn: "/proxy-clients/v2rayn.png",
  flclash: "/proxy-clients/flclash.ico",
  hiddify: "/proxy-clients/hiddify.ico",
  "clash-nyanpasu": "/proxy-clients/clash-nyanpasu.png",
  "mihomo-party": "/proxy-clients/mihomo-party.png",
  nekobox: "/proxy-clients/nekobox.png",
  "clash-for-windows": "/proxy-clients/clash-for-windows.png",
  "gui-for-clash": "/proxy-clients/gui-for-clash.png",
  "generic-proxy": "/proxy-clients/generic-proxy.svg"
};
const sourceMode = ref<"automatic" | "manual">("automatic");
const manualHost = ref("127.0.0.1");
const manualPort = ref("7897");
const manualProtocol = ref<Exclude<ProxyProtocol, "unknown">>("mixed");
const pendingManualEndpoint = ref<ProxyEndpoint>();
const confirmationDialog = ref<HTMLDialogElement>();
const manualAttempted = ref(false);
const inspectingManual = ref(false);
const inspectionWarning = ref<"notListening" | "unknownProtocol" | "protocolMismatch" | "inspectionFailed">();
const selectedClientKey = ref("");
const activeCount = computed(() => props.environment.entries.filter((entry) => entry.exists).length);
const genericProxyIcon = clientIcons["generic-proxy"];
const clientCandidates = computed(() => {
  const clients = new Map<string, ProxyCandidate>();
  for (const candidate of props.candidates) {
    const key = candidateClientKey(candidate);
    const current = clients.get(key);
    if (!current || candidate.id === props.detected?.id || candidate.listening && !current.listening) {
      clients.set(key, candidate);
    }
  }
  return [...clients.values()];
});
const displayedCandidate = computed(() => clientCandidates.value.find((candidate) => candidateClientKey(candidate) === selectedClientKey.value) ?? props.detected ?? clientCandidates.value[0]);
const displayedEndpoint = computed(() => displayedCandidate.value ? candidateEndpoint(displayedCandidate.value) : "");
const detectedIcon = computed(() => candidateIcon(displayedCandidate.value));
const tunSuspected = computed(() => !props.detected?.listening && (props.tun.state === "possible" || props.tun.state === "detected"));
const otherClients = computed(() => clientCandidates.value.filter((candidate) => candidateClientKey(candidate) !== selectedClientKey.value));
const currentClientIndex = computed(() => Math.max(0, clientCandidates.value.findIndex((candidate) => candidateClientKey(candidate) === selectedClientKey.value)));
const activeCandidateCount = computed(() => clientCandidates.value.filter((candidate) => candidate.listening).length);
const observedCandidateCount = computed(() => clientCandidates.value.length + (tunSuspected.value ? 1 : 0));
const automaticDetectionLabel = computed(() => {
  if (props.loading && observedCandidateCount.value === 0) return props.copy.autoDetected;
  return props.copy.autoDetectedCount
    .replace("{active}", String(activeCandidateCount.value))
    .replace("{total}", String(observedCandidateCount.value));
});
const environmentMatchesDisplayedCandidate = computed(() => {
  const candidate = displayedCandidate.value;
  if (!candidate) return props.environment.matchesActiveProxy;
  const selectedEntries = props.environment.entries.filter((entry) => {
    const key = managedVariableKey(entry.name);
    return key !== undefined && props.selectedVariables.includes(key);
  });
  return selectedEntries.length === props.selectedVariables.length
    && selectedEntries.every((entry) => entry.exists && entry.value && environmentValueMatchesCandidate(entry.value, candidate));
});
const effectiveEnvironmentState = computed(() => displayedCandidate.value
  && props.environment.state !== "disabled"
  && !environmentMatchesDisplayedCandidate.value
  ? "mismatch"
  : props.environment.state);
const stateLabel = computed(() => ({ disabled: props.copy.environmentDisabled, partial: props.copy.environmentPartial, enabled: props.copy.environmentEnabled, mismatch: props.copy.environmentMismatch })[effectiveEnvironmentState.value]);
const stateHint = computed(() => ({ disabled: props.copy.environmentOffHint, partial: props.copy.partialHint, enabled: props.copy.environmentOnHint, mismatch: props.copy.mismatchHint })[effectiveEnvironmentState.value]);
const canApplyAutomatic = computed(() => Boolean(displayedCandidate.value?.listening));
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

function candidateIcon(candidate?: ProxyCandidate): string {
  return candidate ? clientIcons[candidate.iconKey ?? ""] ?? genericProxyIcon : genericProxyIcon;
}

function useGenericIcon(event: Event) {
  const image = event.currentTarget as HTMLImageElement;
  if (!image.src.endsWith(genericProxyIcon)) image.src = genericProxyIcon;
}

function candidateClientKey(candidate: ProxyCandidate): string {
  if (candidate.pid !== undefined) return `pid:${candidate.pid}`;
  return `name:${(candidate.processName || candidate.clientName || candidate.id).toLocaleLowerCase()}`;
}

function candidateEndpoint(candidate: ProxyCandidate): string {
  return `${candidate.host}:${candidate.port}`;
}

function environmentValueMatchesCandidate(value: string, candidate: ProxyCandidate): boolean {
  try {
    const parsed = new URL(value);
    const host = parsed.hostname.replace(/^\[|\]$/g, "").toLocaleLowerCase();
    const candidateHost = candidate.host.replace(/^\[|\]$/g, "").toLocaleLowerCase();
    const sameHost = host === candidateHost
      || [host, candidateHost].every((item) => item === "localhost" || item === "127.0.0.1" || item === "::1");
    return sameHost && Number(parsed.port) === candidate.port;
  } catch {
    return false;
  }
}

function selectClient(candidate: ProxyCandidate) {
  selectedClientKey.value = candidateClientKey(candidate);
}

function showAdjacentClient(direction: -1 | 1) {
  const nextIndex = currentClientIndex.value + direction;
  const candidate = clientCandidates.value[nextIndex];
  if (candidate) selectClient(candidate);
}

watch([clientCandidates, () => props.detected?.id], ([clients]) => {
  if (clients.some((candidate) => candidateClientKey(candidate) === selectedClientKey.value)) return;
  const initial = clients.find((candidate) => candidate.id === props.detected?.id) ?? clients[0];
  selectedClientKey.value = initial ? candidateClientKey(initial) : "";
}, { immediate: true });

function protocolsCompatible(selected: ProxyProtocol, detected: ProxyProtocol): boolean {
  return selected === detected
    || detected === "mixed" && (selected === "http" || selected === "socks5");
}

function matchesDetectedEndpoint(endpoint: ProxyEndpoint): boolean {
  const candidate = displayedCandidate.value;
  if (!candidate?.listening) return false;
  const host = endpoint.host.toLowerCase();
  const detectedHost = candidate.host.toLowerCase();
  const sameHost = host === detectedHost
    || [host, detectedHost].every((value) => value === "localhost" || value === "127.0.0.1" || value === "::1");
  return sameHost && endpoint.port === candidate.port;
}

async function requestManualApply() {
  manualAttempted.value = true;
  if (!manualValid.value) return;
  pendingManualEndpoint.value = { host: manualHost.value.trim(), port: Number(manualPort.value), protocol: manualProtocol.value };
  inspectionWarning.value = undefined;
  inspectingManual.value = true;
  try {
    if (matchesDetectedEndpoint(pendingManualEndpoint.value)) {
      if (!protocolsCompatible(pendingManualEndpoint.value.protocol, displayedCandidate.value!.protocol)) inspectionWarning.value = "protocolMismatch";
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
        <div v-if="displayedCandidate && !tunSuspected" class="client-heading">
          <span class="client-art"><img :src="detectedIcon" alt="" @error="useGenericIcon" /></span>
          <div><h1>{{ displayedCandidate.clientName || copy.localProxy }}</h1><p><span class="status-dot" :class="{ quiet: !displayedCandidate.listening }"></span>{{ displayedCandidate.listening ? copy.listening : copy.notListening }}</p></div>
          <div class="endpoint-line"><div class="endpoint-address"><code>{{ displayedEndpoint }}</code><button type="button" :aria-label="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" :title="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" @click="emit('copyEndpoint', displayedCandidate)"><svg v-if="!copiedEndpoint" viewBox="0 0 20 20" aria-hidden="true"><rect x="6.5" y="6.5" width="9" height="9" rx="1.6"/><path d="M13.5 6.5V5A1.5 1.5 0 0 0 12 3.5H5A1.5 1.5 0 0 0 3.5 5v7A1.5 1.5 0 0 0 5 13.5h1.5"/></svg><svg v-else viewBox="0 0 20 20" aria-hidden="true"><path d="m4.5 10.2 3.2 3.2 7.8-7.8"/></svg></button></div><span>{{ displayedCandidate.protocol }} · {{ displayedCandidate.confidence }} {{ copy.autoConfidence }}</span></div>
        </div>
        <div v-else-if="tunSuspected" class="client-heading suspected-client">
          <span class="client-art generic"><img :src="genericProxyIcon" alt="" @error="useGenericIcon" /></span>
          <div><h1>{{ copy.suspectedTunProxy }}</h1><p><span class="status-dot warning"></span>{{ copy.suspectedTunProxyState }}</p></div>
          <div class="endpoint-line suspected-endpoint"><code>{{ tun.interfaceName || copy.assistantTun }}</code><span>{{ copy.suspectedTunProxyHint }}</span></div>
        </div>
        <div v-else class="empty-state"><span class="client-art generic"><img :src="detectedIcon" alt="" @error="useGenericIcon" /></span><div><h1>{{ loading ? copy.detecting : copy.noProxy }}</h1><p>{{ copy.noProxyHint }}</p></div></div>

        <div v-if="clientCandidates.length > 1 && !tunSuspected" class="proxy-client-pager">
          <div class="candidate-results-heading"><strong>{{ copy.otherProxyClients }}</strong><span>{{ otherClients.length }}</span></div>
          <div class="other-client-list">
            <button v-for="candidate in otherClients" :key="candidateClientKey(candidate)" type="button" @click="selectClient(candidate)">
              <span class="candidate-icon"><img :src="candidateIcon(candidate)" alt="" @error="useGenericIcon" /></span>
              <span>{{ candidate.clientName || candidate.processName || copy.localProxy }}</span>
            </button>
          </div>
          <div class="client-page-controls">
            <button type="button" :disabled="currentClientIndex === 0" :aria-label="copy.previousProxyClient" :title="copy.previousProxyClient" @click="showAdjacentClient(-1)"><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m12.5 5-5 5 5 5" /></svg></button>
            <span>{{ currentClientIndex + 1 }} / {{ clientCandidates.length }}</span>
            <button type="button" :disabled="currentClientIndex === clientCandidates.length - 1" :aria-label="copy.nextProxyClient" :title="copy.nextProxyClient" @click="showAdjacentClient(1)"><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m7.5 5 5 5-5 5" /></svg></button>
          </div>
        </div>
      </div>

      <NetworkObservationPanel :copy="copy" :system-proxy="systemProxy" :tun="tun" :loading="loading" context="console" />

      <div class="environment-layer" :class="`state-${effectiveEnvironmentState}`">
        <div class="environment-summary"><div><h2>{{ copy.proxyEnvironment }}</h2><p>{{ copy.environmentLayerHint }}</p></div><div class="environment-state"><span>{{ stateLabel }}</span><p>{{ stateHint }}</p></div></div>
        <div class="source-selector" role="group" :aria-label="copy.proxySource">
          <button type="button" :class="{ active: sourceMode === 'automatic' }" @click="sourceMode = 'automatic'"><span></span><strong>{{ copy.autoDetect }}</strong><small>{{ displayedCandidate ? displayedEndpoint : copy.noProxy }}</small></button>
          <button type="button" :class="{ active: sourceMode === 'manual' }" @click="sourceMode = 'manual'"><span></span><strong>{{ copy.manualProxy }}</strong><small>{{ copy.manualProxyHint }}</small></button>
        </div>
        <form v-if="sourceMode === 'manual'" class="manual-endpoint" @submit.prevent="requestManualApply">
          <label><span class="field-label"><span>{{ copy.host }}</span><HelpTooltip :label="copy.aboutHost" :text="copy.hostDescription" /></span><input v-model="manualHost" autocomplete="off" spellcheck="false" placeholder="127.0.0.1" :aria-invalid="manualAttempted && !manualHostValid" aria-describedby="manual-host-error" /><small v-if="manualAttempted && !manualHostValid" id="manual-host-error" class="field-error">{{ copy.invalidHost }}</small></label>
          <label><span class="field-label"><span>{{ copy.port }}</span><HelpTooltip :label="copy.aboutPort" :text="copy.portDescription" /></span><input v-model="manualPort" inputmode="numeric" autocomplete="off" placeholder="7897" :aria-invalid="manualAttempted && !manualPortValid" aria-describedby="manual-port-error" /><small v-if="manualAttempted && !manualPortValid" id="manual-port-error" class="field-error">{{ copy.invalidPort }}</small></label>
          <label><span class="field-label"><span>{{ copy.protocol }}</span><HelpTooltip :label="copy.aboutProtocol" :text="copy.protocolDescription" /></span><select v-model="manualProtocol"><option value="http">HTTP</option><option value="socks5">SOCKS5</option><option value="mixed">Mixed</option></select></label>
          <button class="primary-action" type="submit" :disabled="toggling || inspectingManual">{{ inspectingManual ? copy.checkingEndpoint : toggling ? copy.enabling : copy.applyManualProxy }}</button>
        </form>
        <div v-else class="environment-actions">
          <button v-if="effectiveEnvironmentState === 'disabled' || effectiveEnvironmentState === 'mismatch' || !environmentMatchesDisplayedCandidate" class="primary-action" type="button" :disabled="!canApplyAutomatic || toggling" @click="displayedCandidate && emit('applyDetected', displayedCandidate)">{{ toggling ? copy.enabling : effectiveEnvironmentState === 'mismatch' ? copy.syncToSelectedProxy : copy.applyDetectedProxy }}</button>
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
