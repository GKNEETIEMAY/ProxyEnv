<script setup lang="ts">
import { watch } from "vue";
import type { Copy } from "../../../shared/i18n";
const props = defineProps<{ visible: boolean; sound: boolean; copy: Copy }>();
defineEmits<{ open: []; dismiss: [] }>();
watch(() => props.visible, async (visible) => {
  if (!visible || !props.sound || document.hidden) return;
  let context: AudioContext | undefined;
  try {
    context = new AudioContext();
    // Autoplay restrictions must not leave a pending resume or audio resource.
    if (context.state !== "running") return;
    const tone = context.createOscillator(), gain = context.createGain();
    tone.frequency.value = 660;
    gain.gain.setValueAtTime(0.035, context.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, context.currentTime + 0.18);
    tone.connect(gain); gain.connect(context.destination);
    tone.start(); tone.stop(context.currentTime + 0.2);
    await new Promise<void>(resolve => { tone.onended = () => resolve(); });
  } catch { /* A denied sound must not prevent the visible notification. */ }
  finally { void context?.close().catch(() => {}); }
});
</script>
<template>
  <aside v-if="visible" class="bridge-notice" role="alert" :aria-label="copy.rbNotifications">
    <div class="bridge-notice-heading"><strong>ProxyEnv</strong><button type="button" :aria-label="copy.rbNotificationClose" @click="$emit('dismiss')"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" /></svg></button></div>
    <p>{{ copy.rbReconnectAttention }}</p>
    <button class="secondary-action" type="button" @click="$emit('open')">{{ copy.rbReconnectAction }}</button>
  </aside>
</template>
<style scoped>
.bridge-notice { position:fixed; inset-inline-end:20px; bottom:20px; z-index:100; width:min(340px,calc(100vw - 40px)); padding:18px; border:1px solid var(--line); border-radius:12px; background:var(--surface-strong); color:var(--ink); }
.bridge-notice-heading { display:flex; align-items:center; justify-content:space-between; gap:16px; }
.bridge-notice-heading button { display:grid; place-items:center; width:28px; height:28px; border:0; border-radius:6px; background:transparent; color:inherit; cursor:pointer; }
.bridge-notice-heading svg { width:16px; height:16px; stroke:currentColor; stroke-width:1.5; fill:none; }
.bridge-notice p { margin:12px 0; font-size:14px; line-height:1.6; overflow-wrap:anywhere; }
.bridge-notice button:focus-visible { outline:2px solid var(--accent); outline-offset:3px; }
</style>
