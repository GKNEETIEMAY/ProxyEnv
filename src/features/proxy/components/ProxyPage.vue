<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import type { Copy } from "../../../shared/i18n";
import type { EnvironmentStatus, ManagedProxyVariable, ProxyCandidate, ProxyEndpoint, ProxyEndpointInspection, TunObservation } from "../../../shared/types";
import type { ConnectivityView } from "../connectivity";
import { candidateEndpoint, candidateIcon, candidateUsable, groupProxyCandidates, supportsProxyVariable, useGenericIcon } from "../client-presentation";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
import StatusIndicator from "../../../shared/components/StatusIndicator.vue";
import NetworkObservationPanel from "../../network-observation/components/NetworkObservationPanel.vue";
import LocalConnectivityCard from "./LocalConnectivityCard.vue";
import LocalProxyManual from "./LocalProxyManual.vue";
import LocalEnvironmentVariables from "./LocalEnvironmentVariables.vue";

const props = defineProps<{ copy: Copy; environment: EnvironmentStatus; candidates: ProxyCandidate[]; detected?: ProxyCandidate; systemProxy?: ProxyCandidate; tun: TunObservation; error: string; loading: boolean; stale: boolean; toggling: boolean; copiedEndpoint: boolean; selectedVariables: ManagedProxyVariable[]; inspectManualEndpoint: (endpoint: ProxyEndpoint) => Promise<ProxyEndpointInspection>; connectivity: ConnectivityView; now: number }>();
const emit = defineEmits<{ refresh: []; applyDetected: []; selectActive: [candidateId: string]; applyManual: [endpoint: ProxyEndpoint]; disable: []; restore: []; copyEndpoint: [candidate: ProxyCandidate]; toggleVariable: [name: string]; openAssistant: []; testConnectivity: [] }>();
const localView = ref<"overview" | "advanced">("overview");
const content = ref<HTMLElement>();
const active = computed(() => props.environment.activeProxy);
const clientGroups = computed(() => groupProxyCandidates(props.candidates, active.value.selectedCandidateId));
const recommended = computed(() => props.candidates.find(candidateUsable));
const source = computed(() => ({ auto: props.copy.activeProxyAuto, user: props.copy.activeProxyUser, systemProxy: props.copy.activeProxySystem, manual: props.copy.manualProxy })[active.value.selectionSource]);
const stateLabel = computed(() => ({ disabled: props.copy.environmentDisabled, partial: props.copy.environmentPartial, enabled: props.copy.environmentEnabled, mismatch: props.copy.environmentMismatch })[props.environment.state]);
const stateHint = computed(() => ({ disabled: props.copy.environmentOffHint, partial: props.copy.partialHint, enabled: props.copy.environmentOnHint, mismatch: props.copy.mismatchHint })[props.environment.state]);
const scope = computed(() => (["http", "https", "all"] as const).filter(name => props.selectedVariables.includes(name)).map(name => `${name.toUpperCase()}_PROXY`));
const scopeSupported = computed(() => props.selectedVariables.some(name => supportsProxyVariable(active.value.candidate?.protocol ?? "unknown", name)));
const managedCount = computed(() => props.environment.entries.filter(entry => entry.exists && /^(HTTP|HTTPS|ALL)_PROXY$/i.test(entry.name)).length);
const detectionSources = computed(() => ({ windowsSystemProxy: props.copy.windowsSystemProxy, processListener: props.copy.localSourceProcess, protocolProbe: props.copy.localSourceProbe, clientConfig: props.copy.localSourceConfig, composite: props.copy.localSourceComposite }));
async function showAdvanced(target?: string) {
  localView.value = "advanced";
  await nextTick();
  const element = target ? content.value?.querySelector<HTMLElement>(target) : undefined;
  if (element) { element.scrollIntoView({ block: "start" }); element.querySelector<HTMLElement>("button:not(:disabled),input:not(:disabled)")?.focus({ preventScroll: true }); }
  else if (content.value) content.value.scrollTop = 0;
}
function showOverview() { localView.value = "overview"; if (content.value) content.value.scrollTop = 0; }
</script>

<template>
  <main class="page workspace-page home-page">
    <section class="workspace-panel local-workspace">
      <header class="local-workspace-heading">
        <h1>{{ copy.navLocal }}</h1>
        <div class="local-toolbar">
          <div class="local-view-switch" role="group" :aria-label="copy.localViews">
            <button type="button" :aria-pressed="localView === 'overview'" @click="showOverview">{{ copy.localOverview }}</button>
            <button type="button" :aria-pressed="localView === 'advanced'" @click="showAdvanced()">{{ copy.localAdvanced }}</button>
          </div>
          <button class="refresh-button" type="button" :disabled="loading || toggling" @click="emit('refresh')"><svg :class="{ spinning: loading }" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.34 5.66M20 5v6h-6" /></svg>{{ copy.refresh }}</button>
        </div>
      </header>
      <div ref="content" class="local-workspace-content" :class="{ 'is-overview': localView === 'overview' }" tabindex="0" :aria-label="copy.navLocal" role="region">
        <p v-if="stale" class="local-stale" role="status">{{ copy.localStale }}</p>
        <section class="local-active local-section">
          <div class="local-section-heading"><h2>{{ copy.currentActiveProxy }}</h2><div class="local-active-tools"><LocalConnectivityCard :copy="copy" :connectivity="connectivity" :now="now" :available="active.available" :busy="toggling" :detailed="false" inline @details="showAdvanced('.local-connectivity')" /><button class="local-text-button" type="button" @click="showAdvanced('.local-candidates')">{{ copy.localSwitch }}</button></div></div>
          <div class="local-active-identity">
            <span class="client-art"><img :src="candidateIcon(detected)" alt="" @error="useGenericIcon" /></span>
            <div class="local-active-name"><strong>{{ detected ? active.selectionSource === 'manual' ? copy.manualProxy : detected.clientName || detected.processName || copy.localProxy : loading ? copy.detecting : copy.noProxy }}</strong>
              <div class="local-active-status"><StatusIndicator :state="stale ? 'idle' : active.available ? 'healthy' : detected ? 'warning' : 'idle'" :label="active.available ? copy.listening : detected ? copy.localUnavailable : copy.notListening" /><span v-if="detected">{{ source }}</span><HelpTooltip :label="copy.currentActiveProxy" :text="copy.activeProxySharedHint" /></div>
            </div>
            <div v-if="detected" class="local-active-address">
              <code :title="candidateEndpoint(detected)">{{ candidateEndpoint(detected) }}</code><span>{{ detected.protocol }}</span>
              <button class="local-copy-button" type="button" :aria-label="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" :title="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" @click="emit('copyEndpoint', detected)"><svg viewBox="0 0 20 20" aria-hidden="true"><path v-if="copiedEndpoint" d="m4.5 10.2 3.2 3.2 7.8-7.8"/><template v-else><rect x="6.5" y="6.5" width="9" height="9" rx="1.6"/><path d="M13.5 6.5V5A1.5 1.5 0 0 0 12 3.5H5A1.5 1.5 0 0 0 3.5 5v7A1.5 1.5 0 0 0 5 13.5h1.5"/></template></svg></button>
            </div>
          </div>
          <p v-if="!active.available" class="local-caption">{{ detected ? copy.localUnavailableHint : copy.noProxyHint }} <button class="local-text-button" type="button" @click="showAdvanced('.local-manual')">{{ copy.localManual }}</button></p>
        </section>

        <NetworkObservationPanel :copy="copy" :system-proxy="systemProxy" :tun="tun" :loading="loading" :compact="localView === 'overview'" context="console" />
        <LocalConnectivityCard v-if="localView === 'advanced'" :copy="copy" :connectivity="connectivity" :now="now" :available="active.available" :busy="toggling" detailed @test="emit('testConnectivity')" />

        <section v-show="localView === 'advanced'" class="local-candidates local-section">
          <header class="local-section-heading"><div class="local-title-help"><h2>{{ copy.localCandidates }} <span class="local-caption">{{ clientGroups.length }}</span></h2><HelpTooltip :label="copy.localRecommendationTitle" :text="`${copy.localRecommendationHint} ${copy.localSelectionHint}`" pinnable /></div><button class="local-text-button" type="button" @click="showAdvanced('.local-manual')">{{ copy.localManual }}</button></header>
          <ul v-if="candidates.length" class="local-candidate-list">
            <li v-for="group in clientGroups" :key="group.key">
              <button class="local-client-choice" type="button" :aria-pressed="group.candidate.id === active.selectedCandidateId" :disabled="toggling || !candidateUsable(group.candidate)" @click="emit('selectActive', group.candidate.id)">
                <img :src="candidateIcon(group.candidate)" alt="" @error="useGenericIcon" />
                <span class="local-candidate-copy"><strong>{{ group.candidate.clientName || group.candidate.processName || copy.localProxy }}</strong><code>{{ candidateEndpoint(group.candidate) }} · {{ group.candidate.protocol }}</code><small>{{ group.candidate.source.map(item => detectionSources[item]).join(' · ') }}</small></span>
                <span class="local-candidate-state"><small v-if="group.candidate.id === recommended?.id">{{ copy.localRecommended }}</small><StatusIndicator :state="group.candidate.id === active.selectedCandidateId ? active.available ? 'healthy' : 'warning' : 'idle'" :label="group.candidate.id === active.selectedCandidateId ? copy.localSelected : candidateUsable(group.candidate) ? copy.localProtocolRecognized : group.candidate.listening ? copy.localUnconfirmed : copy.notListening" /></span>
              </button>
              <details v-if="group.endpoints.length > 1" class="local-endpoint-details">
                <summary>{{ copy.localEndpointCount.replace('{count}', String(group.endpoints.length)) }}</summary>
                <button v-for="endpoint in group.endpoints" :key="endpoint.id" type="button" :aria-pressed="endpoint.id === active.selectedCandidateId" :disabled="toggling || !candidateUsable(endpoint)" @click="emit('selectActive', endpoint.id)"><code>{{ candidateEndpoint(endpoint) }} · {{ endpoint.protocol }}</code><span>{{ endpoint.id === active.selectedCandidateId ? copy.localSelected : endpoint.id === recommended?.id ? copy.localRecommended : candidateUsable(endpoint) ? copy.localProtocolRecognized : endpoint.listening ? copy.localUnconfirmed : copy.notListening }}</span></button>
              </details>
            </li>
          </ul>
          <p v-else class="local-caption">{{ copy.noProxyHint }}</p>
        </section>

        <section class="local-environment local-section" :class="`state-${environment.state}`">
          <header class="local-environment-heading">
            <div class="local-title-help"><h2>{{ copy.proxyEnvironment }}</h2><HelpTooltip :label="copy.proxyEnvironment" :text="`${copy.localActualValuesHint} ${stateHint} ${copy.localSyncScope}: ${scope.join(' · ')}. ${copy.localSyncEffect}`" pinnable /></div>
            <div class="local-environment-tools"><StatusIndicator :state="stale || environment.state === 'disabled' ? 'idle' : environment.state === 'enabled' ? 'healthy' : 'warning'" :label="stateLabel" /><button v-if="localView === 'overview'" class="local-text-button local-scope-adjust" type="button" :title="scope.join(' · ')" @click="showAdvanced('.variables-section')">{{ copy.localScopeAction }}<svg viewBox="0 0 20 20" aria-hidden="true"><path d="m8 5 5 5-5 5" /></svg></button></div>
          </header>
          <p v-if="environment.state === 'partial' || environment.state === 'mismatch'" class="local-scope-warning">{{ stateHint }}</p>
          <LocalEnvironmentVariables :copy="copy" :environment="environment" :selected-variables="selectedVariables" :toggling="toggling" :editable="localView === 'advanced'" @toggle-variable="emit('toggleVariable', $event)" />
          <p v-if="active.available && !scopeSupported" class="local-scope-warning" role="status">{{ copy.localScopeUnsupported }}</p>
          <div class="environment-actions">
            <button class="primary-action local-sync-action" type="button" :disabled="!active.available || !scopeSupported || toggling" @click="emit('applyDetected')">{{ toggling ? copy.enabling : copy.syncActiveProxy }}</button>
            <button class="secondary-action danger-action" type="button" :disabled="toggling || managedCount === 0" @click="emit('disable')">{{ copy.disableProxyEnvironment }}</button>
            <button class="secondary-action" type="button" :disabled="toggling || !environment.snapshotAvailable" @click="emit('restore')">{{ copy.restorePrevious }}</button>
          </div>
        </section>
        <LocalProxyManual v-show="localView === 'advanced'" :copy="copy" :toggling="toggling" :selected-variables="selectedVariables" :inspect-manual-endpoint="inspectManualEndpoint" @apply-manual="emit('applyManual', $event)" />
        <section class="assistant-entry">
          <span class="assistant-entry-icon" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M4 6.5h16v10H4zM8 20h8M12 16.5V20M8 10l2.2 2.2L14.8 8" /></svg></span>
          <div><h2>{{ copy.assistantEntryTitle }}</h2><p>{{ copy.assistantEntryHint }}</p></div>
          <button class="secondary-action" type="button" @click="emit('openAssistant')">{{ copy.assistantEntryAction }}</button>
        </section>
      </div>
    </section>
  </main>
</template>

<style src="../local-workspace.css"></style>
