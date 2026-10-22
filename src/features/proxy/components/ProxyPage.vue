<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import type { Copy } from "../../../shared/i18n";
import type { EnvironmentStatus, ManagedProxyVariable, ProxyCandidate, ProxyEndpoint, ProxyEndpointInspection, TunObservation } from "../../../shared/types";
import type { ConnectivityView } from "../connectivity";
import { candidateEndpoint, candidateIcon, useGenericIcon } from "../client-presentation";
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
const source = computed(() => ({ auto: props.copy.activeProxyAuto, user: props.copy.activeProxyUser, systemProxy: props.copy.activeProxySystem, manual: props.copy.manualProxy })[active.value.selectionSource]);
const stateLabel = computed(() => ({ disabled: props.copy.environmentDisabled, partial: props.copy.environmentPartial, enabled: props.copy.environmentEnabled, mismatch: props.copy.environmentMismatch })[props.environment.state]);
const stateHint = computed(() => ({ disabled: props.copy.environmentOffHint, partial: props.copy.partialHint, enabled: props.copy.environmentOnHint, mismatch: props.copy.mismatchHint })[props.environment.state]);
const scope = computed(() => (["http", "https", "all"] as const).filter(name => props.selectedVariables.includes(name)).map(name => `${name.toUpperCase()}_PROXY`));
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
          <div class="local-section-heading"><h2>{{ copy.currentActiveProxy }}</h2><button class="local-text-button" type="button" @click="showAdvanced('.local-candidates')">{{ copy.localSwitch }}</button></div>
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

        <section v-show="localView === 'advanced'" class="local-candidates local-section">
          <header class="local-section-heading"><h2>{{ copy.localCandidates }} <span class="local-caption">{{ candidates.length }}</span></h2><button class="local-text-button" type="button" @click="showAdvanced('.local-manual')">{{ copy.localManual }}</button></header>
          <ul v-if="candidates.length" class="local-candidate-list">
            <li v-for="candidate in candidates" :key="candidate.id">
              <button type="button" :aria-pressed="candidate.id === active.selectedCandidateId" :disabled="toggling || !candidate.listening || candidate.protocol === 'unknown'" @click="emit('selectActive', candidate.id)">
                <img :src="candidateIcon(candidate)" alt="" @error="useGenericIcon" />
                <span class="local-candidate-copy"><strong>{{ candidate.clientName || candidate.processName || copy.localProxy }}</strong><code>{{ candidateEndpoint(candidate) }} · {{ candidate.protocol }}</code><small>{{ candidate.source.map(item => detectionSources[item]).join(' · ') }}</small></span>
                <StatusIndicator :state="candidate.id === active.selectedCandidateId ? active.available ? 'healthy' : 'warning' : 'idle'" :label="candidate.id === active.selectedCandidateId ? copy.localSelected : candidate.listening ? copy.listening : copy.notListening" />
              </button>
            </li>
          </ul>
          <p v-else class="local-caption">{{ copy.noProxyHint }}</p>
        </section>

        <section class="local-environment local-section" :class="`state-${environment.state}`">
          <header class="environment-summary">
            <div><h2>{{ copy.proxyEnvironment }}</h2><p>{{ copy.environmentLayerHint }}</p></div>
            <div class="environment-state" :class="{ 'is-stale': stale }" role="status"><span>{{ stateLabel }}</span><p>{{ stateHint }}</p></div>
          </header>
          <div class="local-sync-scope">
            <span class="local-scope-label">{{ copy.localSyncScope }}<HelpTooltip :label="copy.localSyncScope" :text="copy.localSyncEffect" /></span>
            <div class="local-scope-values"><code v-for="name in scope" :key="name">{{ name }}</code></div>
            <button class="local-text-button local-scope-adjust" type="button" @click="showAdvanced('.variables-section')">{{ copy.localAdjust }}<svg viewBox="0 0 20 20" aria-hidden="true"><path d="m8 5 5 5-5 5" /></svg></button>
          </div>
          <div class="environment-actions">
            <button class="primary-action local-sync-action" type="button" :disabled="!active.available || toggling" @click="emit('applyDetected')">{{ toggling ? copy.enabling : copy.syncActiveProxy }}</button>
            <button class="secondary-action danger-action" type="button" :disabled="toggling || managedCount === 0" @click="emit('disable')">{{ copy.disableProxyEnvironment }}</button>
            <button class="secondary-action" type="button" :disabled="toggling || !environment.snapshotAvailable" @click="emit('restore')">{{ copy.restorePrevious }}</button>
          </div>
          <p class="local-sync-note">{{ copy.localSyncClearHint }}</p>
        </section>
        <LocalProxyManual v-show="localView === 'advanced'" :copy="copy" :toggling="toggling" :selected-variables="selectedVariables" :inspect-manual-endpoint="inspectManualEndpoint" @apply-manual="emit('applyManual', $event)" />
        <LocalEnvironmentVariables v-show="localView === 'advanced'" :copy="copy" :environment="environment" :selected-variables="selectedVariables" :toggling="toggling" @toggle-variable="emit('toggleVariable', $event)" />
        <NetworkObservationPanel :copy="copy" :system-proxy="systemProxy" :tun="tun" :loading="loading" :compact="localView === 'overview'" context="console" />
        <LocalConnectivityCard :copy="copy" :connectivity="connectivity" :now="now" :available="active.available" :busy="toggling" :detailed="localView === 'advanced'" @test="emit('testConnectivity')" @details="showAdvanced('.local-connectivity')" />
        <section class="assistant-entry">
          <span class="assistant-entry-icon" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M4 6.5h16v10H4zM8 20h8M12 16.5V20M8 10l2.2 2.2L14.8 8" /></svg></span>
          <div><h2>{{ copy.assistantEntryTitle }}</h2><p>{{ copy.assistantEntryHint }}</p></div>
          <button class="primary-action" type="button" @click="emit('openAssistant')">{{ copy.assistantEntryAction }}</button>
        </section>
      </div>
    </section>
  </main>
</template>

<style src="../local-workspace.css"></style>
