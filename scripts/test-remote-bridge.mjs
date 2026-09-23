import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync, rmSync, statSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import ts from "typescript";

const git = process.platform === "win32" ? spawnSync("where.exe",["git"],{encoding:"utf8"}).stdout?.trim().split(/\r?\n/)[0] : undefined;
const shell = process.env.PROXYENV_TEST_SHELL || (git ? resolve(dirname(git),"../bin/bash.exe") : "/bin/sh");
const available = existsSync(shell);
const python = process.env.PROXYENV_TEST_PYTHON || (process.platform === "win32"
  ? spawnSync("where.exe",["python"],{encoding:"utf8"}).stdout?.trim().split(/\r?\n/)[0]
  : spawnSync("sh",["-c","command -v python3"],{encoding:"utf8"}).stdout?.trim());
const root = resolve(".debug-tmp");
mkdirSync(root,{recursive:true});
const script = readFileSync("src-tauri/src/features/remote_bridge/remote.sh","utf8");
const legacyCatalogJson = script.match(/codex_catalog_json\(\) \{\s+printf '%s\\n' '([^']+)'/)?.[1];
const posix = p => p.replaceAll("\\", "/").replace(/^([A-Za-z]):/, (_, drive) => `/${drive.toLowerCase()}`);
function fixture({jsonEngine="python3",claudeLocation="path",privateGroup=false,sharedGroup=false}={}) {
  const directory=mkdtempSync(join(root,"bridge-test-"));
  const home=join(directory,"home"), bin=join(directory,"bin");
  mkdirSync(home);mkdirSync(bin);
  const mock=(name,body)=>writeFileSync(join(bin,name),`#!/bin/sh\n${body}\n`,{mode:0o755});
  mock("uname","printf Linux");
  mock("ss",'printf "%s" "${TEST_LISTENERS:-}"');
  // MSYS has no flock or Unix permission model. Only these platform adapters
  // are mocked; file content, hashing, writes, readback and restore are real.
  mock("flock","exit 0");
  if(privateGroup) mock("id",'[ "$1" != -gn ] || { /usr/bin/id -un; exit; }; /usr/bin/id "$@"');
  if(sharedGroup) mock("id",'[ "$1" != -gn ] || { printf lab-users; exit; }; /usr/bin/id "$@"');
  if (python) mock(jsonEngine,`exec '${posix(python).replaceAll("'", "'\\''")}' "$@"`);
  if(process.platform==="win32") mock("stat",'[ "$2" != %a ] || { printf 700; exit; }; /usr/bin/stat "$@"');
  mock("mv",'for target do :; done; if [ "${TEST_FAIL_REPLACE:-}" = 1 ] && [ ! -e "$HOME/.replace-failed" ]; then case "$target" in *config.toml|*settings.json) touch "$HOME/.replace-failed"; exit 1;; esac; fi; /usr/bin/mv "$@"');
  mock("codex",'printf "%s\\n" "${TEST_CODEX_VERSION:-codex-cli 0.134.0}"');
  const claudeBody='if [ "$1" = --version ]; then printf "2.1.227 (Claude Code)\\n"; exit; fi; case "${TEST_CLAUDE_VERIFY:-verified}" in verified) printf \'{"result":"PROXYENV_VERIFY_OK"}\\n\';; auth) printf \'login required secret-fixture\\n\' >&2; exit 1;; route) printf \'gateway connection refused secret-fixture\\n\' >&2; exit 1;; timeout) exit 124;; *) printf \'unexpected secret-fixture\\n\' >&2; exit 1;; esac';
  if(["native","nvm"].includes(claudeLocation)) {
    const userBin=claudeLocation==="native" ? join(home,".local/bin") : join(home,".nvm/versions/node/v22.0.0/bin");
    mkdirSync(userBin,{recursive:true,mode:claudeLocation==="nvm" ? 0o775 : 0o700});
    writeFileSync(join(userBin,"claude"),`#!/bin/sh\n${claudeBody}\n`,{mode:0o755});
  } else if(claudeLocation==="path") mock("claude",claudeBody);
  mock("curl",'[ "${TEST_CURL_RESULT:-ok}" = ok ]');
  const run=(operation,tool="codex",port=25721,expected="absent",env={})=>{
    let backupHash="absent";
    if(operation==="restore") { const reviewed=run("restore-preview",tool,port); if(reviewed.error) return reviewed; if(expected==="absent") expected=reviewed.expectedHash; backupHash=reviewed.backupHash; }
    const profileModel=env.TEST_PROFILE_MODEL || "fixture-model";
    const profileCatalog=env.TEST_PROFILE_CATALOG || JSON.stringify({models:[{slug:profileModel,future:{preserved:true}}],futureRoot:[1,2]});
    const profileHash=createHash("sha256").update(profileModel).update(Buffer.from([0])).update(profileCatalog).digest("hex");
    const needsProfile=tool==="codex" && ["preview","apply"].includes(operation);
    const claudeProfile=env.TEST_CLAUDE_PROFILE || '{"model":"fixture-claude"}';
    const claudeProfileHash=createHash("sha256").update(claudeProfile).digest("hex");
    const needsClaudeProfile=tool==="claude" && ["preview","apply"].includes(operation);
    const sessionToken=env.TEST_SESSION_TOKEN || "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const input=`export HOME='${posix(home)}'\nexport PATH='${posix(bin)}:/usr/bin:/bin'\noperation='${operation}'\ntool='${tool}'\nport=${port}\nports='${env.TEST_REMOTE_PORT ?? 17897}'\nexpected='${expected}'\nexpected_backup='${backupHash}'\nrepair_permissions='${env.TEST_REPAIR_PERMISSIONS === "1" ? "true" : "false"}'\nprotocol='http'\nscheme='http'\nsession_id='0123456789abcdef0123456789abcdef'\nsession_token='${sessionToken}'\nPROXYENV_SESSION_TOKEN="$session_token"\nexport PROXYENV_SESSION_TOKEN\nprofile_model_b64='${needsProfile ? Buffer.from(profileModel).toString("base64") : ""}'\nprofile_catalog_b64='${needsProfile ? Buffer.from(profileCatalog).toString("base64") : ""}'\nprofile_settings_b64='${needsClaudeProfile ? Buffer.from(claudeProfile).toString("base64") : ""}'\nprofile_hash='${needsProfile ? profileHash : needsClaudeProfile ? claudeProfileHash : ""}'\n${script}`;
    const result=spawnSync(shell,["-s"],{input,encoding:"utf8",timeout:60000,env:{...process.env,CODEX_HOME:"",CLAUDE_CONFIG_DIR:"",HOME:posix(home),PATH:`${posix(bin)}:/usr/bin:/bin`,...env}});
    assert.equal(result.status,0,result.stderr || result.error?.message);
    return JSON.parse(result.stdout.trim());
  };
  return {home,run,cleanup:()=>{ assert.ok(resolve(directory).startsWith(root + (process.platform==="win32"?"\\":"/"))); rmSync(directory,{recursive:true,force:true}); }};
}
test("remote port preflight rejects occupation and wildcard listeners",{skip:!available},()=>{
  const f=fixture();try {
    for(const remotePort of [17897, 10809]) {
      const input={TEST_REMOTE_PORT:remotePort};
      assert.equal(f.run("check","codex",25721,"absent",input).verified,true);
      assert.equal(f.run("check","codex",25721,"absent",{...input,TEST_LISTENERS:`LISTEN 0 128 127.0.0.1:${remotePort} 0.0.0.0:*`}).error,"portInUse");
      assert.equal(f.run("verify","codex",25721,"absent",{...input,TEST_LISTENERS:`LISTEN 0 128 0.0.0.0:${remotePort} 0.0.0.0:*`}).error,"unsafeBinding");
      assert.equal(f.run("verify","codex",25721,"absent",{...input,TEST_LISTENERS:`LISTEN 0 128 127.0.0.1:${remotePort} 0.0.0.0:*`}).verified,true);
      assert.equal(f.run("verify","codex",25721,"absent",{...input,TEST_LISTENERS:`LISTEN 0 128 [::]:${remotePort} [::]:*`}).error,"unsafeBinding");
    }
  } finally { f.cleanup(); }
});
for(const tool of ["codex","claude"]) test(`${tool}: preview, apply, stale preview, repeat apply and restore`,{skip:!available || !python},()=>{
  const f=fixture();try {
    const folder=join(f.home,tool==="codex"?".codex":".claude");
    const file=join(folder,tool==="codex"?"config.toml":"settings.json");
    const preview=f.run("preview",tool);assert.equal(preview.expectedHash,"absent");assert.equal(existsSync(folder),false);
    assert.equal(f.run("apply",tool).configured,true);assert.equal(existsSync(file),true);
    const applied=readFileSync(file,"utf8");assert.match(applied,/127\.0\.0\.1:25721/);
    if(tool==="codex") assert.match(applied,/http_headers = \{ "X-ProxyEnv-Session" = "[0-9a-f]{64}" \}/);
    else assert.match(applied,/"ANTHROPIC_CUSTOM_HEADERS": "X-ProxyEnv-Session: [0-9a-f]{64}"/);
    const firstStatus=f.run("status",tool);
    assert.equal(firstStatus.configured,true);assert.equal(firstStatus.previousPort,25721);
    assert.equal(firstStatus.remoteModel,tool==="codex"?"fixture-model":null);
    assert.equal(firstStatus.profileHash.length,64);
    const otherPortStatus=f.run("status",tool,25722);
    assert.equal(otherPortStatus.configured,false);assert.equal(otherPortStatus.previousPort,25721);
    assert.equal(f.run("apply",tool,25722).error,"configConflict");assert.equal(readFileSync(file,"utf8"),applied);
    const next=f.run("preview",tool);assert.equal(next.previousPort,25721);
    assert.equal(f.run("apply",tool,25722,next.expectedHash).configured,true);
    assert.equal(f.run("restore",tool).configured,false);assert.equal(existsSync(file),false);
    assert.equal(f.run("restore",tool).error,"noBackup");
  } finally { f.cleanup(); }
});
for(const tool of ["codex","claude"]) test(`${tool}: reconnect rotates the owned session credential without a new user decision`,{skip:!available || !python},()=>{
  const f=fixture();try {
    const rotated="abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
    const initial=f.run("preview",tool);
    assert.equal(f.run("apply",tool,25721,initial.expectedHash).configured,true);
    const stale=f.run("status",tool,25721,"absent",{TEST_SESSION_TOKEN:rotated});
    assert.equal(stale.configured,false);
    assert.equal(stale.owned,true);
    const preview=f.run("preview",tool,25721,"absent",{TEST_SESSION_TOKEN:rotated});
    assert.equal(f.run("apply",tool,25721,preview.expectedHash,{TEST_SESSION_TOKEN:rotated}).configured,true);
    assert.equal(f.run("status",tool,25721,"absent",{TEST_SESSION_TOKEN:rotated}).configured,true);
    const file=join(f.home,tool==="codex"?".codex/config.toml":".claude/settings.json");
    const content=readFileSync(file,"utf8");
    assert.match(content,new RegExp(rotated));
    assert.doesNotMatch(content,/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/);
  } finally { f.cleanup(); }
});
test("managed remote environment is session-owned and removable",{skip:!available},()=>{
  const f=fixture();try {
    const token="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const applied=f.run("session-env-apply","codex",17897,"absent",{TEST_SESSION_TOKEN:token});
    assert.deepEqual(applied,{sessionEnvironment:"applied"});
    const file=join(f.home,".proxyenv/sessions/0123456789abcdef0123456789abcdef/env.sh");
    const content=readFileSync(file,"utf8");
    assert.match(content,/HTTP_PROXY='http:\/\/proxyenv:[0-9a-f]{64}@127\.0\.0\.1:17897'/);
    assert.match(content,/NO_PROXY='localhost,127\.0\.0\.1,::1'/);
    assert.ok(!JSON.stringify(applied).includes(token));
    assert.deepEqual(f.run("session-env-remove"),{sessionEnvironment:"removed"});
    assert.equal(existsSync(file),false);
  } finally { f.cleanup(); }
});
test("Claude request verification returns only an allowlisted state",{skip:!available || !python},()=>{
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
test("Codex synchronizes the selected model and opaque catalog, then restores the original profile",{skip:!available || !python},()=>{
  const f=fixture();try {
    const file=join(f.home,".codex/config.toml");
    const profileCatalog=join(f.home,".codex/proxyenv-codex-model-catalog.json");
    mkdirSync(dirname(file),{recursive:true});
    const original='# local preference\nmodel_provider = "openai"\nmodel = "gpt-original"\nmodel_catalog_json = "original-models.json"\n\n[mcp_servers.demo]\ncommand = "demo"\n';
    writeFileSync(file,original);
    const catalog=JSON.stringify({models:[{slug:"deepseek-flash",future:{opaque:true}}],futureRoot:{preserved:[1,2,3]}});
    const preview=f.run("preview","codex",25721,"absent",{TEST_PROFILE_MODEL:"deepseek-flash",TEST_PROFILE_CATALOG:catalog});
    assert.equal(preview.previousPort,null);
    assert.equal(f.run("apply","codex",25721,preview.expectedHash,{TEST_PROFILE_MODEL:"deepseek-flash",TEST_PROFILE_CATALOG:catalog}).configured,true);
    const enabled=readFileSync(file,"utf8");
    assert.match(enabled,/model = "deepseek-flash"/);
    assert.match(enabled,/model_catalog_json = "proxyenv-codex-model-catalog\.json"/);
    assert.match(enabled,/model_provider = "proxyenv_bridge"/);
    assert.match(enabled,/\[mcp_servers\.demo\]/);
    assert.equal(readFileSync(profileCatalog,"utf8"),catalog);
    const status=f.run("status");
    assert.equal(status.configured,true);
    assert.equal(status.remoteModel,"deepseek-flash");
    assert.equal(status.profileHash.length,64);
    assert.equal(f.run("restore").configured,false);
    assert.equal(readFileSync(file,"utf8"),original);
    assert.equal(existsSync(profileCatalog),false);
  } finally { f.cleanup(); }
});
test("Codex profile switch reconciles remote drift and restores the pre-enable profile",{skip:!available || !python},()=>{
  const f=fixture();try {
    const first=f.run("preview");
    assert.equal(f.run("apply","codex",25721,first.expectedHash).configured,true);
    const file=join(f.home,".codex/config.toml");
    const changed=readFileSync(file,"utf8")+'\n[user_change]\nvalue = "keep"\n';
    writeFileSync(file,changed);
    const preview=f.run("preview","codex",25721);
    assert.equal(preview.remoteChanged,true);
    assert.equal(f.run("apply","codex",25721,preview.expectedHash).configured,true);
    assert.match(readFileSync(file,"utf8"),/\[user_change\]/);
    assert.equal(f.run("restore").configured,false);
    assert.equal(readFileSync(file,"utf8").replaceAll("\r\n","\n").trim(),'[user_change]\nvalue = "keep"');
  } finally { f.cleanup(); }
});
test("Codex adopts a verified legacy extension transaction and restores its official profile",{skip:!available || !python},()=>{
  const f=fixture();try {
    const file=join(f.home,".codex/config.toml");
    const legacyBackup=`${file}.proxyenv-extension-original`;
    const legacyState=`${file}.proxyenv-extension-state`;
    mkdirSync(dirname(file),{recursive:true});
    const original='model_provider = "openai"\nmodel = "gpt-original"\n';
    const legacy='model_provider = "proxyenv_bridge"\nmodel = "gpt-original"\n\n[model_providers.proxyenv_bridge]\nname = "ProxyEnv CC Switch"\nbase_url = "http://127.0.0.1:25721/v1"\nwire_api = "responses"\nrequires_openai_auth = false\nsupports_websockets = false\n';
    const changed=`${legacy}\n[user_change]\nvalue = "keep"\n`;
    writeFileSync(file,changed);writeFileSync(legacyBackup,original);
    writeFileSync(legacyState,JSON.stringify({schema:1,tool:"codex",port:25721,contextHash:"legacy",state:"applied",originalHash:createHash("sha256").update(original).digest("hex"),appliedHash:createHash("sha256").update(legacy).digest("hex")}));
    const preview=f.run("preview");
    const applied=f.run("apply","codex",25721,preview.expectedHash);
    assert.equal(applied.configured,true,JSON.stringify(applied));
    assert.equal(existsSync(legacyState),false);assert.equal(existsSync(legacyBackup),false);
    assert.equal(f.run("restore").configured,false);
    const restored=readFileSync(file,"utf8").replaceAll("\r\n","\n");
    assert.match(restored,/model_provider = "openai"/);assert.match(restored,/model = "gpt-original"/);assert.match(restored,/\[user_change\]/);
  } finally { f.cleanup(); }
});
test("Codex profile can update model and catalog without rebuilding runtime requests",{skip:!available || !python},()=>{
  const f=fixture();try {
    const first=f.run("preview");
    assert.equal(f.run("apply","codex",25721,first.expectedHash).configured,true);
    const nextCatalog=JSON.stringify({models:[{slug:"kimi-k2.5",unknownFutureField:["kept"]}]});
    const next=f.run("preview","codex",25721,"absent",{TEST_PROFILE_MODEL:"kimi-k2.5",TEST_PROFILE_CATALOG:nextCatalog});
    assert.equal(next.remoteChanged,false);
    assert.equal(f.run("apply","codex",25721,next.expectedHash,{TEST_PROFILE_MODEL:"kimi-k2.5",TEST_PROFILE_CATALOG:nextCatalog}).configured,true);
    assert.match(readFileSync(join(f.home,".codex/config.toml"),"utf8"),/model = "kimi-k2\.5"/);
    assert.equal(readFileSync(join(f.home,".codex/proxyenv-codex-model-catalog.json"),"utf8"),nextCatalog);
  } finally { f.cleanup(); }
});
test("Codex migrates the owned legacy neutral profile through its verified backup",{skip:!available || !python},()=>{
  const f=fixture();try {
    const file=join(f.home,".codex/config.toml");
    const backup=`${file}.proxyenv-original`;
    const marker=`${file}.proxyenv-applied`;
    const legacyCatalog=join(f.home,".codex/.proxyenv-bridge-model-catalog.json");
    mkdirSync(dirname(file),{recursive:true});
    const original='model_provider = "openai"\nmodel = "original-model"\nmodel_catalog_json = "original.json"\n';
    const legacy='model_provider = "proxyenv_bridge"\nmodel = "proxyenv-bridge"\nmodel_catalog_json = ".proxyenv-bridge-model-catalog.json"\n\n[model_providers.proxyenv_bridge]\nname = "ProxyEnv Local Bridge"\nbase_url = "http://127.0.0.1:25721/v1"\nwire_api = "responses"\nrequires_openai_auth = false\nsupports_websockets = false\n';
    writeFileSync(file,legacy);writeFileSync(backup,original);
    assert.ok(legacyCatalogJson);writeFileSync(legacyCatalog,`${legacyCatalogJson}\n`);
    writeFileSync(marker,`${createHash("sha256").update(legacy).digest("hex")} exact\n`);
    const preview=f.run("preview");
    assert.equal(f.run("apply","codex",25721,preview.expectedHash).configured,true);
    assert.match(readFileSync(file,"utf8"),/model = "fixture-model"/);
    assert.equal(existsSync(legacyCatalog),false);
    assert.equal(f.run("restore").configured,false);
    assert.equal(readFileSync(file,"utf8"),original);
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
test("managed proxy terminal loads a private authenticated session environment",()=>{
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
  assert.match(ssh,/\.proxyenv\/sessions\/\{session_id\}\/env\.sh/);
  assert.match(bridge,/fn apply_session_environment/);
  assert.match(bridge,/fn remove_session_environment/);
  assert.match(commands,/fn remote_bridge_launch_proxy_terminal/);
  assert.match(commands,/fn remote_bridge_launch_manual_terminal/);
  assert.match(runtime,/remote_bridge::remote_bridge_launch_proxy_terminal/);
  assert.match(runtime,/remote_bridge::remote_bridge_launch_manual_terminal/);
  assert.match(state,/launchProxyTerminal: \(\) => invoke<void>\("remote_bridge_launch_proxy_terminal"\)/);
  assert.match(state,/launchManualTerminal: \(\) => invoke<void>\("remote_bridge_launch_manual_terminal"\)/);
  assert.match(page,/@click="launchProxyTerminal"/);
  assert.match(page,/remoteBackend\.launchManualTerminal\(\)/);
  assert.match(page,/<details class="remote-advanced">/);
  assert.match(page,/v-if="summary\.environment"/);
  const advanced=page.slice(page.indexOf('<details class="remote-advanced">'),page.indexOf('</details>',page.indexOf('<details class="remote-advanced">')));
  assert.match(advanced,/remoteBackend\.launchMobaxterm\(\)/);
  assert.match(advanced,/remoteBackend\.openVscode\(summary\.target!\.id\)/);
  assert.match(advanced,/v-if="summary\.environment" class="remote-hint"/);
  assert.match(ssh,/fn launch_terminal/);
  assert.match(ssh,/launch_terminal\(target_id, fingerprint, None\)/);
});
test("Claude default settings takeover preserves unrelated fields and restores exact bytes",{skip:!available || !python},()=>{
  const f=fixture();try {
    const settingsFile=join(f.home,".claude/settings.json");
    mkdirSync(dirname(settingsFile),{recursive:true});
    const original='{"theme":"dark","env":{"KEEP_ME":"yes","ANTHROPIC_API_KEY":"secret-fixture"},"permissions":{"allow":["Read"]}}\n';
    writeFileSync(settingsFile,original);
    const preview=f.run("preview","claude");
    assert.equal(preview.configExists,true);
    assert.equal(preview.previousPort,null);
    assert.equal(f.run("apply","claude",25721,preview.expectedHash).configured,true);
    const applied=JSON.parse(readFileSync(settingsFile,"utf8"));
    assert.equal(applied.theme,"dark");
    assert.equal(applied.env.KEEP_ME,"yes");
    assert.equal(applied.env.ANTHROPIC_BASE_URL,"http://127.0.0.1:25721");
    assert.equal(applied.env.ANTHROPIC_AUTH_TOKEN,"PROXY_MANAGED");
    assert.equal("ANTHROPIC_API_KEY" in applied.env,false);
    assert.deepEqual(applied.permissions,{allow:["Read"]});
    assert.equal(f.run("restore","claude").configured,false);
    assert.equal(readFileSync(settingsFile,"utf8"),original);
  } finally { f.cleanup(); }
});
test("Claude malformed or ambiguous settings fail closed without writes",{skip:!available || !python},()=>{
  const f=fixture();try {
    const settingsFile=join(f.home,".claude/settings.json");
    mkdirSync(dirname(settingsFile),{recursive:true});
    for (const original of ['{"env":[]}', '{"env":{},"env":{"x":"y"}}', '{not-json']) {
      writeFileSync(settingsFile,original);
      assert.equal(f.run("preview","claude").error,"configConflict");
      assert.equal(readFileSync(settingsFile,"utf8"),original);
    }
  } finally { f.cleanup(); }
});
test("Claude settings merger falls back to the Python 2.7-compatible python command",{skip:!available || !python},()=>{
  const f=fixture({jsonEngine:"python"});try {
    const preview=f.run("preview","claude");
    assert.equal(preview.expectedHash,"absent");
    assert.equal(f.run("apply","claude",25721,preview.expectedHash).configured,true);
    assert.equal(JSON.parse(readFileSync(join(f.home,".claude/settings.json"),"utf8")).env.ANTHROPIC_AUTH_TOKEN,"PROXY_MANAGED");
  } finally { f.cleanup(); }
});
test("Claude JSON editor avoids Python 3-only syntax and regular-expression APIs",()=>{
  assert.doesNotMatch(script,/re\.fullmatch/);
  assert.doesNotMatch(script,/(^|[^A-Za-z0-9_])f[\"']/m);
  assert.match(script,/except \(IOError, OSError, UnicodeError, ValueError\):/);
  assert.match(script,/re\.match\(r"\^http:\/\/127\\\.0\\\.0\\\.1:/);
});
test("Claude native installer is discovered without loading interactive shell profiles",{skip:!available || !python},()=>{
  const f=fixture({claudeLocation:"native"});try {
    const preview=f.run("preview","claude");
    assert.equal(preview.version,"2.1.227");
  } finally { f.cleanup(); }
});
test("Claude installed through NVM is discovered without sourcing shell profiles",{skip:!available || !python},()=>{
  const f=fixture({claudeLocation:"nvm"});try {
    const preview=f.run("preview","claude");
    assert.equal(preview.version,"2.1.227");
    assert.match(script,/stat -c %g/);
    assert.match(script,/id -gn/);
    assert.match(script,/id -un/);
    assert.match(script,/0\$mode & 002/);
  } finally { f.cleanup(); }
});
test("managed Codex switch never leaks unrelated remote values and can disable after drift",{skip:!available || !python},()=>{
  const f=fixture();try {
    f.run("apply");const file=join(f.home,".codex/config.toml");
    const external='api_key = "secret-fixture-never-return"\n';writeFileSync(file,external);
    const result=f.run("preview");assert.equal(result.remoteChanged,true);assert.ok(!JSON.stringify(result).includes("secret-fixture"));
    assert.equal(f.run("restore").configured,false);assert.equal(readFileSync(file,"utf8"),external);
  } finally { f.cleanup(); }
});
test("Claude restore preserves unrelated settings changed after takeover",{skip:!available || !python},()=>{
  const f=fixture();try {
    const file=join(f.home,".claude/settings.json");
    mkdirSync(dirname(file),{recursive:true});
    const original='{"theme":"dark"}\n';
    writeFileSync(file,original);
    const preview=f.run("preview","claude");
    assert.equal(f.run("apply","claude",25721,preview.expectedHash).configured,true);
    const external=JSON.parse(readFileSync(file,"utf8"));
    external.theme="light";
    external.env.PRIVATE="secret-fixture-never-return";
    writeFileSync(file,`${JSON.stringify(external)}\n`);
    const result=f.run("restore","claude");
    assert.equal(result.configured,false);
    const restored=JSON.parse(readFileSync(file,"utf8"));
    assert.equal(restored.theme,"light");
    assert.equal(restored.env.PRIVATE,"secret-fixture-never-return");
    assert.equal(restored.env.ANTHROPIC_BASE_URL,undefined);
    assert.ok(!JSON.stringify(result).includes("secret-fixture"));
  } finally { f.cleanup(); }
});
test("Claude managed route follows a regenerated bridge port without losing unrelated edits",{skip:!available || !python},()=>{
  const f=fixture();try {
    const file=join(f.home,".claude/settings.json");
    mkdirSync(dirname(file),{recursive:true});
    writeFileSync(file,'{"theme":"dark","env":{"ANTHROPIC_API_KEY":"secret-fixture"}}\n');
    const first=f.run("preview","claude");
    assert.equal(f.run("apply","claude",25721,first.expectedHash).configured,true);
    assert.equal(f.run("tool-verify","claude",25722).error,"routeOutdated");

    const changed=JSON.parse(readFileSync(file,"utf8"));
    changed.theme="light";
    changed.env.KEEP_ME="added-by-claude";
    writeFileSync(file,`${JSON.stringify(changed,null,2)}\n`);

    const refreshed=f.run("preview","claude",25722);
    assert.equal(refreshed.previousPort,25721,JSON.stringify(refreshed));
    assert.equal(f.run("apply","claude",25722,refreshed.expectedHash).configured,true);
    const updated=JSON.parse(readFileSync(file,"utf8"));
    assert.equal(updated.theme,"light");
    assert.equal(updated.env.KEEP_ME,"added-by-claude");
    assert.equal(updated.env.ANTHROPIC_BASE_URL,"http://127.0.0.1:25722");
    assert.equal(f.run("tool-verify","claude",25722).verification,"verified");

    assert.equal(f.run("restore","claude").configured,false);
    const restored=JSON.parse(readFileSync(file,"utf8"));
    assert.equal(restored.theme,"light");
    assert.equal(restored.env.KEEP_ME,"added-by-claude");
    assert.equal(restored.env.ANTHROPIC_API_KEY,"secret-fixture");
    assert.equal("ANTHROPIC_BASE_URL" in restored.env,false);
    assert.equal("ANTHROPIC_AUTH_TOKEN" in restored.env,false);
  } finally { f.cleanup(); }
});
test("Claude user-private-group permissions need no manual chmod",{skip:!available || !python},()=>{
  const f=fixture({privateGroup:true});try {
    const folder=join(f.home,".claude");
    const file=join(folder,"settings.json");
    mkdirSync(folder,{recursive:true});
    writeFileSync(file,'{"theme":"dark"}\n');
    chmodSync(folder,0o775);
    chmodSync(file,0o664);
    const preview=f.run("preview","claude");
    assert.equal(f.run("apply","claude",25721,preview.expectedHash).configured,true);
    if(process.platform!=="win32") assert.equal(statSync(file).mode & 0o777,0o600);
    assert.match(script,/is_safe_user_content/);
    assert.match(script,/0\$mode & 002/);
    assert.match(script,/id -gn/);
    assert.match(script,/id -un/);
  } finally { f.cleanup(); }
});
test("Claude shared-group permissions are explicitly hardened during confirmed apply",{skip:!available || !python || process.platform==="win32"},()=>{
  const f=fixture({sharedGroup:true});try {
    const folder=join(f.home,".claude");
    const file=join(folder,"settings.json");
    mkdirSync(folder,{recursive:true});
    writeFileSync(file,'{"theme":"dark"}\n');
    chmodSync(folder,0o775);
    chmodSync(file,0o664);
    const preview=f.run("preview","claude");
    assert.equal(preview.permissionHardening,true);
    assert.equal(f.run("apply","claude",25721,preview.expectedHash,{TEST_REPAIR_PERMISSIONS:"1"}).configured,true);
    assert.equal(statSync(folder).mode & 0o777,0o700);
    assert.equal(statSync(file).mode & 0o777,0o600);
  } finally { f.cleanup(); }
});
test("world-writable Claude settings remain unsafe",{skip:!available || !python || process.platform==="win32"},()=>{
  const f=fixture();try {
    const folder=join(f.home,".claude");
    const file=join(folder,"settings.json");
    mkdirSync(folder,{recursive:true});
    writeFileSync(file,'{"theme":"dark"}\n');
    chmodSync(file,0o666);
    assert.equal(f.run("preview","claude").error,"unsafePath");
  } finally { f.cleanup(); }
});
test("Codex older CLI version and custom home fail before writes",{skip:!available},()=>{
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
for(const code of ["sshAuth","sshAuthRejected","sshAuthPromptChanged","sshAuthCompletionTimeout","hostKeyChanged","ptyUnavailable","sshAuthSessionMissing","forwardDenied","unsafeBinding","configConflict","legacyModelSelectionRequired","routeOutdated","rootForbidden","dependencyMissing","jsonEditorMissing","cliMissing","cliUnsupported","customHome","remoteUnsupported","portInUse","activeChanged","ccUnavailable","bridgeUnavailable","toolNotConfigured","toolVerificationUnsupported","noCapability","alreadyConnected","stateUnavailable","processFailed","remoteFailed","networkFailed","targetUnsupported","portAllocationFailed","portRace","random-secret"]) assert.ok(bridgeError(code,copy) && !bridgeError(code,copy).includes("random-secret"));
    assert.equal(
      bridgeError({code:"ccUnavailable",phase:"localDetection",target:"ccSwitch",retryable:true},copy),
      copy.rbCcError,
    );
  }
});

test("CLI launch commands are hidden until their remote configuration is applied",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const adapters=readFileSync("src/features/remote-bridge/tool-adapters.ts","utf8");
  assert.match(page,/v-for="tool in remoteTools"/);
  assert.match(page,/v-if="tool\.launch"/);
  assert.match(page,/v-else>\{\{ copy\.rbCliOverlayMissing \}\}/);
  assert.match(page,/role="switch"/);
  assert.match(page,/@click\.prevent="toggleTool\(tool\.adapter, tool\.inspection\.configured\)"/);
  assert.doesNotMatch(page,/configureLabel|restoreLabel/);
  const dialog=readFileSync("src/features/remote-bridge/components/RemoteToolDialog.vue","utf8");
  assert.match(dialog,/copy\.rbCliOverlayReady/);
  assert.match(dialog,/if \(directCli\) void nextTick\(review\)/);
  assert.match(adapters,/launch: \(summary\) => configured\(summary\) \? definition\.launchCommand : ""/);
  assert.match(adapters,/launchCommand: "codex"/);
  assert.match(adapters,/launchCommand: "claude"/);
});

test("remote bridge resolves consumer and AI ports independently and restores enabled tool state",()=>{
  const bridge=readFileSync("src-tauri/src/features/remote_bridge/mod.rs","utf8");
  const auth=readFileSync("src-tauri/src/features/remote_bridge/ssh_auth.rs","utf8");
  const state=readFileSync("src/features/remote-bridge/state.ts","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(bridge,/runtime_expected_port\(\s*&target,/);
  assert.match(bridge,/DEFAULT_CC_REMOTE_PORT: u16 = 15_721/);
  assert.match(bridge,/resolve_port_pair\(\s*&fingerprint,\s*local_port,\s*runtime_expected_proxy_port/);
  assert.match(bridge,/first_available_port\(/);
  assert.match(bridge,/remote_request\(\s*"status",\s*\*adapter,\s*route_port,\s*None/);
  assert.match(bridge,/refresh_tool_configuration\([\s\S]*&mut summary,[\s\S]*ai_token/);
  assert.match(bridge,/refresh_tool_configuration\([\s\S]*&mut next,[\s\S]*ai_token/);
  assert.match(auth,/super::allocate_ports\(session\.target_id, true\)/);
  assert.match(state,/allocatePorts: \(targetId: string, preferDefaults = true\)/);
  assert.match(page,/remoteBackend\.allocatePorts\(targetId\.value, false\)/);
});

test("Codex and Claude profile polling is bridge-scoped, metadata-only, and debounced before parsing",()=>{
  const bridge=readFileSync("src-tauri/src/features/remote_bridge/mod.rs","utf8");
  const profile=readFileSync("src-tauri/src/features/remote_bridge/local_model.rs","utf8");
  const ssh=readFileSync("src-tauri/src/features/remote_bridge/ssh.rs","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(bridge,/sleep\(Duration::from_secs\(2\)\)/);
  assert.match(bridge,/sleep\(Duration::from_millis\(500\)\)/);
  assert.match(bridge,/sync_changed_profile\(candidate\)/);
  assert.match(bridge,/local_model::profile_stamp_changed\(profile\)/);
  assert.match(bridge,/local_model::claude_profile_stamp_changed\(profile\)/);
  assert.match(ssh,/"localClaudeProfileInvalid"/);
  assert.match(bridge,/claude_profile_state =\s*settings::ProfileSyncState::InvalidLocalProfile/);
  assert.match(page,/claudeProfileLabel\(summary\.claudeProfileState\)/);
  assert.match(profile,/profile_stamp_changed[\s\S]*file_stamp\(&config_path\)[\s\S]*file_stamp\(&profile\.catalog_path\)/);
  const lightweight=profile.slice(profile.indexOf("pub fn profile_stamp_changed"),profile.indexOf("#[cfg(test)]"));
  assert.doesNotMatch(lightweight,/safe_read|from_slice|from_str/);
});

test("Claude model profile is shared, synced, and restored without copying credentials",{skip:!available || !python},()=>{
  const f=fixture();try {
    const file=join(f.home,".claude/settings.json");
    mkdirSync(dirname(file),{recursive:true});
    writeFileSync(file,'{"model":"original","theme":"dark","env":{"ANTHROPIC_AUTH_TOKEN":"original-private","ANTHROPIC_BASE_URL":"https://original.example","KEEP":"remote"}}\n');
    const initial=f.run("preview","claude");
    const profile='{"availableModels":["模型 A"],"env":{"ANTHROPIC_DEFAULT_SONNET_MODEL":"模型 A"},"model":"模型 A"}';
    assert.equal(f.run("apply","claude",25721,initial.expectedHash,{TEST_CLAUDE_PROFILE:profile}).configured,true);
    let value=JSON.parse(readFileSync(file,"utf8"));
    assert.equal(value.model,"模型 A");
    assert.deepEqual(value.availableModels,["模型 A"]);
    assert.equal(value.env.ANTHROPIC_DEFAULT_SONNET_MODEL,"模型 A");
    assert.equal(value.env.ANTHROPIC_AUTH_TOKEN,"PROXY_MANAGED");
    assert.equal(value.env.ANTHROPIC_BASE_URL,"http://127.0.0.1:25721");
    value.theme="light";writeFileSync(file,JSON.stringify(value));
    const changed=f.run("preview","claude");
    assert.equal(f.run("apply","claude",25721,changed.expectedHash,{TEST_CLAUDE_PROFILE:'{"model":"模型 B"}'}).configured,true);
    value=JSON.parse(readFileSync(file,"utf8"));
    assert.equal(value.model,"模型 B");
    assert.equal(value.theme,"light");
    assert.equal(value.availableModels,undefined);
    assert.equal(f.run("restore","claude").configured,false);
    value=JSON.parse(readFileSync(file,"utf8"));
    assert.equal(value.model,"original");
    assert.equal(value.theme,"light");
    assert.equal(value.env.ANTHROPIC_AUTH_TOKEN,"original-private");
    assert.equal(value.env.ANTHROPIC_BASE_URL,"https://original.example");
    assert.equal(value.env.KEEP,"remote");
  } finally { f.cleanup(); }
});

test("Claude and Codex CLI operations use the shared RemoteToolAdapter boundary",()=>{
  const backend=readFileSync("src-tauri/src/features/remote_bridge/tool_adapter.rs","utf8");
  const bridge=readFileSync("src-tauri/src/features/remote_bridge/mod.rs","utf8");
  const frontend=readFileSync("src/features/remote-bridge/tool-adapters.ts","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const dialog=readFileSync("src/features/remote-bridge/components/RemoteToolDialog.vue","utf8");
  for(const method of ["detect","inspect","preview","apply","restore","launch","verify","supported_route_modes","compatibility","request_policy","config_projection"]) {
    assert.match(backend,new RegExp(`fn ${method}\\(`));
  }
  assert.match(backend,/impl RemoteToolAdapter for CodexCliAdapter/);
  assert.match(backend,/impl RemoteToolAdapter for ClaudeCliAdapter/);
  assert.match(backend,/RemoteToolVerification::VerifyPending/);
  assert.match(backend,/~\/\.claude\/settings\.json/);
  assert.match(backend,/~\/\.codex\/config\.toml/);
  assert.match(backend,/fn launch\(&self\) -> &'static str \{\s+"codex"/);
  assert.match(backend,/fn launch\(&self\) -> &'static str \{\s+"claude"/);
  assert.match(bridge,/tool_adapter::by_name\(&tool\)/);
  assert.match(bridge,/verified\["previousPort"\]\.as_u64\(\) != Some\(u64::from\(pending\.port\)\)/);
  assert.match(bridge,/"writeRolledBack"/);
  assert.match(bridge,/"rollbackFailed"/);
  assert.doesNotMatch(bridge,/fn overlay\(/);
  assert.match(frontend,/export interface RemoteToolAdapter/);
  assert.match(frontend,/summary\.tools\?\.find/);
  assert.doesNotMatch(page,/tool(?:\.value)?\s*===\s*["'](?:codex|claude)["']/);
  assert.match(dialog,/adapter\.supportsProfileSync/);
  assert.match(dialog,/adapter\.supportsExtensionConfiguration \|\| restoring/);
  assert.match(frontend,/supportsExtensionConfiguration: true/);
  assert.match(frontend,/extensionUsesSharedProfile: true/);
  assert.match(frontend,/directToggle: true/);
  assert.match(page,/adapter\.preview\(target\.id, configured\)/);
  assert.match(dialog,/profileSurfaceSelected = computed\(\(\) => cli\.value \|\| sharedExtensionProfile\.value\)/);
  assert.match(dialog,/extension\.value && inspection\.value && !sharedExtensionProfile\.value/);
  assert.match(backend,/CodexCliAdapter/);
  assert.doesNotMatch(dialog,/tool === 'codex'/);
  assert.match(dialog,/remoteBackend\.modelSettings\(\)/);
  assert.match(dialog,/remoteBackend\.saveModelSettings/);
  assert.doesNotMatch(dialog,/routeMappings|compatibilityRules|incomingModel|targetModel/);
});

test("Claude verification is a fixed isolated request and never returns model output",()=>{
  const shell=readFileSync("src-tauri/src/features/remote_bridge/remote.sh","utf8");
  const bridge=readFileSync("src-tauri/src/features/remote_bridge/mod.rs","utf8");
  const ssh=readFileSync("src-tauri/src/features/remote_bridge/ssh.rs","utf8");
  const commands=readFileSync("src-tauri/src/commands/remote_bridge.rs","utf8");
  const runtime=readFileSync("src-tauri/src/lib.rs","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(shell,/render "\$previous" >"\$verify_settings"/);
  assert.match(shell,/--settings "\$verify_settings"/);
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
  assert.match(backend,/selection\.tool == "codex" && !selection\.restore/);
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

test("failed atomic replace rolls back and preserves a usable recovery journal",{skip:!available || !python},()=>{
  const f=fixture();try {
    assert.equal(f.run("apply").configured,true);
    const file=join(f.home,".codex/config.toml");
    const before=readFileSync(file,"utf8");
    const preview=f.run("preview");
    assert.equal(f.run("apply","codex",25722,preview.expectedHash,{TEST_FAIL_REPLACE:"1"}).error,"remoteFailed");
    assert.equal(readFileSync(file,"utf8"),before);
    assert.equal(f.run("restore").configured,false);
    assert.equal(existsSync(file),false);
  } finally { f.cleanup(); }
});
test("failed Claude shared-settings replace restores the exact original",{skip:!available || !python},()=>{
  const f=fixture();try {
    const file=join(f.home,".claude/settings.json");
    mkdirSync(dirname(file),{recursive:true});
    const original='{"theme":"dark","env":{"KEEP_ME":"yes"}}\n';
    writeFileSync(file,original);
    const preview=f.run("preview","claude");
    assert.equal(f.run("apply","claude",25721,preview.expectedHash,{TEST_FAIL_REPLACE:"1"}).error,"remoteFailed");
    assert.equal(readFileSync(file,"utf8"),original);
    assert.equal(existsSync(`${file}.proxyenv-original`),false);
    assert.equal(existsSync(`${file}.proxyenv-applied`),false);
  } finally { f.cleanup(); }
});
