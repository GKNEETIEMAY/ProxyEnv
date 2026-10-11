<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import StatusIndicator from "./StatusIndicator.vue";
import { notifications, dismissNotification, pauseNotification, resumeNotification, clearNotifications } from "../state/notifications";

defineProps<{ closeLabel: string }>();
const host = ref<HTMLElement>();
const target = ref<string | HTMLElement>("body");
async function syncHost() {
  // Outside a modal, body is inert: place the same host inside the active dialog.
  target.value = [...document.querySelectorAll<HTMLDialogElement>("dialog[open]")].at(-1) ?? "body";
  await nextTick();
  const element = host.value;
  if (!element || typeof element.showPopover !== "function") return;
  // Re-enter the top layer if a dialog opened after the previous notice.
  if (element.matches(":popover-open")) element.hidePopover();
  if (notifications.length) element.showPopover();
}
watch(() => notifications.map(notice => `${notice.id}:${notice.kind}:${notice.message}`), syncHost, { flush: "post" });
onMounted(() => document.addEventListener("close", syncHost, true));
function resumeIfIdle(id: number, event: Event) {
  if (!(event.currentTarget as HTMLElement).matches(":hover, :focus-within")) resumeNotification(id);
}
onBeforeUnmount(() => { document.removeEventListener("close", syncHost, true); clearNotifications(); });
</script>

<template>
  <Teleport :to="target">
    <div ref="host" class="notification-center" popover="manual" :class="{ 'has-notifications':notifications.length > 0 }">
      <TransitionGroup name="notification" tag="div" class="notification-stack">
        <section v-for="notice in notifications" :key="notice.id" class="notification-card" :data-kind="notice.kind" :role="notice.kind === 'error' ? 'alert' : 'status'" :aria-live="notice.kind === 'error' ? 'assertive' : 'polite'" aria-atomic="true" @mouseenter="pauseNotification(notice.id)" @mouseleave="resumeIfIdle(notice.id, $event)" @focusin="pauseNotification(notice.id)" @focusout="resumeIfIdle(notice.id, $event)">
          <StatusIndicator class="notification-mark" :state="notice.kind === 'success' ? 'healthy' : notice.kind === 'error' ? 'failed' : notice.kind === 'warning' ? 'warning' : 'idle'" label="" aria-hidden="true" />
          <div class="notification-copy"><strong v-if="notice.title">{{ notice.title }}</strong><p>{{ notice.message }}</p></div>
          <button type="button" class="notification-close" :aria-label="closeLabel" :title="closeLabel" @click="dismissNotification(notice.id)"><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m6 6 8 8m0-8-8 8" /></svg></button>
        </section>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.notification-center { position:fixed; inset:82px 20px auto auto; width:min(360px,calc(100vw - 32px)); height:auto; max-height:calc(100dvh - 100px); margin:0; padding:0; border:0; background:transparent; overflow:visible; color:var(--ink); z-index:10000; pointer-events:none; display:none; }
.notification-center.has-notifications { display:block; }
.notification-center::backdrop { background:transparent; pointer-events:none; }
.notification-stack { display:grid; gap:8px; max-height:calc(100dvh - 100px); padding:2px; overflow-y:auto; scrollbar-width:thin; }
.notification-card { display:flex; align-items:flex-start; gap:10px; padding:12px; border:1px solid var(--line-strong); border-radius:12px; background:var(--surface-strong); pointer-events:auto; -webkit-app-region:no-drag; }
.notification-mark { flex:none; margin-top:2px; }
.notification-copy { flex:1; min-width:0; max-height:240px; overflow-y:auto; user-select:text; scrollbar-width:thin; }
.notification-copy strong { display:block; margin-bottom:3px; font-size:12px; font-weight:650; }
.notification-copy p { margin:0; font-size:12px; line-height:1.6; overflow-wrap:anywhere; }
.notification-close { display:grid; place-items:center; flex:none; width:28px; height:28px; margin:-3px -3px 0 0; padding:5px; border:0; border-radius:8px; background:transparent; color:var(--muted); cursor:pointer; }
.notification-close:hover { background:var(--surface); color:var(--ink); }
.notification-close:focus-visible { outline:2px solid var(--focus); outline-offset:2px; }
.notification-close svg { width:16px; height:16px; fill:none; stroke:currentColor; stroke-width:1.6; stroke-linecap:round; }
.notification-enter-active,.notification-leave-active { transition:opacity 160ms ease-out,transform 160ms ease-out; }
.notification-enter-from,.notification-leave-to { opacity:0; transform:translateY(-6px); }
@media(max-width:540px) { .notification-center { inset:76px 16px auto auto; } }
@media(prefers-reduced-motion:reduce) { .notification-enter-active,.notification-leave-active { transition:none; } }
@media(forced-colors:active) { .notification-card { border:1px solid CanvasText; } }
</style>
