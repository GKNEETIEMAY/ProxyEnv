<script setup lang="ts">
import { computed } from "vue";

// Adapted from the supplied proxyenv-pixel-mascots-v2 sprites; decorative, not official logos.
// Codex contour: pixel-quantized from LobeHub's Codex SVG; see PixelMascotSprite.LICENSE.
const props = defineProps<{
  kind: "claude" | "codex";
  action: "idle" | "coding" | "sleep" | "music" | "coffee" | "reading" | "plant" | "game";
  paused?: boolean;
  delay?: number;
  sceneId?: number;
  partner?: boolean;
  sharedPlant?: boolean;
}>();
const emit = defineEmits<{ complete: [sceneId: number | undefined] }>();
const generation = props.sceneId;
const iterations = { idle:2, coding:6, sleep:2, music:6, coffee:2, reading:2, plant:2, game:6 };
const cycleSeconds = { idle:2.8, coding:.7, sleep:2.2, music:.65, coffee:2.8, reading:2.8, plant:2.8, game:.7 };
const timing = computed(() => ({
  "--mascot-iterations": iterations[props.action],
  "--mascot-delay": `${props.delay ?? 0}ms`,
  "--mascot-scene-duration": `${cycleSeconds[props.action] * iterations[props.action] * 1000 + (props.delay ?? 0)}ms`,
}));
</script>

<template>
  <span class="pixel-mascot pixel-mascot__clock" :class="[`pixel-mascot--${kind}`, `pixel-mascot--${action}`, { 'is-paused': paused, 'is-partner': partner }]" :style="timing" aria-hidden="true" @animationend.self="emit('complete', generation)">
    <svg class="pixel-mascot__svg" viewBox="0 0 128 112" aria-hidden="true" focusable="false">
      <g class="pixel-mascot__actor">
        <template v-if="kind === 'claude'">
          <g class="claude-body">
            <path class="claude-silhouette" d="M24 28h80v16h12v24h-12v12h-8v12h-8V80H76v12h-8V80H52v12h-8V80H32v12h-8V68H12V44h12Z" />
            <path class="claude-shade" d="M24 32h8v44h64v4H24V68H12V48h12Z" />
            <rect class="claude-highlight" x="32" y="32" width="64" height="4" />
            <rect class="claude-eye" x="44" y="44" width="8" height="12" />
            <rect class="claude-eye" x="80" y="44" width="8" height="12" />
          </g>
          <g v-if="action === 'coding'" class="prop prop--laptop">
            <rect class="laptop-back" x="52" y="60" width="60" height="30" />
            <rect class="laptop-edge" x="48" y="86" width="68" height="8" />
            <rect class="laptop-screen" x="60" y="66" width="44" height="16" />
            <rect class="code" x="66" y="70" width="4" height="4" />
            <rect class="code" x="72" y="74" width="4" height="4" />
            <rect class="code" x="78" y="70" width="8" height="4" />
            <rect class="code" x="88" y="74" width="8" height="4" />
            <rect class="cursor" x="98" y="70" width="3" height="8" />
            <rect class="typing-hand" x="58" y="84" width="8" height="6" /><rect class="typing-hand typing-hand--2" x="82" y="84" width="8" height="6" />
          </g>
          <g v-if="action === 'music'" class="prop prop--headphones">
            <rect class="headphone-band" x="34" y="24" width="56" height="5" />
            <rect class="headphone-band" x="30" y="28" width="5" height="20" />
            <rect class="headphone-band" x="89" y="28" width="5" height="20" />
            <rect class="headphone-cup" x="26" y="42" width="10" height="20" />
            <rect class="headphone-cup" x="90" y="42" width="10" height="20" />
            <rect class="music-note" x="105" y="24" width="5" height="16" />
            <rect class="music-note" x="110" y="24" width="10" height="5" />
            <rect class="music-note" x="101" y="38" width="9" height="6" />
          </g>
          <g v-if="action === 'sleep'" class="prop prop--sleep">
            <rect class="blanket" x="24" y="64" width="80" height="36" />
            <rect class="pillow" x="24" y="56" width="64" height="8" />
            <path class="zzz" d="M96 24h12v4h-4v4h-4v4h8v4H96v-4h4v-4h4v-4h-8Z"/>
            <path class="zzz zzz--2" d="M112 8h12v4h-4v4h-4v4h8v4h-12v-4h4v-4h4v-4h-8Z"/>
          </g>
        </template>
        <template v-else>
          <g class="codex-body">
            <path class="codex-outline" d="M48 12H68V16H72V20H96V24H100V28H104V52H108V56H112V76H108V84H104V88H92V100H88V104H80V108H60V104H56V100H32V96H28V92H24V68H20V64H16V44H20V36H24V32H36V20H40V16H48Z" />
            <path class="codex-blob" d="M48 16H68V20H72V24H96V28H100V52H104V56H108V76H104V84H92V88H88V100H80V104H60V100H56V96H32V92H28V68H24V64H20V44H24V36H36V32H40V20H48Z" />
            <path class="codex-highlight" d="M48 20h16v4H48v4h-4v8h-4V24h8ZM28 40h8v4h-8v8h-4v8h-4V44h4v-4Z" />
            <path class="codex-glyph" d="M44 40h8v4h4v4h4v8h-4v4h-4v4h-8v-8h4v-8h-4Z" />
            <rect class="codex-glyph" x="68" y="58" width="20" height="6" />
          </g>
          <g v-if="action === 'coding'" class="prop prop--terminal">
            <rect class="terminal-back" x="58" y="64" width="58" height="28" />
            <rect class="terminal-edge" x="54" y="90" width="66" height="7" />
            <rect class="terminal-glyph" x="66" y="72" width="4" height="4" />
            <rect class="terminal-glyph" x="70" y="76" width="4" height="4" />
            <rect class="terminal-glyph" x="66" y="80" width="4" height="4" />
            <rect class="terminal-glyph" x="80" y="80" width="16" height="4" />
            <rect class="terminal-cursor" x="99" y="80" width="4" height="4" />
            <rect class="typing-hand" x="64" y="89" width="8" height="5" /><rect class="typing-hand typing-hand--2" x="88" y="89" width="8" height="5" />
          </g>
          <g v-if="action === 'music'" class="prop prop--headphones">
            <rect class="headphone-band" x="34" y="18" width="56" height="5" />
            <rect class="headphone-band" x="30" y="22" width="5" height="20" />
            <rect class="headphone-band" x="89" y="22" width="5" height="20" />
            <rect class="headphone-cup" x="24" y="38" width="12" height="22" />
            <rect class="headphone-cup" x="90" y="38" width="12" height="22" />
            <rect class="music-note" x="108" y="24" width="5" height="16" />
            <rect class="music-note" x="113" y="24" width="10" height="5" />
            <rect class="music-note" x="104" y="38" width="9" height="6" />
          </g>
          <g v-if="action === 'sleep'" class="prop prop--sleep">
            <rect class="blanket codex-blanket" x="20" y="64" width="88" height="36" />
            <rect class="pillow" x="20" y="56" width="76" height="8" />
            <path class="zzz" d="M96 24h12v4h-4v4h-4v4h8v4H96v-4h4v-4h4v-4h-8Z"/>
            <path class="zzz zzz--2" d="M112 8h12v4h-4v4h-4v4h8v4h-12v-4h4v-4h4v-4h-8Z"/>
          </g>
        </template>
        <g v-if="action === 'coffee'" class="prop prop--coffee">
          <path class="steam" d="M94 38h4v-6h-4v-8h4" fill="none" stroke-width="3" />
          <rect class="cup-rim" x="88" y="46" width="26" height="6" />
          <rect class="cup" x="88" y="52" width="26" height="18" />
          <path class="cup-handle" d="M114 52h8v12h-8" fill="none" stroke-width="4" />
          <rect class="cup-shade" x="92" y="66" width="18" height="4" />
          <rect class="holding-hand" x="84" y="56" width="8" height="8" />
        </g>
        <g v-if="action === 'reading'" class="prop prop--reading">
          <path class="book-edge" d="M56 60h22l8 5 8-5h22v30H94l-8 5-8-5H56Z" />
          <path class="book-page" d="M60 64h18l6 4v22l-6-4H60Zm28 4 6-4h18v22H94l-6 4Z" />
          <path class="book-line" d="M64 70h12m-12 6h12m20-6h12m-12 6h12" stroke-width="2" />
        </g>
        <g v-if="action === 'plant'" class="prop prop--plant" :transform="partner ? 'translate(128 0) scale(-1 1)' : undefined">
          <g v-if="!sharedPlant" class="solo-plant">
            <rect class="plant-pot" x="94" y="82" width="24" height="8" />
            <rect class="plant-pot" x="98" y="90" width="16" height="12" />
            <path class="plant-stem" d="M106 82V62m0 9-8-8m8 12 8-8" fill="none" stroke-width="4" />
            <path class="plant-leaf" d="M88 54h12v12H88Zm20 2h14v12h-14Z" />
          </g>
          <g class="watering-can">
            <rect class="can-body" x="68" y="48" width="22" height="18" />
            <path class="can-handle" d="M68 52h-7v10h7" fill="none" stroke-width="4" />
            <path class="can-body" d="m90 50 10 8-4 5-8-7Z" />
            <path v-if="!sharedPlant" class="water" d="M100 65h3v4h-3Zm4 5h3v4h-3Zm-8 1h3v4h-3Z" />
          </g>
        </g>
        <g v-if="action === 'game'" class="prop prop--game">
          <path class="controller" d="M42 61h44v5h6v22H78v-8H50v8H36V66h6Z" />
          <path class="controller-button" d="M45 67h4v4h4v4h-4v4h-4v-4h-4v-4h4Zm29 1h4v4h-4Zm9 7h4v4h-4Z" />
        </g>
      </g>
    </svg>
  </span>
</template>

<style scoped>
.pixel-mascot { --claude-body:#e5a08a; --claude-shade:#d58c77; --claude-highlight:#efb29c; --codex-body:#91a8df; --codex-edge:#7c95cc; --codex-highlight:#bbcced; --prop-dark:#525660; --prop-edge:#41464f; --prop-screen:#343a43; --prop-light:#e1e4e5; --prop-cool:#aac6de; --prop-warm:#e0b89b; --prop-green:#99b48e; display:inline-flex; width:128px; height:112px; flex:none; align-items:center; justify-content:center; line-height:0; user-select:none; pointer-events:none; }
.pixel-mascot__svg { width:100%; height:100%; overflow:visible; shape-rendering:crispEdges; image-rendering:pixelated; }
/* An HTML clock owns scene completion, independent of SVG motion and its cancellation. */
.pixel-mascot__clock { animation:scene-complete var(--mascot-scene-duration) linear 1 both; }
.pixel-mascot__clock.is-paused { animation-play-state:paused !important; }
.pixel-mascot__actor { transform-origin:64px 60px; animation:scene-hold 2.8s linear infinite; }
.pixel-mascot--idle .pixel-mascot__actor { animation-name:pixel-float; animation-timing-function:ease-in-out; }
.claude-silhouette { fill:var(--claude-body); }
.claude-body .claude-shade { fill:var(--claude-shade); }
.claude-body .claude-highlight { fill:var(--claude-highlight); }
.claude-body .claude-eye { fill:#393536; transform-box:fill-box; transform-origin:center; animation:eye-blink 4.2s steps(1,end) infinite; }
.laptop-back { fill:var(--prop-dark); }
.laptop-edge { fill:var(--prop-edge); }
.laptop-screen { fill:var(--prop-screen); }
.code,.cursor { fill:var(--prop-green); }
.cursor { animation:cursor-blink .8s steps(1,end) infinite; }
.codex-outline { fill:var(--codex-edge); }
.codex-blob { fill:var(--codex-body); }
.codex-highlight { fill:var(--codex-highlight); }
.codex-glyph { fill:#f6f7fc; }
.terminal-back { fill:var(--prop-dark); }
.terminal-edge { fill:var(--prop-edge); }
.terminal-glyph,.terminal-cursor { fill:var(--prop-cool); }
.terminal-cursor { animation:cursor-blink .75s steps(1,end) infinite; }
.prop { opacity:1; }
.holding-hand,.typing-hand { fill:var(--claude-body); }
.pixel-mascot--codex .holding-hand,.pixel-mascot--codex .typing-hand { fill:var(--codex-body); }
.typing-hand { animation:typing .35s steps(1,end) infinite alternate; }
.typing-hand--2 { animation-delay:.175s; }
.prop--coffee { transform-origin:96px 64px; animation:coffee-sip 2.8s ease-in-out infinite; }
.book-page { transform-origin:86px 76px; animation:page-turn 2.8s ease-in-out infinite; }
.zzz { animation:dream-rise 2.2s ease-in-out infinite; }
.zzz--2 { animation-delay:.55s; }
.headphone-band { fill:var(--prop-dark); }
.headphone-cup { fill:var(--prop-edge); }
.music-note { fill:var(--prop-warm); }
.blanket { fill:var(--prop-cool); }
.codex-blanket { fill:var(--codex-highlight); }
.pillow { fill:var(--prop-light); }
.zzz { fill:var(--codex-edge); }
.cup,.book-page { fill:var(--prop-light); }
.cup-rim,.cup-shade,.can-body,.controller { fill:var(--prop-dark); }
.cup-handle,.can-handle { stroke:var(--prop-dark); }
.steam { stroke:var(--prop-warm); animation:steam-rise 1.8s ease-in-out infinite; }
.book-edge { fill:var(--prop-cool); }
.book-line { stroke:var(--codex-edge); }
.plant-pot { fill:var(--claude-shade); }
.plant-stem { stroke:var(--prop-green); }
.plant-leaf { fill:var(--prop-green); }
.water { fill:var(--prop-cool); animation:water-drop .9s steps(2,end) infinite; }
.controller-button { fill:var(--prop-light); }
.pixel-mascot--coffee .prop--coffee,.pixel-mascot--reading .prop--reading,.pixel-mascot--plant .prop--plant,.pixel-mascot--game .prop--game { opacity:1; }
.pixel-mascot--plant .watering-can { transform-origin:82px 57px; animation:watering 2.8s steps(2,end) infinite; }
.pixel-mascot--game .pixel-mascot__actor { animation:scene-hold .7s linear infinite; }
.pixel-mascot--game .controller-button { animation:code-pulse .7s steps(2,end) infinite alternate; }
.pixel-mascot--coding .prop--laptop,.pixel-mascot--coding .prop--terminal { opacity:1; }
.pixel-mascot--coding .pixel-mascot__actor { animation:scene-hold .7s linear infinite; }
.pixel-mascot--coding .code,.pixel-mascot--coding .terminal-glyph { animation:code-pulse .55s steps(2,end) infinite alternate; }
.pixel-mascot--sleep .prop--sleep { opacity:1; }
.pixel-mascot--sleep .pixel-mascot__actor { animation:sleepy-breathe 2.2s ease-in-out infinite; }
.pixel-mascot--sleep .claude-eye { animation:none; transform:scaleY(.18); }
.pixel-mascot--sleep .codex-glyph { opacity:.35; }
.pixel-mascot--sleep .claude-body { transform:translateY(16px) scaleY(.75); }
.pixel-mascot--sleep .codex-body { transform:translate(16px,12px) scale(.75); }
.pixel-mascot--music .prop--headphones { opacity:1; }
.pixel-mascot--music .pixel-mascot__actor { animation:music-bob .65s ease-in-out infinite; }
.pixel-mascot .pixel-mascot__actor { animation-iteration-count:var(--mascot-iterations); animation-delay:var(--mascot-delay); animation-fill-mode:both; }
.is-paused :deep(*) { animation-play-state:paused !important; }
:global(:root[data-theme="dark"] .pixel-mascot) { --claude-body:#ca9582; --claude-shade:#b68070; --claude-highlight:#dfa994; --codex-body:#879ccb; --codex-edge:#6e83b3; --codex-highlight:#aebfe1; --prop-dark:#8e949e; --prop-edge:#727b89; --prop-screen:#38414e; --prop-light:#c3c8d0; --prop-cool:#93abc9; --prop-warm:#c9aa8c; --prop-green:#9cae91; }
@keyframes pixel-float { 0%,100% { transform:translateY(0); } 50% { transform:translateY(-3px); } }
@keyframes scene-hold { from,to { transform:translate(0,0); } }
@keyframes scene-complete { from,to { opacity:1; } }
@keyframes eye-blink { 0%,92%,100% { transform:scaleY(1); } 94%,96% { transform:scaleY(.18); } }
@keyframes cursor-blink { 0%,45% { opacity:1; } 46%,100% { opacity:0; } }
@keyframes code-pulse { from { opacity:.42; } to { opacity:1; } }
@keyframes sleepy-breathe { 0%,100% { transform:translateY(1px) scaleY(1); } 50% { transform:translateY(3px) scaleY(.97); } }
@keyframes music-bob { 0%,100% { transform:translateY(0) rotate(0deg); } 50% { transform:translateY(-3px) rotate(-1deg); } }
@keyframes steam-rise { from { opacity:.2; transform:translateY(2px); } to { opacity:.7; transform:translateY(-3px); } }
@keyframes water-drop { from { opacity:1; transform:translateY(0); } to { opacity:.3; transform:translateY(5px); } }
@keyframes watering { 0%,100% { transform:rotate(0deg); } 50% { transform:rotate(8deg); } }
@keyframes typing { from { transform:translateY(0); } to { transform:translateY(-2px); } }
@keyframes coffee-sip { 0%,20%,100% { transform:translate(0,0) rotate(0); } 40%,65% { transform:translate(-24px,-8px) rotate(-8deg); } }
@keyframes page-turn { 0%,30%,100% { transform:scaleX(1); } 50%,60% { transform:scaleX(.88); } }
@keyframes dream-rise { 0% { opacity:.2; transform:translateY(3px); } 60% { opacity:.8; } 100% { opacity:0; transform:translateY(-5px); } }
@media (prefers-reduced-motion:reduce) {
  .pixel-mascot :deep(*) { animation:none !important; }
  /* Keep only the non-visual clock; override the app-wide .01ms reduced-motion rule. */
  .pixel-mascot__clock { animation-duration:var(--mascot-scene-duration) !important; animation-iteration-count:1 !important; }
}
</style>
