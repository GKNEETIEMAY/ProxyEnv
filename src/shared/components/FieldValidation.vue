<script setup lang="ts">
import HelpTooltip from "./HelpTooltip.vue";

defineProps<{ id: string; message: string }>();
</script>

<template>
  <span v-if="message" class="field-validation">
    <HelpTooltip :label="message" :text="message" tone="error" />
    <span :id="id" class="field-validation-description">{{ message }}</span>
  </span>
</template>

<style scoped>
.field-validation { display:inline-flex; flex:none; }
.field-validation-description { position:absolute; width:1px; height:1px; padding:0; margin:-1px; overflow:hidden; clip-path:inset(50%); white-space:nowrap; border:0; }
</style>
