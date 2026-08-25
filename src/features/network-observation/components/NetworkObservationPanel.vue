<script setup lang="ts">
import { computed } from "vue";
import type { Copy } from "../../../shared/i18n";
import type { ProxyCandidate, TunObservation } from "../../../shared/types";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";

type ObservationTone = "positive" | "warning" | "negative" | "neutral";
type ObservationIcon = "proxy" | "windows" | "tunnel";
type ObservationItem = {
  key: string;
  label: string;
  value: string;
  detail?: string;
  tone: ObservationTone;
  icon: ObservationIcon;
  help?: { label: string; text: string };
};

const props = withDefaults(defineProps<{
  copy: Copy;
  systemProxy?: ProxyCandidate;
  tun: TunObservation;
  proxyAvailable?: boolean;
  showLocalProxy?: boolean;
  loading?: boolean;
  context?: "console" | "assistant";
}>(), {
  proxyAvailable: false,
  showLocalProxy: false,
  loading: false,
  context: "assistant"
});

const tunLabel = computed(() => ({
  notDetected: props.copy.tunNotDetected,
  possible: props.copy.tunPossible,
  detected: props.copy.tunDetected,
  unknown: props.copy.tunUnknown
})[props.tun.state]);

const tunTone = computed<ObservationTone>(() => ({
  notDetected: "negative",
  possible: "warning",
  detected: "positive",
  unknown: "neutral"
})[props.tun.state] as ObservationTone);

const items = computed<ObservationItem[]>(() => {
  const result: ObservationItem[] = [];
  if (props.showLocalProxy) {
    result.push({
      key: "local-proxy",
      label: props.copy.assistantLocalProxy,
      value: props.proxyAvailable ? props.copy.assistantAvailable : props.copy.assistantUnavailable,
      tone: props.proxyAvailable ? "positive" : "negative",
      icon: "proxy"
    });
  }

  const systemProxyPending = props.loading && !props.systemProxy;
  result.push({
    key: "system-proxy",
    label: props.copy.windowsSystemProxy,
    value: systemProxyPending
      ? props.copy.networkObserving
      : props.systemProxy ? props.copy.systemProxyOn : props.copy.systemProxyOff,
    detail: props.systemProxy
      ? `${props.systemProxy.host}:${props.systemProxy.port}`
      : systemProxyPending ? undefined : props.copy.systemProxyDirect,
    tone: systemProxyPending ? "neutral" : props.systemProxy ? "positive" : "negative",
    icon: "windows"
  });

  result.push({
    key: "tun",
    label: props.copy.assistantTun,
    value: props.loading && props.tun.state === "unknown" ? props.copy.networkObserving : tunLabel.value,
    detail: props.tun.interfaceName
      ?? (props.tun.evidence.length ? `${props.copy.assistantTunEvidence}: ${props.tun.evidence.length}` : undefined),
    tone: props.loading && props.tun.state === "unknown" ? "neutral" : tunTone.value,
    icon: "tunnel",
    help: { label: props.copy.aboutTun, text: props.copy.tunDescription }
  });
  return result;
});
</script>

<template>
  <section class="network-observation-panel" :class="[`context-${context}`, { 'has-local-proxy': showLocalProxy }]" :aria-busy="loading">
    <div class="network-observation-heading">
      <div><h2>{{ copy.assistantCurrentNetwork }}</h2><p>{{ copy.assistantObservationHint }}</p></div>
      <span class="read-only-badge"><svg viewBox="0 0 20 20" aria-hidden="true"><rect x="5" y="8.5" width="10" height="7.5" rx="2"/><path d="M7.5 8.5V6.7a2.5 2.5 0 0 1 5 0v1.8"/></svg>{{ copy.readOnly }}</span>
    </div>

    <div class="network-observation-grid" role="status" aria-live="polite">
      <div v-for="item in items" :key="item.key" class="network-observation-item" :class="`tone-${item.tone}`">
        <span class="observation-symbol" aria-hidden="true">
          <svg v-if="item.icon === 'windows'" viewBox="0 0 24 24" class="filled-icon"><path d="M4 5.5 10.6 4v7H4v-5.5Zm8.1-1.8L20 2v9h-7.9V3.7ZM4 12.6h6.6v7L4 18.2v-5.6Zm8.1 0H20v9l-7.9-1.7v-7.3Z" /></svg>
          <svg v-else-if="item.icon === 'tunnel'" viewBox="0 0 24 24"><path d="M5 7.5h5.5a3 3 0 0 1 3 3v3a3 3 0 0 0 3 3H20M17 13.5l3 3-3 3M7 4.5l-3 3 3 3" /></svg>
          <svg v-else viewBox="0 0 24 24"><path d="M8.5 8.5 6.8 6.8a3 3 0 0 0-4.2 4.2l2.8 2.8a3 3 0 0 0 4.2 0l1.2-1.2m4.7 2.9 1.7 1.7a3 3 0 0 0 4.2-4.2l-2.8-2.8a3 3 0 0 0-4.2 0l-1.2 1.2M8.5 15.5l7-7" /></svg>
        </span>
        <div class="observation-copy">
          <div class="observation-label"><span>{{ item.label }}</span><HelpTooltip v-if="item.help" :label="item.help.label" :text="item.help.text" /></div>
          <strong>{{ item.value }}</strong><code v-if="item.detail">{{ item.detail }}</code>
        </div>
        <span class="observation-state-mark" aria-hidden="true">
          <svg v-if="item.tone === 'positive'" viewBox="0 0 20 20"><path d="m4.5 10.2 3.2 3.2 7.8-7.8" /></svg>
          <svg v-else-if="item.tone === 'negative'" viewBox="0 0 20 20"><path d="m6 6 8 8m0-8-8 8" /></svg>
          <svg v-else-if="item.tone === 'warning'" viewBox="0 0 20 20"><path d="M10 5.5v5.7m0 3.1v.01" /></svg>
          <svg v-else viewBox="0 0 20 20"><path d="M7.8 7.5A2.4 2.4 0 0 1 10.2 5c1.5 0 2.6.9 2.6 2.2 0 1.8-2.8 2-2.8 4.1m0 3.2v.01" /></svg>
        </span>
      </div>
    </div>
  </section>
</template>
