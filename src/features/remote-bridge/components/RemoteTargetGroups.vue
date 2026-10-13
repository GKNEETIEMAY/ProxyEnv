<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import type { RemoteBridgeCopy } from "../../../shared/i18n/remote-bridge";
import type { RemoteTarget, RemoteTargetSource } from "../state";
import { sshTargetAddress as address } from "../ssh-presentation";

const props = defineProps<{
  targets: RemoteTarget[];
  selectedId: string;
  copy: RemoteBridgeCopy;
  disabled?: boolean;
}>();
const emit = defineEmits<{
  select: [id: string];
  edit: [target: RemoteTarget];
  remove: [target: RemoteTarget];
}>();

const order: RemoteTargetSource[] = ["openssh", "vscode", "mobaxterm", "manual"];
const labels = computed<Record<RemoteTargetSource, string>>(() => ({
  openssh: props.copy.rbSourceOpenSsh,
  vscode: props.copy.rbSourceVscode,
  mobaxterm: props.copy.rbSourceMoba,
  manual: props.copy.rbSourceManual,
}));
const orderedTargets = computed(() => order.flatMap((source) => props.targets.filter((target) => target.source === source)));
const list = ref<HTMLElement>();
const menuPosition = ref({ top: "0px", left: "0px" });

function closeMenus(event?: Event) {
  const inside = event?.target instanceof Element ? event.target.closest(".remote-target-menu") : null;
  list.value?.querySelectorAll<HTMLDetailsElement>("details[open]").forEach((menu) => {
    if (menu !== inside) menu.open = false;
  });
}

async function placeMenu(event: Event) {
  const menu = event.target as HTMLDetailsElement;
  if (!menu.open) return;
  closeMenus(event);
  await nextTick();
  if (!menu.open) return;
  const anchor = menu.querySelector("summary")!.getBoundingClientRect();
  const popover = menu.querySelector<HTMLElement>(".remote-target-menu-popover")!;
  const height = popover.offsetHeight;
  const top = anchor.bottom + height + 8 > window.innerHeight - 12
    ? Math.max(12, anchor.top - height - 6)
    : anchor.bottom + 6;
  menuPosition.value = {
    top: `${top}px`,
    left: `${Math.max(12, Math.min(anchor.right - popover.offsetWidth, window.innerWidth - popover.offsetWidth - 12))}px`,
  };
}

function dismissWithKeyboard(event: KeyboardEvent) {
  if (event.key === "Escape") closeMenus();
}

onMounted(() => {
  document.addEventListener("pointerdown", closeMenus);
  document.addEventListener("keydown", dismissWithKeyboard);
  document.addEventListener("scroll", closeMenus, true);
});
onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", closeMenus);
  document.removeEventListener("keydown", dismissWithKeyboard);
  document.removeEventListener("scroll", closeMenus, true);
});


function authentication(target: RemoteTarget): string {
  return ({
    identityFile: props.copy.rbTargetAuthIdentity,
    agent: props.copy.rbTargetAuthAgent,
    password: props.copy.rbTargetAuthPassword,
    keyboardInteractive: props.copy.rbTargetAuthPassword,
    unknown: props.copy.rbTargetAuthAutomatic,
  })[target.authenticationMethod];
}

function runMenuAction(event: MouseEvent, action: "edit" | "remove", target: RemoteTarget) {
  const details = (event.currentTarget as HTMLElement).closest("details");
  details?.removeAttribute("open");
  if (action === "edit") emit("edit", target);
  else emit("remove", target);
}
</script>

<template>
  <div ref="list" class="remote-target-list" role="radiogroup" :aria-label="copy.rbConnectionList" @click.self="!disabled && emit('select', '')">
    <article
      v-for="(target, index) in orderedTargets"
      :key="target.id"
      class="remote-target"
      :class="{
        selected: selectedId === target.id,
        unavailable: !target.available,
        'menu-up': index === orderedTargets.length - 1,
      }"
    >
          <button
            class="remote-target-main"
            type="button"
            role="radio"
            :aria-checked="selectedId === target.id"
            :disabled="disabled || !target.available"
            :title="!target.available ? copy.rbTargetUnsupported : undefined"
            @click="emit('select', target.id)"
          >
            <span class="remote-target-mark" aria-hidden="true"></span>
            <span class="remote-target-copy">
              <strong>{{ target.displayName }}</strong>
              <code>{{ address(target) }}</code>
              <span class="remote-target-meta">{{ labels[target.source] }} · {{ authentication(target) }}</span>
            </span>
          </button>
          <span class="remote-target-state" :class="{ unavailable: !target.available }" :title="target.available ? copy.rbTargetAvailable : copy.rbTargetUnsupported">
            <span aria-hidden="true"></span>
            {{ target.available ? copy.rbTargetAvailable : copy.rbTargetUnavailableShort }}
          </span>
          <details class="remote-target-menu" @click.stop @toggle="placeMenu">
            <summary :aria-label="copy.rbConnectionMoreActions">
              <svg aria-hidden="true" viewBox="0 0 20 20">
                <circle cx="4" cy="10" r="1.5" />
                <circle cx="10" cy="10" r="1.5" />
                <circle cx="16" cy="10" r="1.5" />
              </svg>
            </summary>
            <div class="remote-target-menu-popover" role="menu" :style="menuPosition">
              <button type="button" role="menuitem" @click="runMenuAction($event, 'edit', target)">{{ copy.rbConnectionEdit }}</button>
              <button class="danger" type="button" role="menuitem" @click="runMenuAction($event, 'remove', target)">{{ target.source === 'manual' ? copy.rbConnectionRemove : copy.rbConnectionHide }}</button>
            </div>
          </details>
    </article>
  </div>
</template>

<style scoped>
.remote-target-list { display:grid; gap:6px; }
.remote-target { position:relative; display:grid; min-width:0; height:var(--ssh-row-height,72px); align-items:center; grid-template-columns:minmax(0,1fr) auto auto; border:1px solid var(--line); border-left:3px solid transparent; border-radius:11px; background:var(--surface-strong); transition:border-color .16s ease,background-color .16s ease,box-shadow .16s ease; }
.remote-target:hover:not(.unavailable) { border-color:var(--line-strong); background:color-mix(in srgb,var(--surface-strong) 88%,var(--accent-soft)); }
.remote-target.selected { border-color:color-mix(in srgb,var(--accent) 44%,var(--line)); border-left-color:var(--accent); background:color-mix(in srgb,var(--surface-strong) 94%,var(--accent)); }
.remote-target.unavailable { opacity:.68; }
.remote-target:focus-within { box-shadow:0 0 0 3px color-mix(in srgb,var(--focus) 24%,transparent); }
.remote-target-main { display:grid; min-width:0; height:100%; padding:6px 8px 6px 12px; align-items:center; grid-template-columns:16px minmax(0,1fr); gap:10px; border:0; color:inherit; text-align:left; background:transparent; cursor:pointer; }
.remote-target-main:disabled { cursor:not-allowed; }
.remote-target-main:focus-visible { outline:0; }
.remote-target-mark { width:14px; height:14px; border:1px solid var(--line-strong); border-radius:50%; box-shadow:inset 0 0 0 3px var(--surface-strong); background:transparent; }
.remote-target.selected .remote-target-mark { border-color:var(--accent); background:var(--accent); }
.remote-target-copy { display:grid; min-width:0; gap:2px; }
.remote-target-copy strong,.remote-target-copy code,.remote-target-meta { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.remote-target-copy strong { font-size:13px; font-weight:700; }
.remote-target-copy code { color:var(--muted); font:11px/1.4 ui-monospace,SFMono-Regular,Consolas,monospace; }
.remote-target-meta { color:var(--muted); font-size:10px; line-height:1.35; }
.remote-target-state { display:inline-flex; margin-right:2px; align-items:center; gap:5px; color:var(--success); font-size:10px; font-weight:680; white-space:nowrap; }
.remote-target-state > span { width:6px; height:6px; border-radius:50%; background:currentColor; }
.remote-target-state.unavailable { color:var(--danger); }
.remote-target-menu { position:relative; margin-right:7px; }
.remote-target-menu summary { display:grid; width:28px; height:28px; place-items:center; border-radius:7px; color:var(--muted); cursor:pointer; list-style:none; }
.remote-target-menu summary::-webkit-details-marker { display:none; }
.remote-target-menu summary:hover,.remote-target-menu[open] summary { color:var(--accent-strong); background:var(--accent-soft); }
.remote-target-menu summary:focus-visible { outline:2px solid var(--focus); outline-offset:2px; }
.remote-target-menu svg { width:16px; height:16px; fill:currentColor; }
.remote-target-menu-popover { position:fixed; z-index:30; display:grid; width:max-content; min-width:128px; padding:5px; border:1px solid var(--line); border-radius:10px; background:var(--surface); box-shadow:0 12px 32px rgba(35,31,27,.14); }
.remote-target-menu-popover button { min-height:30px; padding:6px 9px; border:0; border-radius:7px; color:var(--text); text-align:left; background:transparent; cursor:pointer; font-size:11px; }
.remote-target-menu-popover button:hover { background:var(--surface-strong); }
.remote-target-menu-popover button.danger { color:var(--danger); }
@media (prefers-reduced-motion:reduce) {
  .remote-target { transition-duration:.01ms; }
}
</style>
