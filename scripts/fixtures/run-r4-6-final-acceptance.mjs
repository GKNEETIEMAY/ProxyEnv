import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { appendFileSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");
const bridge = "src-tauri/src/features/remote_bridge";
const logRoot = join(root, "artifacts/v0.3.0/remote-bridge-r4-6/acceptance");
const args = process.argv.slice(2);
assert.ok(args.every(arg => ["--wry", "--f4", "--f5", "--f6"].includes(arg))
  && args.filter(arg => arg !== "--wry").length <= 1, "Usage: run-r4-6-final-acceptance.mjs [--f4|--f5|--f6] [--wry]");
const wry = args.includes("--wry");
const phase = args.includes("--f6") ? 6 : args.includes("--f5") ? 5 : args.includes("--f4") ? 4 : 0;
const observationBase = "e223bced951e313f30f969e7c6d5a623e403dd51";
assert.equal(process.platform, "win32", "Final exit acceptance checks Windows Job Objects");
mkdirSync(logRoot, { recursive: true });
const batch = mkdtempSync(join(logRoot, "isolated-"));
const checkout = join(batch, "source");
mkdirSync(checkout);

function command(program, args, options = {}) {
  const run = spawnSync(program, args, {
    cwd: root, encoding: "utf8", timeout: 120_000, windowsHide: true, ...options,
  });
  if (run.error) throw run.error;
  return run;
}
function success(run, label) {
  assert.equal(run.status, 0, `${label}\n${run.stdout ?? ""}\n${run.stderr ?? ""}`);
  return run.stdout;
}
function fixture(name) {
  return readFileSync(join(import.meta.dirname, name), "utf8");
}
function instrument(path, anchor, replacement) {
  const file = join(checkout, bridge, path);
  const source = readFileSync(file, "utf8");
  assert.equal(source.split(anchor).length, 2, `Expected one seam in ${path} at ${anchor.split("\n")[0].trim()}`);
  writeFileSync(file, source.replace(anchor, replacement));
}
function replaceCase(name, end, file) {
  replaceSection(`#[test]\nfn ${name}() {`, end, fixture(file).trimEnd() + "\n");
}
function replaceSection(anchor, end, replacement) {
  const path = join(checkout, bridge, "acceptance.rs");
  const source = readFileSync(path, "utf8");
  const start = source.indexOf(anchor);
  const boundary = source.indexOf(end, start);
  assert.ok(start >= 0 && boundary > start, anchor);
  writeFileSync(path, source.slice(0, start) + replacement + source.slice(boundary));
}
const head = phase ? success(command("git", ["rev-parse", "HEAD"]), "identify candidate").trim() : observationBase;
console.log(`Acceptance source ${head} (${phase ? `F${phase} candidate` : "original observation"})\nExit host: ${wry ? "Tauri/Wry" : "native Windows process"}\nEvidence ${batch}`);
const archive = join(batch, "source.tar");
success(command("git", ["archive", "--format=tar", `--output=${archive}`, head]), "archive source");
success(command("tar", ["-xf", archive, "-C", checkout]), "extract source");
if (phase) {
  const changed = success(command("git", ["diff", "--name-only", "HEAD", "--", "src-tauri/src"]), "identify candidate overlays")
    .trim().split(/\r?\n/).filter(Boolean);
  for (const path of changed) {
    copyFileSync(join(root, path), join(checkout, path));
    console.log(`Candidate overlay ${path}`);
  }
}
appendFileSync(join(checkout, bridge, "tests.rs"), "\n" + fixture("r4-6-lifecycle-regressions.rs"));
writeFileSync(join(checkout, bridge, "acceptance.rs"), fixture("r4-6-final-acceptance.rs"));
if (wry) {
  const buildScript = join(checkout, "src-tauri/build.rs");
  const source = readFileSync(buildScript, "utf8");
  const main = "fn main() {";
  assert.equal(source.split(main).length, 2, "Expected isolated build-script entry");
  writeFileSync(buildScript, source.replace(main, `${main}
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
`));
  instrument("acceptance.rs", `        wait_for_exit_request(&dir, &mode, first, second, port);
        run_exit_callback(&dir);
        std::process::exit(0);`, `        let mut context = tauri::generate_context!();
        context.config_mut().app.windows.clear();
        let app = tauri::Builder::<tauri::Wry>::default().any_thread().build(context).unwrap();
        ${phase >= 6 ? `let exit_ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        app.run(move |app, event| {
            if matches!(event, tauri::RunEvent::Ready) {
                wait_for_exit_request(&dir, &mode, first, second, port);
                app.exit(0);
                app.exit(0);
            }
            let requested = matches!(event, tauri::RunEvent::ExitRequested { .. });
            if matches!(event, tauri::RunEvent::Exit) {
                save_exit_snapshot(&dir);
            }
            crate::handle_application_exit(app, event, &exit_ready);
            if requested && !dir.join("shutdown.requested").exists() {
                mark(&dir, "shutdown.requested");
            }
        });` : `app.run(move |app, event| match event {
            tauri::RunEvent::Ready => {
                wait_for_exit_request(&dir, &mode, first, second, port);
                app.exit(0);
            }
            tauri::RunEvent::Exit => run_exit_callback(&dir),
            _ => (),
        });`}`);
}
appendFileSync(join(checkout, bridge, "mod.rs"), "\n#[cfg(test)]\nmod acceptance;\n");
appendFileSync(join(checkout, bridge, "ssh/ssh_auth.rs"), "\n" + fixture("r4-6-acceptance-auth.rs"));
if (phase >= 4) {
  instrument("ssh/ssh_auth.rs", "super::super::mark_interactive_connecting().unwrap();",
    "super::super::mark_interactive_connecting(id).unwrap();");
  replaceCase("connection_lifecycle", "\nstruct Transport;", "r4-6-fixed-connection.rs");
}
if (phase >= 5) {
  replaceCase("background_lifecycle", "\nmod process_exit {", "r4-6-fixed-background.rs");
  instrument("acceptance.rs", '            fs::write(path, request["port"].to_string()).unwrap();',
    `            fs::write(path, request["port"].to_string()).unwrap();
            if request["port"] == 27892 { return Err("remoteFailed".into()); }`);
}
if (phase >= 6) {
  replaceSection("        match mode {", '\n        mark(&dir, "release");',
    "        assert_shutdown_result(&dir, mode, &exit_state, completed, [primary_started, primary_done, additional_started, additional_done]);");
  replaceSection("    fn run_exit_callback(dir: &Path) {", "\n    fn observe(mode: &str) {",
    fixture("r4-6-fixed-exit.rs").trimEnd() + "\n");
  instrument("acceptance.rs", 'let (first, primary) = managed_resource(&dir, "primary", mode == "busy");',
    'logging::initialize(dir.join("logs"));\n        let (first, primary) = managed_resource(&dir, "primary", matches!(mode.as_str(), "busy" | "timeout"));');
  instrument("acceptance.rs", 'if mode == "contended" {', 'if matches!(mode, "contended" | "lock-timeout") {');
  instrument("acceptance.rs", '        mark(&dir, "quit");', `        mark(&dir, "quit");
        wait_until("shutdown was not requested", LIMIT, || dir.join("shutdown.requested").exists());
        if matches!(mode, "busy" | "contended") {
            assert!(app.0.try_wait().unwrap().is_none());
            mark(&dir, "release");
        }`);
  instrument("acceptance.rs", '        mark(&dir, "release");\n    }', "    }");
  instrument("acceptance.rs", '["normal", "busy", "contended"]', '["normal", "busy", "contended", "timeout", "lock-timeout"]');
}
for (const [path, signature, body] of [
  ["ssh/mod.rs", "pub fn targets() -> BridgeResult<Vec<RemoteTarget>> {",
    "return Ok(vec![super::acceptance::target(\"fixture-a\"), super::acceptance::target(\"fixture-b\")]);"],
  ["ssh/mod.rs", "pub fn fingerprint(target_id: &str) -> BridgeResult<String> {",
    "return Ok(super::acceptance::fingerprint(target_id));"],
  ["ssh/mod.rs", "pub fn remote(alias: &str, mut request: serde_json::Value) -> BridgeResult<serde_json::Value> {",
    "return super::acceptance::remote(alias, &request);"],
  ["mod.rs", "fn test_proxy_egress(expected_generation: Option<u64>, only_unchecked: bool) -> BridgeResult<()> {",
    "return Ok(());"],
  ["mod.rs", "fn restore_vscode_network(state: &mut Store) -> BridgeResult<()> {", "return Ok(());"],
  ["mod.rs", "fn configure_discovered_tools(target_id: &str, fingerprint: &str, generation: u64, ticket: u64) {",
    "return;"],
]) {
  const acceptance = path.includes("/") ? "super::acceptance" : "acceptance";
  instrument(path, signature, `${signature}\n    #[cfg(test)]\n    if ${acceptance}::enabled() { ${body} }`);
}
instrument("tooling/extension.rs", "pub(super) fn discover(alias: &str) -> BridgeResult<Inspection> {",
  "pub(super) fn discover(alias: &str) -> BridgeResult<Inspection> {\n    #[cfg(test)]\n    if super::acceptance::enabled() { return Err(\"remoteUnsupported\".into()); }");
const authTaken = phase ? "    let password = session" : "    if session.password_cache_eligible {";
instrument("ssh/ssh_auth.rs", authTaken, "    #[cfg(test)]\n    super::acceptance::auth_taken(session_id);\n" + authTaken);
const shutdownReceipt = phase >= 6 ? "        let _ = finished.send(outcome);" : "        let _ = finished.send(());";
instrument("mod.rs", shutdownReceipt, "        #[cfg(test)]\n        acceptance::shutdown_finished();\n" + shutdownReceipt);
instrument("mod.rs", "        let post_connect_started = Instant::now();",
  "        #[cfg(test)]\n        let _worker = acceptance::PostConnectWorker;\n        let post_connect_started = Instant::now();");

const manifest = join(checkout, "src-tauri/Cargo.toml");
const build = command("cargo", ["test", "--manifest-path", manifest, "--lib", "--locked", "--offline", "--no-run",
  "--message-format=json"], {
  timeout: 900_000, maxBuffer: 24 * 1024 * 1024,
  env: { ...process.env, CARGO_TARGET_DIR: join(logRoot, wry ? "target-wry" : "target-native") },
});
const buildLog = join(batch, "build.jsonl");
writeFileSync(buildLog, build.stdout);
writeFileSync(join(batch, "build.err"), build.stderr);
assert.equal(build.status, 0, `Acceptance archive failed to compile. See ${buildLog} and build.err`);
const messages = build.stdout.trim().split(/\r?\n/).map(line => JSON.parse(line));
const testBinary = messages.find(
  entry => entry.reason === "compiler-artifact" && entry.target.name === "proxyenv_lib"
    && entry.profile.test && entry.executable,
);
assert.ok(testBinary?.executable);
const executable = join(batch, "acceptance.exe");
copyFileSync(testBinary.executable, executable);
testBinary.executable = executable;
const isolatedBuildLog = join(batch, "isolated-build.jsonl");
writeFileSync(isolatedBuildLog, messages.map(message => JSON.stringify(message)).join("\n"));

const regressionEnv = { ...process.env };
delete regressionEnv.ROUTESPAN_R46_SCRATCH;
delete regressionEnv.ROUTESPAN_R46_EXIT_DIR;
const regression = command(process.execPath, [join(import.meta.dirname, "run-r4-6-lifecycle-regressions.mjs"), isolatedBuildLog],
  { env: regressionEnv });
writeFileSync(join(batch, "regressions.log"), regression.stdout + regression.stderr);
process.stdout.write(success(regression, "six existing lifecycle cases"));

const cases = ["connection_lifecycle"];
if (!phase || phase >= 5) cases.push("background_lifecycle");
if (!phase || phase >= 6) cases.push("application_exit");
for (const name of cases) {
  const scratch = mkdtempSync(join(batch, `${name}-`));
  const isolatedEnv = { ...process.env, ROUTESPAN_R46_SCRATCH: scratch };
  for (const name of ["HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "TEMP", "TMP", "XDG_CONFIG_HOME"]) {
    isolatedEnv[name] = scratch;
  }
  const run = command(executable, ["--exact", `features::remote_bridge::acceptance::${name}`, "--nocapture"],
    { timeout: 60_000, env: isolatedEnv });
  writeFileSync(join(scratch, "acceptance.log"), run.stdout + run.stderr);
  const output = success(run, name);
  assert.match(output, /test result: ok\. 1 passed; 0 failed;/);
  process.stdout.write(output);
}
