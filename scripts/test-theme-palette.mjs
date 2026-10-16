import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = readFileSync("src/shared/theme/palette.ts", "utf8");
const exports = {};
vm.runInNewContext(ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText, { exports });
const { palettes, normalizePalette, readPalette, savePalette, readAppearance, saveAppearance, applyTheme, paletteStorageKey, appearanceStorageKey } = exports;
function storage() { const data = new Map(); return { data, getItem: key => data.get(key) ?? null, setItem: (key, value) => data.set(key, value) }; }

test("palette preferences are strict, non-sensitive and independent of settings.json", () => {
  const local = storage();
  for (const value of [null, undefined, "", "BLUE", " blue", {}, "secret", '"teal"']) assert.equal(normalizePalette(value), "warm");
  for (const palette of palettes) { savePalette(palette, local); assert.equal(readPalette(local), palette); }
  assert.deepEqual([...local.data.keys()], [paletteStorageKey]);
  assert.equal(paletteStorageKey, "routespan.ui.palette.v1");
  assert.equal(readPalette(), "warm", "missing/blocked browser storage is harmless");
});
test("storage failures keep selection usable for this process", () => {
  const blocked = { getItem() { throw new Error("denied"); }, setItem() { throw new Error("quota"); } };
  assert.equal(readPalette(blocked), "warm"); assert.equal(readAppearance(blocked), "system");
  assert.doesNotThrow(() => savePalette("blue", blocked));
  assert.doesNotThrow(() => saveAppearance("dark", blocked));
  const root = { dataset: {} }; applyTheme(root, "light", "teal", true);
  assert.deepEqual(root.dataset, { palette: "teal", theme: "light" });
});
test("mode cache supports restart; system changes never replace the selected palette", () => {
  const local = storage(); const root = { dataset: {} };
  for (const mode of ["system", "light", "dark"]) {
    saveAppearance(mode, local); savePalette("blue", local);
    assert.equal(readAppearance(local), mode);
    for (const dark of [true, false]) {
      applyTheme(root, readAppearance(local), readPalette(local), dark);
      assert.equal(root.dataset.palette, "blue");
      assert.equal(root.dataset.theme, mode === "system" ? dark ? "dark" : "light" : mode);
    }
  }
  local.data.set(appearanceStorageKey, "bad"); assert.equal(readAppearance(local), "system");
});
test("palette never enters the old backend settings schema or replaces legacy keys", () => {
  const shell = readFileSync("src/app/AppShell.vue", "utf8");
  const rust = readFileSync("src-tauri/src/services/settings.rs", "utf8");
  const schema = rust.slice(rust.indexOf("pub struct AppSettings"), rust.indexOf("impl Default for AppSettings"));
  assert.doesNotMatch(schema, /palette|appearance/);
  assert.match(rust, /deny_unknown_fields/);
  assert.match(shell, /v-model:palette="palette"/);
  assert.match(shell, /watch\(palette, value => \{ if \(!reviewPreview\) savePalette\(value\)/);
  assert.match(readFileSync("src/main.ts", "utf8"), /applyTheme\([\s\S]*?createApp\(App\)\.mount/);
  assert.doesNotMatch(source, /removeItem|clear\(|proxyenv\./);
});

// Resolve the shipped CSS declarations, not a duplicated JavaScript color table.
const rules = [readFileSync("src/shared/styles/index.css", "utf8").split("* { box-sizing")[0],
  readFileSync("src/shared/theme/palettes.css", "utf8").split("@media (forced-colors")[0]].join("\n");
function tokens(palette, mode) {
  const values = {};
  for (const [, rawSelector, body] of rules.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const selector = rawSelector.replace(/\/\*[\s\S]*?\*\//g, "").trim();
    if (!selector.startsWith(":root")) continue;
    const theme = selector.match(/data-theme="([^"]+)"/)?.[1];
    const selectedPalette = selector.match(/data-palette="([^"]+)"/)?.[1];
    if ((theme && theme !== mode) || (selectedPalette && selectedPalette !== palette)) continue;
    for (const [, name, value] of body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) values[name] = value.trim();
  }
  const resolve = name => values[name]?.startsWith("var(") ? resolve(values[name].slice(4, -1)) : values[name];
  return resolve;
}
function luminance(hex) {
  assert.match(hex, /^#[\da-f]{6}$/i);
  const rgb = [1, 3, 5].map(offset => parseInt(hex.slice(offset, offset + 2), 16) / 255)
    .map(value => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4);
  return rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722;
}
function contrast(a, b) { const x = luminance(a), y = luminance(b); return (Math.max(x, y) + .05) / (Math.min(x, y) + .05); }
const report = [];
for (const palette of palettes) for (const mode of ["light", "dark"]) test(`${palette}/${mode}: shipped text, states, actions, selected and focus contrast`, () => {
  const get = tokens(palette, mode);
  const pairs = [];
  for (const surface of ["canvas", "surface", "surface-strong"]) {
    for (const foreground of ["ink", "muted", "faint", "accent-strong"]) pairs.push([foreground, surface, 4.5]);
    for (const foreground of ["control-border", "focus-ring", "action-primary-bg"]) pairs.push([foreground, surface, 3]);
  }
  pairs.push(["action-primary-fg", "action-primary-bg", 4.5], ["action-primary-fg", "action-primary-hover", 4.5],
    ["action-secondary-fg", "action-secondary-bg", 4.5], ["accent-strong", "accent-soft", 4.5],
    ["focus-ring", "accent-soft", 3], ["accent", "accent-soft", 3], ["selection-fg", "selection-bg", 4.5],
    ["control-thumb", "action-primary-bg", 3], ["control-thumb-off", "line", 3], ["action-danger-fg", "action-danger-bg", 4.5]);
  for (const state of ["success", "warning", "danger"]) pairs.push([state, `${state}-soft`, 4.5]);
  const rows = pairs.map(([foreground, background, required]) => ({ palette, mode, foreground, background,
    foregroundHex: get(`--${foreground}`), backgroundHex: get(`--${background}`), required,
    ratio: contrast(get(`--${foreground}`), get(`--${background}`)) }));
  report.push(...rows);
  const failed = rows.filter(row => row.ratio < row.required);
  assert.equal(failed.length, 0, JSON.stringify(failed, null, 2));
});
test("theme controls retain native semantics, semantic state roles and forced-color focus", () => {
  const settings = readFileSync("src/features/settings/components/SettingsPage.vue", "utf8");
  assert.match(settings, /role="radiogroup" :aria-label="copy.palette"/);
  assert.match(settings, /v-model="palette" type="radio"/);
  assert.match(settings, /palette-selected/);
  const css = readFileSync("src/shared/theme/palettes.css", "utf8");
  assert.match(css, /forced-colors: active/);
  assert.doesNotMatch(css, /--(?:success|warning|danger)(?:-soft)?:/);
});
if (process.argv.includes("--report")) {
  process.on("beforeExit", () => {
    mkdirSync("artifacts/v0.2.1", { recursive: true });
    writeFileSync("artifacts/v0.2.1/theme-contrast.json", JSON.stringify({ source: "shipped CSS roles (browser computed checks separate)", pairs: report }, null, 2));
  });
}
