<script setup lang="ts">
import type { RemoteBridgeCopy } from "../../../shared/i18n/remote-bridge";
import type { BridgeSummary } from "../state";
import { targetLabel } from "../state";
defineProps<{ copy: RemoteBridgeCopy; summary: BridgeSummary }>();
const emit=defineEmits<{ open: [] }>();
</script>
<template>
  <section class="assistant-entry remote-bridge-entry">
    <span class="assistant-entry-icon" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M4 8h15m-4-4 4 4-4 4M20 16H5m4-4-4 4 4 4" /></svg></span>
    <div><h2>{{ copy.rbTitle }}</h2><p v-if="summary.alias">{{targetLabel(summary.alias)}}<span class="remote-state" :data-state="summary.status">{{copy.rbStates[summary.status]}}</span></p><p v-else>{{copy.rbHint}}</p></div>
    <button class="primary-action" type="button" @click="emit('open')">{{ summary.alias ? copy.rbView : copy.rbOpen }}</button>
  </section>
</template>
