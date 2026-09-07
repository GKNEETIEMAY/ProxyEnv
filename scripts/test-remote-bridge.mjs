import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import ts from "typescript";

const git = process.platform === "win32" ? spawnSync("where.exe",["git"],{encoding:"utf8"}).stdout?.trim().split(/\r?\n/)[0] : undefined;
const shell = process.env.PROXYENV_TEST_SHELL || (git ? resolve(dirname(git),"../bin/bash.exe") : "/bin/sh");
const available = existsSync(shell);
const root = resolve(".debug-tmp");
mkdirSync(root,{recursive:true});
const script = readFileSync("src-tauri/src/features/remote_bridge/remote.sh","utf8");
const posix = p => p.replaceAll("\\", "/").replace(/^([A-Za-z]):/, (_, drive) => `/${drive.toLowerCase()}`);
function fixture() {
  const directory=mkdtempSync(join(root,"bridge-test-"));
  const home=join(directory,"home"), bin=join(directory,"bin");
  mkdirSync(home);mkdirSync(bin);
  const mock=(name,body)=>writeFileSync(join(bin,name),`#!/bin/sh\n${body}\n`,{mode:0o755});
  mock("uname","printf Linux");
  mock("ss",'printf "%s" "${TEST_LISTENERS:-}"');
  // MSYS has no flock or Unix permission model. Only these platform adapters
  // are mocked; file content, hashing, writes, readback and restore are real.
  mock("flock","exit 0");
  if(process.platform==="win32") mock("stat",'[ "$2" != %a ] || { printf 700; exit; }; /usr/bin/stat "$@"');
  mock("mv",'for target do :; done; if [ "${TEST_FAIL_REPLACE:-}" = 1 ] && [ ! -e "$HOME/.replace-failed" ]; then case "$target" in *.config.toml|*bridge.json) touch "$HOME/.replace-failed"; exit 1;; esac; fi; /usr/bin/mv "$@"');
  mock("codex",'printf "%s\\n" "${TEST_CODEX_VERSION:-codex-cli 0.134.0}"');
  mock("claude",'printf "2.1.0 (Claude Code)\\n"');
  const run=(operation,tool="codex",port=25721,expected="absent",env={},expectedState="absent")=>{
    let backupHash="absent";
    if(operation==="restore") { const reviewed=run("restore-preview",tool,port); if(reviewed.error) return reviewed; if(expected==="absent") expected=reviewed.expectedHash; backupHash=reviewed.backupHash; }
    const input=`export HOME='${posix(home)}'\nexport PATH='${posix(bin)}:/usr/bin:/bin'\noperation='${operation}'\ntool='${tool}'\nport=${port}\nports='17897'\nexpected='${expected}'\nexpected_backup='${backupHash}'\nexpected_state='${expectedState}'\nscheme='http'\n${script}`;
    const result=spawnSync(shell,["-s"],{input,encoding:"utf8",timeout:20000,env:{...process.env,CODEX_HOME:"",CLAUDE_CONFIG_DIR:"",HOME:posix(home),PATH:`${posix(bin)}:/usr/bin:/bin`,...env}});
    assert.equal(result.status,0,result.stderr || result.error?.message);
    return JSON.parse(result.stdout.trim());
  };
  return {home,run,cleanup:()=>{ assert.ok(resolve(directory).startsWith(root + (process.platform==="win32"?"\\":"/"))); rmSync(directory,{recursive:true,force:true}); }};
}
test("remote port preflight rejects occupation and wildcard listeners",{skip:!available},()=>{
  const f=fixture();try {
    assert.equal(f.run("check").verified,true);
    assert.equal(f.run("check","codex",25721,"absent",{TEST_LISTENERS:"LISTEN 0 128 127.0.0.1:17897 0.0.0.0:*"}).error,"portInUse");
    assert.equal(f.run("verify","codex",25721,"absent",{TEST_LISTENERS:"LISTEN 0 128 0.0.0.0:17897 0.0.0.0:*"}).error,"unsafeBinding");
    assert.equal(f.run("verify","codex",25721,"absent",{TEST_LISTENERS:"LISTEN 0 128 127.0.0.1:17897 0.0.0.0:*"}).verified,true);
    assert.equal(f.run("verify","codex",25721,"absent",{TEST_LISTENERS:"LISTEN 0 128 [::]:17897 [::]:*"}).error,"unsafeBinding");
  } finally { f.cleanup(); }
});
for(const tool of ["codex","claude"]) test(`${tool}: preview, apply, stale preview, repeat apply and restore`,{skip:!available},()=>{
  const f=fixture();try {
    const folder=join(f.home,tool==="codex"?".codex":".claude");
    const file=join(folder,tool==="codex"?"proxyenv_bridge.config.toml":"proxyenv-bridge.json");
    const preview=f.run("preview",tool);assert.equal(preview.expectedHash,"absent");assert.equal(existsSync(folder),false);
    assert.equal(f.run("apply",tool).configured,true);
    const applied=readFileSync(file,"utf8");assert.match(applied,/127\.0\.0\.1:25721/);
    assert.equal(f.run("apply",tool,25722).error,"configConflict");assert.equal(readFileSync(file,"utf8"),applied);
    const next=f.run("preview",tool);assert.equal(next.previousPort,25721);
    assert.equal(f.run("apply",tool,25722,next.expectedHash,{},next.stateHash).configured,true);
    assert.equal(f.run("restore",tool).configured,false);assert.equal(existsSync(file),false);
    assert.equal(f.run("restore",tool).error,"noBackup");
  } finally { f.cleanup(); }
});
test("Claude onboarding is completed without pre-trusting a project or discarding state",{skip:!available},()=>{
  const f=fixture();try {
    const stateFile=join(f.home,".claude.json");
    const original={theme:"dark",nested:{hasCompletedOnboarding:false},projects:{}};
    writeFileSync(stateFile,`${JSON.stringify(original)}\n`);
    const preview=f.run("preview","claude");
    assert.equal(preview.onboardingRequired,true);
    assert.equal(f.run("apply","claude",25721,preview.expectedHash,{},preview.stateHash).configured,true);
    const applied=JSON.parse(readFileSync(stateFile,"utf8"));
    assert.equal(applied.hasCompletedOnboarding,true);
    assert.equal(applied.theme,"dark");
    assert.equal(applied.nested.hasCompletedOnboarding,false);
    assert.deepEqual(applied.projects,{});
    const unchanged=f.run("preview","claude");
    assert.equal(unchanged.onboardingRequired,false);
  } finally { f.cleanup(); }
});
test("Claude onboarding refuses malformed state without writing an overlay",{skip:!available},()=>{
  const f=fixture();try {
    writeFileSync(join(f.home,".claude.json"),'{"hasCompletedOnboarding":"not-a-boolean"}\n');
    assert.equal(f.run("preview","claude").error,"configConflict");
    assert.equal(existsSync(join(f.home,".claude/proxyenv-bridge.json")),false);
  } finally { f.cleanup(); }
});
test("third-party edit and unknown config never leak or get overwritten",{skip:!available},()=>{
  const f=fixture();try {
    f.run("apply");const file=join(f.home,".codex/proxyenv_bridge.config.toml");
    const external='api_key = "secret-fixture-never-return"\n';writeFileSync(file,external);
    const result=f.run("preview");assert.equal(result.error,"configConflict");assert.ok(!JSON.stringify(result).includes("secret-fixture"));
    assert.equal(f.run("restore").error,"configConflict");assert.equal(readFileSync(file,"utf8"),external);
  } finally { f.cleanup(); }
});
test("Codex older profile format and custom home fail before writes",{skip:!available},()=>{
  const f=fixture();try {
    assert.equal(f.run("preview","codex",25721,"absent",{TEST_CODEX_VERSION:"codex-cli 0.133.0"}).error,"cliUnsupported");
    assert.equal(f.run("preview","codex",25721,"absent",{CODEX_HOME:"/different"}).error,"customHome");
    assert.equal(existsSync(join(f.home,".codex")),false);
  } finally { f.cleanup(); }
});
test("all remote UI labels and error categories are localized",async()=>{
  const source=readFileSync("src/shared/i18n/remote-bridge.ts","utf8");
  const compiled=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext}}).outputText;
  const {remoteBridgeMessages:messages,bridgeError}=await import(`data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`);
  for(const [locale,copy] of Object.entries(messages)) {
    assert.deepEqual(Object.keys(copy).sort(),Object.keys(messages.en).sort(),locale);
    for(const state of ["disconnected","connecting","connected","stale","unavailable","error"]) assert.ok(copy.rbStates[state]);
    for(const code of ["sshAuth","forwardDenied","unsafeBinding","configConflict","rootForbidden","portInUse","activeChanged","ccUnavailable","bridgeUnavailable","noCapability","alreadyConnected","stateUnavailable","processFailed","remoteFailed","networkFailed","targetUnsupported","portAllocationFailed","portRace","random-secret"]) assert.ok(bridgeError(code,copy) && !bridgeError(code,copy).includes("random-secret"));
    assert.equal(
      bridgeError({code:"ccUnavailable",phase:"localDetection",target:"ccSwitch",retryable:true},copy),
      copy.rbCcError,
    );
  }
});

test("CLI launch commands are hidden until their remote overlays are verified",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(page,/v-if="summary\.codexConfigured"[\s\S]*?\{\{ codexLaunch \}\}/);
  assert.match(page,/v-if="summary\.claudeConfigured"[\s\S]*?\{\{ claudeLaunch \}\}/);
  assert.match(page,/v-else>\{\{ copy\.rbConfigureBeforeLaunch \}\}/);
});

test("remote target paths reuse the Windows extended-path display cleanup",async()=>{
  const source=readFileSync("src/shared/utils/path.ts","utf8");
  const compiled=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext}}).outputText;
  const {withoutWindowsExtendedPathPrefix}=await import(`data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`);
  assert.equal(withoutWindowsExtendedPathPrefix("\\\\?\\C:\\Users\\demo\\.ssh\\config"),"C:\\Users\\demo\\.ssh\\config");
  assert.equal(withoutWindowsExtendedPathPrefix("\\??\\D:\\SSH\\config"),"D:\\SSH\\config");
  assert.equal(withoutWindowsExtendedPathPrefix("\\\\?\\UNC\\server\\share\\config"),"\\\\server\\share\\config");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(page,/withoutWindowsExtendedPathPrefix\(target\.configPath\)/);
  assert.match(page,/withoutWindowsExtendedPathPrefix\(activeTarget\.configPath\)/);
});

test("failed atomic replace rolls back and preserves a usable recovery journal",{skip:!available},()=>{
  const f=fixture();try {
    assert.equal(f.run("apply").configured,true);
    const file=join(f.home,".codex/proxyenv_bridge.config.toml");
    const before=readFileSync(file,"utf8");
    const preview=f.run("preview");
    assert.equal(f.run("apply","codex",25722,preview.expectedHash,{TEST_FAIL_REPLACE:"1"}).error,"remoteFailed");
    assert.equal(readFileSync(file,"utf8"),before);
    assert.equal(f.run("restore").configured,false);
    assert.equal(existsSync(file),false);
  } finally { f.cleanup(); }
});
