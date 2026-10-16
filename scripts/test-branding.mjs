import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync, readdirSync } from "node:fs";
import { createHash } from "node:crypto";
import ts from "typescript";

const read = path => readFileSync(path, "utf8");
const moduleUrl = (path, replacements = {}) => {
  let source = read(path);
  for (const [from, to] of Object.entries(replacements)) source = source.replace(from, to);
  return `data:text/javascript;base64,${Buffer.from(ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText).toString("base64")}`;
};
const { messages } = await import(moduleUrl("src/shared/i18n/index.ts", {
  '"./remote-bridge"': JSON.stringify(moduleUrl("src/shared/i18n/remote-bridge.ts")),
  '"./diagnostic-report"': JSON.stringify(moduleUrl("src/shared/i18n/diagnostic-report.ts")),
  '"./desktop"': JSON.stringify(moduleUrl("src/shared/i18n/desktop.ts")),
}));
const config = JSON.parse(read("src-tauri/tauri.conf.json"));

test("four locales show RouteSpan name, slogan and report labels without renaming copy keys", () => {
  const slogans = { en: "Extend local capabilities to remote environments.", "zh-CN": "让本地能力，延伸到远程。", ja: "ローカルの能力を、リモート環境へ。", ko: "로컬의 기능을 원격 환경으로 확장하세요." };
  for (const [locale, copy] of Object.entries(messages)) {
    assert.equal(copy.appName, locale === "zh-CN" ? "境枢 RouteSpan" : "RouteSpan");
    assert.equal(copy.appTagline, slogans[locale]);
    assert.ok(copy.reportTitle.startsWith("RouteSpan"));
    assert.ok(copy.reportVersion.startsWith("RouteSpan"));
    assert.ok(copy.disableProxyEnvironment, "Compatibility property names are not display text");
    const checkValues = object => {
      for (const value of Object.values(object)) {
        if (typeof value === "string") assert.doesNotMatch(value, /ProxyEnv|EnvHub/);
        else if (value && typeof value === "object") checkValues(value);
      }
    };
    checkValues(copy);
  }
});

test("all branding uses the exact latest pale user PNG instead of SVG or plate artwork", () => {
  assert.equal(createHash("sha256").update(readFileSync("assets/brand/routespan-mark-source.png")).digest("hex"), "98d69dcf748bafcbdc91deffffbfbe7867e2131fabac450104e36b1933506ff2");
  assert.equal(readdirSync("assets/brand").some(name => name.endsWith(".svg")), false);
  assert.equal(readdirSync("public/brand").some(name => name.endsWith(".svg")), false);
  assert.deepEqual(readFileSync("public/brand/routespan-mark.png"), readFileSync("assets/brand/routespan-mark.png"));
  assert.match(read("scripts/generate-brand-icons.mjs"), /resize\(source\("mark"\), 1024, "assets\/brand\/routespan-appicon-1024\.png"\)/);
  assert.doesNotMatch(read("scripts/generate-brand-icons.mjs"), /source\("(?:appicon|lockup)/);
  for (const path of ["src/app/components/AppHeader.vue", "src/features/settings/components/SettingsPage.vue"]) {
    assert.match(read(path), /<BrandMark/);
    assert.doesNotMatch(read(path), /brand-arrow-blue|brand-arrow-white|brand-symbol/);
  }
  assert.match(read("src/shared/components/BrandMark.vue"), /routespan-mark\.png\?url/);
  assert.doesNotMatch(read("src/shared/components/BrandMark.vue"), /\.svg|<svg/);
  assert.match(read("src/features/remote-bridge/components/RemoteBridgePage.vue"), /v-if="activeTarget\.source !== 'manual' && activeTarget\.configPath"/);
});

test("generated PNG and Windows ICO sizes retain native icon coverage", () => {
  for (const [path, width] of [["assets/brand/routespan-mark.png", 256], ["assets/brand/routespan-appicon-1024.png", 1024], ["src-tauri/icons/tray.png", 32], ["src-tauri/icons/tray-20.png", 20], ["src-tauri/icons/32x32.png", 32], ["src-tauri/icons/64x64.png", 64], ["src-tauri/icons/128x128.png", 128], ["src-tauri/icons/128x128@2x.png", 256]]) {
    const bytes = readFileSync(path);
    assert.equal(bytes.toString("hex", 0, 8), "89504e470d0a1a0a");
    assert.equal(bytes.readUInt32BE(16), width, path);
    assert.equal(bytes.readUInt32BE(20), width, path);
    assert.equal(bytes[25], 6, `${path} retains RGBA transparency`);
  }
  const ico = readFileSync("src-tauri/icons/icon.ico");
  assert.equal(ico.readUInt16LE(2), 1);
  const sizes = Array.from({ length: ico.readUInt16LE(4) }, (_, i) => ico[6 + i * 16] || 256);
  for (const size of [16, 24, 32, 48, 64, 256]) assert.ok(sizes.includes(size));
});

test("native window, tray, notifications and terminal use display branding only", () => {
  assert.equal(config.productName, "RouteSpan");
  assert.equal(config.app.windows[0].title, "境枢 RouteSpan");
  const tray = read("src-tauri/src/desktop/tray.rs");
  for (const text of ["退出 RouteSpan", "Quit RouteSpan", "RouteSpan を終了", "RouteSpan 종료"]) assert.ok(tray.includes(text));
  assert.match(tray, /\.tooltip\("RouteSpan"\)/);
  assert.match(tray, /include_image!\("icons\/tray\.png"\)/);
  assert.match(read("src-tauri/src/desktop/notifications.rs"), /RouteSpan · \{title\}/);
  assert.match(read("src-tauri/src/desktop/notifications.rs"), /app\.config\(\)\.identifier/);
  assert.match(read("src-tauri/src/features/remote_bridge/ssh.rs"), /WindowTitle='RouteSpan Proxy Terminal'/);
});

test("package, storage, session, repository and updater identities stay compatible", () => {
  assert.equal(JSON.parse(read("package.json")).name, "proxyenv");
  assert.match(read("src-tauri/Cargo.toml"), /\[package\]\s+name = "proxyenv"/);
  assert.match(read("src-tauri/Cargo.toml"), /\[lib\]\s+name = "proxyenv_lib"/);
  assert.equal(config.identifier, "io.github.gkneeteiemay.proxyenv");
  assert.deepEqual(config.plugins.updater.endpoints, ["https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest/download/latest.json"]);
  assert.equal(config.plugins.updater.pubkey, "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEE5MEFBN0QwNzRDQzE0OUUKUldTZUZNeDAwS2NLcWFqSXlFSVU0eHRRQnlEeWtoRWUwaFNoTitRVmN0NE9uMmxlZW1Bc3pSdFkK");
  for (const file of ["src-tauri/src/services/settings.rs", "src-tauri/src/environment/snapshot.rs"]) assert.match(read(file), /path\.join\("ProxyEnv"\)/);
  assert.match(read("src/features/remote-bridge/components/RemoteBridgePage.vue"), /proxyenv\.remoteBridge\.lastTargetId/);
  assert.match(read("src-tauri/src/features/remote_bridge/authenticated_relay.rs"), /x-proxyenv-session/i);
  assert.match(read("src-tauri/src/features/remote_bridge/remote.sh"), /\.proxyenv/);
  assert.match(read("src/features/settings/update-availability.ts"), /repos\/GKNEETIEMAY\/ProxyEnv\/releases\/latest/);
  assert.match(read("src/app/AppShell.vue"), /fetchLatestRelease\(fetch, controller.signal\)/);
  assert.match(read("src/features/settings/update.ts"), /proxyenv-release:end/);
  const workflow = read(".github/workflows/release.yml");
  assert.match(workflow, /target\/release\/proxyenv\.exe/);
  assert.match(workflow, /RouteSpan-\$version-windows-x64-portable\.exe/);
  assert.match(workflow, /name: RouteSpan-\$\{\{ github\.ref_name \}\}-windows-x64/);
  assert.match(workflow, /draft: true/);
});

test("historical stable release notes remain the original ProxyEnv records", () => {
  const hashes = { "0.1.1": "16426be2dd020c63dc05ee27b81d8ad08ed4f56b07306fdfb14e48f76b1c8afe", "0.1.2": "ddea6472deddbf451bf1965d8216fb8ecbe5d4404dc8d702fec946ebeff89b2e", "0.1.3": "f6e8bd45cebb9e975f73de74cf724c4c89df0d1ca1f13ffe2a825ef0a35a9b7b", "0.1.4": "80468fd1c8c06fd1a1f1ed48051503611d42010f3addc0cb943fd08d157d3262" };
  for (const [version, hash] of Object.entries(hashes)) {
    // Git checkouts may normalize line endings; the compatibility record uses LF.
    const content = read(`.github/release-notes/v${version}.md`).replace(/\r\n/g, "\n");
    assert.equal(createHash("sha256").update(content).digest("hex"), hash);
  }
});
