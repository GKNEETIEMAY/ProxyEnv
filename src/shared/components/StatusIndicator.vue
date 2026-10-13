<script setup lang="ts">
import type { CheckState } from "../types";

defineProps<{
  state: CheckState;
  label: string;
}>();
</script>

<template>
  <span class="check-status" :data-state="state" role="status">
    <svg v-if="state === 'checking'" class="check-status-spinner" viewBox="0 0 20 20" aria-hidden="true">
      <path d="M16.2 7.1A7 7 0 1 0 17 10" />
    </svg>
    <svg v-else-if="state === 'healthy'" viewBox="0 0 20 20" aria-hidden="true">
      <path d="m5.5 10.2 2.8 2.8 6.2-6.4" />
    </svg>
    <svg v-else-if="state === 'warning'" viewBox="0 0 20 20" aria-hidden="true">
      <path d="M10 5.3v5.8M10 14.4v.1" />
    </svg>
    <svg v-else-if="state === 'failed'" viewBox="0 0 20 20" aria-hidden="true">
      <path d="m6.4 6.4 7.2 7.2m0-7.2-7.2 7.2" />
    </svg>
    <svg v-else viewBox="0 0 20 20" aria-hidden="true">
      <circle cx="10" cy="10" r="2" />
    </svg>
    <span>{{ label }}</span>
  </span>
</template>

<style scoped>
.check-status { display:inline-flex; min-width:0; align-items:center; gap:7px; color:var(--muted); font-size:11px; line-height:1.45; }
.check-status svg { width:17px; height:17px; flex:none; padding:2px; overflow:visible; border-radius:50%; stroke:currentColor; stroke-width:1.8; stroke-linecap:round; stroke-linejoin:round; fill:none; background:color-mix(in srgb,currentColor 10%,transparent); }
.check-status[data-state="healthy"] { color:var(--success); }
.check-status[data-state="warning"] { color:var(--warning); }
.check-status[data-state="failed"] { color:var(--danger); }
.check-status[data-state="disabled"],.check-status[data-state="idle"] { color:var(--muted); }
.check-status-spinner { animation:check-status-spin .8s linear infinite; }
@keyframes check-status-spin { to { transform:rotate(360deg); } }
@media (prefers-reduced-motion:reduce) { .check-status-spinner { animation:none; } }
</style>
