<script setup lang="ts">
import type { CheckState } from "../../../shared/types";
import StatusIndicator from "../../../shared/components/StatusIndicator.vue";
import HelpTooltip from "../../../shared/components/HelpTooltip.vue";
defineProps<{ label:string; state:CheckState; status:string; help?:string; detail?:string }>();
</script>
<template>
  <div class="advanced-status-row">
    <span class="advanced-row-label" :title="label"><slot name="icon" /><span class="advanced-label-text">{{ label }}</span><HelpTooltip v-if="help" pinnable :label="label" :text="help" /></span>
    <StatusIndicator :state="state" :label="status" :title="status" />
    <small v-if="detail">{{ detail }}</small>
  </div>
</template>
<style scoped>
.advanced-status-row { --advanced-status-width:min(180px,58%); display:grid; grid-template-columns:minmax(0,1fr) var(--advanced-status-width); align-items:center; gap:5px 8px; min-height:29px; }
.advanced-row-label { display:inline-flex; align-items:center; gap:6px; min-width:0; font-size:12px; }
.advanced-label-text { min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.advanced-row-label :deep(img),.advanced-row-label :deep(.remote-tool-icon) { width:17px; height:17px; flex:none; object-fit:contain; }
.advanced-row-label :deep(.help-tooltip) { flex:none; }
.advanced-status-row :deep(.check-status) { width:100%; height:26px; padding:3px 7px; border-radius:8px; background:color-mix(in srgb,currentColor 5%,transparent); white-space:nowrap; }
.advanced-status-row :deep(.check-status > span) { min-width:0; overflow:hidden; text-overflow:ellipsis; }
.advanced-status-row small { grid-column:1 / -1; color:var(--muted); font-size:11px; overflow-wrap:anywhere; }
</style>
