import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import ts from "typescript";

const compiled = ts.transpileModule(readFileSync("src/features/remote-bridge/mascotGame.ts", "utf8"), {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
const { createGame, advanceGame, snakeCells, snakeRoute, jumpPose } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`);
const sequence = values => { let index = 0; return () => values[index++ % values.length]; };

test("solo scenes randomly choose a playable snake or jumping scene; duets always duel", () => {
  assert.equal(createGame(false, () => 0).mode, "snake");
  assert.equal(createGame(false, () => .99).mode, "jump");
  assert.equal(createGame(true, () => 0).mode, "duel");
  assert.equal(createGame(true, () => .99).mode, "duel");
});

test("duel controls, attacks and randomized HP damage share one frame, without mutating previous frames", () => {
  const random = sequence([.1, .9, .7, .3, .6, .2, .8]);
  let frame = createGame(true, random);
  const damages = new Set();
  const victims = new Set();
  for (let tick = 0; tick < 120; tick++) {
    const before = structuredClone(frame);
    const next = advanceGame(frame, random);
    assert.deepEqual(frame, before);
    for (const hp of next.health) assert.ok(hp >= 0 && hp <= 100);
    if (next.damaged !== null) {
      const defender = next.damaged;
      assert.equal(next.inputs[next.attacker], "action");
      assert.equal(next.health[defender], frame.health[defender] - next.damage);
      assert.equal(next.health[next.attacker], frame.health[next.attacker]);
      assert.equal(next.tick % 3, 2, "damage only at the contact phase");
      damages.add(next.damage); victims.add(defender);
    }
    frame = next;
  }
  assert.ok(damages.size > 1, "damage is not a static repeated health animation");
  assert.equal(victims.size, 2, "either controlled fighter can take damage");
  assert.ok(frame.health.includes(0));
  assert.deepEqual(advanceGame(frame, random).health, frame.health, "winner stays until the scene finishes");
});

test("snake moves between adjacent cells, eats, grows and never intersects itself or spawns food on its body", () => {
  for (let index = 0; index < snakeRoute.length; index++) {
    const a = snakeRoute[index], b = snakeRoute[(index + 1) % snakeRoute.length];
    assert.equal(Math.abs(a[0] - b[0]) + Math.abs(a[1] - b[1]), 1);
  }
  let frame = createGame(false, () => 0);
  for (let tick = 0; tick < 400; tick++) {
    const before = frame;
    frame = advanceGame(frame, () => .5);
    const cells = snakeCells(frame).map(cell => cell.join(","));
    assert.equal(new Set(cells).size, frame.length);
    assert.ok(!cells.includes(snakeRoute[frame.food].join(",")));
    assert.notDeepEqual(snakeCells(before)[0], snakeCells(frame)[0]);
    if (frame.score > before.score) assert.equal(frame.head, before.food);
  }
  assert.ok(frame.score > 10 && frame.length === 10);
});

test("jumping game has running, a real jump arc and coin collection synchronized with action presses", () => {
  let frame = createGame(false, () => .99);
  const heights = new Set();
  for (let tick = 0; tick < 16; tick++) {
    const before = frame;
    frame = advanceGame(frame);
    heights.add(jumpPose(frame.tick).y);
    if (frame.score > before.score) {
      assert.equal(frame.inputs[0], "action");
      assert.equal(jumpPose(frame.tick).y, 30);
    }
  }
  assert.equal(frame.score, 2); assert.ok(heights.size >= 4);
});

test("mascot props use actual page turning, upright sipping, visible notes and a paused reduced-motion game clock", () => {
  const sprite = readFileSync("src/features/remote-bridge/components/PixelMascotSprite.vue", "utf8");
  const game = readFileSync("src/features/remote-bridge/components/PixelMascotGame.vue", "utf8");
  assert.match(sprite, /class="book-turning-page"/);
  assert.match(sprite, /@keyframes page-turn[^}]+}[\s\S]*?scaleX\(-1\)/);
  assert.doesNotMatch(sprite.match(/@keyframes coffee-sip[^\n]+/)[0], /rotate/);
  assert.doesNotMatch(sprite.match(/@keyframes music-bob[^\n]+/)[0], /rotate/);
  assert.match(sprite, /music-note-rise/);
  assert.match(sprite, /class="can-body" x="68" y="68"/);
  assert.match(sprite, /controller-dpad/);
  assert.match(game, /@animationiteration\.self="tick"/);
  assert.match(game, /if \(props\.paused\) return/);
  assert.match(game, /animation-play-state:paused !important/);
  assert.match(game, /prefers-reduced-motion:reduce[\s\S]*animation:none !important/);
  assert.doesNotMatch(game, /setInterval|setTimeout|requestAnimationFrame|fetch\(|addEventListener/);
});

test("sleep keeps the actor still with closed eyes and blanket-only breathing; PK has no monitor", () => {
  const sprite = readFileSync("src/features/remote-bridge/components/PixelMascotSprite.vue", "utf8");
  const game = readFileSync("src/features/remote-bridge/components/PixelMascotGame.vue", "utf8");
  assert.match(sprite, /class="sleep-bed"/);
  assert.match(sprite, /class="sleep-face"/);
  assert.match(sprite, /\.sleep-cover \{ animation:sleepy-breathe/);
  assert.match(sprite, /\.pixel-mascot--sleep \.pixel-mascot__actor \{ animation:scene-hold/);
  assert.doesNotMatch(sprite.match(/@keyframes sleepy-breathe[^\n]+/)[0], /scale|rotate/);
  assert.doesNotMatch(sprite.match(/\.pixel-mascot--sleep \.claude-body[^\n]+/)[0], /scale/);
  assert.match(game, /<g v-if="frame\.mode !== 'duel'" class="game-monitor">/);
});

test("book faces its reader, coding hands stay behind the computer and idle preserves the mascot", () => {
  const sprite = readFileSync("src/features/remote-bridge/components/PixelMascotSprite.vue", "utf8");
  assert.match(sprite, /class="book-page-stack"/);
  assert.match(sprite, /class="book-cover"/);
  assert.match(sprite, /class="book-spine"/);
  assert.match(sprite, /const iterations = [^\n]*reading:1/);
  assert.match(sprite, /const cycleSeconds = [^\n]*reading:5\.6/);
  assert.match(sprite, /animation:page-turn 5\.6s/);
  assert.doesNotMatch(sprite, /class="book-line"|typing-hand/);
  assert.ok(sprite.indexOf('class="book-turning-page"') < sprite.indexOf('class="book-cover"'), "pages turn behind the outer covers");
  assert.match(sprite, /\.pixel-mascot--claude \.prop--game[^\n]*translateY\(-10px\) rotate\(-6deg\)/);
  assert.match(sprite, /\.pixel-mascot--idle \.claude-body[^\n]*transform-origin:center bottom; animation:idle-breathe/);
  assert.match(sprite, /@keyframes idle-breathe[^\n]*scale\(1\.015\)/);
  assert.doesNotMatch(sprite, /rest-cushion|rest-legs|rest-fold|rest-breath|prop--rest|pixel-float/);
  assert.doesNotMatch(sprite, /action === 'sleep' \|\| action === 'idle'/);
  assert.doesNotMatch(sprite, /\.pixel-mascot--idle \.(?:claude-eye|codex-glyph)[^\n]*opacity:0/);
});
