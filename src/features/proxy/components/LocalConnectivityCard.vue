<script setup lang="ts">
import { computed } from "vue";
import type { Copy } from "../../../shared/i18n";
import type { ConnectivityErrorKind } from "../../../shared/types";
import type { ConnectivityView } from "../connectivity";
import { connectivityExpired, connectivityTone } from "../connectivity";
import StatusIndicator from "../../../shared/components/StatusIndicator.vue";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";

const props = defineProps<{ copy: Copy; connectivity: ConnectivityView; now: number; available: boolean; busy: boolean; detailed: boolean; inline?: boolean }>();
const emit = defineEmits<{ test: []; details: [] }>();
const result = computed(() => props.connectivity.result);
const expired = computed(() => !!result.value && connectivityExpired(result.value, props.now));
const tone = computed(() => props.connectivity.testing ? "checking" : props.connectivity.failed ? "warning" : connectivityTone(result.value, expired.value));
const summary = computed(() => {
  if (props.connectivity.testing) return props.copy.localTesting;
  if (props.connectivity.failed) return props.copy.localTestFailed;
  if (!result.value) return props.copy.localNotTested;
  if (expired.value) return props.copy.localExpired;
  if (!result.value.localListenerOk) return props.copy.notListening;
  if (!result.value.targets.length) return props.copy.localUnconfirmed;
  return props.copy.localTestPassed.replace("{passed}", String(result.value.targets.filter(target => target.success).length)).replace("{total}", String(result.value.targets.length));
});
const errors = computed<Record<ConnectivityErrorKind, string>>(() => ({
  proxyUnavailable: props.copy.localErrorUnavailable, proxyHandshakeFailed: props.copy.localErrorHandshake,
  connectTimeout: props.copy.localErrorTimeout, tlsFailed: props.copy.localErrorTls, remoteRejected: props.copy.localErrorRejected,
  httpStatus: props.copy.localErrorHttp, networkError: props.copy.localErrorNetwork, unknown: props.copy.localUnknown
}));
const testedAt = computed(() => result.value && Number.isFinite(Date.parse(result.value.testedAt)) ? new Date(result.value.testedAt).toLocaleString(document.documentElement.lang || undefined) : "");
const testHelp = computed(() => [props.copy.localTestHint, props.copy.localAutoCheckHint, testedAt.value ? `${props.copy.localTestedAt}: ${testedAt.value}${props.connectivity.cached ? ` · ${props.copy.localCached}` : ""}` : ""].filter(Boolean).join("\n"));
</script>

<template>
  <button v-if="inline" class="local-connectivity-trigger" type="button" :title="copy.localTestHint" @click="emit('details')"><span>{{ copy.localConnectivity }}</span><StatusIndicator :state="tone" :label="summary" /></button>
  <section v-else class="local-connectivity local-section" :aria-busy="connectivity.testing">
    <header class="local-section-heading">
      <div class="local-title-help"><h2>{{ copy.localConnectivity }}</h2><HelpTooltip :label="copy.localConnectivity" :text="testHelp" pinnable /></div>
    </header>
    <div class="local-connectivity-toolbar">
      <div class="local-connectivity-readings">
        <div class="local-connectivity-summary" aria-live="polite"><StatusIndicator :state="tone" :label="summary" /></div>
        <div v-if="result && detailed" class="local-probe-stages">
          <span>{{ copy.localListener }} <StatusIndicator :state="expired ? 'idle' : result.localListenerOk ? 'healthy' : 'failed'" :label="result.localListenerOk ? copy.listening : copy.notListening" /></span>
          <span>{{ copy.localProtocol }} <StatusIndicator :state="expired ? 'idle' : !result.localListenerOk ? 'idle' : result.protocolOk ? 'healthy' : 'warning'" :label="!result.localListenerOk ? copy.localSkipped : result.protocolOk ? copy.localConfirmed : copy.localUnconfirmed" /></span>
        </div>
      </div>
      <button class="secondary-action" type="button" :disabled="!available || busy || connectivity.testing" @click="emit('test')">{{ connectivity.testing ? copy.localTesting : copy.localTest }}</button>
    </div>
    <p v-if="connectivity.failed" class="local-caption">{{ copy.localTestRetry }}</p>
    <div v-if="result && detailed" class="local-connectivity-details" :class="{ 'is-expired': expired }">
      <ul v-if="result.targets.length" class="local-targets">
        <li v-for="target in result.targets" :key="target.name">
          <strong>{{ target.name }}</strong>
          <StatusIndicator :state="expired ? 'idle' : target.success ? 'healthy' : 'warning'" :label="target.success ? copy.localSuccess : errors[target.errorKind ?? 'unknown']" />
          <code><span v-if="target.statusCode !== null">HTTP {{ target.statusCode }} · </span>{{ target.durationMs ?? '—' }} ms</code>
        </li>
      </ul>
      <p v-else class="local-caption">{{ copy.localTest }} · {{ copy.localSkipped }}</p>
    </div>
  </section>
</template>
