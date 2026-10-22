<script setup lang="ts">
import { ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Copy } from "../../shared/i18n";
defineProps<{ copy: Copy }>();
defineEmits<{ dismiss: [] }>();
const failed = ref(false);
async function openRepository() {
  failed.value = false;
  try { await openUrl("https://github.com/GKNEETIEMAY/ProxyEnv"); }
  catch { failed.value = true; }
}
</script>
<template>
  <aside class="star-prompt" :aria-label="copy.starAction">
    <svg class="star-symbol" viewBox="0 0 24 24" aria-hidden="true"><path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2-5.6-3-5.6 3 1.1-6.2L3 9.6l6.2-.9Z" /></svg>
    <p>{{ copy.starPrompt }}<span v-if="failed" role="status">{{ copy.starOpenFailed }}</span></p>
    <button class="star-action" type="button" @click="openRepository">{{ copy.starAction }}</button>
    <button class="star-dismiss" type="button" :aria-label="copy.starDismiss" :title="copy.starDismiss" @click="$emit('dismiss')"><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m6 6 8 8m0-8-8 8" /></svg></button>
  </aside>
</template>
<style scoped>
.star-prompt { display:flex; align-items:center; gap:10px; padding:12px 16px; border:1px solid var(--line); border-radius:10px; background:var(--warning-soft); font-size:12px; }
.star-symbol { width:18px; height:18px; flex:none; fill:var(--warning); }
.star-prompt p { flex:1; min-width:0; margin:0; line-height:1.5; color:var(--ink); }
.star-prompt p span { display:block; color:var(--danger); }
.star-action { flex:none; padding:6px 10px; border:1px solid var(--line-strong); border-radius:6px; color:var(--ink); background:var(--surface); cursor:pointer; }
.star-action:hover { background:var(--surface-strong); }
.star-dismiss { display:grid; place-items:center; flex:none; width:28px; height:28px; padding:0; background:none; color:var(--muted); cursor:pointer; }
.star-dismiss:hover { color:var(--ink); background:var(--surface-strong); }
.star-dismiss svg { width:18px; fill:none; stroke:currentColor; stroke-width:1.5; stroke-linecap:round; }
</style>
