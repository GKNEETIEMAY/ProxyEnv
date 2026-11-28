<script setup lang="ts">
import { computed } from "vue";
import type { RemoteBridgeCopy } from "../../../shared/i18n/remote-bridge";
import type { TargetPlatform, WindowsBridgeView } from "../state";

const props = defineProps<{
  copy: RemoteBridgeCopy;
  platform: TargetPlatform;
  windowsBridge?: WindowsBridgeView;
  pending: boolean;
  error: string;
  disabled: boolean;
}>();
defineEmits<{ probe: []; cancel: [] }>();
const label = computed(() => ({
  linux: "Linux", windows: "Windows",
  unknown: props.copy.rbPlatformUnknown, unsupported: props.copy.rbTargetUnsupported,
})[props.platform]);
const transport = computed(() => ({
  notPrepared: props.copy.rbStates.disconnected,
  preparing: props.copy.rbAuthBuildingBridge,
  awaitingAuthorization: props.copy.rbPlatformAwaitAuthorization,
  unavailable: props.copy.rbStates.unavailable,
  closing: props.copy.rbPlatformClosing,
  disconnected: props.copy.rbStates.disconnected,
  closedAbnormally: props.copy.rbAuthBridgeFailed,
  cleanupIncomplete: props.copy.rbPlatformCleanupIncomplete,
})[props.windowsBridge?.transport ?? "unavailable"]);
const terminal = computed(() => ({
  unprepared: props.copy.rbOverviewNotEnabled,
  awaitingEnvironment: props.copy.rbPlatformAwaitEnvironment,
  prepared: props.copy.rbPlatformEnvironmentPrepared,
  unavailable: props.copy.rbStates.unavailable,
  closed: props.copy.rbStates.disconnected,
})[props.windowsBridge?.terminal ?? "unavailable"]);
</script>

<template>
  <section class="remote-platform-status">
    <div class="remote-platform-row">
      <span class="remote-platform-label">{{ copy.rbTargetPlatform }} <strong aria-live="polite">{{ pending ? copy.rbBusy : label }}</strong></span>
      <button v-if="pending" class="text-action" type="button" @click="$emit('cancel')">{{ copy.rbCancel }}</button>
      <button v-else class="text-action" type="button" :disabled="disabled" @click="$emit('probe')">{{ copy.rbProbePlatform }}</button>
    </div>
    <p v-if="error" class="notice notice-warning" role="alert">{{ error }}</p>
    <div v-if="platform === 'windows'" class="remote-platform-windows">
      <dl aria-live="polite">
        <div><dt>{{ copy.rbStatus }}</dt><dd>{{ transport }}</dd></div>
        <div><dt>{{ copy.rbManagedTerminal }}</dt><dd>{{ terminal }}</dd></div>
      </dl>
      <p id="remote-windows-reason">{{ copy.rbWindowsBridgeUnavailable }}</p>
    </div>
  </section>
</template>

<style scoped>
.remote-platform-status { border-block-start: 1px solid var(--line); padding-block: 10px; font-size: 13px; line-height: 1.5; }
.remote-platform-row, dl { display: flex; flex-wrap: wrap; align-items: baseline; gap: 8px 18px; }
.remote-platform-row { justify-content: space-between; }
.remote-platform-label, dt, .remote-platform-windows p { color: var(--muted); }
.remote-platform-label strong, dd { color: var(--ink); margin-inline-start: 5px; font-weight: 600; }
dl { margin: 8px 0 0; }
dl div { display: flex; flex-wrap: wrap; }
.remote-platform-windows p { margin-block: 6px 0; overflow-wrap: anywhere; }
</style>
