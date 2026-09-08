<script setup lang="ts">
import type { CheckState } from "../types";
import HelpHint from "./HelpHint.vue";
import LastChecked from "./LastChecked.vue";
import StatusIndicator from "./StatusIndicator.vue";

defineProps<{
  label: string;
  state: CheckState;
  stateLabel: string;
  checkedAt: number | null;
  lastCheckedLabel: string;
  helpLabel: string;
  helpHeadings: { check: string; success: string; failure: string; next: string };
  helpContent: { check: string; success: string; failure: string; next: string };
}>();
</script>

<template>
  <div class="check-row">
    <div class="check-row-copy">
      <div class="check-row-label"><strong>{{ label }}</strong><HelpHint :label="helpLabel" :headings="helpHeadings" :content="helpContent" /></div>
      <slot name="detail" />
    </div>
    <div class="check-row-result">
      <StatusIndicator :state="state" :label="stateLabel" />
      <LastChecked :label="lastCheckedLabel" :checked-at="checkedAt" />
    </div>
    <div v-if="$slots.actions" class="check-row-actions"><slot name="actions" /></div>
  </div>
</template>

<style scoped>
.check-row { display:grid; min-width:0; min-height:62px; padding:12px 0; align-items:center; grid-template-columns:minmax(0,1fr) minmax(150px,.75fr) auto; gap:16px; border-top:1px solid var(--line); }
.check-row-copy,.check-row-result { display:grid; min-width:0; gap:4px; }
.check-row-label { display:flex; min-width:0; align-items:center; gap:6px; }
.check-row-label strong { min-width:0; overflow-wrap:anywhere; font-size:12px; font-weight:650; }
.check-row-result { justify-items:start; }
.check-row-actions { display:flex; align-items:center; justify-content:flex-end; gap:8px; }
:deep(.check-row-detail) { min-width:0; margin:0; color:var(--muted); font-size:11px; line-height:1.5; overflow-wrap:anywhere; }
@media (max-width:680px) {
  .check-row { grid-template-columns:minmax(0,1fr) auto; gap:8px 12px; }
  .check-row-result { justify-items:end; text-align:end; }
  .check-row-actions { grid-column:1/-1; justify-content:flex-start; }
}
</style>
