<script setup lang="ts">
import { computed } from "vue";
import type { Copy } from "../../../shared/i18n";
import type {
  EnvironmentStatus,
  ManagedProxyVariable,
  ProxyCandidate
} from "../../../shared/types";

const props = defineProps<{
  copy: Copy;
  environment: EnvironmentStatus;
  detected?: ProxyCandidate;
  endpoint: string;
  error: string;
  loading: boolean;
  toggling: boolean;
  copiedEndpoint: boolean;
  selectedVariables: ManagedProxyVariable[];
}>();

const emit = defineEmits<{
  refresh: [];
  toggle: [];
  copyEndpoint: [];
  toggleVariable: [name: string];
}>();

const clientIcons: Record<string, string> = {
  "clash-verge-rev": "/proxy-clients/clash-verge-rev.png",
  v2rayn: "/proxy-clients/v2rayn.png",
  flclash: "/proxy-clients/flclash.ico",
  hiddify: "/proxy-clients/hiddify.ico",
  "clash-nyanpasu": "/proxy-clients/clash-nyanpasu.png",
  "generic-proxy": "/proxy-clients/generic-proxy.svg"
};

const detectedIcon = computed(() => props.detected
  ? clientIcons[props.detected.iconKey ?? ""] ?? clientIcons["generic-proxy"]
  : clientIcons["generic-proxy"]);
const activeCount = computed(() => props.environment.entries.filter((entry) => entry.exists).length);
const environmentConfigured = computed(() => props.environment.state !== "disabled");

function managedVariableKey(name: string): ManagedProxyVariable | undefined {
  const normalized = name.toUpperCase();
  if (normalized === "HTTP_PROXY") return "http";
  if (normalized === "HTTPS_PROXY") return "https";
  if (normalized === "ALL_PROXY") return "all";
  return undefined;
}

function variableDescription(name: string): string {
  const normalized = name.toUpperCase();
  if (normalized === "HTTP_PROXY") return props.copy.httpProxyDescription;
  if (normalized === "HTTPS_PROXY") return props.copy.httpsProxyDescription;
  if (normalized === "ALL_PROXY") return props.copy.allProxyDescription;
  return props.copy.noProxyDescription;
}

function variableActionLabel(name: string, action: "write" | "about"): string {
  const template = action === "write" ? props.copy.writeVariable : props.copy.aboutVariable;
  return template.replace("{name}", name);
}

function isManagedVariableSelected(name: string): boolean {
  const key = managedVariableKey(name);
  return key !== undefined && props.selectedVariables.includes(key);
}

function isLastManagedVariable(name: string): boolean {
  return isManagedVariableSelected(name) && props.selectedVariables.length === 1;
}
</script>

<template>
  <main class="page home-page">
    <div v-if="error" class="notice notice-error" role="alert">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 8v5m0 3.5v.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg>
      <p><strong>{{ copy.operationFailed }}</strong><span>{{ error }} · {{ copy.retryHint }}</span></p>
    </div>
    <div v-if="environment.warning" class="notice notice-warning">
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 8v5m0 3.5v.01M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" /></svg>
      <p><strong>{{ copy.attention }}</strong><span>{{ environment.warning }}</span></p>
    </div>

    <section class="proxy-stage" :class="{ enabled: environmentConfigured }">
      <div class="proxy-identity">
        <span class="section-title">{{ copy.currentProxy }}</span>
        <div v-if="detected" class="client-heading">
          <span class="client-art"><img :src="detectedIcon" alt="" /></span>
          <div>
            <h1>{{ detected.clientName || copy.localProxy }}</h1>
            <p><span class="status-dot" :class="{ quiet: !detected.listening }"></span>{{ detected.listening ? copy.listening : copy.notListening }}</p>
          </div>
        </div>
        <div v-else class="empty-state">
          <span class="client-art generic"><img :src="detectedIcon" alt="" /></span>
          <div><h1>{{ loading ? copy.detecting : copy.noProxy }}</h1><p>{{ copy.noProxyHint }}</p></div>
        </div>
        <div v-if="detected" class="endpoint-line">
          <div class="endpoint-address">
            <code>{{ endpoint }}</code>
            <button type="button" :aria-label="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" :title="copiedEndpoint ? copy.endpointCopied : copy.copyEndpoint" @click="emit('copyEndpoint')">
              <svg v-if="!copiedEndpoint" viewBox="0 0 20 20" aria-hidden="true"><rect x="6.5" y="6.5" width="9" height="9" rx="1.6"/><path d="M13.5 6.5V5A1.5 1.5 0 0 0 12 3.5H5A1.5 1.5 0 0 0 3.5 5v7A1.5 1.5 0 0 0 5 13.5h1.5"/></svg>
              <svg v-else viewBox="0 0 20 20" aria-hidden="true"><path d="m4.5 10.2 3.2 3.2 7.8-7.8"/></svg>
            </button>
          </div>
          <span>{{ detected.protocol }} · {{ detected.confidence }} {{ copy.autoConfidence }}</span>
        </div>
      </div>

      <div class="proxy-action">
        <div>
          <span class="section-title">{{ copy.proxyEnvironment }}</span>
          <strong>{{ environmentConfigured ? copy.enabled : copy.disabled }}</strong>
          <p>{{ environmentConfigured ? copy.environmentOnHint : copy.environmentOffHint }}</p>
        </div>
        <button class="toggle-control" :class="{ active: environmentConfigured }" type="button" :disabled="loading || toggling" :aria-pressed="environmentConfigured" @click="emit('toggle')">
          <span class="toggle-track"><span></span></span>
          <b>{{ toggling ? copy.enabling : environmentConfigured ? copy.enabled : copy.disabled }}</b>
        </button>
      </div>
    </section>

    <section class="variables-section">
      <div class="section-header">
        <div><h2>{{ copy.variables }}</h2><p>{{ copy.variablesHint }}</p></div>
        <button class="refresh-button" type="button" :disabled="loading" @click="emit('refresh')">
          <svg :class="{ spinning: loading }" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.34 5.66M20 5v6h-6" /></svg>{{ copy.refresh }}
        </button>
      </div>
      <div v-if="environment.entries.length" class="variable-table">
        <div v-for="entry in environment.entries" :key="entry.name" class="variable-row">
          <span class="variable-indicator" :class="{ active: entry.exists }"></span>
          <div class="variable-label">
            <code class="variable-name">{{ entry.name }}</code>
            <span class="variable-help" tabindex="0" :aria-label="variableActionLabel(entry.name, 'about')" :aria-describedby="`variable-help-${entry.name}`" @mousedown.prevent>
              <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="7.5"/><path d="M7.9 7.8a2.25 2.25 0 0 1 4.3.9c0 1.55-2.2 1.75-2.2 3.2M10 14.6v.01"/></svg>
              <p :id="`variable-help-${entry.name}`" role="tooltip">{{ variableDescription(entry.name) }}</p>
            </span>
          </div>
          <code class="variable-value" :class="{ unset: !entry.exists }">{{ entry.value ?? copy.unset }}</code>
          <label v-if="managedVariableKey(entry.name)" class="variable-choice" :class="{ locked: isLastManagedVariable(entry.name) }">
            <input type="checkbox" :checked="isManagedVariableSelected(entry.name)" :disabled="isLastManagedVariable(entry.name)" :aria-label="variableActionLabel(entry.name, 'write')" @change="emit('toggleVariable', entry.name)" />
            <span aria-hidden="true"><svg viewBox="0 0 16 16"><path d="m4 8.2 2.5 2.5L12 5.5"/></svg></span>
          </label>
          <span v-else class="variable-choice-spacer"></span>
        </div>
      </div>
      <div v-else class="skeleton-list" :aria-label="copy.detecting"><span v-for="item in 4" :key="item"></span></div>
      <p class="table-caption">{{ activeCount }} / {{ environment.entries.length }} · {{ copy.localOnly }}</p>
    </section>
  </main>
</template>
