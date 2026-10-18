<script setup lang="ts">
import { computed, ref } from "vue";
import { advanceGame, createGame, jumpPose, snakeCells, snakeRoute, type GameInput } from "../mascotGame";

const props = defineProps<{ duo: boolean; paused: boolean; kind: "claude" | "codex" }>();
const emit = defineEmits<{ input: [inputs: [GameInput, GameInput]] }>();
const frame = ref(createGame(props.duo));
const cells = computed(() => snakeCells(frame.value));
const food = computed(() => snakeRoute[frame.value.food]!);
const jumper = computed(() => jumpPose(frame.value.tick));
const phase = computed(() => frame.value.health.includes(0) ? 0 : frame.value.tick % 3);
function tick() {
  if (props.paused) return;
  frame.value = advanceGame(frame.value);
  emit("input", frame.value.inputs);
}
function fighterX(player: number) {
  if (frame.value.health.includes(0)) return player ? 70 : 16;
  const moving = phase.value === 1 || phase.value === 2;
  return player ? moving ? 60 : 70 : moving ? 26 : 16;
}
</script>

<template>
  <span class="pixel-game" :class="{ 'is-paused': paused }" :data-game="frame.mode" :data-tick="frame.tick" aria-hidden="true">
    <!-- A CSS clock pauses with the scene. Reduced motion has no game ticks. -->
    <span class="pixel-game-clock" @animationiteration.self="tick" />
    <svg viewBox="0 0 112 88" focusable="false">
      <g v-if="frame.mode !== 'duel'" class="game-monitor">
        <path class="screen-frame" d="M0 0h112v80H64v4h12v4H36v-4h12v-4H0Z" />
        <path class="screen" d="M4 4h104v70H4Z" />
        <path class="screen-led" d="M99 76h4v2h-4Z" />
      </g>
      <g v-if="frame.mode === 'duel'" class="pixel-arena">
        <path class="arena-floor" d="M8 64h96v3H8Z" />
        <rect class="health-track" x="10" y="10" width="38" height="7" /><rect class="health-track" x="64" y="10" width="38" height="7" />
        <rect class="player-health player-health--claude" x="11" y="11" :width="36 * frame.health[0] / 100" height="5" :data-health="frame.health[0]" />
        <rect class="player-health player-health--codex" :x="101 - 36 * frame.health[1] / 100" y="11" :width="36 * frame.health[1] / 100" height="5" :data-health="frame.health[1]" />
        <g class="pixel-fighter pixel-fighter--claude" :class="{ 'is-hit': frame.damaged === 0 }" :transform="`translate(${fighterX(0)} 0)`">
          <path d="M0 34h20v4h4v12h-4v4h-4v10h-5V54H5v10H0V50h-4V38h4Z" />
          <path class="fighter-face" d="M4 39h3v4H4Zm10 0h3v4h-3Z" />
          <path class="fighter-arm" :d="frame.attacker === 0 && phase ? 'M20 45h14v5H20Z' : 'M20 46h6v5h-6Z'" />
          <path class="fighter-foot" :transform="`translate(${frame.tick % 2 ? 2 : 0} 0)`" d="M-1 60h6v4h-6Zm12 0h6v4h-6Z" />
        </g>
        <g class="pixel-fighter pixel-fighter--codex" :class="{ 'is-hit': frame.damaged === 1 }" :transform="`translate(${fighterX(1)} 0)`">
          <path d="M4 32h12v4h8v4h4v14h-4v4h-8v6H8v-6H0v-4h-4V40h4v-4h4Z" />
          <path class="fighter-glyph" d="m4 40 4 4-4 4v-4Zm9 8h7v3h-7Z" />
          <path class="fighter-arm" :d="frame.attacker === 1 && phase ? 'M-14 45H0v5h-14Z' : 'M-6 46H0v5h-6Z'" />
          <path class="fighter-foot" :transform="`translate(${frame.tick % 2 ? -2 : 0} 0)`" d="M3 60h6v4H3Zm10 0h6v4h-6Z" />
        </g>
        <g v-if="frame.damaged !== null" class="game-impact" :transform="`translate(${frame.damaged === 0 ? 31 : 67} 39)`">
          <path d="M-6-2h6v-6h4v6h6v4H4v6H0V2h-6Z" />
          <text x="0" y="-12" text-anchor="middle">−{{ frame.damage }}</text>
        </g>
        <path v-if="frame.health.includes(0)" class="winner-spark" :transform="`translate(${frame.health[0] ? 26 : 80} 24)`" d="M-8-2h6v-6h4v6h6v4H2v6h-4V2h-6Z" />
      </g>
      <g v-else-if="frame.mode === 'snake'">
        <text class="game-score" x="9" y="14">{{ String(frame.score).padStart(2, '0') }}</text>
        <rect v-for="(cell, index) in cells" :key="index" :class="['snake-cell', { 'snake-head': index === 0 }]" :x="8 + cell[0] * 6" :y="14 + cell[1] * 6" width="5" height="5" />
        <path class="snake-eye" :transform="`translate(${8 + cells[0]![0] * 6} ${14 + cells[0]![1] * 6})`" d="M1 1h1v1H1Zm2 0h1v1H3Z" />
        <rect class="game-food" :x="8 + food[0] * 6" :y="14 + food[1] * 6" width="5" height="5" />
        <rect class="food-leaf" :x="10 + food[0] * 6" :y="12 + food[1] * 6" width="3" height="2" />
      </g>
      <g v-else class="pixel-platformer">
        <text class="game-score" x="9" y="14">{{ String(frame.score).padStart(2, '0') }}</text>
        <path class="arena-floor" d="M8 64h96v4H8Z" />
        <path class="game-platform" d="M45 49h21v4H45Z" />
        <path v-if="frame.tick % 8 < 4" class="game-food" d="M54 23h6v3h3v6h-3v3h-6v-3h-3v-6h3Z" />
        <path v-else class="collected-spark" d="M51 21h3v4h-3Zm12 0h3v4h-3Zm-6-6h3v4h-3Z" />
        <g class="game-runner" :class="`game-runner--${kind}`" :transform="`translate(${jumper.x} ${jumper.y})`">
          <path d="M0-10h8v3h3v7H8v4H5V0H2v4h-3V-7h1Z" />
          <path class="fighter-face" d="M5-7h2v2H5Z" />
          <path class="fighter-foot" :d="frame.tick % 2 ? 'M-2 2h4v2h-4Zm8-1h4v2H6Z' : 'M0 1h4v2H0Zm5 2h4v2H5Z'" />
        </g>
        <path class="game-obstacle" :transform="`translate(${70 - frame.tick % 8 * 2} 59)`" d="M0-4h8v8H0Zm-2 8h4v2h-4Zm8 0h4v2H6Z" />
      </g>
    </svg>
  </span>
</template>

<style scoped>
.pixel-game { --game-bezel:#626875; --game-screen:#edf0ef; --game-rule:#b7c0c2; --game-ink:#414b59; --game-claude:#d98f77; --game-codex:#7c97ce; --game-food:#dca35e; --game-snake:#789a79; position:absolute; bottom:18px; right:8px; width:120px; height:94px; line-height:0; }
.pixel-game svg { width:100%; height:100%; overflow:visible; shape-rendering:crispEdges; }
.pixel-game-clock { position:absolute; width:0; height:0; animation:game-tick .4s linear infinite; }
.screen-frame { fill:var(--game-bezel); }
.screen { fill:var(--game-screen); }
.arena-floor,.health-track,.game-platform { fill:var(--game-rule); }
.player-health--claude,.pixel-fighter--claude,.game-runner--claude { fill:var(--game-claude); }
.player-health--codex,.pixel-fighter--codex,.game-runner--codex { fill:var(--game-codex); }
.player-health { transition:width .15s linear,x .15s linear; }
.fighter-face,.fighter-foot,.snake-eye { fill:var(--game-ink); }
.fighter-glyph { fill:#f6f7fc; }
.is-hit { opacity:.45; }
.game-impact,.game-food,.winner-spark,.collected-spark { fill:var(--game-food); }
.game-impact text,.game-score { fill:var(--game-ink); font-family:ui-monospace,monospace; font-size:8px; font-weight:700; }
.snake-cell,.food-leaf { fill:var(--game-snake); }
.snake-head { stroke:var(--game-ink); stroke-width:.5; }
.snake-eye { fill:var(--game-screen); }
.game-obstacle { fill:var(--game-codex); }
.screen-led { fill:var(--game-snake); }
.is-paused .pixel-game-clock { animation-play-state:paused !important; }
:global(:root[data-theme="dark"] .pixel-game) { --game-bezel:#667285; --game-screen:#242c38; --game-rule:#465366; --game-ink:#d7dfe8; --game-claude:#d9a08b; --game-codex:#9fb6e5; --game-food:#e1bd82; --game-snake:#9dbd9a; }
:global(:root[data-theme="dark"] .snake-eye) { fill:#242c38; }
@keyframes game-tick { from,to { opacity:1; } }
@media (prefers-reduced-motion:reduce) { .pixel-game-clock { animation:none !important; } .player-health { transition:none !important; } }
</style>
