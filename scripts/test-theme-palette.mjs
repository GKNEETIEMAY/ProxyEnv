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
    // Fields and checkbox marks retain their stronger boundary and focus roles.
    for (const foreground of ["control-border", "focus-ring"]) pairs.push([foreground, surface, 3]);
  }
  pairs.push(["action-primary-fg", "action-primary-bg", 4.5], ["action-primary-fg", "action-primary-hover", 4.5],
    ["action-secondary-fg", "action-secondary-bg", 4.5], ["accent-strong", "accent-soft", 4.5],
    ["focus-ring", "accent-soft", 3], ["accent-strong", "accent-soft", 3], ["selection-fg", "selection-bg", 4.5],
    ["action-danger-fg", "action-danger-bg", 4.5]);
  for (const state of ["success", "warning", "danger"]) pairs.push([state, `${state}-soft`, 4.5]);
  const rows = pairs.map(([foreground, background, required]) => ({ palette, mode, foreground, background,
    foregroundHex: get(`--${foreground}`), backgroundHex: get(`--${background}`), required,
    ratio: contrast(get(`--${foreground}`), get(`--${background}`)) }));
  for (const background of ["action-primary-bg", "action-primary-hover"]) rows.push({ palette, mode,
    foreground: "action-button-fg", background, foregroundHex: get("--action-button-fg"), backgroundHex: get(`--${background}`),
    required: 4.5, ratio: contrast(get("--action-button-fg"), get(`--${background}`)),
    exception: mode === "light" && palette !== "neutral"
      ? "User-requested white labels on the exact pastel fills; does not meet WCAG AA text contrast. Buttons only, light mode only." : null });
  // Record the requested softer switch edges honestly, not as AA-compliant boundaries.
  for (const [foreground, background] of [["control-thumb", "action-primary-bg"], ["control-thumb-off", "line"], ["switch-border", "surface-strong"], ["action-primary-bg", "surface-strong"]]) {
    const ratio = contrast(get(`--${foreground}`), get(`--${background}`));
    rows.push({ palette, mode, foreground, background, foregroundHex: get(`--${foreground}`), backgroundHex: get(`--${background}`),
      required: 3, ratio, exception: ratio < 3
        ? "User-requested soft switch edges and white thumbs; does not meet WCAG AA non-text contrast. Position and keyboard focus remain distinct." : null });
  }
  const checkRatio = contrast(get("--control-check-fg"), get("--action-primary-bg"));
  rows.push({ palette, mode, foreground: "control-check-fg", background: "action-primary-bg",
    foregroundHex: get("--control-check-fg"), backgroundHex: get("--action-primary-bg"), required: 3, ratio: checkRatio,
    exception: checkRatio < 3 ? "User-requested white environment-variable checkmarks on the existing fills; does not meet WCAG AA non-text contrast. Strong borders and keyboard focus remain unchanged." : null });
  report.push(...rows);
  const failed = rows.filter(row => row.ratio < row.required && !row.exception);
  assert.equal(failed.length, 0, JSON.stringify(failed, null, 2));
});
test("button labels keep their theme rules; environment-variable checkmarks use their own white role", () => {
  for (const palette of palettes) for (const mode of ["light", "dark"]) {
    const get = tokens(palette, mode);
    assert.equal(get("--action-button-fg"), mode === "light" ? "#ffffff" : get("--action-primary-fg"));
    if (palette !== "neutral") assert.notEqual(get("--action-primary-fg"), "#ffffff");
  }
  const css = readFileSync("src/shared/styles/index.css", "utf8");
  for (const selector of [".primary-action", ".install-update-button"]) {
    const rule = css.slice(css.indexOf(`${selector} {`)).split("}")[0];
    assert.match(rule, /color: var\(--action-button-fg\)/);
  }
  const step = css.slice(css.indexOf(".assistant-steps li.active {")).split("}")[0];
  assert.match(step, /color: var\(--action-primary-fg\)/);
  const check = css.slice(css.indexOf(".variable-choice input:checked + span {")).split("}")[0];
  assert.match(check, /color: var\(--control-check-fg\)/);
  for (const palette of palettes) for (const mode of ["light", "dark"]) assert.equal(tokens(palette, mode)("--control-check-fg"), "#ffffff");
  assert.match(readFileSync("src/shared/theme/palettes.css", "utf8"), /\.variable-choice input:checked \+ span \{ color: HighlightText; background: Highlight; forced-color-adjust: none;/);
  const exceptions = report.filter(row => row.exception && row.foreground === "action-button-fg");
  assert.equal(exceptions.length, 6, "only three light palettes, normal and hover");
  assert.ok(exceptions.every(row => row.foregroundHex === "#ffffff"));
});

test("supplied pastel accents are exact; neutral surfaces stay gray rather than blue-gray", () => {
  for (const [palette, hex] of [["warm", "#f89868"], ["blue", "#70b0f8"], ["teal", "#80e0d8"]]) {
    for (const mode of ["light", "dark"]) {
      const get = tokens(palette, mode);
      assert.equal(get("--accent"), hex);
      assert.equal(get("--action-primary-bg"), hex);
    }
  }
  for (const mode of ["light", "dark"]) for (const role of ["canvas", "surface", "surface-strong", "ink", "muted", "faint", "line", "line-strong", "accent", "accent-strong", "accent-soft", "action-primary-bg", "action-primary-hover", "action-primary-fg", "control-border", "focus-ring", "selection-bg", "selection-fg"]) {
    const hex = tokens("neutral", mode)(`--${role}`);
    const rgb = [1, 3, 5].map(offset => parseInt(hex.slice(offset, offset + 2), 16));
    assert.equal(Math.max(...rgb) - Math.min(...rgb), 0, `${mode}/${role} must be neutral, not warm or blue-gray`);
  }
});

test("buttons and switches use softer edges without weakening field borders or keyboard focus", () => {
  const css = readFileSync("src/shared/styles/index.css", "utf8");
  assert.match(css, /\.primary-action \{[^}]*border: 1px solid var\(--action-primary-bg\)/);
  assert.match(css, /\.secondary-action \{[^}]*border: 1px solid var\(--button-border\)/);
  assert.match(css, /\.secondary-action:hover \{[^}]*border-color: var\(--button-hover-border\)/);
  for (const selector of [".toggle-track", ".switch-input"]) {
    const rule = css.slice(css.indexOf(`${selector} {`)).split("}")[0];
    assert.match(rule, /border: 1px solid var\(--switch-border\)/, selector);
  }
  for (const selector of [".toggle-control.active .toggle-track", ".switch-input:checked", ".install-update-button"]) {
    const rule = css.slice(css.indexOf(`${selector} {`)).split("}")[0];
    assert.match(rule, /border-color: var\(--action-primary-bg\)/, selector);
  }
  for (const palette of palettes) for (const mode of ["light", "dark"]) {
    const get = tokens(palette, mode);
    assert.equal(get("--button-border"), get("--line"));
    assert.equal(get("--button-hover-border"), get("--line-strong"));
    assert.equal(get("--switch-border"), get("--line-strong"));
    for (const role of ["button-border", "switch-border"]) assert.ok(contrast(get(`--${role}`), get("--surface-strong")) < contrast(get("--control-border"), get("--surface-strong")));
  }
  assert.match(css, /\.variable-choice input:checked \+ span \{[^}]*border-color: var\(--control-border\)/);
  assert.match(css, /\.palette-options input:checked \+ span \{[^}]*border-color:var\(--button-selected-border\)/);
  assert.match(css, /\.theme-options input:checked \+ span \{[^}]*border-color: var\(--button-selected-border\)/);
  assert.match(css, /button:focus-visible[^}]*outline: 2px solid var\(--focus-ring\)/);
  assert.match(readFileSync("src/app/components/AppHeader.vue", "utf8"), /\.update-available-button \{ color:var\(--accent-strong\)/);
});
test("shared switches keep white thumbs without dark rims and retain the agreed names", () => {
  for (const palette of palettes) for (const mode of ["light", "dark"]) for (const role of ["control-thumb", "control-thumb-off", "control-thumb-border", "control-thumb-off-border"]) {
    assert.equal(tokens(palette, mode)(`--${role}`), "#ffffff");
  }
  const css = readFileSync("src/shared/styles/index.css", "utf8");
  for (const selector of [".toggle-track > span", ".switch-input::before"]) {
    const rule = css.slice(css.indexOf(`${selector} {`)).split("}")[0];
    assert.match(rule, /border: 1px solid var\(--control-thumb-off-border\)/);
    assert.match(rule, /background: var\(--control-thumb-off\)/);
  }
  for (const selector of [".toggle-control.active .toggle-track > span", ".switch-input:checked::before"]) {
    const rule = css.slice(css.indexOf(`${selector} {`)).split("}")[0];
    assert.match(rule, /border-color: var\(--control-thumb-border\)/);
    assert.match(rule, /background: var\(--control-thumb\)/);
  }
  const copy = readFileSync("src/shared/i18n/desktop.ts", "utf8");
  assert.match(copy, /paletteWarm: "温暖橙", paletteBlue: "睿智蓝", paletteTeal: "未来青", paletteNeutral: "常规白"/);
});
test("all switch sizes center thumbs independently of borders, line height and horizontal movement", () => {
  const css = readFileSync("src/shared/styles/index.css", "utf8");
  const input = css.slice(css.indexOf(".switch-input {")).split("}")[0];
  assert.match(input, /position: relative/);
  assert.match(input, /padding: 0/);
  for (const selector of [".toggle-track > span", ".switch-input::before"]) {
    const rule = css.slice(css.indexOf(`${selector} {`)).split("}")[0];
    assert.match(rule, /position: absolute/);
    assert.match(rule, /top: 50%/);
    assert.match(rule, /translate: 0 -50%/);
  }
  const thumb = css.slice(css.indexOf(".switch-input::before {")).split("}")[0];
  assert.match(thumb, /box-sizing: border-box/);
  assert.match(thumb, /margin: 0/);
  assert.match(css, /\.switch-input:checked::before \{[^}]*transform: translateX\(20px\)/);
  const overview = readFileSync("src/features/remote-bridge/components/RemoteBridgeOverview.vue", "utf8");
  assert.match(overview, /\.bridge-skill-action:checked::before \{ transform:translateX\(16px\)/);
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
