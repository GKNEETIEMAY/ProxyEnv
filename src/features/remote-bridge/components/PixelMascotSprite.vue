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
  gameInput?: "left" | "right" | "up" | "down" | "action";
}>();
const emit = defineEmits<{ complete: [sceneId: number | undefined] }>();
const generation = props.sceneId;
const iterations = { idle:2, coding:6, sleep:2, music:6, coffee:2, reading:1, plant:2, game:12 };
const cycleSeconds = { idle:2.8, coding:.7, sleep:2.2, music:.65, coffee:2.8, reading:5.6, plant:2.8, game:.7 };
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
        <g v-if="action === 'sleep'" class="sleep-bed">
          <path class="pillow" d="M18 62h8v-4h28v4h12v24H18v-4h-4V66h4Z" />
          <path class="pillow-fold" d="M22 66h4v12h-4Zm4 12h28v4H26Z" />
          <path class="mattress" d="M14 98h100v8H14Z" />
        </g>
        <template v-if="kind === 'claude'">
          <g class="claude-body">
            <path class="claude-silhouette" d="M24 28h80v16h12v24h-12v12h-8v12h-8V80H76v12h-8V80H52v12h-8V80H32v12h-8V68H12V44h12Z" />
            <path class="claude-shade" d="M24 32h8v44h64v4H24V68H12V48h12Z" />
            <rect class="claude-highlight" x="32" y="32" width="64" height="4" />
            <rect class="claude-eye" x="44" y="44" width="8" height="12" />
            <rect class="claude-eye" x="80" y="44" width="8" height="12" />
            <path v-if="action === 'sleep'" class="sleep-face" d="M42 49h4v3h8v-3h4m20 0h4v3h8v-3h4" />
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
          </g>
          <g v-if="action === 'music'" class="prop prop--headphones">
            <rect class="headphone-band" x="34" y="24" width="56" height="5" />
            <rect class="headphone-band" x="30" y="28" width="5" height="20" />
            <rect class="headphone-band" x="89" y="28" width="5" height="20" />
            <rect class="headphone-cup" x="26" y="42" width="10" height="20" />
            <rect class="headphone-cup" x="90" y="42" width="10" height="20" />
          </g>
        </template>
        <template v-else>
          <g class="codex-body">
            <path class="codex-outline" d="M48 12H68V16H72V20H96V24H100V28H104V52H108V56H112V76H108V84H104V88H92V100H88V104H80V108H60V104H56V100H32V96H28V92H24V68H20V64H16V44H20V36H24V32H36V20H40V16H48Z" />
            <path class="codex-blob" d="M48 16H68V20H72V24H96V28H100V52H104V56H108V76H104V84H92V88H88V100H80V104H60V100H56V96H32V92H28V68H24V64H20V44H24V36H36V32H40V20H48Z" />
            <path class="codex-highlight" d="M48 20h16v4H48v4h-4v8h-4V24h8ZM28 40h8v4h-8v8h-4v8h-4V44h4v-4Z" />
            <path class="codex-glyph" d="M44 40h8v4h4v4h4v8h-4v4h-4v4h-8v-8h4v-8h-4Z" />
            <rect class="codex-glyph" x="68" y="58" width="20" height="6" />
            <path v-if="action === 'sleep'" class="sleep-face" d="M40 49h4v3h8v-3h4m16 0h4v3h8v-3h4" />
          </g>
          <g v-if="action === 'coding'" class="prop prop--terminal">
            <rect class="terminal-back" x="58" y="64" width="58" height="28" />
            <rect class="terminal-edge" x="54" y="90" width="66" height="7" />
            <rect class="terminal-glyph" x="66" y="72" width="4" height="4" />
            <rect class="terminal-glyph" x="70" y="76" width="4" height="4" />
            <rect class="terminal-glyph" x="66" y="80" width="4" height="4" />
            <rect class="terminal-glyph" x="80" y="80" width="16" height="4" />
            <rect class="terminal-cursor" x="99" y="80" width="4" height="4" />
          </g>
          <g v-if="action === 'music'" class="prop prop--headphones">
            <rect class="headphone-band" x="34" y="18" width="56" height="5" />
            <rect class="headphone-band" x="30" y="22" width="5" height="20" />
            <rect class="headphone-band" x="89" y="22" width="5" height="20" />
            <rect class="headphone-cup" x="24" y="38" width="12" height="22" />
            <rect class="headphone-cup" x="90" y="38" width="12" height="22" />
          </g>
        </template>
        <g v-if="action === 'sleep'" class="prop prop--sleep">
          <g class="sleep-cover">
            <path class="blanket" d="M20 80h8v-4h24v-4h28v4h24v4h8v22H20Z" />
            <path class="blanket-fold" d="M28 80h24v-4h28v4h24v4H80v-4H52v4H28Z" />
            <path class="blanket-seam" d="M28 94h12v3H28Zm20 0h12v3H48Zm20 0h12v3H68Zm20 0h12v3H88Z" />
            <path class="holding-hand" d="M32 76h10v6H32Zm50 0h10v6H82Z" />
          </g>
          <path class="zzz" d="M98 28h10v3h-3v3h-3v3h6v3H98v-3h3v-3h3v-3h-6Z"/>
          <path class="zzz zzz--2" d="M112 10h10v3h-3v3h-3v3h6v3h-10v-3h3v-3h3v-3h-6Z"/>
        </g>
        <g v-if="action === 'music'" class="music-notes">
          <path class="music-note" d="M108 24h4v16h-4v4h-8v-8h8Zm4 0h12v4h-12Z" />
          <path class="music-note music-note--2" d="M8 12h4v16H8v4H0v-8h8Zm4 0h8v4h-8Z" />
        </g>
        <path v-if="action === 'coffee' && kind === 'claude'" class="sip-mouth" d="M66 65h8v3h-8Z" />
        <g v-if="action === 'coffee'" class="prop prop--coffee">
          <path class="steam" d="M94 66v-5h4v-7h-4" fill="none" stroke-width="3" />
          <rect class="cup-rim" x="88" y="72" width="22" height="4" />
          <rect class="cup" x="88" y="76" width="22" height="16" />
          <path class="cup-handle" d="M110 77h8v10h-8" fill="none" stroke-width="4" />
          <rect class="cup-shade" x="92" y="88" width="14" height="4" />
          <rect class="holding-hand" x="108" y="82" width="8" height="8" />
        </g>
        <g v-if="action === 'reading'" class="prop prop--reading">
          <!-- A normal open book: the mascot reads the inside, with both hands supporting its sides. -->
          <path class="book-page-stack" d="M35 67h17l12 4 12-4h17v5H76l-12 4-12-4H35Z" />
          <path class="book-page-edge" d="M37 70h14l13 4 13-4h14" fill="none" stroke-width="1" />
          <g class="book-turning-page">
            <path class="book-leaf" d="m64 74 12-6h16v6H76l-12 4Z" />
          </g>
          <rect class="holding-hand reading-hand" x="92" y="82" width="8" height="10" />
          <path class="book-cover" d="M32 70h20l12 4 12-4h20v26H76l-12 4-12-4H32Z" />
          <path class="book-cover-shade" d="M32 93h20l12 4 12-4h20v3H76l-12 4-12-4H32Z" />
          <path class="book-spine" d="M63 75h2v22h-2Z" />
          <rect class="holding-hand" x="28" y="82" width="8" height="10" />
        </g>
        <g v-if="action === 'plant'" class="prop prop--plant" :transform="partner ? 'translate(128 0) scale(-1 1)' : undefined">
          <g v-if="!sharedPlant" class="solo-plant">
            <rect class="plant-pot" x="94" y="82" width="24" height="8" />
            <rect class="plant-pot" x="98" y="90" width="16" height="12" />
            <path class="plant-stem" d="M106 82V62m0 9-8-8m8 12 8-8" fill="none" stroke-width="4" />
            <path class="plant-leaf" d="M88 54h12v12H88Zm20 2h14v12h-14Z" />
          </g>
          <g class="watering-can">
            <rect class="can-body" x="68" y="68" width="22" height="16" />
            <path class="can-handle" d="M68 70h-7v10h7" fill="none" stroke-width="4" />
            <path class="can-body" d="m90 70 12 5-3 5-11-5Z" />
            <rect class="holding-hand" x="61" y="72" width="8" height="7" />
            <path v-if="!sharedPlant" class="water" d="M102 80h3v4h-3Zm4 6h3v4h-3Zm-6 5h3v4h-3Z" />
          </g>
        </g>
        <g v-if="action === 'game'" class="prop prop--game" :data-input="gameInput">
          <path class="controller" d="M42 75h44v5h6v20H78v-8H50v8H36V80h6Z" />
          <path class="controller-button controller-dpad" d="M45 81h4v4h4v4h-4v4h-4v-4h-4v-4h4Z" />
          <path class="controller-button controller-action" d="M74 82h4v4h-4Zm9 7h4v4h-4Z" />
          <rect class="holding-hand game-hand game-hand--left" x="35" y="86" width="8" height="9" />
          <rect class="holding-hand game-hand game-hand--right" x="85" y="86" width="8" height="9" />
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
.pixel-mascot--idle .claude-body,.pixel-mascot--idle .codex-body { transform-box:fill-box; transform-origin:center bottom; animation:idle-breathe 2.8s ease-in-out infinite; }
.pixel-mascot--idle rect.codex-glyph { animation:idle-cursor 4.2s steps(1,end) infinite; }
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
.holding-hand { fill:var(--claude-body); }
.pixel-mascot--codex .holding-hand { fill:var(--codex-body); }
.prop--coffee { animation:coffee-sip 2.8s ease-in-out infinite; }
.sip-mouth { fill:#393536; }
.pixel-mascot--reading .claude-eye { translate:0 2px; }
.pixel-mascot--reading .codex-body { transform:translateY(2px); }
.book-turning-page { transform-origin:64px 74px; animation:page-turn 5.6s ease-in-out infinite; }
.book-leaf { fill:var(--prop-page); stroke:var(--prop-cool); stroke-width:1; }
.book-page-stack { fill:var(--prop-page); }
.book-page-edge { stroke:var(--prop-cool); }
.book-cover { fill:var(--prop-cool); }
.book-cover-shade { fill:var(--codex-edge); }
.book-spine { fill:var(--codex-edge); }
.reading-hand { animation:reading-hand 5.6s ease-in-out infinite; }
.zzz { animation:dream-rise 2.2s ease-in-out infinite; }
.zzz--2 { animation-delay:.55s; }
.headphone-band { fill:var(--prop-dark); }
.headphone-cup { fill:var(--prop-edge); }
.music-note { fill:var(--codex-edge); animation:music-note-rise 1.3s steps(4,end) infinite; }
.music-note--2 { fill:var(--claude-shade); animation-delay:-.65s; }
.blanket { fill:var(--prop-cool); }
.pixel-mascot--codex .blanket { fill:var(--codex-highlight); }
.pillow { fill:var(--prop-light); }
.pillow-fold,.mattress { fill:var(--prop-cool); }
.blanket-fold,.blanket-seam { fill:var(--codex-edge); opacity:.45; }
.sleep-face { fill:none; stroke:#393536; stroke-width:3; }
.sleep-cover { animation:sleepy-breathe 2.2s ease-in-out infinite; }
.zzz { fill:var(--codex-edge); }
.cup { fill:var(--prop-light); }
.cup-rim,.cup-shade,.can-body,.controller { fill:var(--prop-dark); }
.cup-handle,.can-handle { stroke:var(--prop-dark); }
.steam { stroke:var(--prop-warm); animation:steam-rise 1.8s ease-in-out infinite; }
.plant-pot { fill:var(--claude-shade); }
.plant-stem { stroke:var(--prop-green); }
.plant-leaf { fill:var(--prop-green); }
.water { fill:var(--prop-cool); animation:water-drop .9s steps(2,end) infinite; }
.controller-button { fill:var(--prop-light); }
.pixel-mascot--coffee .prop--coffee,.pixel-mascot--reading .prop--reading,.pixel-mascot--plant .prop--plant,.pixel-mascot--game .prop--game { opacity:1; }
.pixel-mascot--plant .watering-can { transform-origin:82px 76px; animation:watering 2.8s ease-in-out infinite; }
.pixel-mascot--game .pixel-mascot__actor { animation:scene-hold .7s linear infinite; }
.prop--game { transform:translateY(-6px); }
.pixel-mascot--claude .prop--game { transform-origin:64px 86px; transform:translateY(-10px) rotate(-6deg); }
.prop--game[data-input="action"] .controller-action,.prop--game[data-input="up"] .controller-action { fill:var(--prop-warm); }
.prop--game[data-input="left"] .controller-dpad,.prop--game[data-input="right"] .controller-dpad,.prop--game[data-input="down"] .controller-dpad { fill:var(--prop-cool); }
.prop--game[data-input="action"] .game-hand--right,.prop--game[data-input="up"] .game-hand--right { transform:translate(-3px,-2px); }
.prop--game[data-input="left"] .game-hand--left,.prop--game[data-input="right"] .game-hand--left { transform:translate(3px,-2px); }
.pixel-mascot--coding .prop--laptop,.pixel-mascot--coding .prop--terminal { opacity:1; }
.pixel-mascot--coding .pixel-mascot__actor { animation:scene-hold .7s linear infinite; }
.pixel-mascot--coding .code,.pixel-mascot--coding .terminal-glyph { animation:code-pulse .55s steps(2,end) infinite alternate; }
.pixel-mascot--sleep .prop--sleep { opacity:1; }
.pixel-mascot--sleep .pixel-mascot__actor { animation:scene-hold 2.2s linear infinite; }
.pixel-mascot--sleep .claude-eye { animation:none; opacity:0; }
.pixel-mascot--sleep .codex-glyph { opacity:0; }
.pixel-mascot--sleep .claude-body { transform:translateY(10px); }
.pixel-mascot--sleep .codex-body { transform:translate(6px,10px) scale(.9); }
.pixel-mascot--music .prop--headphones { opacity:1; }
.pixel-mascot--music .pixel-mascot__actor { animation:music-bob .65s ease-in-out infinite; }
.pixel-mascot .pixel-mascot__actor { animation-iteration-count:var(--mascot-iterations); animation-delay:var(--mascot-delay); animation-fill-mode:both; }
.is-paused :deep(*) { animation-play-state:paused !important; }
.pixel-mascot { --prop-page:#f6f0e2; }
:global(:root[data-theme="dark"] .pixel-mascot) { --claude-body:#ca9582; --claude-shade:#b68070; --claude-highlight:#dfa994; --codex-body:#879ccb; --codex-edge:#6e83b3; --codex-highlight:#aebfe1; --prop-dark:#636d7d; --prop-edge:#424c5c; --prop-screen:#242c38; --prop-light:#b9c3d2; --prop-page:#d2d7dd; --prop-cool:#93abc9; --prop-warm:#d7b68e; --prop-green:#9cae91; }
:global(:root[data-theme="dark"] .pixel-mascot .music-note) { fill:var(--codex-highlight); }
:global(:root[data-theme="dark"] .pixel-mascot .music-note--2) { fill:var(--prop-warm); }
@keyframes idle-breathe { 0%,100% { transform:scale(1); } 50% { transform:scale(1.015); } }
@keyframes idle-cursor { 0%,92%,100% { opacity:1; } 94%,96% { opacity:.45; } }
@keyframes scene-hold { from,to { transform:translate(0,0); } }
@keyframes scene-complete { from,to { opacity:1; } }
@keyframes eye-blink { 0%,92%,100% { transform:scaleY(1); } 94%,96% { transform:scaleY(.18); } }
@keyframes cursor-blink { 0%,45% { opacity:1; } 46%,100% { opacity:0; } }
@keyframes code-pulse { from { opacity:.42; } to { opacity:1; } }
@keyframes sleepy-breathe { 0%,100% { transform:translateY(0); } 50% { transform:translateY(-1px); } }
@keyframes music-bob { 0%,100% { transform:translateY(0); } 50% { transform:translateY(-2px); } }
@keyframes music-note-rise { 0% { opacity:0; transform:translateY(6px); } 20%,65% { opacity:1; } 100% { opacity:0; transform:translateY(-10px); } }
@keyframes steam-rise { from { opacity:.2; transform:translateY(2px); } to { opacity:.7; transform:translateY(-3px); } }
@keyframes water-drop { from { opacity:1; transform:translateY(0); } to { opacity:.3; transform:translateY(5px); } }
@keyframes watering { 0%,100% { transform:rotate(0deg); } 50% { transform:rotate(8deg); } }
/* The rim reaches the mouth below the eyes; the cup stays upright instead of pouring. */
@keyframes coffee-sip { 0%,18%,90%,100% { transform:translate(0,0); } 38%,62% { transform:translate(-18px,-6px); } 70% { transform:translate(-16px,-4px); } }
@keyframes page-turn { 0%,68% { transform:scaleX(1); opacity:0; } 70%,73% { transform:scaleX(1); opacity:1; } 80% { transform:translateY(-2px) scaleX(.08); opacity:1; } 86%,90% { transform:scaleX(-1); opacity:1; } 92%,100% { transform:scaleX(-1); opacity:0; } }
@keyframes reading-hand { 0%,66%,100% { transform:translate(0,0); } 70%,73%,94% { transform:translate(-6px,-10px); } 80% { transform:translate(-28px,-16px); } 86%,90% { transform:translate(-50px,-10px); } }
@keyframes dream-rise { 0% { opacity:.2; transform:translateY(3px); } 60% { opacity:.8; } 100% { opacity:0; transform:translateY(-5px); } }
@media (prefers-reduced-motion:reduce) {
  .pixel-mascot :deep(*) { animation:none !important; }
  /* Keep only the non-visual clock; override the app-wide .01ms reduced-motion rule. */
  .pixel-mascot__clock { animation-duration:var(--mascot-scene-duration) !important; animation-iteration-count:1 !important; }
}
</style>
