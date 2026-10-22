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
const { checkUpdateMetadata, fetchInstalledRelease, fetchLatestRelease, resolveUpdate, updateFailure, UpdateRequestError } = load("src/features/settings/update-availability.ts");
const { UpdateInstaller } = load("src/features/settings/update-installer.ts");
const ok = value => ({ status: "fulfilled", value });
const fail = reason => ({ status: "rejected", reason });
const release = (version = "0.2.2") => ({ tag_name: `v${version}`, html_url: `https://github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v${version}`,
  body: `notes-${version}`, published_at: "2026-10-16T00:00:00Z", draft: false, prerelease: false });
const metadata = (version = "0.2.2") => ({ version, body: `metadata-${version}`, date: "2026-10-16T00:00:00Z" });

test("stable version parsing rejects previews, garbage, shorthand and unsafe numbers", () => {
  for (const version of ["v0.2.1", "0.2.1", "12.30.100"]) assert.ok(stableVersion(version));
  for (const version of ["0.2.1-rc.1", "v0.2.1preview", "0.2.1+build", "0.2", "0.02.1", " 0.2.1", "0.2.1 ", "0.2.9007199254740992", "V0.2.1"])
    assert.equal(stableVersion(version), null, version);
  for (const value of [null, undefined, 2, {}, []]) assert.equal(stableVersion(value), null);
  assert.equal(compareVersions("0.2.10", "0.2.9"), 1);
  assert.equal(compareVersions("v0.2.1", "0.2.1"), 0);
  assert.throws(() => compareVersions("0.2.1-rc.1", "0.2.0"));
});

test("installed changelog fetches the exact running tag independently of latest", async () => {
  let requested;
  const current = release("0.2.1");
  const result = await fetchInstalledRelease(async url => {
    requested = url;
    return { status: 200, ok: true, json: async () => current };
  }, {}, "0.2.1");
  assert.equal(requested, "https://api.github.com/repos/GKNEETIEMAY/ProxyEnv/releases/tags/v0.2.1");
  assert.equal(result.body, "notes-0.2.1");
  await fetchLatestRelease(async url => {
    assert.equal(url, "https://api.github.com/repos/GKNEETIEMAY/ProxyEnv/releases/latest");
    return { status: 404 };
  }, {});
  assert.equal(resolveUpdate("0.2.1", ok(release()), ok(metadata()), true).body, "notes-0.2.2");
});

test("installed changelog never substitutes a different, malformed or unpublished release", async () => {
  for (const value of [release("0.2.2"), release("0.2.0"), { ...release("0.2.1"), draft: true }, { ...release("0.2.1"), body: {} }]) {
    await assert.rejects(fetchInstalledRelease(async () => ({ status: 200, ok: true, json: async () => value }), {}, "0.2.1"), error => error.category === "invalidResponse");
  }
  assert.equal(await fetchInstalledRelease(async () => ({ status: 404 }), {}, "0.2.1"), null);
  await assert.rejects(fetchInstalledRelease(async () => { throw Error("must not fetch"); }, {}, "bad"), error => error.category === "invalidResponse");
  await assert.rejects(fetchInstalledRelease(async () => { throw Error("network"); }, {}, "0.2.1"), /network/);
});
test("v0.2.0 to v0.2.1 is an upgrade and v0.2.1 never offers the same or older release", () => {
  const handle = metadata("0.2.1");
  const result = resolveUpdate("0.2.0", ok(release("0.2.1")), ok(handle), true);
  assert.equal(result.state, "available");
  assert.equal(result.version, "0.2.1");
  assert.equal(result.handle, handle);
  for (const version of ["0.2.0", "0.2.1"]) {
    const current = resolveUpdate("0.2.1", ok(release(version)), ok(metadata(version)), true);
    assert.equal(current.state, "latest");
    assert.equal(current.handle, null);
  }
});

test("announcement, version, notes, URL and install handle come from one resolved version", () => {
  const handle = metadata();
  const result = resolveUpdate("0.2.1", ok(release()), ok(handle), true);
  assert.equal(result.state, "available");
  assert.equal(result.version, "0.2.2");
  assert.equal(result.body, "notes-0.2.2");
  assert.equal(result.handle, handle);
  assert.equal(result.url, release().html_url);
});
test("disagreeing sources disable installation and do not expose mismatched notes or URL", () => {
  const result = resolveUpdate("0.2.1", ok(release("0.2.3")), ok(metadata()), true);
  assert.equal(result.state, "conflict");
  for (const key of ["version", "url", "body", "date"]) assert.equal(result[key], "");
  assert.equal(result.handle, null);
});
test("API limit can fall back to official metadata, not a fake latest result", () => {
  const result = resolveUpdate("0.2.1", fail(new UpdateRequestError("rateLimit")), ok(metadata()), true);
  assert.equal(result.state, "available");
  assert.equal(result.source, "metadata");
  assert.equal(result.body, "metadata-0.2.2");
  assert.equal(resolveUpdate("0.2.1", fail(new Error("fetch failed")), ok(null), true).state, "error");
});
test("missing metadata and unsupported bundles only offer manual official downloads", () => {
  for (const result of [resolveUpdate("0.2.1", ok(release()), ok(null), true),
    resolveUpdate("0.2.1", ok(release()), ok(metadata()), false)]) {
    assert.equal(result.state, "manual");
    assert.equal(result.handle, null);
  }
});
test("first metadata connection failure recovers within the same check", async () => {
  for (const message of ["network request failed", "request timed out"]) {
    let calls = 0;
    const handle = metadata("0.2.1"), announcement = release("0.2.1");
    const result = await checkUpdateMetadata("0.2.0", Promise.resolve(announcement), async () => {
      if (++calls === 1) throw new Error(message);
      return handle;
    });
    const snapshot = resolveUpdate("0.2.0", ok(announcement), ok(result), true);
    assert.equal(calls, 2);
    assert.equal(snapshot.state, "available");
    assert.equal(snapshot.handle, handle);
  }
});
test("empty metadata retries only when the announcement proves a newer stable release", async () => {
  for (const [announcement, expected] of [[release(), 2], [release("0.2.1"), 1], [null, 1], [release("0.2.2-rc.1"), 1]]) {
    let calls = 0;
    const result = await checkUpdateMetadata("0.2.1", Promise.resolve(announcement), async () => ++calls === 1 ? null : metadata());
    assert.equal(calls, expected);
    assert.equal(result?.version ?? null, expected === 2 ? "0.2.2" : null);
  }
});
test("persistent transport failure is bounded and never described as missing signed metadata", async () => {
  let calls = 0;
  const [result] = await Promise.allSettled([checkUpdateMetadata("0.2.1", Promise.resolve(release()), async () => {
    calls++; throw new Error("network private path");
  })]);
  assert.equal(calls, 2);
  const snapshot = resolveUpdate("0.2.1", ok(release()), result, true);
  assert.equal(snapshot.state, "manual");
  assert.equal(snapshot.failure, "network");
  assert.equal(snapshot.handle, null);
  assert.equal(snapshot.version, "0.2.2");
});
test("trust and validation errors never retry or enable installation", async () => {
  for (const category of ["tls", "signature", "invalidResponse", "proxy", "rateLimit", "service"]) {
    let calls = 0;
    const [result] = await Promise.allSettled([checkUpdateMetadata("0.2.1", Promise.resolve(release()), async () => {
      calls++; throw new UpdateRequestError(category);
    })]);
    assert.equal(calls, 1, category);
    const snapshot = resolveUpdate("0.2.1", ok(release()), result, true);
    assert.equal(snapshot.failure, category);
    assert.equal(snapshot.handle, null);
  }
});
test("latest needs valid announcement evidence; 404 is unpublished, not latest", () => {
  assert.equal(resolveUpdate("0.2.1", ok(release("0.2.1")), ok(null), true).state, "latest");
  assert.equal(resolveUpdate("0.2.1", ok(null), ok(null), true).state, "unpublished");
  assert.equal(resolveUpdate("0.2.1", ok(null), fail(new Error("timeout")), true).state, "error");
  assert.equal(resolveUpdate("bad", ok(release()), ok(metadata()), true).failure, "invalidResponse");
});
test("malformed or nonstable source data cannot be installed", () => {
  for (const entry of [{ ...release(), draft: true }, { ...release(), prerelease: true }, release("0.2.2-rc.1"),
    { ...release(), html_url: release("0.2.3").html_url }, { ...release(), body: {} },
    { ...release(), html_url: `${release().html_url}?token=secret` },
    { ...release(), html_url: `https://user:secret@github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v0.2.2` }]) {
    assert.equal(resolveUpdate("0.2.1", ok(entry), ok(metadata()), true).failure, "invalidResponse");
  }
  assert.equal(resolveUpdate("0.2.1", ok(release()), ok(metadata("0.2.2junk")), true).failure, "invalidResponse");
  assert.equal(isOfficialReleaseUrl("https://github.com.evil.test/GKNEETIEMAY/ProxyEnv/releases/"), false);
});
test("failures become allowlisted categories, never raw transport messages", () => {
  for (const [message, category] of [["AbortError", "timeout"], ["TLS certificate", "tls"], ["proxy authentication 407 secret", "proxy"],
    ["signature failed secret", "signature"], ["fetch failed http://secret@private", "network"], ["unknown private path", "service"]])
    assert.equal(updateFailure(new Error(message)), category);
});
test("HTTP errors distinguish evidenced limits from other 403 and service failures", async () => {
  for (const [status, remaining, category] of [[429, null, "rateLimit"], [403, "0", "rateLimit"], [403, "12", "service"], [403, null, "service"], [500, null, "service"], [502, null, "service"]]) {
    await assert.rejects(fetchLatestRelease(async () => ({ status, ok: false, headers: { get: () => remaining } }), {}), error => error.category === category);
  }
  assert.equal(await fetchLatestRelease(async () => ({ status: 404 }), {}), null);
  await assert.rejects(fetchLatestRelease(async () => ({ status: 200, ok: true, json() { throw new Error("bad json private content"); } }), {}), error => error.category === "invalidResponse");
  for (const value of [null, "0.2.2", 2, []])
    await assert.rejects(fetchLatestRelease(async () => ({ status: 200, ok: true, json: async () => value }), {}), error => error.category === "invalidResponse");
});
test("metadata without notes cannot enable fallback installation", () => {
  const result = resolveUpdate("0.2.1", fail(new Error("network")), ok({ version: "0.2.2" }), true);
  assert.equal(result.state, "manual");
  assert.equal(result.handle, null);
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
  assert.match(shell, /settingsReady = true;[\s\S]*?void checkForUpdates\(\)/);
  assert.match(shell, /:inert="updateExclusive"/);
  assert.match(shell, /if \(installer\.guardReleased\) updateExclusive\.value = false/);
  assert.match(read("src/features/settings/components/UpdateInstallDialog.vue"), /type="button" autofocus/);
  const permissions = read("src-tauri/capabilities/default.json");
  assert.doesNotMatch(permissions, /updater:allow-download-and-install/);
  assert.match(permissions, /updater:allow-download/);
  assert.match(permissions, /updater:allow-install/);
});
