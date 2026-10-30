import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import vm from "node:vm";
import ts from "typescript";

const modules = new Map();
function load(path) {
  path = resolve(path);
  if (modules.has(path)) return modules.get(path);
  const exports = {};
  modules.set(path, exports);
  const code = ts.transpileModule(readFileSync(path, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  vm.runInNewContext(code, { exports, URL, Error, Promise,
    require: name => load(resolve(dirname(path), `${name}.ts`)),
  }, { filename: path });
  return exports;
}
const { stableVersion, compareVersions, isOfficialReleaseUrl } = load("src/features/settings/update.ts");
const { UpdateMetadataClient, verifiedInstallHandle, retryTransport, validMetadata, resolveUpdate, updateFailure, UpdateRequestError } = load("src/features/settings/update-availability.ts");
const { UpdateInstaller } = load("src/features/settings/update-installer.ts");
const metadata = (version = "0.2.2") => ({ version, body: `notes-${version}`, date: "2026-10-30T00:00:00Z" });
function storage(initial = null) {
  let value = initial;
  return { getItem() { return value; }, setItem(_, next) { value = next; } };
}

test("stable version parsing rejects previews, garbage, shorthand and unsafe numbers", () => {
  for (const version of ["v0.2.1", "0.2.1", "12.30.100"]) assert.ok(stableVersion(version));
  for (const version of ["0.2.1-rc.1", "v0.2.1preview", "0.2.1+build", "0.2", "0.02.1", " 0.2.1", "0.2.1 ", "0.2.9007199254740992", "V0.2.1"])
    assert.equal(stableVersion(version), null, version);
  for (const value of [null, undefined, 2, {}, []]) assert.equal(stableVersion(value), null);
  assert.equal(compareVersions("0.2.10", "0.2.9"), 1);
  assert.equal(compareVersions("v0.2.1", "0.2.1"), 0);
  assert.throws(() => compareVersions("0.2.1-rc.1", "0.2.0"));
  assert.equal(isOfficialReleaseUrl("https://github.com.evil.test/GKNEETIEMAY/ProxyEnv/releases/"), false);
});

test("official manifest distinguishes equal, ahead, available, manual and missing releases without REST", () => {
  assert.equal(resolveUpdate("0.2.2", metadata(), true).state, "latest");
  const ahead = resolveUpdate("0.2.2", metadata("0.2.1"), true);
  assert.equal(ahead.state, "ahead");
  assert.equal(ahead.version, "0.2.1");
  const newer = resolveUpdate("0.2.1", metadata(), true);
  assert.equal(newer.state, "available");
  assert.equal(newer.url, "https://github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v0.2.2");
  assert.equal(newer.body, "notes-0.2.2");
  assert.equal(resolveUpdate("0.2.1", metadata(), false).state, "manual");
  assert.equal(resolveUpdate("0.2.1", { ...metadata(), body: "" }, true).state, "manual");
  assert.equal(resolveUpdate("0.2.1", null, true).state, "unpublished");
  assert.equal(resolveUpdate("bad", metadata(), true).failure, "invalidResponse");
  for (const invalid of [metadata("0.2.2-rc.1"), { ...metadata(), body: {} }, { ...metadata(), date: "bad" }, { ...metadata(), body: "x".repeat(80_001) }]) {
    assert.equal(validMetadata(invalid), false);
    assert.equal(resolveUpdate("0.2.1", invalid, true).failure, "invalidResponse");
  }
});

test("automatic checks cache six hours across restart; manual checks bypass TTL after 30 seconds", async () => {
  let now = 100_000, calls = 0;
  const disk = storage();
  const fetcher = async () => { calls++; return metadata(); };
  const client = new UpdateMetadataClient(fetcher, disk, () => now);
  assert.equal((await client.get()).cached, false);
  assert.equal((await client.get("manual")).cached, true);
  assert.equal(calls, 1);
  now += 31_000;
  const restarted = new UpdateMetadataClient(fetcher, disk, () => now);
  assert.equal((await restarted.get()).cached, true);
  assert.equal(calls, 1);
  assert.equal((await restarted.get("manual")).cached, false);
  assert.equal(calls, 2);
  now += 6 * 3600_000;
  assert.equal((await restarted.get()).cached, false);
  assert.equal(calls, 3);
  assert.equal((await restarted.get("install")).cached, false);
  assert.equal(calls, 4);
  assert.doesNotMatch(disk.getItem(), /signature|https:|handle|rid/);
});

test("concurrent and repeated clicks share a single bounded request", async () => {
  let calls = 0, finish;
  const client = new UpdateMetadataClient(() => { calls++; return new Promise(done => { finish = done; }); }, storage(), () => 100_000);
  const first = client.get(), second = client.get("manual");
  assert.equal(first, second);
  finish(metadata());
  await Promise.all([first, second]);
  await client.get("manual");
  assert.equal(calls, 1);
});

test("rate-limit cooldown survives restart and cannot be bypassed by manual or install requests", async () => {
  let now = 100_000, calls = 0;
  const disk = storage();
  const first = new UpdateMetadataClient(async () => { calls++; throw { category: "rateLimit", retryAt: 400_000 }; }, disk, () => now);
  await assert.rejects(first.get(), e => e.category === "rateLimit" && e.retryAt === 400_000);
  const restarted = new UpdateMetadataClient(async () => { calls++; return metadata(); }, disk, () => now);
  for (const mode of ["automatic", "manual", "install"]) await assert.rejects(restarted.get(mode), e => e.category === "rateLimit");
  assert.equal(calls, 1);
  now = 400_000;
  assert.equal((await restarted.get("manual")).cached, false);
  assert.equal(calls, 2);
});

test("malformed cooldown gets a minimum wait, storage failures do not block live checks", async () => {
  const disk = storage();
  const client = new UpdateMetadataClient(async () => { throw { category: "rateLimit", retryAt: "private" }; }, disk, () => 100_000);
  await assert.rejects(client.get(), e => e.retryAt === 160_000);
  const broken = { getItem() { throw Error("denied"); }, setItem() { throw Error("full"); } };
  const live = new UpdateMetadataClient(async () => metadata(), broken, () => 100_000);
  assert.equal((await live.get()).metadata.version, "0.2.2");
  assert.equal((await live.get()).cached, true);
});

test("a Tauri rate-limit error during install preflight also persists a cooldown", async () => {
  const disk = storage();
  const client = new UpdateMetadataClient(async () => metadata(), disk, () => 100_000);
  await client.get();
  client.rememberFailure(new Error("HTTP 429"));
  const restarted = new UpdateMetadataClient(async () => { throw Error("must not request"); }, disk, () => 100_000);
  await assert.rejects(restarted.get("install"), e => e.category === "rateLimit" && e.retryAt === 160_000);
});

test("corrupt or future caches cannot provide version evidence; network failures stay errors", async () => {
  for (const raw of ["{bad", "x".repeat(100_001), JSON.stringify({ checkedAt: 200_000, retryAt: 0, metadata: metadata() }), JSON.stringify({ checkedAt: 10, retryAt: 0, metadata: metadata("bad") })]) {
    let calls = 0;
    const client = new UpdateMetadataClient(async () => { calls++; return metadata(); }, storage(raw), () => 100_000);
    assert.equal((await client.get()).cached, false);
    assert.equal(calls, 1);
  }
  let calls = 0;
  const failed = new UpdateMetadataClient(async () => { calls++; throw { category: "network" }; }, storage(), () => 100_000);
  await assert.rejects(failed.get(), e => e.category === "network");
  assert.equal(calls, 2);
});

test("transient metadata failures retry once; trust failures and limits never retry", async () => {
  for (const category of ["network", "timeout", "tls", "signature", "invalidResponse", "rateLimit", "service", "proxy"]) {
    let calls = 0;
    await assert.rejects(retryTransport(async () => { calls++; throw new UpdateRequestError(category); }), e => e.category === category);
    assert.equal(calls, ["network", "timeout"].includes(category) ? 2 : 1);
  }
});

test("install gets a fresh matching official handle and rejects downgrades and changed versions", async () => {
  let calls = 0, closed = 0;
  const handle = { version: "0.2.2", async close() { closed++; } };
  const checked = async () => { calls++; return handle; };
  assert.equal(await verifiedInstallHandle("0.2.1", "0.2.2", checked), handle);
  assert.equal(calls, 1);
  for (const current of ["0.2.2", "0.2.3", "bad"]) await assert.rejects(verifiedInstallHandle(current, "0.2.2", checked));
  assert.equal(calls, 1);
  await assert.rejects(verifiedInstallHandle("0.2.1", "0.2.3", checked), e => e.category === "invalidResponse");
  assert.equal(closed, 1);
});

test("first empty official handle retries once but never installs a null or failed handle", async () => {
  let calls = 0;
  const handle = { version: "0.2.2", async close() {} };
  assert.equal(await verifiedInstallHandle("0.2.1", "0.2.2", async () => ++calls === 1 ? null : handle), handle);
  assert.equal(calls, 2);
  calls = 0;
  assert.equal(await verifiedInstallHandle("0.2.1", "0.2.2", async () => { calls++; return null; }), null);
  assert.equal(calls, 2);
  for (const category of ["tls", "signature", "invalidResponse", "rateLimit"]) {
    calls = 0;
    await assert.rejects(verifiedInstallHandle("0.2.1", "0.2.2", async () => { calls++; throw new UpdateRequestError(category); }));
    assert.equal(calls, 1);
  }
});

test("failures are allowlisted and never expose raw transport messages", () => {
  for (const [message, category] of [["AbortError", "timeout"], ["TLS certificate", "tls"], ["proxy authentication 407 secret", "proxy"], ["signature failed secret", "signature"], ["fetch failed http://secret@private", "network"], ["unknown private path", "service"]])
    assert.equal(updateFailure(new Error(message)), category);
  assert.equal(updateFailure({ category: "private-token" }), "service");
});

test("current notes are bundled, REST is absent and check/install trust boundaries remain fixed", () => {
  const shell = readFileSync("src/app/AppShell.vue", "utf8");
  const notes = readFileSync("src/features/settings/installed-notes.ts", "utf8");
  const version = JSON.parse(readFileSync("package.json", "utf8")).version;
  assert.ok(notes.includes(`v${version}.md?raw`));
  assert.ok(notes.includes(`installedReleaseVersion = "${version}"`));
  assert.doesNotMatch(shell, /fetchLatestRelease|fetchInstalledRelease|api\.github\.com|allowDowngrades/);
  assert.match(shell, /appVersion.value === installedReleaseVersion \? installedReleaseNotes/);
  assert.match(shell, /metadataClient.get\("install"\)/);
  assert.match(shell, /verifiedInstallHandle\(appVersion.value, expectedVersion/);
  assert.match(shell, /check\(\{ timeout: 10_000 \}\)/);
  const native = readFileSync("src-tauri/src/desktop/update_metadata.rs", "utf8");
  const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
  assert.ok(native.includes(config.plugins.updater.endpoints[0]));
  assert.doesNotMatch(native, /danger_accept_invalid|api\.github\.com/);
});

test("all desktop update locales survive the English fallback", () => {
  const { messages } = load("src/shared/i18n/index.ts");
  const { desktopMessages } = load("src/shared/i18n/desktop.ts");
  for (const locale of ["en", "zh-CN", "ja", "ko"])
    for (const [key, value] of Object.entries(desktopMessages[locale]))
      assert.equal(messages[locale][key], value, `${locale}.${key}`);
});

function fixture() {
  const calls = [];
  const guard = {
    async prepare(confirmed) { calls.push(`prepare:${confirmed}`); return 1; },
    async validate() { calls.push("validate"); }, async release() { calls.push("release"); },
  };
  const handle = {
    async download(onProgress, options) { calls.push("download"); assert.equal(options.timeout, 90000); onProgress({ event: "Finished" }); calls.push("verified"); },
    async install() { calls.push("install"); }, async close() { calls.push("close"); },
  };
  const installer = new UpdateInstaller(guard);
  const run = (confirm = async () => true) => installer.run(handle, confirm, phase => calls.push(`phase:${phase}`), () => {});
  return { calls, guard, handle, installer, run };
}
test("download and signature verification finish before final backend validation and install", async () => {
  const f = fixture();
  assert.equal(await f.run(), "installed");
  assert.deepEqual(f.calls, ["prepare:false", "phase:download", "download", "verified", "validate", "phase:install", "install", "release", "close"]);
});
test("live bridge confirmation defaults to later and does not start a download", async () => {
  const f = fixture();
  f.guard.prepare = async () => { throw "updateConfirmationRequired"; };
  assert.equal(await f.run(async () => false), "deferred");
  assert.deepEqual(f.calls, []);
  assert.equal(f.installer.busy, false);
});
test("confirmation re-acquires current backend state instead of reusing a stale UI snapshot", async () => {
  const f = fixture();
  f.guard.prepare = async confirmed => { f.calls.push(`prepare:${confirmed}`); if (!confirmed) throw "updateConfirmationRequired"; return 2; };
  await f.run();
  assert.deepEqual(f.calls.slice(0, 2), ["prepare:false", "prepare:true"]);
});
test("authentication or preparation busy state fails closed without download", async () => {
  const f = fixture(); f.guard.prepare = async () => { throw "updateBusy"; };
  await assert.rejects(f.run(), error => error === "updateBusy");
  assert.deepEqual(f.calls, []);
});
test("signature failure or final state change never installs and always releases the gate", async () => {
  for (const phase of ["download", "validate", "install"]) {
    const f = fixture();
    (phase === "validate" ? f.guard : f.handle)[phase] = async () => { throw new Error(`${phase} failed`); };
    await assert.rejects(f.run());
    if (phase !== "install") assert.equal(f.calls.includes("install"), false);
    assert.deepEqual(f.calls.slice(-2), ["release", "close"]);
    assert.equal(f.installer.busy, false);
  }
});
test("abandoning download immediately releases bridge changes but cannot install when it finishes", async () => {
  const f = fixture(); let finish, started;
  const downloading = new Promise(done => { started = done; });
  f.handle.download = () => new Promise(done => { finish = done; f.calls.push("download"); started(); });
  const result = f.run(); await downloading;
  await f.installer.cancel();
  assert.equal(f.calls.at(-1), "release");
  finish(); assert.equal(await result, "cancelled");
  assert.equal(f.calls.includes("validate"), false);
  assert.equal(f.calls.includes("install"), false);
  assert.equal(f.calls.filter(call => call === "release").length, 1);
});
test("a double click cannot download twice or acquire another update lease", async () => {
  const f = fixture(); let finish, started;
  const downloading = new Promise(done => { started = done; });
  f.handle.download = () => new Promise(done => { finish = done; started(); });
  const first = f.run(); await downloading;
  await assert.rejects(f.run(), /updateBusy/);
  finish(); await first;
  assert.equal(f.calls.filter(call => call === "prepare:false").length, 1);
});

test("cancel plus transport failure keeps the gate released and cannot install", async () => {
  const f = fixture(); let rejectDownload, started;
  const downloading = new Promise(done => { started = done; });
  f.handle.download = () => new Promise((_, reject) => { rejectDownload = reject; started(); });
  const result = f.run(); await downloading;
  await f.installer.cancel();
  assert.equal(f.installer.guardReleased, true);
  rejectDownload(new Error("network failed"));
  await assert.rejects(result, /network failed/);
  assert.equal(f.calls.includes("install"), false);
  assert.equal(f.calls.at(-1), "close");
});

test("failed release is not reported as a recovered gate", async () => {
  const f = fixture();
  f.guard.release = async () => { throw new Error("IPC unavailable"); };
  await assert.rejects(f.run(), /IPC unavailable/);
  assert.equal(f.installer.guardReleased, false);
  assert.equal(f.installer.busy, false);
  assert.equal(f.calls.at(-1), "close");
});
test("backend entry points and monitors share the gate; Windows has no post-install relaunch guard", () => {
  const read = path => readFileSync(path, "utf8");
  assert.match(read("src-tauri/src/commands/remote_bridge.rs"), /bridge_operation\(\)/);
  for (const path of ["src-tauri/src/features/remote_bridge/mod.rs", "src-tauri/src/features/remote_bridge/skills.rs"])
    assert.match(read(path), /sleep\(Duration::from_secs\(2\)\);\s*let Ok\(_operation\) = crate::desktop::updates::bridge_operation/);
  assert.match(read("src-tauri/src/desktop/updates.rs"), /bridge::update_activity\(\)/);
  assert.match(read("src-tauri/src/features/remote_bridge/mod.rs"), /ssh_auth::update_busy\(\)/);
  const shell = read("src/app/AppShell.vue");
  assert.doesNotMatch(shell, /downloadAndInstall\(|relaunch\(/);
  assert.match(shell, /settingsReady = true;[\s\S]*?void checkForUpdates\(false\)/);
  assert.match(shell, /:inert="updateExclusive"/);
  assert.match(shell, /if \(installer\.guardReleased\) updateExclusive\.value = false/);
  assert.match(read("src/features/settings/components/UpdateInstallDialog.vue"), /type="button" autofocus/);
  const permissions = read("src-tauri/capabilities/default.json");
  assert.doesNotMatch(permissions, /updater:allow-download-and-install/);
  assert.match(permissions, /updater:allow-download/);
  assert.match(permissions, /updater:allow-install/);
});
