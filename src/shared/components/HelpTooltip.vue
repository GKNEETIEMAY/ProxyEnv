<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, useId } from "vue";

const props = defineProps<{
  label: string;
  text?: string;
  tone?: "help" | "error";
  pinnable?: boolean;
}>();

const tooltipId = `help-${useId().replaceAll(":", "")}`;
const trigger = ref<HTMLElement>();
const tooltip = ref<HTMLElement>();
const tooltipTarget = ref<string | HTMLElement>("body");
const visible = ref(false);
const pinned = ref(false);
const positioned = ref(false);
const position = ref({ top: "0px", left: "0px", width: "0px" });

function placeTooltip() {
  if (!trigger.value || !tooltip.value) return;
  const triggerRect = trigger.value.getBoundingClientRect();
  const width = Math.min(390, window.innerWidth - 32);
  const left = Math.min(
    Math.max(triggerRect.left + triggerRect.width / 2 - width / 2, 16),
    window.innerWidth - width - 16
  );
  tooltip.value.style.width = `${width}px`;
  tooltip.value.style.left = `${left}px`;
  const tooltipHeight = tooltip.value.offsetHeight;
  const below = triggerRect.bottom + 8;
  const top = below + tooltipHeight <= window.innerHeight - 12
    ? below
    : Math.max(12, triggerRect.top - tooltipHeight - 8);
  position.value = { top: `${top}px`, left: `${left}px`, width: `${width}px` };
  positioned.value = true;
}

async function showTooltip() {
  // Native modal dialogs are in the top layer; body-level tooltips would be inert behind them.
  tooltipTarget.value = trigger.value?.closest<HTMLElement>("dialog[open]") ?? "body";
  positioned.value = false;
  const triggerRect = trigger.value?.getBoundingClientRect();
  const width = Math.min(390, window.innerWidth - 32);
  if (triggerRect) {
    position.value = {
      top: `${triggerRect.bottom + 8}px`,
      left: `${Math.min(Math.max(triggerRect.left + triggerRect.width / 2 - width / 2, 16), window.innerWidth - width - 16)}px`,
      width: `${width}px`
    };
  }
  visible.value = true;
  await nextTick();
  if (!visible.value) return;
  placeTooltip();
  window.addEventListener("resize", placeTooltip);
  window.addEventListener("scroll", placeTooltip, true);
}

function hideTooltip() {
  pinned.value = false;
  visible.value = false;
  positioned.value = false;
  window.removeEventListener("resize", placeTooltip);
  window.removeEventListener("scroll", placeTooltip, true);
  window.removeEventListener("pointerdown", dismissOutside, true);
  window.removeEventListener("keydown", dismissOnEscape, true);
}

function hideUnlessPinned() {
  if (!pinned.value) hideTooltip();
}
function dismissOutside(event: PointerEvent) {
  if (!trigger.value?.contains(event.target as Node) && !tooltip.value?.contains(event.target as Node)) hideTooltip();
}
function dismissOnEscape(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  event.preventDefault();
  event.stopPropagation();
  hideTooltip();
}
function togglePinned(event: Event) {
  if (!props.pinnable) return;
  event.preventDefault();
  event.stopPropagation();
  if (pinned.value) { hideTooltip(); return; }
  pinned.value = true;
  void showTooltip();
  window.addEventListener("pointerdown", dismissOutside, true);
  window.addEventListener("keydown", dismissOnEscape, true);
}
function triggerKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" || event.key === " ") togglePinned(event);
}

onBeforeUnmount(hideTooltip);
</script>

<template>
  <span
    ref="trigger"
    class="help-tooltip"
    :class="{ 'help-tooltip-error': tone === 'error' }"
    tabindex="0"
    :role="pinnable ? 'button' : undefined"
    :aria-expanded="pinnable ? pinned : undefined"
    :aria-label="label"
    :aria-describedby="visible ? tooltipId : undefined"
    @mouseenter="showTooltip"
    @mouseleave="hideUnlessPinned"
    @focus="showTooltip"
    @blur="hideUnlessPinned"
    @click="togglePinned"
    @keydown="triggerKeydown"
    @mousedown.prevent
  >
    <svg viewBox="0 0 20 20" aria-hidden="true">
      <circle cx="10" cy="10" r="7.5" />
      <path v-if="tone === 'error'" d="M10 5.8v5.5M10 14v.01" />
      <path v-else d="M7.9 7.8a2.25 2.25 0 0 1 4.3.9c0 1.55-2.2 1.75-2.2 3.2M10 14.6v.01" />
    </svg>
  </span>
  <Teleport :to="tooltipTarget">
    <span v-if="visible" :id="tooltipId" ref="tooltip" class="help-tooltip-content" :class="{ positioned, pinned }" role="tooltip" :style="position"><slot>{{ text }}</slot></span>
  </Teleport>
</template>

<style scoped>
.help-tooltip-error,.help-tooltip-error:hover,.help-tooltip-error:focus { color:var(--danger); }
.help-tooltip-content.pinned { pointer-events:auto; user-select:text; }
</style>
