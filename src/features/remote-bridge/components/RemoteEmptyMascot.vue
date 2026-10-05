<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import PixelMascotSprite from "./PixelMascotSprite.vue";

const props = defineProps<{ paused?: boolean }>();
const kinds = ["claude", "codex"] as const;
const actions = ["idle", "coding", "sleep", "music", "coffee", "reading", "plant", "game"] as const;
type Scene = { kinds: (typeof kinds)[number][]; action: (typeof actions)[number] };
const soloScenes: Scene[] = kinds.flatMap(kind => actions.map(action => ({ kinds:[kind], action })));
const duoScenes: Scene[] = (["coding", "sleep", "plant", "game", "music"] as const)
  .map(action => ({ kinds:[...kinds], action }));
function randomChoice<T>(choices: readonly T[]): T {
  return choices[Math.floor(Math.random() * choices.length)]!;
}
const scene = ref(randomChoice(soloScenes));
const sceneId = ref(0);
const switching = ref(false);
const pendingNext = ref(false);
const stage = ref<HTMLElement>();
const onScreen = ref(false);
const documentVisible = ref(!document.hidden);
const paused = computed(() => props.paused || !onScreen.value || !documentVisible.value);
let observer: IntersectionObserver | undefined;

function advanceScene() {
  if (switching.value) return;
  if (paused.value) { pendingNext.value = true; return; }
  pendingNext.value = false;
  sceneId.value += 1;
  // Every third completed scene is a duet. Never interrupt an actor mid-action.
  const choices = sceneId.value % 3 === 2 ? duoScenes : soloScenes.filter(candidate =>
    candidate.action !== scene.value.action || candidate.kinds[0] !== scene.value.kinds[0]);
  scene.value = randomChoice(choices);
}
watch(paused, value => { if (!value && pendingNext.value) advanceScene(); });
function updateVisibility() { documentVisible.value = !document.hidden; }
onMounted(() => {
  observer = new IntersectionObserver(([entry]) => { onScreen.value = !!entry?.isIntersecting; });
  if (stage.value) observer.observe(stage.value);
  document.addEventListener("visibilitychange", updateVisibility);
});
onBeforeUnmount(() => {
  observer?.disconnect();
  document.removeEventListener("visibilitychange", updateVisibility);
});
</script>

<template>
  <span ref="stage" class="remote-empty-mascot" :class="{ 'is-paused': paused }" aria-hidden="true">
    <Transition name="pixel-scene" mode="out-in" @before-leave="switching = true" @after-enter="switching = false">
      <span :key="sceneId" class="pixel-scene" :class="{ 'pixel-scene--duo': scene.kinds.length === 2 }" :data-scene="sceneId">
        <PixelMascotSprite v-for="(kind, index) in scene.kinds" :key="kind" :kind="kind" :action="scene.action"
          :paused="paused" :delay="scene.kinds.length > 1 ? index * 180 : 0"
          @complete="index === scene.kinds.length - 1 && advanceScene()" />
        <svg v-if="scene.kinds.length === 2" class="pixel-duet-signal" :class="`pixel-duet-signal--${scene.action}`"
          viewBox="0 0 40 24" aria-hidden="true" focusable="false">
          <g v-if="scene.action === 'music'">
            <rect x="16" y="3" width="4" height="15" /><rect x="20" y="3" width="10" height="4" /><rect x="10" y="15" width="10" height="5" />
          </g>
          <g v-else-if="scene.action === 'sleep'">
            <path d="M14 4h12v4h-4v4h-4v4h8v4H14v-4h4v-4h4V8h-8Z" />
          </g>
          <g v-else-if="scene.action === 'game'">
            <circle cx="20" cy="12" r="9" />
            <path class="pixel-ball-detail" d="M18 4h4v16h-4Z" />
          </g>
          <g v-else-if="scene.action === 'plant'">
            <path d="M18 22V12H8V4h8v4h4V2h12v8h-8v4h-2v8Z" />
          </g>
          <g v-else>
            <path d="M2 4h16v10h-4v4h-4v-4H2Zm20 4h16v10h-8v4h-4v-4h-4Z" />
          </g>
        </svg>
      </span>
    </Transition>
  </span>
</template>

<style scoped>
/* Keep the decorative stage geometry independent of the active scene. */
.remote-empty-mascot { display:grid; width:min(264px,100%); height:clamp(100px,18dvh,144px); margin-top:12px; place-items:center; user-select:none; pointer-events:none; }
.pixel-scene { position:relative; display:flex; width:100%; height:100%; align-items:center; justify-content:center; }
.pixel-scene--duo { justify-content:space-between; }
.pixel-scene--duo :deep(.pixel-mascot) { width:clamp(72px,13dvh,104px); height:clamp(72px,13dvh,104px); }
.pixel-duet-signal { position:absolute; top:30%; left:calc(50% - 16px); width:32px; height:20px; fill:#d7b393; shape-rendering:crispEdges; animation:pixel-duet-exchange 1.4s ease-in-out infinite alternate; }
.pixel-duet-signal--plant { fill:#99b48e; }
.pixel-duet-signal--sleep { fill:#a4b8d9; }
.pixel-duet-signal--music { animation-name:pixel-duet-note; }
.pixel-duet-signal--game { top:56%; fill:#aac6de; animation:pixel-duet-play 1.4s ease-in-out infinite alternate; }
.pixel-ball-detail { fill:#7c95cc; }
.is-paused :deep(*) { animation-play-state:paused !important; }
.pixel-scene-enter-active { transition:opacity 180ms ease-out,transform 180ms ease-out; }
.pixel-scene-leave-active { transition:opacity 120ms ease-in,transform 120ms ease-in; }
.pixel-scene-enter-from { opacity:0; transform:translateY(2px); }
.pixel-scene-leave-to { opacity:0; transform:translateY(-2px); }
@keyframes pixel-duet-exchange { from { opacity:.45; transform:translateX(-6px); } to { opacity:.85; transform:translateX(6px); } }
@keyframes pixel-duet-note { from { opacity:.5; transform:translateY(2px) rotate(-6deg); } to { opacity:.85; transform:translateY(-3px) rotate(6deg); } }
@keyframes pixel-duet-play { 0% { transform:translate(-24px,4px) rotate(-20deg); } 50% { transform:translate(0,-8px) rotate(0deg); } 100% { transform:translate(24px,4px) rotate(20deg); } }
@media (prefers-reduced-motion:reduce) { .remote-empty-mascot :deep(*) { animation:none !important; transition:none !important; } }
</style>
