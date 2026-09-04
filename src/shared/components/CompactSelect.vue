<script setup lang="ts" generic="T extends string">
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from "vue";

const props = defineProps<{ label: string; options: { value: T; label: string }[]; autofocus?: boolean; disabled?: boolean }>();
const model = defineModel<T>({ required: true });
const root = ref<HTMLElement>();
const trigger = ref<HTMLButtonElement>();
const list = ref<HTMLElement>();
const opened = ref(false);
const highlighted = ref(0);
const id = `select-${useId().replaceAll(":", "")}`;
const selected = computed(() => props.options.find(option => option.value === model.value));
let search = "";
let lastKeyTime = 0;

function close() {
  opened.value = false;
  document.removeEventListener("pointerdown", outside, true);
}
function outside(event: Event) {
  if (!root.value?.contains(event.target as Node)) close();
}
function reveal() {
  list.value?.children[highlighted.value]?.scrollIntoView({ block: "nearest" });
}
async function open(last = false) {
  if (props.disabled || !props.options.length) return;
  highlighted.value = last ? props.options.length - 1 : Math.max(0, props.options.findIndex(option => option.value === model.value));
  search = "";
  opened.value = true;
  document.addEventListener("pointerdown", outside, true);
  await nextTick();
  if (opened.value) reveal();
}
function choose(index: number) {
  const option = props.options[index];
  if (props.disabled || !option) return;
  model.value = option.value;
  close();
  trigger.value?.focus({ preventScroll: true });
}
function keydown(event: KeyboardEvent) {
  if (event.key === "Tab") { close(); return; }
  if (event.key === "Escape" && opened.value) {
    event.preventDefault(); event.stopPropagation(); close(); return;
  }
  if (["ArrowDown", "ArrowUp", "Home", "End", "Enter", " "].includes(event.key)) {
    event.preventDefault();
    if (!opened.value) { void open(event.key === "End"); return; }
    if (event.key === "Enter" || event.key === " ") { choose(highlighted.value); return; }
    if (event.key === "Home") highlighted.value = 0;
    else if (event.key === "End") highlighted.value = props.options.length - 1;
    else highlighted.value = (highlighted.value + (event.key === "ArrowDown" ? 1 : -1) + props.options.length) % props.options.length;
    reveal();
  } else if (opened.value && event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
    const now = Date.now();
    search = (now - lastKeyTime > 700 ? "" : search) + event.key.toLocaleLowerCase();
    lastKeyTime = now;
    const index = props.options.findIndex(option => option.label.toLocaleLowerCase().startsWith(search));
    if (index >= 0) { highlighted.value = index; reveal(); }
  }
}
watch(() => JSON.stringify([model.value, props.disabled, props.options.map(option => option.value)]), close);
onBeforeUnmount(close);
</script>

<template>
  <div ref="root" class="compact-select">
    <button ref="trigger" class="compact-select-trigger" type="button" role="combobox" aria-haspopup="listbox"
      :aria-label="label" :title="selected?.label" :aria-expanded="opened" :aria-controls="opened ? id : undefined"
      :aria-activedescendant="opened ? `${id}-${highlighted}` : undefined"
      :autofocus="autofocus" :disabled="disabled || !options.length" @blur="close" @keydown="keydown" @click="opened ? close() : open()">
      <span>{{ selected?.label }}</span>
      <svg :class="{ expanded: opened }" viewBox="0 0 20 20" aria-hidden="true"><path d="m6 8 4 4 4-4" /></svg>
    </button>
    <!-- Stay inside the dialog's top layer; a body teleport would become inert behind it. -->
    <div v-if="opened" :id="id" ref="list" class="compact-select-list" role="listbox" :aria-label="label" @mousedown.prevent>
      <div v-for="(option, index) in options" :id="`${id}-${index}`" :key="option.value" role="option"
        :aria-selected="option.value === model" :class="{ highlighted: index === highlighted }"
        @mouseenter="highlighted = index" @click="choose(index)">
        <span>{{ option.label }}</span>
        <svg v-if="option.value === model" viewBox="0 0 20 20" aria-hidden="true"><path d="m4 10 4 4 8-8" /></svg>
      </div>
    </div>
  </div>
</template>

<style scoped>
.compact-select { position: relative; min-width: 0; }
.compact-select-trigger { display: flex; width: 100%; min-height: 36px; align-items: center; justify-content: space-between; gap: 12px; padding: 8px 11px; border: 1px solid var(--line-strong); border-radius: 10px; color: var(--ink); background: var(--surface); font: inherit; text-align: start; cursor: pointer; }
.compact-select-trigger:hover:not(:disabled), .compact-select-trigger[aria-expanded="true"] { border-color: var(--accent); }
.compact-select-trigger:disabled { opacity: .5; cursor: default; }
.compact-select-trigger > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
svg { width: 16px; height: 16px; flex: none; fill: none; stroke: currentColor; stroke-width: 1.55; stroke-linecap: round; stroke-linejoin: round; }
.compact-select-trigger svg { color: var(--muted); }
.expanded { transform: rotate(180deg); }
.compact-select-list { position: absolute; z-index: 5; top: calc(100% + 6px); inset-inline: 0; max-height: min(240px, 40dvh); overflow-y: auto; overscroll-behavior: contain; padding: 5px; border-radius: 12px; background: var(--surface-strong); box-shadow: 0 8px 24px color-mix(in srgb, var(--ink) 18%, transparent); scrollbar-width: thin; scrollbar-color: var(--line-strong) transparent; }
.compact-select-list > div { display: flex; min-height: 34px; align-items: center; justify-content: space-between; gap: 8px; padding: 8px; border-radius: 8px; cursor: pointer; overflow-wrap: anywhere; }
.compact-select-list > div.highlighted { background: var(--surface); outline: 1px solid var(--line-strong); outline-offset: -1px; }
.compact-select-list > div[aria-selected="true"] { background: var(--accent-soft); color: var(--accent-strong); }
</style>
