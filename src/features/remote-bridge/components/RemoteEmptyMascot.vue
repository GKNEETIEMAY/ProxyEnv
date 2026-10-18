<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import PixelMascotSprite from "./PixelMascotSprite.vue";
import PixelMascotGame from "./PixelMascotGame.vue";
import type { GameInput } from "../mascotGame";

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
const gameInputs = ref<[GameInput, GameInput]>(["right", "left"]);
const switching = ref(false);
const pendingNext = ref(false);
const stage = ref<HTMLElement>();
const onScreen = ref(false);
const documentVisible = ref(!document.hidden);
const paused = computed(() => props.paused || !onScreen.value || !documentVisible.value);
const completedActors = new Set<string>();
let observer: IntersectionObserver | undefined;

function advanceScene() {
  if (switching.value || paused.value) { pendingNext.value = true; return; }
  pendingNext.value = false;
  completedActors.clear();
  sceneId.value += 1;
  gameInputs.value = ["right", "left"];
  // Every third completed scene is a duet. Never interrupt an actor mid-action.
  const choices = sceneId.value % 3 === 2 ? duoScenes : soloScenes.filter(candidate =>
    candidate.action !== scene.value.action || candidate.kinds[0] !== scene.value.kinds[0]);
  scene.value = randomChoice(choices);
}
function finishActor(kind: string, generation: number | undefined) {
  if (generation !== sceneId.value || completedActors.has(kind)) return;
  completedActors.add(kind);
  if (completedActors.size === scene.value.kinds.length) advanceScene();
}
watch([paused, switching], ([isPaused, isSwitching]) => { if (!isPaused && !isSwitching && pendingNext.value) advanceScene(); });
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
    <Transition name="pixel-scene" @before-leave="switching = true" @after-enter="switching = false">
      <span :key="sceneId" class="pixel-scene" :class="{ 'pixel-scene--duo': scene.kinds.length === 2, 'pixel-scene--game': scene.action === 'game' }" :data-scene="sceneId" :data-action="scene.action">
        <PixelMascotSprite v-for="(kind, index) in scene.kinds" :key="kind" :kind="kind" :action="scene.action"
          :paused="paused" :scene-id="sceneId" :partner="index === 1" :shared-plant="scene.kinds.length === 2 && scene.action === 'plant'" :delay="scene.kinds.length > 1 ? index * 180 : 0"
          :game-input="gameInputs[index]"
          @complete="finishActor(kind, $event)" />
        <svg v-if="scene.kinds.length === 2 && scene.action === 'plant'" class="pixel-shared-prop pixel-shared-plant" viewBox="0 0 96 80" aria-hidden="true">
          <g class="shared-water shared-water--left"><path d="M-20 32h4v4h-4Zm16 2h4v4h-4Zm16 4h4v4h-4Zm16 8h4v4h-4Zm12 6h4v4h-4Z"/></g>
          <g class="shared-water shared-water--right"><path d="M112 32h4v4h-4Zm-16 2h4v4h-4Zm-16 4h4v4h-4Zm-16 8h4v4h-4Zm-12 6h4v4h-4Z"/></g>
          <g class="shared-flower"><path class="shared-stem" d="M46 32h4v28h-4Z"/><path class="shared-leaf" d="M34 40h8v4h4v8h-8v-4h-4Zm16 8h4v-4h8v8h-4v4h-8Z"/><path class="shared-petals" d="M40 24h4v-4h8v4h4v8h-4v4h-8v-4h-4Z"/><rect class="flower-center" x="44" y="24" width="8" height="8"/></g>
          <g class="shared-pot"><path d="M34 58h28v8H34Zm4 8h20v12H38Z"/></g>
        </svg>
        <PixelMascotGame v-else-if="scene.action === 'game'" :duo="scene.kinds.length === 2" :kind="scene.kinds[0]!" :paused="paused" @input="gameInputs = $event" />
        <svg v-else-if="scene.kinds.length === 2" class="pixel-duet-signal" :class="`pixel-duet-signal--${scene.action}`"
          viewBox="0 0 40 24" aria-hidden="true" focusable="false">
          <g v-if="scene.action === 'music'">
            <rect x="16" y="3" width="4" height="15" /><rect x="20" y="3" width="10" height="4" /><rect x="10" y="15" width="10" height="5" />
          </g>
          <g v-else-if="scene.action === 'sleep'">
            <path d="M14 4h12v4h-4v4h-4v4h8v4H14v-4h4v-4h4V8h-8Z" />
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
.remote-empty-mascot { --scene-claude:#e5a08a; --scene-codex:#91a8df; --scene-leaf:#7f9d73; --scene-water:#8db6d3; display:grid; width:min(288px,100%); height:144px; margin-top:12px; place-items:center; user-select:none; pointer-events:none; }
.pixel-scene { grid-area:1 / 1; position:relative; display:flex; width:100%; height:100%; align-items:center; justify-content:center; }
.pixel-scene--duo { justify-content:space-between; }
.pixel-scene--duo :deep(.pixel-mascot) { width:96px; height:84px; }
.pixel-scene--game:not(.pixel-scene--duo) { justify-content:flex-start; }
.pixel-scene--game:not(.pixel-scene--duo) :deep(.pixel-mascot) { margin-left:8px; }
.pixel-scene--duo :deep(.pixel-game) { width:108px; height:85px; right:calc(50% - 54px); bottom:20px; }
.pixel-duet-signal { position:absolute; top:30%; left:calc(50% - 16px); width:32px; height:20px; fill:#d7b393; shape-rendering:crispEdges; animation:pixel-duet-exchange 1.4s ease-in-out infinite alternate; }
.pixel-duet-signal--sleep { fill:#a4b8d9; }
.pixel-duet-signal--music { animation-name:pixel-duet-note; }
.pixel-shared-prop { position:absolute; left:calc(50% - 48px); width:96px; height:80px; overflow:visible; shape-rendering:crispEdges; }
.pixel-shared-plant { bottom:8px; }
.shared-water { fill:var(--scene-water); animation:shared-watering 1.4s steps(3,end) infinite; }
.shared-water--right { animation-delay:.18s; }
.shared-stem,.shared-leaf { fill:var(--scene-leaf); }
.shared-petals,.shared-pot { fill:var(--scene-claude); }
.flower-center { fill:#e2be77; }
.shared-flower { transform-origin:48px 60px; animation:flower-drink 2.8s ease-in-out infinite; }
:global(:root[data-theme="dark"] .remote-empty-mascot) { --scene-claude:#ca9582; --scene-codex:#879ccb; --scene-leaf:#9cae91; --scene-water:#93abc9; }
.is-paused :deep(*) { animation-play-state:paused !important; }
.pixel-scene-enter-active { transition:opacity 180ms ease-out; }
.pixel-scene-leave-active { transition:opacity 120ms ease-in; }
.pixel-scene-enter-from,.pixel-scene-leave-to { opacity:0; }
@keyframes pixel-duet-exchange { from { opacity:.45; transform:translateX(-6px); } to { opacity:.85; transform:translateX(6px); } }
@keyframes pixel-duet-note { from { opacity:.5; transform:translateY(2px); } to { opacity:.85; transform:translateY(-3px); } }
@keyframes shared-watering { 0% { opacity:0; transform:translateY(-4px); } 30%,70% { opacity:1; } 100% { opacity:0; transform:translateY(6px); } }
@keyframes flower-drink { 0%,100% { transform:rotate(0); } 45% { transform:rotate(-4deg); } 75% { transform:rotate(3deg); } }
@media (prefers-reduced-motion:reduce) { .remote-empty-mascot :deep(*:not(.pixel-mascot__clock)) { animation:none !important; transition:none !important; } }
</style>
