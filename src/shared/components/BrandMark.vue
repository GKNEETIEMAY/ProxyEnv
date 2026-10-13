<script setup lang="ts">
import mark from "../../../assets/brand/routespan-mark.png?url";

withDefaults(defineProps<{
  size?: "small" | "medium" | "large";
  decorative?: boolean;
}>(), { size: "medium", decorative: true });
</script>

<template>
  <img class="brand-mark" :class="`brand-mark-${size}`" :src="mark" :alt="decorative ? '' : 'RouteSpan'" :aria-hidden="decorative ? true : undefined" draggable="false" />
</template>

<style scoped>
.brand-mark { display: block; flex: none; object-fit: contain; }
.brand-mark-small { width: 24px; height: 24px; }
.brand-mark-medium { width: 38px; height: 38px; }
.brand-mark-large { width: 52px; height: 52px; }
</style>
