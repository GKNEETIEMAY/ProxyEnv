<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  label: string;
  checkedAt: number | null;
}>();

const value = computed(() => props.checkedAt
  ? new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit", second: "2-digit" }).format(props.checkedAt)
  : "—");
</script>

<template>
  <span class="last-checked"><span>{{ label }}</span><time v-if="checkedAt" :datetime="new Date(checkedAt).toISOString()">{{ value }}</time><span v-else>{{ value }}</span></span>
</template>

<style scoped>
.last-checked { display:inline-flex; min-width:0; align-items:baseline; gap:5px; color:var(--muted); font-size:10px; font-variant-numeric:tabular-nums; }
</style>
