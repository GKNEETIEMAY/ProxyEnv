<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, useId } from "vue";

defineProps<{
  label: string;
  text: string;
}>();

const tooltipId = `help-${useId().replaceAll(":", "")}`;
const trigger = ref<HTMLElement>();
const tooltip = ref<HTMLElement>();
const visible = ref(false);
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
  placeTooltip();
  window.addEventListener("resize", placeTooltip);
  window.addEventListener("scroll", placeTooltip, true);
}

function hideTooltip() {
  visible.value = false;
  positioned.value = false;
  window.removeEventListener("resize", placeTooltip);
  window.removeEventListener("scroll", placeTooltip, true);
}

onBeforeUnmount(hideTooltip);
</script>

<template>
  <span
    ref="trigger"
    class="help-tooltip"
    tabindex="0"
    :aria-label="label"
    :aria-describedby="visible ? tooltipId : undefined"
    @mouseenter="showTooltip"
    @mouseleave="hideTooltip"
    @focus="showTooltip"
    @blur="hideTooltip"
    @mousedown.prevent
  >
    <svg viewBox="0 0 20 20" aria-hidden="true">
      <circle cx="10" cy="10" r="7.5" />
      <path d="M7.9 7.8a2.25 2.25 0 0 1 4.3.9c0 1.55-2.2 1.75-2.2 3.2M10 14.6v.01" />
    </svg>
  </span>
  <Teleport to="body">
    <span v-if="visible" :id="tooltipId" ref="tooltip" class="help-tooltip-content" :class="{ positioned }" role="tooltip" :style="position">{{ text }}</span>
  </Teleport>
</template>
