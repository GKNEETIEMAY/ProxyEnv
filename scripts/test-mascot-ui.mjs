import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";

const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const browser = await chromium.launch({ headless: true,
  args: ["--no-proxy-server"],
  ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath: process.env.PROXYENV_LAYOUT_BROWSER } : {}) });
const base = process.env.PROXYENV_LAYOUT_URL || "http://127.0.0.1:1420";
const output = resolve(".debug-tmp/mascot-polish");
mkdirSync(output, { recursive: true });
const page = await browser.newPage({ viewport: { width: 980, height: 800 }, reducedMotion: "no-preference" });
const errors = [];
page.on("pageerror", error => errors.push(error.message));

async function enter(sample) {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.goto(`${base}/?impeccable-review=remote`);
  await page.locator(".remote-current-summary").waitFor();
  await page.evaluate(value => { window.savedRandom = Math.random; Math.random = () => value; }, sample);
  await page.locator(".remote-setup-actions .secondary-action").click();
  await page.locator(".remote-empty-mascot:not(.is-paused)").waitFor();
  await page.evaluate(() => { Math.random = window.savedRandom; });
}

try {
  for (const [index, mode] of [[7, "snake"], [15, "jump"]]) {
    await enter((index + .5) / 16);
    const game = page.locator(".pixel-game");
    assert.equal(await game.getAttribute("data-game"), mode);
    assert.equal(await game.locator(".game-monitor").count(), 1, "solo mini-games keep their screen");
    await page.waitForFunction(() => Number(document.querySelector(".pixel-game")?.dataset.tick) > 1);
    await page.locator(".primary-nav button").first().click();
    await page.locator(".remote-empty-mascot.is-paused").waitFor({ state: "attached" });
    const before = await game.getAttribute("data-tick");
    await page.waitForTimeout(900);
    assert.equal(await game.getAttribute("data-tick"), before, "hidden page pauses the game state, not just artwork");
    await page.locator(".primary-nav button").nth(1).click();
    await page.emulateMedia({ reducedMotion: "reduce" });
    const reduced = await game.getAttribute("data-tick");
    await page.waitForTimeout(600);
    assert.equal(await game.getAttribute("data-tick"), reduced, "reduced motion has no game ticks");
  }

  await enter(.01);
  await page.locator(".pixel-mascot__clock").evaluate(element => element.dispatchEvent(new Event("animationend")));
  await page.waitForFunction(() => document.querySelector('.pixel-scene[data-scene="1"]') && !document.querySelector(".pixel-scene-enter-active"));
  await page.evaluate(() => { Math.random = () => .7; });
  await page.locator(".pixel-mascot__clock").evaluate(element => element.dispatchEvent(new Event("animationend")));
  await page.locator('[data-game="duel"]').waitFor();
  await page.evaluate(() => { Math.random = window.savedRandom; });
  await page.waitForFunction(() => [...document.querySelectorAll(".player-health")].some(element => Number(element.dataset.health) < 100));
  assert.equal(await page.locator(".pixel-fighter").count(), 2);
  assert.equal(await page.locator(".prop--game").count(), 2);
  assert.equal(await page.locator(".game-monitor, .screen-frame, .screen-led").count(), 0, "PK has no monitor, backdrop or stand");
  const original = await page.locator(".remote-empty-mascot").boundingBox();
  await page.locator(".pixel-mascot__clock").first().evaluate(element => element.dispatchEvent(new Event("animationend")));
  assert.equal(await page.locator(".pixel-scene").getAttribute("data-scene"), "2", "one player finishing cannot interrupt a duet");
  await page.locator(".pixel-mascot__clock").last().evaluate(element => element.dispatchEvent(new Event("animationend")));
  await page.locator('.pixel-scene[data-scene="3"]').waitFor();
  assert.deepEqual(await page.locator(".remote-empty-mascot").boundingBox(), original, "game completion causes no layout jump");

  // One batched gallery of the actual components, not replacement artwork.
  await page.evaluate(async () => {
    const source = await (await fetch("/src/features/remote-bridge/components/PixelMascotSprite.vue")).text();
    const vuePath = source.match(/from "([^"]*\/vue\.js[^"]*)"/)[1];
    const { createApp, h } = await import(vuePath);
    const { default: Sprite } = await import("/src/features/remote-bridge/components/PixelMascotSprite.vue");
    const { default: Game } = await import("/src/features/remote-bridge/components/PixelMascotGame.vue");
    const { default: Empty } = await import("/src/features/remote-bridge/components/RemoteEmptyMascot.vue");
    const gallery = document.createElement("div"); gallery.id = "mascot-gallery";
    gallery.style.cssText = "position:absolute;top:0;left:0;z-index:9999;background:var(--surface);display:grid;grid-template-columns:repeat(3,288px);gap:8px;padding:16px;width:912px;height:max-content;box-sizing:border-box;align-content:start;";
    document.body.append(gallery);
    const tile = (label, content) => h("section", { style: "position:relative;width:288px;height:162px;display:grid;place-items:center;background:var(--surface-strong);color:var(--ink);" }, [h("span", { style: "position:absolute;left:12px;top:8px;font:12px sans-serif;" }, label), content]);
    window.mascotGalleryApp = createApp({ render: () => h("div", { style: "display:contents" }, [
      ...["claude", "codex"].flatMap(kind => ["coffee", "reading", "plant", "music", "sleep", "game", "idle", "coding"].map(action => tile(`${kind} · ${action}`, h(Sprite, { kind, action, gameInput: "action" })))),
      tile("snake · auto play", h(Game, { duo: false, kind: "claude", paused: false })),
      tile("duet · PK", h(Game, { duo: true, kind: "codex", paused: false })),
      tile("duet · shared plant", h(Empty)),
    ]) });
    window.mascotGalleryApp.mount(gallery);
  });
  // Force gallery-only fixtures without adding shipped debug controls or a preview route.
  // Put the final stage into its real shared-watering scene before freezing the gallery.
  const sharedStage = page.locator("#mascot-gallery section").last();
  await sharedStage.scrollIntoViewIfNeeded();
  await sharedStage.locator(".remote-empty-mascot:not(.is-paused)").waitFor();
  await sharedStage.locator(".pixel-mascot__clock").evaluate(element => element.dispatchEvent(new Event("animationend")));
  await sharedStage.locator('.pixel-scene[data-scene="1"]').waitFor();
  await page.waitForFunction(() => !document.querySelector("#mascot-gallery .pixel-scene-enter-active, #mascot-gallery .pixel-scene-leave-active"));
  await page.evaluate(() => { Math.random = () => .5; });
  await sharedStage.locator('.pixel-scene[data-scene="1"] .pixel-mascot__clock').evaluate(element => element.dispatchEvent(new Event("animationend")));
  await sharedStage.locator(".pixel-shared-plant").waitFor();
  await page.evaluate(() => { Math.random = window.savedRandom; });
  await page.evaluate(() => {
    const tiles = document.querySelectorAll("#mascot-gallery section");
    const soloGame = tiles[16].querySelector(".pixel-game");
    if (soloGame?.dataset.game !== "snake") tiles[16].firstElementChild.textContent = "jump · auto play";
    for (const element of document.querySelectorAll("#mascot-gallery .pixel-mascot__clock")) {
      for (const animation of element.getAnimations()) animation.pause();
    }
  });
  await page.waitForTimeout(1700);
  await page.evaluate(() => {
    for (const animation of document.querySelector("#mascot-gallery").getAnimations({ subtree: true })) {
      animation.pause();
      const name = animation.animationName?.replace(/-[a-f\d]{8}$/, "");
      if (name === "coffee-sip") animation.currentTime = 1650;
      if (["page-turn", "reading-hand"].includes(name)) animation.currentTime = 4816;
      if (name === "music-note-rise") animation.currentTime = 600;
      if (name === "idle-breathe") animation.currentTime = 1400;
      if (["eye-blink", "idle-cursor"].includes(name)) animation.currentTime = 0;
    }
  });
  const geometry = await page.evaluate(() => {
    const tiles = [...document.querySelectorAll("#mascot-gallery section")];
    const bounds = element => element.getBoundingClientRect().toJSON();
    return tiles.slice(0, 16).map(tile => {
      const idleBody = tile.querySelector(".pixel-mascot--idle .claude-body, .pixel-mascot--idle .codex-body");
      let idlePose;
      if (idleBody) {
        const breathing = idleBody.getAnimations().find(animation => animation.animationName.startsWith("idle-breathe"));
        breathing.currentTime = 0;
        const before = bounds(idleBody);
        breathing.currentTime = 1400;
        idlePose = { before, after: bounds(idleBody), transform: getComputedStyle(idleBody).transform,
          faceOpacity: getComputedStyle(tile.querySelector(".claude-eye, .codex-glyph")).opacity,
          extras: tile.querySelectorAll(".prop--rest, .rest-cushion, .sleep-face").length };
      }
      const pageTurn = tile.querySelector(".book-turning-page")?.getAnimations()[0];
      let readingTiming;
      if (pageTurn) {
        pageTurn.currentTime = 2000;
        readingTiming = { duration: pageTurn.effect.getTiming().duration,
          restingOpacity: getComputedStyle(tile.querySelector(".book-turning-page")).opacity };
        pageTurn.currentTime = 4816;
      }
      return { label: tile.firstElementChild.textContent,
      eye: tile.querySelector(".claude-eye, .codex-glyph") && bounds(tile.querySelector(".claude-eye, .codex-glyph")),
      actor: bounds(tile.querySelector(".pixel-mascot__svg")),
      cup: tile.querySelector(".cup-rim") && bounds(tile.querySelector(".cup-rim")),
      can: tile.querySelector(".watering-can") && bounds(tile.querySelector(".watering-can")),
      controller: tile.querySelector(".controller") && bounds(tile.querySelector(".controller")),
      pageTransform: tile.querySelector(".book-turning-page") && getComputedStyle(tile.querySelector(".book-turning-page")).transform,
      cover: tile.querySelector(".book-cover") && bounds(tile.querySelector(".book-cover")),
      page: tile.querySelector(".book-page-stack") && bounds(tile.querySelector(".book-page-stack")),
      readingHands: tile.querySelectorAll(".prop--reading .holding-hand").length,
      readingTiming,
      codingHands: tile.querySelectorAll(".prop--laptop .holding-hand, .prop--terminal .holding-hand, .typing-hand").length,
      idlePose,
      noteOpacity: tile.querySelector(".music-note") && getComputedStyle(tile.querySelector(".music-note")).opacity,
      actorTransform: getComputedStyle(tile.querySelector(".pixel-mascot__actor")).transform,
      sleepFace: !!tile.querySelector(".sleep-face"),
      sleepBodyTransform: tile.querySelector(".sleep-cover") && getComputedStyle(tile.querySelector(".claude-body, .codex-body")).transform,
      blanketAnimation: tile.querySelector(".sleep-cover") && getComputedStyle(tile.querySelector(".sleep-cover")).animationName,
    }; });
  });
  for (const item of geometry) {
    for (const prop of [item.cup, item.can, item.controller].filter(Boolean)) {
      assert.ok(prop.top >= item.eye.bottom, `${item.label}: props never cover the eyes/glyph`);
    }
    if (item.pageTransform) assert.ok(Number(item.pageTransform.slice(7).split(",")[0]) < 0, "page visibly turns across its spine");
    if (item.cover) {
      assert.ok(item.page.bottom < item.cover.bottom && item.page.height < item.cover.height, "only foreshortened page edges show above the outer covers");
      assert.equal(item.readingHands, 2, "page turn reuses the holding hand, without an extra hand");
      assert.ok(item.cover.height / item.cover.width >= .4 && item.cover.height / item.cover.width <= .6, "the open book has readable book proportions, not a thin bar");
      assert.ok(Math.abs((item.cover.left + item.cover.right) / 2 - (item.actor.left + item.actor.right) / 2) < 1, "the book is held centrally in front of its reader");
      assert.ok(item.page.top >= item.eye.bottom, "the book remains below the reader's face");
      assert.equal(item.readingTiming.duration, 5600, "reading turns one page per 5.6 seconds");
      assert.equal(item.readingTiming.restingOpacity, "0", "the mascot reads quietly before turning a page");
    }
    if (item.label.includes("coding")) assert.equal(item.codingHands, 0, "computer backs occlude the typing hands");
    if (item.idlePose) {
      const pose = item.idlePose;
      assert.equal(pose.faceOpacity, "1", "idle retains the original face");
      assert.equal(pose.extras, 0, "idle has no invented crossed legs, cushion or breathing brackets");
      assert.ok(pose.after.width > pose.before.width, "idle has subtle breathing feedback");
      assert.ok(Math.abs(pose.after.bottom - pose.before.bottom) < .1, "breathing keeps the base planted");
      const matrix = pose.transform.slice(7).split(",").map(Number);
      assert.ok(matrix[0] > 1 && matrix[0] <= 1.016);
      assert.equal(matrix[0], matrix[3], "breathing does not squash the silhouette");
      assert.equal(matrix[1], 0); assert.equal(matrix[2], 0);
      assert.equal(item.actorTransform, "matrix(1, 0, 0, 1, 0, 0)", "idle does not float or tilt");
    }
    if (item.noteOpacity) assert.ok(Number(item.noteOpacity) > .5, "music has visible notes");
    if (item.label.includes("music")) assert.match(item.actorTransform, /^matrix\(1, 0, 0, 1,/, "bobbing never tilts the whole actor");
    if (item.label.includes("sleep")) {
      assert.ok(item.sleepFace);
      assert.equal(item.actorTransform, "matrix(1, 0, 0, 1, 0, 0)", "sleeping actor does not bounce or squash");
      const matrix = item.sleepBodyTransform.slice(7).split(",").map(Number);
      assert.equal(matrix[0], matrix[3], "resting pose preserves body proportions");
      assert.match(item.blanketAnimation, /^sleepy-breathe(?:-[a-f\d]{8})?$/);
    }
  }
  for (const theme of ["light", "dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    await page.locator("#mascot-gallery").screenshot({ path: resolve(output, `actions-${theme}.png`) });
  }
  await page.evaluate(() => { window.mascotGalleryApp.unmount(); document.querySelector("#mascot-gallery").remove(); });
  for (const [width, height, theme] of [[980, 800, "light"], [980, 800, "dark"], [620, 720, "dark"]]) {
    await page.setViewportSize({ width, height });
    await enter(((theme === "light" ? 0 : 8) + .5) / 16);
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    await page.waitForTimeout(850);
    const stage = await page.locator(".remote-empty-mascot").boundingBox();
    assert.ok(stage.x >= 0 && stage.x + stage.width <= width, "stage fits the real empty-state panel");
    await page.screenshot({ path: resolve(output, `empty-${width}-${theme}.png`) });
  }
  assert.deepEqual(errors, []);
  console.log("PASS: prop clearance, page turn, notes, non-tilting actors; snake/jump/duel; HP damage, scene completion, hidden/reduced-motion pause; light/dark/narrow captures.");
} finally { await browser.close(); }
