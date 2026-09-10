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
  mock("claude",'if [ "$1" = --version ]; then printf "2.1.0 (Claude Code)\\n"; exit; fi; case "${TEST_CLAUDE_VERIFY:-verified}" in verified) printf \'{"result":"PROXYENV_VERIFY_OK"}\\n\';; auth) printf \'login required secret-fixture\\n\' >&2; exit 1;; route) printf \'gateway connection refused secret-fixture\\n\' >&2; exit 1;; timeout) exit 124;; *) printf \'unexpected secret-fixture\\n\' >&2; exit 1;; esac');
  mock("curl",'[ "${TEST_CURL_RESULT:-ok}" = ok ]');
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
test("Claude request verification returns only an allowlisted state",{skip:!available},()=>{
  const f=fixture();try {
    assert.equal(f.run("apply","claude").configured,true);
    const cases = [
      ["verified","verified"],
      ["auth","authenticationRequired"],
      ["route","routeUnavailable"],
      ["timeout","timedOut"],
      ["failed","failed"],
    ];
    for (const [fixtureState, expected] of cases) {
      const result=f.run("tool-verify","claude",25721,"absent",{TEST_CLAUDE_VERIFY:fixtureState});
      assert.deepEqual(result,{verification:expected});
      assert.ok(!JSON.stringify(result).includes("secret-fixture"));
    }
    assert.equal(f.run("tool-verify","codex").error,"invalidRequest");
  } finally { f.cleanup(); }
});
test("server internet observation is independent from bridge port checks",{skip:!available},()=>{
  const f=fixture();try {
    assert.equal(f.run("internet").internet,"reachable");
    assert.equal(f.run("internet","codex",25721,"absent",{TEST_CURL_RESULT:"failed"}).internet,"unreachable");
  } finally { f.cleanup(); }
});
test("remote status UI uses shared checks and keeps network capabilities independent",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const shell=readFileSync("src/app/AppShell.vue","utf8");
  for(const component of ["CheckRow","StatusIndicator","LastChecked"]) assert.match(page,new RegExp(`<${component}`));
  assert.match(page,/remoteBackend\.checkNetwork/);
  assert.match(page,/remoteBackend\.detectCc/);
  assert.match(page,/serverInternetCheck/);
  assert.match(page,/localProxyCheck/);
  assert.match(page,/ccCheck/);
  assert.match(page,/emit\("connected", outcome\.summary\)/);
  assert.match(shell,/<div class="view-stage">/);
  assert.match(shell,/<div class="view-outlet">/);
  assert.match(shell,/<div v-show="view === 'local'" class="view-pane">/);
  assert.match(shell,/<div v-if="remoteViewMounted" v-show="view === 'remote'" class="view-pane">/);
  assert.doesNotMatch(shell,/view-fade/);
  assert.match(shell,/@connected="acceptRemoteBridgeSummary"/);
  assert.doesNotMatch(page,/remote-steps|reviewedRequest|authPromptCopy\.notice/);
  assert.match(page,/remoteBackend\.preview\(selected\)\.then\(\(\) => remoteBackend\.connect\(selected\)\)/);
  for(const file of ["StatusIndicator.vue","CheckRow.vue","HelpHint.vue","LastChecked.vue"]) {
    assert.ok(existsSync(join("src/shared/components",file)),file);
  }
});
test("interactive SSH auth is PTY-backed and only reuses DPAPI-protected bridge passwords",()=>{
  const ssh=readFileSync("src-tauri/src/features/remote_bridge/ssh.rs","utf8");
  const auth=readFileSync("src-tauri/src/features/remote_bridge/ssh_auth.rs","utf8");
  const credentials=readFileSync("src-tauri/src/features/remote_bridge/credential_cache.rs","utf8");
  const commands=readFileSync("src-tauri/src/commands/remote_bridge.rs","utf8");
  const runtime=readFileSync("src-tauri/src/lib.rs","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(ssh,/-oBatchMode=yes/);
  assert.match(ssh,/-oBatchMode=no/);
  assert.match(ssh,/-oStrictHostKeyChecking=yes/);
  assert.match(ssh,/-oStrictHostKeyChecking=ask/);
  assert.match(ssh,/-oPasswordAuthentication=yes/);
  assert.match(ssh,/-oKbdInteractiveAuthentication=yes/);
  assert.match(auth,/native_pty_system\(\)/);
  for(const promptType of ["Password","KeyPassphrase","HostKeyConfirmation","VerificationCode","KeyboardInteractive","Unknown"]) assert.match(auth,new RegExp(promptType));
  assert.match(auth,/PROMPT_WAIT_TIMEOUT/);
  assert.doesNotMatch(auth,/PROMPT_FALLBACK_DELAY/);
  assert.match(auth,/current_prompt/);
  assert.match(auth,/prompt_id/);
  assert.match(auth,/sshAuthPromptUnavailable/);
  assert.match(auth,/TerminalControlParser/);
  assert.match(auth,/\\x1b\[1;1R/);
  assert.match(auth,/COMPLETION_WAIT_TIMEOUT/);
  assert.match(auth,/interactive_check_remote_command/);
  assert.doesNotMatch(auth,/source_after_auth/);
  assert.match(auth,/authenticated && remote_result_ready/);
  assert.match(auth,/Zeroizing::new\(response\)/);
  assert.match(auth,/credential_cache::protect/);
  assert.match(auth,/credential_cache::reveal/);
  assert.match(auth,/PromptType::Password \| PromptType::HostKeyConfirmation/);
  assert.match(credentials,/CryptProtectData/);
  assert.match(credentials,/CryptUnprotectData/);
  assert.match(credentials,/CRYPTPROTECT_UI_FORBIDDEN/);
  assert.match(credentials,/clear_if_matches/);
  const tunnelBody=ssh.slice(ssh.indexOf("pub fn tunnel"),ssh.indexOf("pub(super) fn extension_remote"));
  assert.match(tunnelBody,/remote_target_command\(&request\.target_id\)/);
  assert.doesNotMatch(tunnelBody,/let \(mut cmd, destination\) = target_command/);
  assert.match(credentials,/prompt\.contains\("password"\)/);
  for(const forbiddenPrompt of ["passphrase","verification","one-time","otp"]) {
    assert.match(credentials,new RegExp(`prompt\\.contains\\(\\"${forbiddenPrompt}\\"\\)`));
  }
  assert.doesNotMatch(auth,/\.arg\(response\)/);
  const submitBody=auth.slice(auth.indexOf("pub fn submit"),auth.indexOf("pub fn confirm_host"));
  assert.match(submitBody,/matching_prompt\(session\.current_prompt\.as_ref\(\), prompt_id\)/);
  assert.doesNotMatch(submitBody,/parse_ssh_prompt/);
  for(const command of ["ssh_auth_begin","ssh_auth_state","ssh_auth_submit","ssh_auth_confirm_host","ssh_auth_finish","ssh_auth_cancel"]) {
    assert.match(commands,new RegExp(`fn ${command}`));
    assert.match(runtime,new RegExp(`remote_bridge::${command}`));
  }
  assert.match(page,/bridgeErrorCode\(cause\) === "sshAuth"/);
  assert.match(page,/authPrompt\?\.secret \? 'password' : 'text'/);
  assert.match(page,/remoteBackend\.sshAuthSubmit/);
  assert.match(page,/remoteBackend\.sshAuthConfirmHost/);
  assert.match(page,/authPromptUnavailable/);
  assert.match(page,/authCompleting/);
  assert.match(page,/!authPromptUnavailable && !authSession\?\.auth\.authenticated/);
  assert.match(page,/retryInteractiveAuth/);
  assert.match(page,/authSession\.diagnostic\.cprRequests/);
  assert.doesNotMatch(page,/Authentication response|认证响应/);
});
test("managed proxy terminal is one-click, shell-scoped, and keeps manual export advanced",()=>{
  const ssh=readFileSync("src-tauri/src/features/remote_bridge/ssh.rs","utf8");
  const bridge=readFileSync("src-tauri/src/features/remote_bridge/mod.rs","utf8");
  const commands=readFileSync("src-tauri/src/commands/remote_bridge.rs","utf8");
  const runtime=readFileSync("src-tauri/src/lib.rs","utf8");
  const state=readFileSync("src/features/remote-bridge/state.ts","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(ssh,/ManagedTerminal/);
  assert.match(ssh,/CREATE_NEW_CONSOLE/);
  assert.match(ssh,/CreateProcessW/);
  assert.match(ssh,/STARTUPINFOW/);
  assert.match(ssh,/WindowsPowerShell\/v1\.0\/powershell\.exe/);
  assert.match(ssh,/-NoExit/);
  assert.match(ssh,/PROXYENV_SSH_LAUNCH/);
  assert.match(ssh,/SSH_ASKPASS_REQUIRE/);
  assert.match(ssh,/exec \\\"\$\{\{SHELL:-\/bin\/sh\}\}\\\" -i/);
  for(const name of ["HTTP_PROXY","HTTPS_PROXY","ALL_PROXY","NO_PROXY"]) assert.match(bridge,new RegExp(name));
  assert.match(commands,/fn remote_bridge_launch_proxy_terminal/);
  assert.match(commands,/fn remote_bridge_launch_manual_terminal/);
  assert.match(runtime,/remote_bridge::remote_bridge_launch_proxy_terminal/);
  assert.match(runtime,/remote_bridge::remote_bridge_launch_manual_terminal/);
  assert.match(state,/launchProxyTerminal: \(\) => invoke<void>\("remote_bridge_launch_proxy_terminal"\)/);
  assert.match(state,/launchManualTerminal: \(\) => invoke<void>\("remote_bridge_launch_manual_terminal"\)/);
  assert.match(page,/@click="launchProxyTerminal"/);
  assert.match(page,/remoteBackend\.launchManualTerminal\(\)/);
  assert.match(page,/<details class="remote-advanced">/);
  assert.match(page,/copyValue\(summary\.environment\)/);
  const advanced=page.slice(page.indexOf('<details class="remote-advanced">'),page.indexOf('</details>',page.indexOf('<details class="remote-advanced">')));
  assert.match(advanced,/remoteBackend\.launchMobaxterm\(\)/);
  assert.match(advanced,/remoteBackend\.openVscode\(summary\.target!\.id\)/);
  assert.match(advanced,/copy\.rbExternalClientHint/);
  assert.match(ssh,/fn launch_terminal/);
  assert.match(ssh,/launch_terminal\(target_id, fingerprint, None\)/);
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
    for(const key of ["rbAuthInteractionError","rbAuthCompleting","rbAuthCompletingTitle","rbAuthCompletingDescription","rbAuthRemoteCheckFailure","rbAuthRemoteCheckFailureTitle","rbAuthPromptUnavailableTitle","rbAuthPromptUnavailableDescription","rbAuthPromptUnavailableHint","rbAuthRetry","rbAuthOpenDiagnostic","rbAuthDiagnosticBytes","rbAuthDiagnosticPrintable","rbAuthDiagnosticCpr","rbAuthDiagnosticPrompt","rbAuthDiagnosticMarker","rbAuthDiagnosticResult","rbAuthDiagnosticClosed","rbAuthCompletionTimeout","rbVerifyClaude","rbVerifyClaudeHint","rbToolVerifyPending","rbToolVerified","rbToolAuthRequired","rbToolRouteUnavailable","rbToolVerifyTimedOut","rbToolVerifyFailed"]) assert.ok(copy[key],`${locale}:${key}`);
    for(const code of ["sshAuth","sshAuthRejected","sshAuthPromptChanged","sshAuthCompletionTimeout","hostKeyChanged","ptyUnavailable","sshAuthSessionMissing","forwardDenied","unsafeBinding","configConflict","rootForbidden","portInUse","activeChanged","ccUnavailable","bridgeUnavailable","toolNotConfigured","toolVerificationUnsupported","noCapability","alreadyConnected","stateUnavailable","processFailed","remoteFailed","networkFailed","targetUnsupported","portAllocationFailed","portRace","random-secret"]) assert.ok(bridgeError(code,copy) && !bridgeError(code,copy).includes("random-secret"));
    assert.equal(
      bridgeError({code:"ccUnavailable",phase:"localDetection",target:"ccSwitch",retryable:true},copy),
      copy.rbCcError,
    );
  }
});

test("CLI launch commands are hidden until their remote overlays are configured",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const adapters=readFileSync("src/features/remote-bridge/tool-adapters.ts","utf8");
  assert.match(page,/v-for="tool in remoteTools"/);
  assert.match(page,/v-if="tool\.launch"/);
  assert.match(page,/v-else>\{\{ copy\.rbConfigureBeforeLaunch \}\}/);
  assert.match(adapters,/launch: \(summary\) => configured\(summary\) \? definition\.launchCommand : ""/);
});

test("Claude and Codex CLI operations use the shared RemoteToolAdapter boundary",()=>{
  const backend=readFileSync("src-tauri/src/features/remote_bridge/tool_adapter.rs","utf8");
  const bridge=readFileSync("src-tauri/src/features/remote_bridge/mod.rs","utf8");
  const frontend=readFileSync("src/features/remote-bridge/tool-adapters.ts","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const dialog=readFileSync("src/features/remote-bridge/components/RemoteToolDialog.vue","utf8");
  for(const method of ["detect","inspect","preview","apply","restore","launch","verify","supported_route_modes","compatibility"]) {
    assert.match(backend,new RegExp(`fn ${method}\\(`));
  }
  assert.match(backend,/impl RemoteToolAdapter for CodexCliAdapter/);
  assert.match(backend,/impl RemoteToolAdapter for ClaudeCliAdapter/);
  assert.match(backend,/RemoteToolVerification::VerifyPending/);
  assert.match(bridge,/tool_adapter::by_name\(&tool\)/);
  assert.doesNotMatch(bridge,/fn overlay\(/);
  assert.match(frontend,/export interface RemoteToolAdapter/);
  assert.match(frontend,/summary\.tools\?\.find/);
  for(const source of [page,dialog]) assert.doesNotMatch(source,/tool(?:\.value)?\s*===\s*["'](?:codex|claude)["']/);
});

test("Claude verification is a fixed isolated request and never returns model output",()=>{
  const shell=readFileSync("src-tauri/src/features/remote_bridge/remote.sh","utf8");
  const bridge=readFileSync("src-tauri/src/features/remote_bridge/mod.rs","utf8");
  const ssh=readFileSync("src-tauri/src/features/remote_bridge/ssh.rs","utf8");
  const commands=readFileSync("src-tauri/src/commands/remote_bridge.rs","utf8");
  const runtime=readFileSync("src-tauri/src/lib.rs","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(shell,/--settings "\$file"/);
  assert.match(shell,/--setting-sources ""/);
  assert.match(shell,/--strict-mcp-config/);
  assert.match(shell,/--mcp-config '\{"mcpServers":\{\}\}'/);
  assert.match(shell,/--tools ""/);
  assert.match(shell,/--disallowedTools 'mcp__\*'/);
  assert.match(shell,/--no-session-persistence/);
  assert.match(shell,/Reply with exactly PROXYENV_VERIFY_OK/);
  assert.match(shell,/printf '\{"verification":"%s"\}/);
  assert.match(bridge,/pub fn verify_tool/);
  assert.match(ssh,/remote_target_command/);
  assert.match(ssh,/credential_cache::terminal_payload/);
  assert.match(ssh,/if operation == "tool-verify" \{ 90 \} else \{ 25 \}/);
  assert.match(commands,/fn remote_bridge_tool_verify/);
  assert.match(runtime,/remote_bridge::remote_bridge_tool_verify/);
  assert.match(page,/@click="verifyTool\(tool\.adapter\)"/);
});

test("M7 keeps VS Code Server context and extension location conservative",()=>{
  const ssh=readFileSync("src-tauri/src/features/remote_bridge/ssh.rs","utf8");
  const vscode=readFileSync("src-tauri/src/features/remote_bridge/vscode.rs","utf8");
  const helper=readFileSync("scripts/remote-extension/main.mjs","utf8");
  const backend=readFileSync("src-tauri/src/features/remote_bridge/extension.rs","utf8");
  const state=readFileSync("src/features/remote-bridge/state.ts","utf8");
  const dialog=readFileSync("src/features/remote-bridge/components/RemoteToolDialog.vue","utf8");
  const messages=readFileSync("src/shared/i18n/remote-bridge.ts","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const commands=readFileSync("src-tauri/src/commands/remote_bridge.rs","utf8");
  const runtime=readFileSync("src-tauri/src/lib.rs","utf8");
  assert.match(helper,/\.vscode-server-insiders/);
  assert.match(helper,/VSCODE_AGENT_FOLDER/);
  assert.match(helper,/status = selected \? 'detected' : contexts\.length \? 'ambiguous' : 'unsupported'/);
  assert.match(helper,/candidateCount: candidates\.length/);
  assert.match(helper,/versions\.length === 1 \? versions\[0\] : ''/);
  assert.doesNotMatch(helper,/const candidate = candidates\.length === 1/);
  assert.match(backend,/pub struct VscodeRemoteContext/);
  assert.match(state,/export type ExtensionLocationState = "locationUnknown" \| "activeUnknown" \| "remoteConfirmed"/);
  assert.match(messages,/Developer: Show Running Extensions/);
  assert.match(dialog,/locationConfirmed\.value = false;\s+inspection\.value = undefined/);
  assert.match(page,/remoteBackend\.revealTargetConfig/);
  assert.match(page,/remoteBackend\.openVscodeSettings/);
  assert.match(commands,/remote_bridge_reveal_target_config/);
  assert.match(runtime,/remote_bridge_open_vscode_settings/);
  assert.match(ssh,/pub fn target_config_path/);
  assert.match(vscode,/ssh::target_config_path/);
  assert.match(vscode,/System::new_all/);
  assert.match(vscode,/process\.exe\(\)/);
  const targetSelection=page.slice(page.indexOf('<template v-if="!checked">'),page.indexOf('<template v-else>'));
  assert.doesNotMatch(targetSelection,/launchSshTerminal/);
  assert.doesNotMatch(targetSelection,/remoteBackend\.openVscode\(/);
  assert.doesNotMatch(targetSelection,/remoteBackend\.launchMobaxterm\(/);
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
