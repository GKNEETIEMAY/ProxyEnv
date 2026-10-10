<script setup lang="ts">
import type { CheckState } from "../../../shared/types";
import StatusIndicator from "../../../shared/components/StatusIndicator.vue";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
defineProps<{ label:string; state:CheckState; status:string; help?:string; detail?:string }>();
</script>
<template>
  <div class="advanced-status-row">
    <span class="advanced-row-label"><slot name="icon" />{{ label }}<HelpTooltip v-if="help" pinnable :label="label" :text="help" /></span>
    <StatusIndicator :state="state" :label="status" />
    <small v-if="detail">{{ detail }}</small>
  </div>
</template>
<style scoped>
.advanced-status-row { display:flex; flex-wrap:wrap; align-items:center; gap:5px 8px; min-height:29px; }
.advanced-row-label { display:inline-flex; align-items:center; gap:6px; flex:1; min-width:110px; font-size:12px; }
.advanced-row-label :deep(img),.advanced-row-label :deep(.remote-tool-icon) { width:17px; height:17px; flex:none; object-fit:contain; }
.advanced-status-row :deep(.check-status) { padding:3px 7px; border-radius:8px; background:color-mix(in srgb,currentColor 5%,transparent); }
.advanced-status-row small { color:var(--muted); font-size:11px; overflow-wrap:anywhere; }
</style>
