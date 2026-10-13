import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync, mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { createHash } from "node:crypto";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const read = path => readFileSync(resolve(root, path), "utf8").replace(/\r\n/g, "\n");
const pkg = JSON.parse(read("package.json"));
const config = JSON.parse(read("src-tauri/tauri.conf.json"));
const cargo = read("src-tauri/Cargo.toml");
const workflow = read(".github/workflows/release.yml");
const repository = "GKNEETIEMAY/ProxyEnv";
const packageSection = cargo.split("[package]\n")[1]?.split(/^\[/m)[0];

test("v0.2 package, Cargo manifest/lock and Tauri versions agree", () => {
  const cargoVersion = packageSection?.match(/^version = "([^"]+)"$/m)?.[1];
  assert.deepEqual([pkg.version, cargoVersion, config.version], ["0.2.0", "0.2.0", "0.2.0"]);
  const lockedPackage = read("src-tauri/Cargo.lock").split("[[package]]")
    .find(block => /^name = "proxyenv"$/m.test(block));
  assert.match(lockedPackage ?? "", /^version = "0\.2\.0"$/m);
});

test("display brand and protected compatibility identities remain separate", () => {
  assert.equal(config.productName, "RouteSpan");
  assert.equal(config.identifier, "io.github.gkneeteiemay.proxyenv");
  assert.equal(pkg.name, "proxyenv");
  assert.match(packageSection ?? "", /^name = "proxyenv"$/m);
  assert.match(cargo, /\[lib\]\nname = "proxyenv_lib"/);
  assert.deepEqual(config.plugins.updater.endpoints,
    [`https://github.com/${repository}/releases/latest/download/latest.json`]);
  assert.equal(config.plugins.updater.windows.installMode, "passive");
  for (const path of ["src-tauri/src/services/settings.rs", "src-tauri/src/environment/snapshot.rs"])
    assert.match(read(path), /path\.join\("ProxyEnv"\)/);
  assert.match(read("src-tauri/src/features/remote_bridge/remote.sh"), /\.proxyenv/);
  assert.match(read("src-tauri/src/features/remote_bridge/authenticated_relay.rs"), /x-proxyenv-session/i);
  assert.match(read("src/features/remote-bridge/components/RemoteBridgePage.vue"), /proxyenv\.remoteBridge\.lastTargetId/);
});

test("formal tag policy accepts stable versions and rejects prereleases without changing the updater comparator", () => {
  const pattern = workflow.match(/if \(\$version -notmatch '([^']+)'\)/)?.[1];
  assert.equal(pattern, "^\\d+\\.\\d+\\.\\d+$");
  const accepts = version => new RegExp(pattern).test(version);
  for (const version of ["0.2.0", "0.2.1", "1.0.0"]) assert.equal(accepts(version), true, version);
  for (const version of ["0.2.0-rc.1", "0.2.0-beta", "0.2.0+build", "0.2", "v0.2.0", "0.2.0\nextra"])
    assert.equal(accepts(version), false, version);
  assert.match(workflow, /\$packageVersion -ne \$version -or \$tauriVersion -ne \$version -or \$cargoVersion -ne \$version/);
});

test("external artifacts use RouteSpan names; internal binary, signature and NSIS manifest stay consistent", () => {
  assert.match(workflow, /target\/release\/proxyenv\.exe/);
  for (const suffix of ["portable.exe", "setup.exe", "installer.msi"])
    assert.ok(workflow.includes(`RouteSpan-$version-windows-x64-${suffix}`));
  assert.ok(workflow.includes('Copy-Item -LiteralPath $signaturePath -Destination (Join-Path $output "$artifactName.sig")'));
  assert.ok(workflow.includes(`https://github.com/${repository}/releases/download/$env:GITHUB_REF_NAME/RouteSpan-$version-windows-x64-setup.exe`));
  assert.match(workflow, /name: RouteSpan-\$\{\{ github\.ref_name \}\}-windows-x64/);
  assert.match(workflow, /SHA256SUMS\.txt/);
  assert.match(workflow, /actions\/attest@/);
  assert.match(workflow, /draft: true/);
  assert.match(workflow, /TAURI_SIGNING_PRIVATE_KEY is required/);
  assert.match(workflow, /Missing Tauri Updater signature/);
  assert.equal(config.bundle.createUpdaterArtifacts, true);
  assert.equal(pkg.scripts["release:build"], "tauri build --ci -- --locked");
});

test("v0.2 notes retain four compatibility markers, substantive localized sections and user-facing content", () => {
  const notes = read(".github/release-notes/v0.2.0.md");
  const sections = {
    "zh-CN": ["新功能", "改进", "增强", "修复"],
    en: ["New Features", "Improvements", "Enhancements", "Fixes"],
    ja: ["新機能", "改善", "強化", "修正"],
    ko: ["새로운 기능", "개선", "강화", "수정"],
  };
  assert.equal((notes.match(/<!-- proxyenv-release:end -->/g) ?? []).length, 4);
  assert.doesNotMatch(notes, /routespan-release:|\bStage\s*\d|\bM(?:8|10)\b|\b(?:docs|fix|test)\([^)]+\):/i);
  const counts = [];
  for (const [locale, headings] of Object.entries(sections)) {
    const marker = `<!-- proxyenv-release:${locale} -->`;
    assert.equal(notes.split(marker).length, 2, `Exactly one ${locale} block`);
    const block = notes.split(marker)[1].split("<!-- proxyenv-release:end -->")[0];
    assert.deepEqual([...block.matchAll(/^## (.+)$/gm)].map(match => match[1]), headings);
    const chunks = block.split(/^## /m).slice(1);
    const bullets = chunks.map(chunk => (chunk.match(/^- \S/gm) ?? []).length);
    assert.ok(bullets.every(count => count > 0));
    counts.push(bullets);
    for (const tool of ["CC Switch", "Codex", "Claude", "VS Code", "Skills", "MobaXterm", "root", "UID", "SmartScreen"])
      assert.ok(block.includes(tool), `${locale}: ${tool}`);
  }
  for (const count of counts) assert.deepEqual(count, [6, 3, 3, 4]);
  execFileSync(process.execPath, [resolve(root, "scripts/validate-release-notes.mjs"),
    resolve(root, ".github/release-notes/v0.2.0.md")], { cwd: root, stdio: "pipe" });
});

test("release preflight is wired into local checks, CI and signed release before packaging", () => {
  assert.equal(pkg.scripts["test:release"], "node --test scripts/test-release-contract.mjs");
  assert.ok(pkg.scripts.check.includes("pnpm test:release"));
  for (const path of [".github/workflows/ci.yml", ".github/workflows/release.yml"]) {
    const source = read(path);
    assert.match(source, /name: Test release contracts\n\s+run: pnpm test:release/);
    assert.match(source, /pnpm audit --audit-level high/);
  }
  assert.ok(workflow.indexOf("name: Test release contracts") < workflow.indexOf("name: Build Tauri bundles"));
});

test("release RustSec gate is pinned, fail-closed and precedes all artifact packaging", () => {
  const steps = workflow.split(/^      - name: /m).slice(1);
  const findStep = name => {
    const step = steps.find(value => value.startsWith(`${name}\n`));
    assert.ok(step, `Required release step: ${name}`);
    return step;
  };
  const install = findStep("Install pinned Rust security auditor");
  const audit = findStep("Audit Rust dependencies before packaging");
  assert.match(install, /cargo install cargo-audit --version 0\.22\.2 --locked/);
  assert.match(audit, /cargo audit --file src-tauri\/Cargo\.lock/);
  for (const step of [install, audit]) {
    assert.match(step, /shell: pwsh/);
    assert.match(step, /\$ErrorActionPreference = 'Stop'/);
    assert.match(step, /if \(\$LASTEXITCODE -ne 0\) \{ throw /);
    assert.doesNotMatch(step, /continue-on-error:|\bif:|--ignore|--no-fetch|\|\|\s*true/);
  }
  const names = ["Set up Rust", "Install pinned Rust security auditor", "Audit Rust dependencies before packaging",
    "Check Rust formatting", "Run Rust tests with locked dependencies", "Run Clippy with locked dependencies",
    "Build Tauri bundles and signed updater artifacts", "Collect release artifacts and SHA-256 checksums",
    "Attest release artifacts", "Create draft GitHub Release"];
  const indices = names.map(name => steps.indexOf(findStep(name)));
  assert.deepEqual(indices, [...indices].sort((a, b) => a - b));
  assert.doesNotMatch(workflow, /continue-on-error:\s*true|workflow_dispatch:/);
});

test("Rust auditor install and vulnerability failures stop PowerShell before packaging", {
  skip: process.platform !== "win32",
}, () => {
  const ps7 = execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command",
    "(Get-Command pwsh -ErrorAction SilentlyContinue).Source"], { encoding: "utf8" }).trim();
  assert.ok(ps7, "PowerShell 7 is required to verify the release security gate");
  const commands = ["Install pinned Rust security auditor", "Audit Rust dependencies before packaging"].map(name => {
    const step = workflow.split(`      - name: ${name}\n`)[1]?.split("      - name: ")[0];
    assert.ok(step);
    return step.split("        run: |\n")[1].split("\n")
      .map(line => line.startsWith("          ") ? line.slice(10) : line).join("\n");
  }).join("\n");
  const run = (installCode, auditCode) => execFileSync(ps7, ["-NoProfile", "-NonInteractive", "-Command",
    `function cargo { if ($args[0] -eq 'install') { $global:LASTEXITCODE = ${installCode} } else { $global:LASTEXITCODE = ${auditCode} } }\n${commands}\nWrite-Output 'PACKAGING_REACHED'`],
  { encoding: "utf8", timeout: 15000, stdio: "pipe" });
  assert.match(run(0, 0), /PACKAGING_REACHED/);
  for (const [installCode, auditCode] of [[1, 0], [0, 1]]) {
    assert.throws(() => run(installCode, auditCode), error => {
      assert.doesNotMatch(error.stdout.toString(), /PACKAGING_REACHED/);
      assert.match(error.stderr.toString(), /auditor failed|packaging is blocked/);
      return true;
    });
  }
});

test("Windows artifact collector preserves bytes, pairs renamed signatures and targets that NSIS in latest.json", {
  skip: process.platform !== "win32",
}, () => {
  const stage = workflow.split("      - name: Collect release artifacts and SHA-256 checksums\n")[1]
    ?.split("      - name: Attest release artifacts\n")[0];
  assert.ok(stage, "Collector step exists");
  const command = stage.split("        run: |\n")[1].split("\n")
    .map(line => line.startsWith("          ") ? line.slice(10) : line).join("\n");
  const fixture = mkdtempSync(resolve(tmpdir(), "routespan-release-contract-"));
  const setup = "src-tauri/target/release/bundle/nsis/RouteSpan_0.2.0_x64-setup.exe";
  const msi = "src-tauri/target/release/bundle/msi/RouteSpan_0.2.0_x64_en-US.msi";
  const put = (path, value) => {
    mkdirSync(dirname(resolve(fixture, path)), { recursive: true });
    writeFileSync(resolve(fixture, path), value);
  };
  try {
    // Inert fixture bytes, not runnable installers or real signing material.
    put("src-tauri/target/release/proxyenv.exe", "portable fixture");
    put(setup, "NSIS fixture");
    put(`${setup}.sig`, "fixture-nsis-signature");
    put(msi, "MSI fixture");
    put(`${msi}.sig`, "fixture-msi-signature");
    put(".github/release-notes/v0.2.0.md", "Fixture notes");
    // Windows PowerShell 5 lacks utf8NoBOM; the actual workflow runs PowerShell 7.
    // Use pwsh when installed; otherwise do not falsely claim this collector was tested.
    const ps7 = execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command",
      "(Get-Command pwsh -ErrorAction SilentlyContinue).Source"], { encoding: "utf8" }).trim();
    assert.ok(ps7, "PowerShell 7 is required for the release collector test");
    const run = () => execFileSync(ps7, ["-NoProfile", "-NonInteractive", "-Command", command], {
      cwd: fixture, env: { ...process.env, GITHUB_REF_NAME: "v0.2.0" }, timeout: 15000, stdio: "pipe",
    });
    run();
    const output = name => readFileSync(resolve(fixture, "release-artifacts", name));
    const names = { "portable.exe": "portable fixture", "setup.exe": "NSIS fixture", "installer.msi": "MSI fixture" };
    for (const [suffix, bytes] of Object.entries(names))
      assert.equal(output(`RouteSpan-0.2.0-windows-x64-${suffix}`).toString(), bytes);
    assert.equal(output("RouteSpan-0.2.0-windows-x64-setup.exe.sig").toString(), "fixture-nsis-signature");
    assert.equal(output("RouteSpan-0.2.0-windows-x64-installer.msi.sig").toString(), "fixture-msi-signature");
    const latest = JSON.parse(output("latest.json").toString("utf8"));
    assert.equal(latest.version, "0.2.0");
    assert.equal(latest.platforms["windows-x86_64"].signature, "fixture-nsis-signature");
    assert.equal(latest.platforms["windows-x86_64"].url,
      `https://github.com/${repository}/releases/download/v0.2.0/RouteSpan-0.2.0-windows-x64-setup.exe`);
    const lines = output("SHA256SUMS.txt").toString("utf8").trim().split(/\r?\n/);
    assert.equal(lines.length, 6);
    for (const line of lines) {
      const [hash, name] = line.split("  ");
      assert.equal(createHash("sha256").update(output(name)).digest("hex"), hash);
    }
    rmSync(resolve(fixture, "release-artifacts"), { recursive: true });
    put("src-tauri/target/release/bundle/nsis/duplicate.exe", "ambiguous fixture");
    assert.throws(run, /Exactly one NSIS and one MSI installer/);
  } finally {
    rmSync(fixture, { recursive: true });
  }
});
