<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { Copy } from "../../../shared/i18n";
import type { EnvironmentStatus, ManagedProxyVariable, ProxyCandidate, ProxyEndpoint, ProxyProtocol } from "../../../shared/types";

const props = defineProps<{ copy: Copy; environment: EnvironmentStatus; detected?: ProxyCandidate; systemProxy?: ProxyCandidate; endpoint: string; error: string; loading: boolean; toggling: boolean; copiedEndpoint: boolean; selectedVariables: ManagedProxyVariable[] }>();
const emit = defineEmits<{ refresh: []; applyDetected: []; applyManual: [endpoint: ProxyEndpoint]; disable: []; restore: []; copyEndpoint: []; toggleVariable: [name: string] }>();

const clientIcons: Record<string, string> = { "clash-verge-rev": "/proxy-clients/clash-verge-rev.png", v2rayn: "/proxy-clients/v2rayn.png", flclash: "/proxy-clients/flclash.ico", hiddify: "/proxy-clients/hiddify.ico", "clash-nyanpasu": "/proxy-clients/clash-nyanpasu.png", "generic-proxy": "/proxy-clients/generic-proxy.svg" };
const sourceMode = ref<"automatic" | "manual">("automatic");
const manualHost = ref("127.0.0.1");
const manualPort = ref("7897");
const manualProtocol = ref<Exclude<ProxyProtocol, "unknown">>("mixed");
const activeCount = computed(() => props.environment.entries.filter((entry) => entry.exists).length);
const detectedIcon = computed(() => props.detected ? clientIcons[props.detected.iconKey ?? ""] ?? clientIcons["generic-proxy"] : clientIcons["generic-proxy"]);
const stateLabel = computed(() => ({ disabled: props.copy.environmentDisabled, partial: props.copy.environmentPartial, enabled: props.copy.environmentEnabled, mismatch: props.copy.environmentMismatch })[props.environment.state]);
const stateHint = computed(() => ({ disabled: props.copy.environmentOffHint, partial: props.copy.partialHint, enabled: props.copy.environmentOnHint, mismatch: props.copy.mismatchHint })[props.environment.state]);
const canApplyAutomatic = computed(() => Boolean(props.detected?.listening));
const manualPortValid = computed(() => { const port = Number(manualPort.value); return Number.isInteger(port) && port >= 1 && port <= 65535; });
const manualValid = computed(() => manualHost.value.trim().length > 0 && manualPortValid.value);

watch(() => props.detected, (candidate) => { if (!candidate) return; manualHost.value = candidate.host; manualPort.value = String(candidate.port); if (candidate.protocol !== "unknown") manualProtocol.value = candidate.protocol; }, { immediate: true });

function applyManual() { if (manualValid.value) emit("applyManual", { host: manualHost.value.trim(), port: Number(manualPort.value), protocol: manualProtocol.value }); }
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
        <div class="layer-heading"><h2>{{ copy.proxyClient }}</h2><span>{{ copy.autoDetected }}</span></div>
        <div v-if="detected" class="client-heading">
          <span class="client-art"><img :src="detectedIcon" alt="" /></span>
          <div><h1>{{ detected.clientName || copy.localProxy }}</h1><p><span class="status-dot" :class="{ quiet: !detected.listening }"></span>{{ detected.listening ? copy.listening : copy.notListening }}</p></div>
          <div class="endpoint-line"><div class="endpoint-address"><code>{{ endpoint }}</code><button type="button" :aria-label="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" :title="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" @click="emit('copyEndpoint')"><svg v-if="!copiedEndpoint" viewBox="0 0 20 20" aria-hidden="true"><rect x="6.5" y="6.5" width="9" height="9" rx="1.6"/><path d="M13.5 6.5V5A1.5 1.5 0 0 0 12 3.5H5A1.5 1.5 0 0 0 3.5 5v7A1.5 1.5 0 0 0 5 13.5h1.5"/></svg><svg v-else viewBox="0 0 20 20" aria-hidden="true"><path d="m4.5 10.2 3.2 3.2 7.8-7.8"/></svg></button></div><span>{{ detected.protocol }} · {{ detected.confidence }} {{ copy.autoConfidence }}</span></div>
        </div>
        <div v-else class="empty-state"><span class="client-art generic"><img :src="detectedIcon" alt="" /></span><div><h1>{{ loading ? copy.detecting : copy.noProxy }}</h1><p>{{ copy.noProxyHint }}</p></div></div>
      </div>

      <div class="layer-row system-layer">
        <div><h2>{{ copy.windowsSystemProxy }}</h2><p>{{ copy.systemProxyReadOnly }}</p></div>
        <div class="system-state" :class="{ active: systemProxy }"><span class="status-dot" :class="{ quiet: !systemProxy }"></span><div><strong>{{ systemProxy ? copy.systemProxyOn : copy.systemProxyOff }}</strong><code v-if="systemProxy">{{ systemProxy.host }}:{{ systemProxy.port }}</code></div></div>
      </div>

      <div class="environment-layer" :class="`state-${environment.state}`">
        <div class="environment-summary"><div><h2>{{ copy.proxyEnvironment }}</h2><p>{{ copy.environmentLayerHint }}</p></div><div class="environment-state"><span>{{ stateLabel }}</span><p>{{ stateHint }}</p></div></div>
        <div class="source-selector" role="group" :aria-label="copy.proxySource">
          <button type="button" :class="{ active: sourceMode === 'automatic' }" @click="sourceMode = 'automatic'"><span></span><strong>{{ copy.autoDetect }}</strong><small>{{ detected ? endpoint : copy.noProxy }}</small></button>
          <button type="button" :class="{ active: sourceMode === 'manual' }" @click="sourceMode = 'manual'"><span></span><strong>{{ copy.manualProxy }}</strong><small>{{ copy.manualProxyHint }}</small></button>
        </div>
        <form v-if="sourceMode === 'manual'" class="manual-endpoint" @submit.prevent="applyManual">
          <label><span>{{ copy.host }}</span><input v-model="manualHost" autocomplete="off" spellcheck="false" placeholder="127.0.0.1" /></label>
          <label><span>{{ copy.port }}</span><input v-model="manualPort" inputmode="numeric" autocomplete="off" placeholder="7897" :aria-invalid="!manualPortValid" /></label>
          <label><span>{{ copy.protocol }}</span><select v-model="manualProtocol"><option value="http">HTTP</option><option value="socks5">SOCKS5</option><option value="mixed">Mixed</option></select></label>
          <button class="primary-action" type="submit" :disabled="!manualValid || toggling">{{ toggling ? copy.enabling : copy.applyManualProxy }}</button>
        </form>
        <div v-else class="environment-actions">
          <button v-if="environment.state === 'disabled' || environment.state === 'mismatch' || !environment.matchesActiveProxy" class="primary-action" type="button" :disabled="!canApplyAutomatic || toggling" @click="emit('applyDetected')">{{ toggling ? copy.enabling : environment.state === 'mismatch' ? copy.syncToActive : copy.applyDetectedProxy }}</button>
          <button v-if="environment.state !== 'disabled'" class="secondary-action danger-action" type="button" :disabled="toggling" @click="emit('disable')">{{ copy.disableProxyEnvironment }}</button>
          <button v-if="environment.snapshotAvailable" class="secondary-action" type="button" :disabled="toggling" @click="emit('restore')">{{ copy.restorePrevious }}</button>
        </div>
      </div>
    </section>

    <section class="variables-section">
      <div class="section-header"><div><h2>{{ copy.variables }}</h2><p>{{ copy.variablesHint }}</p></div><button class="refresh-button" type="button" :disabled="loading" @click="emit('refresh')"><svg :class="{ spinning: loading }" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.34 5.66M20 5v6h-6" /></svg>{{ copy.refresh }}</button></div>
      <div v-if="environment.entries.length" class="variable-table">
        <div v-for="entry in environment.entries" :key="entry.name" class="variable-row">
          <span class="variable-indicator" :class="{ active: entry.exists }"></span>
          <div class="variable-label"><code class="variable-name">{{ entry.name }}</code><span class="variable-help" tabindex="0" :aria-label="variableActionLabel(entry.name, 'about')" :aria-describedby="`variable-help-${entry.name}`"><svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="7.5"/><path d="M7.9 7.8a2.25 2.25 0 0 1 4.3.9c0 1.55-2.2 1.75-2.2 3.2M10 14.6v.01"/></svg><p :id="`variable-help-${entry.name}`" role="tooltip">{{ variableDescription(entry.name) }}</p></span></div>
          <code class="variable-value" :class="{ unset: !entry.exists }">{{ entry.value ?? copy.unset }}</code>
          <label v-if="managedVariableKey(entry.name)" class="variable-choice" :class="{ locked: isLastManagedVariable(entry.name) }"><input type="checkbox" :checked="isManagedVariableSelected(entry.name)" :disabled="isLastManagedVariable(entry.name)" :aria-label="variableActionLabel(entry.name, 'write')" @change="emit('toggleVariable', entry.name)" /><span aria-hidden="true"><svg viewBox="0 0 16 16"><path d="m4 8.2 2.5 2.5L12 5.5"/></svg></span></label><span v-else class="variable-choice-spacer"></span>
        </div>
      </div>
      <div v-else class="skeleton-list" :aria-label="copy.detecting"><span v-for="item in 4" :key="item"></span></div>
      <p class="table-caption">{{ activeCount }} / {{ environment.entries.length }} · {{ copy.localOnly }}</p>
    </section>
  </main>
</template>
