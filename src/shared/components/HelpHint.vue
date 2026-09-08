<script setup lang="ts">
import HelpTooltip from "./HelpTooltip.vue";

defineProps<{
  label: string;
  headings: { check: string; success: string; failure: string; next: string };
  content: { check: string; success: string; failure: string; next: string };
}>();
</script>

<template>
  <HelpTooltip :label="label">
    <span class="check-help-content">
      <span><strong>{{ headings.check }}</strong>{{ content.check }}</span>
      <span><strong>{{ headings.success }}</strong>{{ content.success }}</span>
      <span><strong>{{ headings.failure }}</strong>{{ content.failure }}</span>
      <span><strong>{{ headings.next }}</strong>{{ content.next }}</span>
    </span>
  </HelpTooltip>
</template>

<style scoped>
.check-help-content { display:grid; gap:8px; }
.check-help-content > span { display:grid; gap:2px; }
.check-help-content strong { color:var(--ink); font-size:10px; font-weight:670; }
</style>
