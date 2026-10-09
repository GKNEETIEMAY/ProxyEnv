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
    assert.match(content,/# ProxyEnv managed session 0123456789abcdef0123456789abcdef/);
    assert.match(content,/HTTP_PROXY='http:\/\/127\.0\.0\.1:17897'/);
    assert.doesNotMatch(content,/proxyenv:|session_token/i);
    assert.match(content,/export NO_PROXY="localhost,127\.0\.0\.1,::1/);
    const sourced=spawnSync(shell,["-s"],{encoding:"utf8",input:`
export HTTP_PROXY=old HTTPS_PROXY=old ALL_PROXY=old http_proxy=stale https_proxy=stale all_proxy=stale
export NO_PROXY=internal.example no_proxy=private.example
. '${posix(file)}'
[ "$HTTP_PROXY" = "$http_proxy" ] && [ "$HTTPS_PROXY" = "$https_proxy" ] || exit 11
[ -z "\${ALL_PROXY:-}" ] && [ -z "\${all_proxy:-}" ] || exit 12
[ "$NO_PROXY" = 'localhost,127.0.0.1,::1,internal.example,private.example' ] || exit 13
[ "$NO_PROXY" = "$no_proxy" ] || exit 14
printf 'environment-ok'
`});
    assert.equal(sourced.status,0,'session environment must clear stale variables and preserve bypasses');
    assert.equal(sourced.stdout,'environment-ok');
    assert.ok(!JSON.stringify(applied).includes(token));
    assert.deepEqual(f.run("session-env-remove"),{sessionEnvironment:"removed"});
    assert.equal(existsSync(file),false);
  } finally { f.cleanup(); }
});
test("managed remote environment refuses foreign same-name files",{skip:!available},()=>{
  const f=fixture();try {
    const directory=join(f.home,".proxyenv/sessions/0123456789abcdef0123456789abcdef");
    const file=join(directory,"env.sh");
    mkdirSync(directory,{recursive:true,mode:0o700});
    writeFileSync(file,"export FOREIGN_VALUE=keep\n",{mode:0o600});
    assert.equal(f.run("session-env-apply","codex",17897).error,"configConflict");
    assert.equal(readFileSync(file,"utf8"),"export FOREIGN_VALUE=keep\n");
    assert.equal(f.run("session-env-remove").error,"configConflict");
    assert.equal(readFileSync(file,"utf8"),"export FOREIGN_VALUE=keep\n");
  } finally { f.cleanup(); }
});

test("general proxy exposes no credential command while AI verification remains session-authenticated",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const bridge=readFileSync("src-tauri/src/features/remote_bridge/mod.rs","utf8");
  const state=readFileSync("src/features/remote-bridge/state.ts","utf8");
  const commands=readFileSync("src-tauri/src/commands/remote_bridge.rs","utf8");
  assert.doesNotMatch(page,/copyProxyPassword|rbCopyProxyPassword|rbProxyAuthUsername/);
  assert.doesNotMatch(state,/proxyPassword|remote_bridge_proxy_password/);
  assert.doesNotMatch(commands,/remote_bridge_proxy_password|proxyCredentialCopy/);
  assert.doesNotMatch(page,/copyValue\(summary\.environment\)|\{\{ summary\.environment \}\}/);
  assert.doesNotMatch(state.match(/export interface BridgeSummary[^\n]+/)[0],/token|password|secret/i);
  assert.doesNotMatch(bridge,/pub fn proxy_password\(/);
  const verify=bridge.slice(bridge.indexOf("pub fn verify_tool("),bridge.indexOf("#[cfg(test)]",bridge.indexOf("pub fn verify_tool(")));
  assert.match(verify,/capability_ready\(state\.child\.is_some\(\), state\.summary\.cc_status\)/);
  assert.match(verify,/sessionToken/);
  assert.doesNotMatch(verify,/summary\.status != Status::Connected/);
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
  const groups=readFileSync("src/features/remote-bridge/components/RemoteTargetGroups.vue","utf8");
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
  assert.match(shell,/<div v-show="view === 'remote'" class="view-pane remote-view-pane">/);
  assert.doesNotMatch(shell,/remoteViewMounted/);
  assert.doesNotMatch(shell,/view-fade/);
  assert.match(shell,/@connected="acceptRemoteBridgeSummary"/);
  assert.doesNotMatch(page,/remote-steps|reviewedRequest|authPromptCopy\.notice/);
  assert.match(page,/async function connectPrepared\(selected: BridgeRequest\)/);
  assert.match(page,/await remoteBackend\.preview\(selected\)/);
  assert.match(page,/return await remoteBackend\.connect\(selected\)/);
  assert.match(groups,/\["openssh", "vscode", "mobaxterm", "manual"\]/);
  assert.match(groups,/:disabled="disabled \|\| !target\.available"/);
  assert.match(page,/<RemoteTargetGroups/);
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
  assert.match(page,/visibleAuthPrompt\?\.secret !== false \? 'password' : 'text'/);
  assert.match(page,/remoteBackend\.sshAuthSubmit/);
  assert.match(page,/remoteBackend\.sshAuthConfirmHost/);
  assert.match(page,/authPromptUnavailable/);
  assert.match(page,/authCompleting/);
  assert.match(page,/authSession\.value\?\.status === "waitingUser" && !!authPrompt\.value/);
  assert.match(page,/:disabled="authIsHostConfirmation \|\| !authCanRespond \|\| authSubmitting"/);
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
  assert.match(page,/@terminal="launchTerminal"/);
  assert.match(page,/remoteBackend\.launchManualTerminal\(\)/);
  assert.match(page,/<div class="remote-advanced-panel">/);
  assert.doesNotMatch(page,/v-if="summary\.environment"/);
  assert.match(page,/remoteBackend\.sessionEnvironmentCommand\(\)/);
  const overview=readFileSync("src/features/remote-bridge/components/RemoteBridgeOverview.vue","utf8");
  assert.match(overview,/summary\.target\?\.canOpenMobaxterm/);
  assert.match(page,/remoteBackend\.launchMobaxterm\(props\.summary\.target!\.id\)/);
  assert.match(page,/@click="copySessionEnvironment"/);
  assert.match(page,/async function openVscode\(\)[\s\S]*remoteBackend\.openVscode\(props\.summary\.target!\.id\)/);
  assert.match(page,/@vscode="perform\(openVscode\)"/);
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
    for (const [code, key] of [['vscodeNetworkBusy','rbVscodeNetworkBusy'], ['vscodeNetworkConflict','rbVscodeNetworkConflict'], ['vscodeNetworkUnsafe','rbVscodeNetworkUnsafe']]) {
      assert.equal(bridgeError({code,phase:'vscodeOpen',target:'vscode'},copy),copy[key],`${locale}:${code}`);
      assert.notEqual(copy[key],copy.rbConfigError);
    }
  }
});

test("CC Switch setup observes its local service without claiming per-agent route status",async()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(page,/const ccUsable = computed\(\(\) => ccDetection\.value\.state === "confirmed"\)/);
  assert.match(page,/watch\(\[\(\) => props\.ccDetection, live, establishingBridge\], \(\) => initializeCapabilities\(false\), \{ immediate: true \}\)/);
  assert.doesNotMatch(page,/scheduleCapabilityPolling|capabilityChecking/);
  assert.doesNotMatch(page,/remoteBackend\.detectCc\(ccLocalPort\.value, true\)/);
  assert.match(page,/ccLocalPort\.value = result\.localPort/);
  assert.match(page,/cc\.value = result\.state === "confirmed" && ccPreferred\.value/);
  assert.match(page,/v-if="ccUsable" class="remote-capability-reminder"/);
  assert.match(page,/copy\.rbCcRouteDetected/);
  assert.match(page,/http:\/\/127\.0\.0\.1:\{\{ ccDetection\.localPort \}\}/);
  assert.match(page,/:disabled="!ccUsable"/);
  const source=readFileSync("src/shared/i18n/remote-bridge.ts","utf8");
  const compiled=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext}}).outputText;
  const {remoteBridgeMessages:messages}=await import(`data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`);
  for(const [locale,copy] of Object.entries(messages)) {
    assert.ok(copy.rbCcRouteReminder,locale);
    assert.ok(copy.rbCcRouteDetected,locale);
    assert.match(copy.rbCcRouteReminder,/Claude\/Codex/);
  }
  assert.match(messages["zh-CN"].rbCcOpenHint,/设置 → 路由 → 本地路由 → 路由总开关/);
});

test("CC Switch detection defaults on and preserves an explicit off choice",async()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const initializer=/function initializeCapabilities\(autoEnable = true\) \{([\s\S]*?)\r?\n\}\r?\n\r?\nfunction rememberCcChoice/;
  const match=page.match(initializer);
  assert.ok(match,"Capability initialization must be extractable with LF or CRLF source lines");
  const body=match[1];
  const lf=page.replaceAll("\r\n","\n");
  assert.equal(lf.match(initializer)?.[1],lf.replaceAll("\n","\r\n").match(initializer)?.[1].replaceAll("\r\n","\n"));
  const value=initial=>({value:initial});
  const context={
    props:{reviewPreview:false,visible:false,ccDetection:{state:"confirmed",localPort:15721}},proxyAvailable:value(true),proxy:value(true),proxyPreferred:value(true),
    cc:value(false),ccPreferred:value(true),ccLocalPort:value(15721),ccDetection:value(null),ccCheck:value(null),
    live:value(false),busy:value(true),establishingBridge:value(false),
  };
  const compiled=ts.transpileModule(`function initialize(autoEnable,context) { const {${Object.keys(context).join(",")}}=context; ${body} }`,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText;
  const initialize=new Function(`${compiled}; return initialize;`)();
  await initialize(true,context);
  assert.equal(context.cc.value,true,"Cached routing must render even during SSH list loading or while hidden");
  context.busy.value=false;
  await initialize(false,context);
  assert.equal(context.cc.value,true,"A later polling result must still enable the default choice");
  context.props.ccDetection={state:"notDetected",localPort:15721};
  await initialize(false,context);
  assert.equal(context.cc.value,false,"Unavailable routing cannot stay enabled");
  context.props.ccDetection={state:"confirmed",localPort:15822};
  await initialize(false,context);
  assert.equal(context.cc.value,true,"Routing recovery restores the default on choice");
  assert.equal(context.ccLocalPort.value,15822);
  context.ccPreferred.value=false;
  context.cc.value=false;
  await initialize(false,context);
  await initialize(true,context);
  assert.equal(context.cc.value,false,"Neither polling nor refresh may override the user's off choice");
  context.live.value=true;
  context.props.ccDetection={state:"confirmed",localPort:15999};
  await initialize(false,context);
  assert.equal(context.ccLocalPort.value,15822,"Established bridges keep their original port");
  assert.doesNotMatch(page,/watch\(ccLocalPort,[\s\S]*?ccDetection\.value = \{ state: "notDetected"/);
  assert.match(page,/@change="rememberCcChoice"/);
  assert.match(page,/ccPreferred\.value = \(event\.target as HTMLInputElement\)\.checked/);
});

test("CC Switch discovery starts with the app, caches results and serializes background refresh",async()=>{
  const {ref}=await import("vue");
  const state=readFileSync("src/features/remote-bridge/state.ts","utf8");
  const source=state.slice(state.indexOf("export function useRemoteBridge()"));
  const compiled=ts.transpileModule(source.replace("export function","function"),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText;
  const mounted=[],unmounted=[],timers=new Map();
  let resolveDetection,calls=0,lastPort,discover;
  const factory=new Function("ref","onMounted","onBeforeUnmount","isTauri","remoteBackend","emptySummary","setTimeout","clearTimeout",`${compiled}; return useRemoteBridge();`);
  const bridge=factory(ref,callback=>mounted.push(callback),callback=>unmounted.push(callback),()=>true,{
    summary:()=>new Promise(()=>{}), // A slow SSH summary must not delay local discovery.
    detectCc:(port,find)=>{calls++;lastPort=port;discover=find;return new Promise(resolve=>{resolveDetection=resolve;});},
  },()=>({status:"disconnected"}),(callback,delay)=>{const id=timers.size+1;timers.set(id,{callback,delay});return id;},id=>timers.delete(id));
  mounted.forEach(callback=>callback());
  assert.equal(calls,1,"Startup must detect CC Switch independently of remote page initialization");
  assert.equal(discover,true);
  assert.equal(lastPort,15721);
  assert.equal(bridge.ccDetection.value,null,"Unfinished detection is not a negative result");
  const first=bridge.refreshCcDetection();
  assert.equal(first,bridge.refreshCcDetection());
  assert.equal(calls,1,"Overlapping refreshes share the same request");
  resolveDetection({state:"confirmed",localPort:15822});
  await first;
  await Promise.resolve();
  assert.equal(bridge.ccDetection.value.localPort,15822);
  const cached=bridge.ccDetection.value;
  const next=bridge.refreshCcDetection();
  assert.equal(lastPort,15822,"Discovery follows a changed CC Switch port");
  assert.equal(bridge.ccDetection.value,cached,"Do not clear the route while refreshing");
  resolveDetection({state:"confirmed",localPort:15822});
  await next;
  assert.equal(bridge.ccDetection.value,cached,"Unchanged observations do not churn the UI");
  assert.equal([...timers.values()][0].delay,2000);
  const changed=bridge.refreshCcDetection();
  resolveDetection({state:"notDetected",localPort:15822});
  await changed;
  assert.equal(bridge.ccDetection.value.state,"notDetected");
  const pending=bridge.refreshCcDetection();
  unmounted.forEach(callback=>callback());
  assert.equal(timers.size,0,"Shutdown stops background discovery");
  resolveDetection({state:"confirmed",localPort:15999});
  await pending;
  assert.equal(bridge.ccDetection.value.state,"notDetected","Late results after shutdown are ignored");
  const shell=readFileSync("src/app/AppShell.vue","utf8");
  assert.match(shell,/:cc-detection="ccDetection"/);
});

test("preloaded network capability adopts late discovery without losing an explicit choice",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const match=page.match(/watch\(proxyAvailable, \(available\) => \{([\s\S]*?)\r?\n\}\);/);
  assert.ok(match,"Hidden startup page must observe late local proxy discovery");
  const value=initial=>({value:initial});
  const context={proxy:value(false),proxyPreferred:value(true),live:value(false),establishingBridge:value(false),updateLocalProxyCheck:()=>{}};
  const observe=new Function("available","context",`const {${Object.keys(context).join(",")}}=context; ${match[1]}`);
  observe(true,context);
  assert.equal(context.proxy.value,true,"Late startup discovery enables the available default");
  context.proxyPreferred.value=false;
  context.proxy.value=false;
  observe(false,context); observe(true,context);
  assert.equal(context.proxy.value,false,"Discovery cannot override an explicit off choice");
  context.proxyPreferred.value=true;
  context.live.value=true;
  observe(true,context);
  assert.equal(context.proxy.value,false,"Existing bridges retain their capability selection");
  context.live.value=false;
  context.establishingBridge.value=true;
  observe(true,context);
  assert.equal(context.proxy.value,false,"Discovery cannot change an in-progress connection");
  assert.match(page,/@change="rememberProxyChoice"/);
  const load=page.match(/async function load\(\) \{([\s\S]*?)\r?\n\}\r?\n\r?\nfunction usePorts/);
  assert.ok(load);
  assert.match(load[1],/remoteBackend\.targets\(\)/);
  assert.doesNotMatch(load[1],/remoteBackend\.(connect|checkNetwork|sshAuthBegin|configApply|setCapability)\(/,"Preloading only discovers local connection metadata");
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
  assert.match(bridge,/remote_request\(\s*"status",\s*adapter,\s*route_port,\s*None,\s*session_token/);
  assert.match(bridge,/refresh_tool_configuration\(\s*&mut tool_summary,\s*ai_token/);
  assert.equal(bridge.match(/start_post_connect\(generation\);/g)?.length,2,"Initial connection and resumed transport both restore tool state through post-connect work");
  assert.match(auth,/super::allocate_ports\(session\.target_id, true\)/);
  assert.match(state,/allocatePorts: \(targetId: string, preferDefaults = true\)/);
  assert.match(page,/remoteBackend\.allocatePorts\(selected\.targetId, false\)/);
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

test("Skills projection is CC Switch-link based, staged, owned and metadata-triggered",()=>{
  const skills=readFileSync("src-tauri/src/features/remote_bridge/skills.rs","utf8");
  const ssh=readFileSync("src-tauri/src/features/remote_bridge/ssh.rs","utf8");
  const remote=readFileSync("src-tauri/src/features/remote_bridge/skill-remote.sh","utf8");
  const commands=readFileSync("src-tauri/src/commands/remote_bridge.rs","utf8");
  const runtime=readFileSync("src-tauri/src/lib.rs","utf8");
  const state=readFileSync("src/features/remote-bridge/state.ts","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(skills,/CODEX_HOME[\s\S]*\.codex/);
  assert.match(skills,/CLAUDE_CONFIG_DIR[\s\S]*\.claude/);
  assert.match(skills,/\.cc-switch[\s\S]*skills/);
  assert.match(skills,/is_link_or_reparse/);
  assert.match(skills,/cc_switch_skill_target/);
  assert.match(skills,/is_direct_cc_switch_target/);
  assert.match(skills,/disabled: Vec<String>/);
  assert.match(skills,/remove_projection\(id\.clone\(\), false\)/);
  assert.match(skills,/value == "\.system"/);
  assert.match(skills,/metadata_stamp_id/);
  assert.match(skills,/sleep\(Duration::from_secs\(2\)\)/);
  assert.match(skills,/sleep\(Duration::from_millis\(500\)\)/);
  assert.match(skills,/MAX_FILES_PER_SKILL/);
  assert.match(skills,/MAX_SKILL_BYTES/);
  assert.match(ssh,/System32\/OpenSSH\/scp\.exe/);
  assert.match(ssh,/"-O"/);
  assert.match(remote,/\.proxyenv-owner/);
  assert.match(remote,/\.proxyenv-manifest/);
  assert.match(remote,/validate_existing "\$destination"/);
  assert.match(remote,/mv "\$stage" "\$destination"/);
  assert.doesNotMatch(remote,/scp\s+-r/);
  assert.doesNotMatch(remote,/\.bashrc|\.profile|sudo\s/);
  assert.match(commands,/remote_bridge_enable_skill/);
  assert.match(commands,/remote_bridge_disable_skill/);
  assert.match(runtime,/remote_bridge_skills/);
  assert.match(state,/enableSkill/);
  assert.match(page,/rbSkillsTitle/);
  assert.match(page,/const skillGroups = computed/);
  assert.match(page,/group\.codex/);
  assert.match(page,/group\.claude/);
  assert.match(page,/scheduleSkillsPolling/);
});

test("connected bridge defaults to a concise overview and preserves advanced state in place",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const copy=readFileSync("src/shared/i18n/remote-bridge.ts","utf8");
  assert.match(page,/const advancedView = ref\(false\)/);
  assert.match(page,/role="group" :aria-label="copy\.rbViewMode"/);
  assert.match(page,/:aria-pressed="!advancedView"/);
  assert.match(page,/:aria-pressed="advancedView"/);
  assert.match(page,/v-show="advancedView" class="remote-runtime-observation"/);
  assert.match(page,/v-show="advancedView" class="remote-next-section remote-advanced-group"/);
  assert.match(page,/<RemoteBridgeOverview v-show="!advancedView"/);
  assert.match(page,/skillsSummaryLabel/);
  assert.match(page,/ref="skillsPanel" v-show="advancedView" tabindex="-1" class="remote-next-section remote-skills"/);
  const overview=readFileSync("src/features/remote-bridge/components/RemoteBridgeOverview.vue","utf8");
  assert.match(overview,/bridge-launch-bar/);
  assert.match(page,/async function manageSkills\(\)[\s\S]*advancedView\.value = true[\s\S]*skillsPanel\.value\?\.focus/);
  assert.doesNotMatch(page,/v-if="advancedView" class="remote-next-section"/);
  for(const key of ["rbSimpleView","rbAdvancedView","rbViewMode","rbToolsTitle","rbSkillsSummary","rbSkillNotLinked","rbConnections","rbOpenTerminal","rbDiagnostics"]) {
    assert.equal(copy.match(new RegExp(`${key}:`,"g"))?.length,4,`${key} must exist in all four locales`);
  }
});

test("overview preserves independent statuses, public port mappings, and existing action boundaries",async()=>{
  const compiled=(path)=>ts.transpileModule(readFileSync(path,"utf8"),{compilerOptions:{module:ts.ModuleKind.ESNext}}).outputText;
  const load=(path)=>import(`data:text/javascript;base64,${Buffer.from(compiled(path)).toString("base64")}`);
  const {bridgeCapabilityState,bridgeEndpointLabel,overviewToolLabel,overviewSkillStatus,overviewServerDirectStatus}=await load("src/features/remote-bridge/overview-presentation.ts");
  const {remoteBridgeMessages}=await load("src/shared/i18n/remote-bridge.ts");
  assert.equal(bridgeEndpointLabel("::1",7897),"[::1]:7897");
  assert.equal(bridgeCapabilityState("connected",false),"disabled");
  assert.equal(bridgeCapabilityState("connected",true),"healthy");
  assert.equal(bridgeCapabilityState("connecting",true),"checking");
  assert.equal(bridgeCapabilityState("stale",true),"warning");
  assert.equal(bridgeCapabilityState("unavailable",true),"failed");
  const summary={cc:{},ccStatus:"connected",claudeProfileState:"synced"};
  for(const copy of Object.values(remoteBridgeMessages)) {
    assert.deepEqual(overviewServerDirectStatus(copy,"healthy"),{state:"healthy",label:copy.rbOverviewDirectAvailable});
    assert.deepEqual(overviewServerDirectStatus(copy,"failed"),{state:"warning",label:copy.rbOverviewDirectUnavailable});
    assert.deepEqual(overviewServerDirectStatus(copy,"warning"),{state:"idle",label:copy.rbOverviewDirectUnknown});
    assert.deepEqual(overviewServerDirectStatus(copy,"idle"),{state:"idle",label:copy.rbCheckIdle});
    assert.deepEqual(overviewServerDirectStatus(copy,"checking"),{state:"checking",label:copy.rbCheckChecking});
    assert.equal(overviewToolLabel(copy,summary,"codex",true),copy.rbOverviewConfigured,"Codex sync cannot be inferred from Claude or configured status");
    assert.equal(overviewToolLabel(copy,summary,"claude",true),copy.rbOverviewSynced);
    assert.equal(overviewToolLabel(copy,summary,"claude",false),copy.rbNotConfigured);
    assert.equal(overviewToolLabel(copy,{...summary,ccStatus:"unavailable"},"claude",true),copy.rbToolRouteUnavailable);
    assert.equal(overviewToolLabel(copy,{...summary,claudeProfileState:"restartRequired"},"claude",true),copy.rbOverviewRestartRequired);
    assert.equal(overviewToolLabel(copy,{...summary,claudeProfileState:"conflict"},"claude",true),copy.rbProfileConflict);
    assert.deepEqual(overviewSkillStatus(copy),{state:"disabled",label:copy.rbSkillNotLinked});
    assert.deepEqual(overviewSkillStatus(copy,{enabled:false,state:"synced"}),{state:"disabled",label:copy.rbAccessDisabled});
    assert.deepEqual(overviewSkillStatus(copy,{enabled:true,state:"conflict"}),{state:"warning",label:copy.rbSkillConflict});
    assert.deepEqual(overviewSkillStatus(copy,{enabled:true,state:"synced"}),{state:"healthy",label:copy.rbSkillSynced});
  }
  const overview=readFileSync("src/features/remote-bridge/components/RemoteBridgeOverview.vue","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(overview,/capability\.route\.local\.host, capability\.route\.local\.port/);
  assert.match(overview,/127\.0\.0\.1:\{\{ capability\.route\.remotePort \}\}/);
  assert.doesNotMatch(overview,/proxy_relay|relayPort|remoteBackend|invoke\(/);
  assert.match(overview,/capability-switch[\s\S]*:checked="!!capability\.route" :disabled="busy \|\| !connected/);
  assert.match(overview,/@click\.prevent="emit\('toggleCapability', capability\.id, !capability\.route\)"/);
  assert.match(page,/remoteBackend\.setCapability\(/);
  assert.match(page,/emit\("connected", summary\)/);
  assert.match(page,/overviewServerDirectStatus\(props\.copy, serverInternetCheck\.value\.state\)/);
  assert.match(page,/class="remote-server-globe" aria-hidden="true"/);
  assert.match(page,/copy\.rbOverviewServerDirectNetwork/);
  const launch=overview.slice(overview.indexOf('<footer class="bridge-launch-bar">'));
  assert.ok(launch.indexOf("rbOpenTerminal")<launch.indexOf("rbVscodeOpen"));
  assert.ok(launch.indexOf("rbVscodeOpen")<launch.indexOf("rbLaunchMobaxterm"));
  assert.doesNotMatch(launch,/v-if="summary\.target/);
  assert.match(overview,/grid-template-columns:repeat\(2,minmax\(0,1fr\)\)/);
  assert.match(overview,/@media \(max-width:760px\)/);
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
  const setupStart=page.indexOf('<div v-if="!live" class="remote-setup-grid">');
  const connectedStart=page.indexOf('<template v-else>',setupStart);
  assert.ok(setupStart>=0 && connectedStart>setupStart,"Setup and connected workspace boundaries must both exist");
  const targetSelection=page.slice(setupStart,connectedStart);
  assert.doesNotMatch(targetSelection,/launchSshTerminal/);
  assert.doesNotMatch(targetSelection,/remoteBackend\.openVscode\(/);
  assert.doesNotMatch(targetSelection,/launchMobaxterm/);
  assert.match(page.slice(connectedStart),/@mobaxterm="perform\(openMobaxterm\)"/);
  const overview=readFileSync("src/features/remote-bridge/components/RemoteBridgeOverview.vue","utf8");
  assert.match(overview,/:disabled="busy \|\| !connected \|\| !summary\.target\?\.canOpenMobaxterm"/);
});

test("two-step setup auto-connects after interactive authentication and keeps one stable modal shell",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(page,/@click="startConnection"/);
  assert.doesNotMatch(page,/@click="checkTarget"/);
  assert.match(page,/outcome\.operation === "check"[\s\S]*connectPrepared\(request\(\)\)/);
  assert.match(page,/connectionPhase\.value = "building"/);
  assert.match(page,/connectionPhase\.value = "succeeded"/);
  assert.match(page,/bridgeErrorCode\(cause\) !== "portInUse"/);
  assert.match(page,/remoteBackend\.allocatePorts\(selected\.targetId, false\)/);
  assert.match(page,/connectionPhase === 'failed' \|\| authSession\?\.status !== 'succeeded'/);
  assert.match(page,/\.remote-auth-dialog \{[^}]*width:min\(400px,calc\(100vw - 40px\)\)[^}]*max-height:calc\(100vh - 40px\)/);
  assert.doesNotMatch(page,/\.remote-auth-dialog \{[^}]*height:min\(/);
  assert.match(page,/authInteractionVisible/);
  assert.match(page,/authSurfaceVisible/);
  assert.match(page,/const visibleAuthPrompt = computed\(\(\) => authPrompt\.value \?\? lastAuthPrompt\.value\)/);
  assert.match(page,/\["authenticating", "building", "succeeded"\]\.includes\(connectionPhase\.value\)/);
  assert.doesNotMatch(page,/remote-auth-context|remote-auth-progress/);
  assert.match(page,/connectionPhase\.value = "authenticating";[\s\S]*showModal\(\)[\s\S]*sshAuthBegin/);
  assert.match(page,/submittedAuthMask\.value = visibleAuthPrompt\.value\?\.secret \? "••••••••" : ""/);
  assert.match(page,/:value="authInputValue"/);
  assert.match(page,/:disabled="authIsHostConfirmation \|\| !authCanRespond \|\| authSubmitting"/);
  assert.equal((page.match(/@click="startConnection"/g)??[]).length,1);
});

test("SSH authentication errors reserve space and copy only displayed information",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(page,/<div id="remote-auth-error" class="remote-auth-error-slot"/);
  assert.doesNotMatch(page,/<div[^>]*v-if=[^>]*class="remote-auth-error-slot"/);
  assert.match(page,/\.remote-auth-error-slot \{[^}]*flex:none[^}]*height:32px[^}]*margin-top:4px/);
  assert.match(page,/\.remote-auth-error-slot p \{[^}]*overflow:auto/);
  assert.match(page,/:aria-describedby="authErrorMessage \? 'remote-auth-error' : undefined"/);
  assert.match(page,/\.remote-auth-dialog pre,\.remote-auth-error-slot p \{[^}]*user-select:text/);
  const copyAction=page.slice(page.indexOf("async function copyAuthPrompt()"),page.indexOf("watch([() => visibleAuthPrompt.value?.message"));
  assert.match(copyAction,/await copyText\(\[prompt, authErrorMessage.value\]/);
  assert.doesNotMatch(copyAction,/authResponse\.value|submittedAuthMask\.value|sshAuthSubmit/);
  assert.match(page,/@click="copyAuthPrompt"/);
  assert.match(page,/remote-auth-rejected/);
  const clipboard=readFileSync("src/shared/utils/clipboard.ts","utf8");
  assert.match(clipboard,/document\.querySelector\("dialog\[open\]"\) \?\? document\.body/);
});

test("SSH authentication dialog keeps a compact width and input-to-action spacing",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(page,/\.confirmation-dialog\.remote-auth-dialog \{[^}]*width:min\(400px,calc\(100vw - 40px\)\)[^}]*max-height:calc\(100vh - 40px\)/);
  assert.match(page,/\.confirmation-dialog\.remote-auth-dialog > form \{[^}]*padding:20px/);
  assert.match(page,/\.remote-auth-dialog \.confirmation-actions \{[^}]*flex:none[^}]*margin-top:8px/);
  assert.match(page,/\.remote-auth-field input \{[^}]*height:40px/);
  assert.match(page,/\.remote-auth-dialog > form \{[^}]*overflow-y:auto/);
});

test("SSH polling does not steal selection from other authentication text",async()=>{
  const { ref, watch, nextTick }=await import("vue");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const sources=page.match(/watch\(\s*\/\/ Watch primitive values separately:[^\n]*\n\s*(\[[\s\S]*?\]),\s*async \(\) =>/);
  assert.ok(sources,"Authentication focus must watch individual primitive values");
  const snapshot=(id="prompt-1",status="waitingUser",attempt=1)=>({prompt:{id,type:"password",attempt},status});
  const authSession=ref(undefined);
  const getters=new Function("authSession",`return ${sources[1]}`)(authSession);
  let focusUpdates=0;
  const stop=watch(getters,()=>{focusUpdates+=1;});
  try {
    authSession.value=snapshot();
    await nextTick();
    assert.equal(focusUpdates,1,"First prompt may focus the input");
    for(let poll=0;poll<5;poll+=1) {
      authSession.value=snapshot();
      await nextTick();
    }
    assert.equal(focusUpdates,1,"Fresh objects containing the same prompt must preserve selection");
    authSession.value=snapshot("prompt-2","waitingUser",2);
    await nextTick();
    assert.equal(focusUpdates,2,"A new authentication challenge may focus the input");
  } finally { stop(); }
  assert.match(page,/\.remote-auth-heading h2,\.remote-auth-heading :deep\(\.check-status span\)[^\n]*user-select:text/);
  assert.match(page,/authInput\.value\?\.focus\(\{ preventScroll: true \}\)/);
});

test("bridge construction keeps disabled loading actions and omits detached busy text",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(page,/const live = computed\(\(\) => !establishingBridge\.value/);
  assert.match(page,/:disabled="busy \|\| establishingBridge \|\| !canStartConnection"/);
  assert.match(page,/establishingBridge \? copy\.rbEstablishingBridge : copy\.rbConnect/);
  assert.match(page,/v-if="authInteractionVisible \|\| authSurfaceVisible"/);
  assert.match(page,/v-if="feedbackText && !establishingBridge && !busy"/);
  assert.doesNotMatch(page,/busy \? copy\.rbBusy/);
  assert.match(page,/@media \(prefers-reduced-motion:reduce\)/);
});

test("MobaXterm launch is available only from the connected overview",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const groups=readFileSync("src/features/remote-bridge/components/RemoteTargetGroups.vue","utf8");
  const state=readFileSync("src/features/remote-bridge/state.ts","utf8");
  const bridge=readFileSync("src-tauri/src/features/remote_bridge/mod.rs","utf8");
  const ssh=readFileSync("src-tauri/src/features/remote_bridge/ssh.rs","utf8");
  assert.match(state,/canOpenMobaxterm: boolean/);
  assert.match(bridge,/pub can_open_mobaxterm: bool/);
  assert.match(ssh,/let can_open_mobaxterm = mobaxterm::available\(\)/);
  assert.match(ssh,/mobaxterm::launch_application\(\)/);
  assert.doesNotMatch(groups,/canOpenMobaxterm|openMoba|rbLaunchMobaxterm/);
  const overview=readFileSync("src/features/remote-bridge/components/RemoteBridgeOverview.vue","utf8");
  assert.match(overview,/summary\.target\?\.canOpenMobaxterm/);
  assert.doesNotMatch(page,/@open-moba=/);
  const advancedStart=page.indexOf('<section v-if="summary.proxy" v-show="advancedView"');
  const advancedEnd=page.indexOf('</section>',advancedStart);
  assert.doesNotMatch(page.slice(advancedStart,advancedEnd),/launchMobaxterm/);
});

test("MobaXterm handoff exposes the shared environment action without claiming AI activation",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(page,/async function openMobaxterm\(\)[\s\S]*await remoteBackend\.launchMobaxterm[\s\S]*mobaOpened\.value = true/);
  assert.match(page,/@mobaxterm="perform\(openMobaxterm\)"/);
  const start=page.indexOf('<section v-if="mobaOpened && sshConnected"');
  const handoff=page.slice(start,page.indexOf('</section>',start));
  assert.match(handoff,/summary\.proxy/);
  assert.match(handoff,/@click="copySessionEnvironment"/);
  assert.match(handoff,/summary\.cc[\s\S]*rbMobaAiHint/);
  assert.doesNotMatch(handoff,/configApply|configPreview|sessionToken|export/);
  assert.match(page,/watch\(sshConnected[\s\S]*mobaOpened\.value = false/);
});

test("terminal handoff prepares pending environment and allows AI-only plain terminals",()=>{
  const bridge=readFileSync("src-tauri/src/features/remote_bridge/mod.rs","utf8");
  const context=bridge.slice(bridge.indexOf('fn proxy_terminal_context()'),bridge.indexOf('pub fn launch_proxy_terminal()'));
  assert.match(context,/session_environment_state != RuntimeState::Ready[\s\S]*apply_session_environment[\s\S]*result\?;/);
  const manual=bridge.slice(bridge.indexOf('pub fn launch_manual_terminal()'),bridge.indexOf('pub fn config_preview('));
  assert.match(manual,/connected_terminal_context\(\)/);
  assert.doesNotMatch(manual,/proxy_terminal_context\(\)/);
  const moba=bridge.slice(bridge.indexOf('pub fn launch_mobaxterm('),bridge.indexOf('pub fn check('));
  assert.match(moba,/connected_terminal_context\(\)[\s\S]*connected_target != target_id[\s\S]*ssh::launch_mobaxterm_target/);
});

test("VS Code network error categories cross the helper and SSH boundary without configuration values",()=>{
  const helper=readFileSync('scripts/remote-extension/main.mjs','utf8');
  const ssh=readFileSync('src-tauri/src/features/remote_bridge/ssh.rs','utf8');
  for (const code of ['vscodeNetworkBusy','vscodeNetworkConflict','vscodeNetworkUnsafe']) {
    assert.ok(helper.includes(code));
    assert.ok(ssh.includes(`"${code}"`));
  }
  assert.match(helper,/error\.message === 'unsafePath'[\s\S]*fail\('vscodeNetworkUnsafe'\)/);
  assert.match(helper,/error\.message === 'configConflict'[\s\S]*fail\('vscodeNetworkConflict'\)/);
});

test("core bridge becomes usable before optional setup and reconnect retries the retained session",()=>{
  const bridge=readFileSync("src-tauri/src/features/remote_bridge/mod.rs","utf8");
  const reconnect=readFileSync("src-tauri/src/features/remote_bridge/reconnect.rs","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const state=readFileSync("src/features/remote-bridge/state.ts","utf8");
  assert.match(bridge,/prepare_post_connect\(&mut summary\)[\s\S]*state\.summary = summary[\s\S]*start_post_connect\(generation\)/);
  assert.match(bridge,/sessionEnv\.apply/);
  assert.match(bridge,/vscodeNetwork\.refresh/);
  assert.match(bridge,/postConnect\.total/);
  assert.match(reconnect,/pub\(super\) fn resume\(state: &mut Store\)/);
  assert.doesNotMatch(reconnect,/reconnect_probe/);
  assert.match(reconnect,/1 => 1,[\s\S]*2 => 2,[\s\S]*3 => 4,[\s\S]*4 => 8,[\s\S]*5 => 15,[\s\S]*_ => 30/);
  assert.match(state,/retryReconnect: \(\) => invoke<BridgeSummary>\("remote_bridge_retry_reconnect"\)/);
  assert.match(page,/await remoteBackend\.retryReconnect\(\)/);
  assert.match(page,/remote-post-connect/);
});

test("remote workspace adapts to the remaining window area without clipping connection actions",()=>{
  const shell=readFileSync("src/shared/styles/index.css","utf8");
  const app=readFileSync("src/app/AppShell.vue","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(shell,/\.app-frame \{[^}]*width: 100%;[^}]*height: 100dvh/);
  assert.doesNotMatch(shell,/width: min\(880px, 100%\)/);
  assert.match(shell,/\.view-stage \{[^}]*min-height: 0;[^}]*flex: 1;[^}]*overflow: auto/);
  assert.match(app,/class="view-pane remote-view-pane"/);
  assert.match(page,/\.remote-bridge-page\.remote-setup-page \{ height:100%;/);
  assert.doesNotMatch(page,/100dvh - 71px|minmax\(326px|minmax\(342px/);
  assert.match(page,/class="remote-current-content"/);
  assert.match(page,/\.remote-current-content \{[^}]*min-height:0;[^}]*overflow-y:auto/);
  assert.match(page,/\.remote-setup-actions \{[^}]*flex:none/);
  assert.match(page,/grid-template-columns:minmax\(0,\.9fr\) minmax\(0,1\.1fr\)/);
});

test("remote SSH empty state finishes actions before random scenes and duets with bounded motion",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const mascot=readFileSync("src/features/remote-bridge/components/RemoteEmptyMascot.vue","utf8");
  const sprite=readFileSync("src/features/remote-bridge/components/PixelMascotSprite.vue","utf8");
  assert.match(page,/<div v-else class="remote-current-empty">[\s\S]*?copy\.rbNoSelectionHint[\s\S]*?<RemoteEmptyMascot :paused="visible === false" \/>/);
  assert.match(mascot,/const scene = ref\(randomChoice\(soloScenes\)\)/);
  assert.match(mascot,/sceneId\.value % 3 === 2 \? duoScenes/);
  assert.match(mascot,/mode="out-in"/);
  assert.match(mascot,/@complete="index === scene\.kinds\.length - 1 && advanceScene\(\)"/);
  assert.match(sprite,/@animationend\.self="emit\('complete'\)"/);
  assert.match(sprite,/animation-iteration-count:var\(--mascot-iterations\)/);
  assert.match(mascot,/height:clamp\(100px,18dvh,144px\)/);
  assert.doesNotMatch(mascot,/setInterval|setTimeout|requestAnimationFrame|fetch\(/);
  assert.match(mascot,/new IntersectionObserver/);
  assert.match(mascot,/observer\?\.disconnect\(\)/);
  assert.match(mascot,/document\.removeEventListener\("visibilitychange"/);
  assert.match(mascot,/aria-hidden="true"/);
  assert.match(mascot,/animation-play-state:paused !important/);
  assert.match(mascot,/prefers-reduced-motion:reduce[\s\S]*animation:none !important/);
  assert.match(sprite,/shape-rendering:crispEdges; image-rendering:pixelated/);
  assert.match(sprite,/--claude-body:#e5a08a/);
  assert.match(sprite,/--codex-body:#91a8df/);
  assert.match(sprite,/class="claude-silhouette"/);
  assert.match(sprite,/H32v12h-8V68H12V44h12Z/); // Left short limb ends at the same height as the right.
  assert.match(sprite,/<path class="codex-outline"/);
  assert.match(sprite,/<path class="codex-blob"/);
  assert.doesNotMatch(mascot,/M18 4h4v6h6v4h-6v6h-4v-6h-6v-4h6Z/);
  assert.match(mascot,/scene\.action === 'game'/);
  assert.match(mascot,/@keyframes pixel-duet-play/);
  assert.match(sprite,/data-theme="dark"/);
});

test("SSH connection manager keeps credentials out and promotes edited imports to full ProxyEnv copies",()=>{
  const store=readFileSync("src-tauri/src/features/remote_bridge/connections.rs","utf8");
  const moba=readFileSync("src-tauri/src/features/remote_bridge/mobaxterm.rs","utf8");
  const state=readFileSync("src/features/remote-bridge/state.ts","utf8");
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(store,/pub struct ManualConnectionInput/);
  assert.match(store,/ManualAuthentication::Automatic/);
  assert.match(store,/ManualAuthentication::Password/);
  assert.match(store,/symlink_metadata/);
  assert.doesNotMatch(store,/pub\s+(password|passphrase|credential)\s*:/i);
  assert.match(moba,/\.arg|\.args\(bookmark_arguments/);
  assert.match(moba,/"-i"\.into\(\)/);
  assert.match(moba,/"-bookmark"\.into\(\)/);
  assert.match(moba,/pub fn launch_application\(\)/);
  assert.match(moba,/dirs::desktop_dir\(\)/);
  assert.match(state,/addConnection: .*remote_bridge_add_connection/);
  assert.match(state,/updateConnection: .*remote_bridge_update_connection/);
  assert.match(state,/removeConnection: .*remote_bridge_remove_connection/);
  assert.match(page,/copy\.rbAddConnection/);
  const toolbar=page.slice(page.indexOf('<div class="remote-status-toolbar">'),page.indexOf('<fieldset',page.indexOf('<div class="remote-status-toolbar">')));
  assert.ok(toolbar.indexOf('copy.rbAddConnection') < toolbar.indexOf('copy.rbRefresh'));
  const setupActions=page.slice(page.indexOf('<div class="remote-setup-actions">'),page.indexOf('</div>',page.indexOf('<div class="remote-setup-actions">')));
  assert.doesNotMatch(setupActions,/copy\.rbAddConnection/);
  assert.match(page,/newConnection\.authentication === 'identityFile'/);
  assert.match(page,/function destinationInputValid\(value: string\)/);
  assert.match(page,/copy\.rbConnectionDestinationInvalid/);
  const authenticationChoices=page.slice(page.indexOf('<fieldset class="remote-auth-choice">'),page.indexOf('</fieldset>',page.indexOf('<fieldset class="remote-auth-choice">')));
  assert.ok(authenticationChoices.indexOf('value="password"') < authenticationChoices.indexOf('value="automatic"'));
  assert.match(page,/newConnection = ref<ManualConnectionInput>\(\{[^}]*authentication: "password"/);
  assert.match(page,/function openConnectionDialog\(\)[\s\S]*newConnection\.value = \{[^}]*authentication: "password"/);
  assert.match(page,/sourceTargetNames/);
  assert.match(page,/hiddenSourceTargets/);
  assert.match(page,/editingManualId/);
  assert.match(page,/copyingSourceId/);
  assert.match(page,/remoteBackend\.updateConnection\(editingManualId\.value, input\)/);
  assert.match(page,/target\.authenticationMethod === "password"/);
  assert.match(page,/hiddenSourceTargets\.value = new Set\(hiddenSourceTargets\.value\)\.add\(sourceCopyId\)/);
  assert.doesNotMatch(page,/editConnectionDialog/);
  assert.match(page,/proxyenv\.remoteBridge\.lastTargetId/);
  assert.match(page,/localStorage\.setItem\(lastTargetStorageKey, nextTarget\)/);
  assert.match(page,/copy\.rbConnectionCopyEditHint/);
});

test("SSH connection explanations reuse question-mark tooltips without hiding validation",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const dialog=page.slice(page.indexOf('<dialog ref="connectionDialog"'),page.indexOf('<dialog ref="removeConnectionDialog"'));
  for(const hint of ["rbConnectionDestinationHint","rbConnectionPortHint","rbConnectionAuthPasswordHint","rbConnectionAuthAutomaticHint","rbConnectionIdentityHint"]) {
    assert.match(dialog,new RegExp(`<HelpTooltip[^>]*:text="copy\\.${hint}"`));
    assert.doesNotMatch(dialog,new RegExp(`<small[^>]*>\\{\\{ copy\\.${hint} \\}\\}`));
  }
  assert.doesNotMatch(dialog,/<p v-if="editingManualId \|\| copyingSourceId"/);
  for (const field of ["name", "destination", "port", "identity"]) assert.match(dialog,new RegExp(`<FieldValidation id="remote-${field}-error"`));
  assert.doesNotMatch(dialog,/class="remote-field-error"/);
  assert.match(dialog,/:aria-invalid="!!connectionFieldErrors.destination"/);
  assert.match(dialog,/<form novalidate/);
  assert.match(dialog,/role="alert">\{\{ connectionSaveErrorText/);
  assert.match(dialog,/value="password"/);
  const tooltip=readFileSync("src/shared/components/HelpTooltip.vue","utf8");
  assert.match(tooltip,/closest<HTMLElement>\("dialog\[open\]"\) \?\? "body"/);
  assert.match(tooltip,/<Teleport :to="tooltipTarget">/);
  assert.match(tooltip,/@mouseenter="showTooltip"/);
  assert.match(tooltip,/@focus="showTooltip"/);
});

test("SSH identity selection uses a native picker and validates without copying key material",()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  assert.match(page,/@click="pickIdentityFile"/);
  assert.match(page,/<div class="remote-identity-option"/);
  assert.match(page,/<button v-if="newConnection.authentication === 'identityFile'"[^>]*@click="pickIdentityFile"/);
  assert.match(page,/\.remote-identity-option \{[^}]*min-height:36px/);
  assert.match(page,/\.remote-identity-option \.secondary-action \{[^}]*min-height:30px/);
  assert.match(page,/\.remote-identity-name.inactive \{ visibility:hidden/);
  assert.doesNotMatch(page,/<div v-if="newConnection.authentication === 'identityFile'"/);
  assert.match(page,/class="remote-identity-name"/);
  assert.doesNotMatch(page,/v-model="newConnection.identityFile"/);
  assert.match(page,/if \(selected && connectionDialog\.value\?\.open/);
  const state=readFileSync("src/features/remote-bridge/state.ts","utf8");
  assert.match(page,/remoteBackend\.pickIdentityFile\(newConnection\.value\.identityFile\)/);
  assert.match(state,/invoke<string \| null>\("remote_bridge_pick_identity_file", \{ initialPath \}\)/);
  const commands=readFileSync("src-tauri/src/commands/remote_bridge.rs","utf8");
  assert.match(commands,/dialog\.blocking_pick_file\(\)/);
  assert.match(commands,/validate_identity_file\(path\)\.ok\(\)/);
  assert.match(commands,/dialog\.set_directory\(parent\)/);
  assert.match(commands,/dialog\.set_file_name\(name\)/);
  assert.match(commands,/validate_identity_file\(path\)\.map\(Some\)/);
  const backend=readFileSync("src-tauri/src/features/remote_bridge/connections.rs","utf8");
  assert.match(backend,/safe_read_first_line\(&path, 1024 \* 1024\)/);
  assert.match(backend,/identityFileUnsupported/);
  const localFile=readFileSync("src-tauri/src/services/local_file.rs","utf8");
  assert.match(localFile,/let mut reader = file\.take\(64\)/);
  assert.match(localFile,/reader\.read\(&mut byte\)/);
  assert.match(readFileSync("src-tauri/src/lib.rs","utf8"),/remote_bridge_pick_identity_file/);
});

test("SSH form uses Unicode name limits and actionable localized errors",async()=>{
  const page=readFileSync("src/features/remote-bridge/components/RemoteBridgePage.vue","utf8");
  const functionSource=page.match(/function displayNameInputValid\(value: string\): boolean \{[\s\S]*?\n\}/)?.[0];
  assert.ok(functionSource);
  const limit=Number(page.match(/const connectionNameLimit = (\d+);/)?.[1]);
  assert.equal(limit,32);
  const source=`const connectionNameLimit=${limit};\n${functionSource}\nexport {displayNameInputValid};`;
  const compiled=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
  const {displayNameInputValid}=await import(`data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`);
  for(const name of ["Lab","中文环境","中".repeat(32),"🧪".repeat(32)," Lab "]) assert.equal(displayNameInputValid(name),true);
  for(const name of ["","   ","x".repeat(33),"中".repeat(33),"🧪".repeat(33),"a\u0007b","a\u0085b"]) assert.equal(displayNameInputValid(name),false);
  const limiterSource=page.match(/function limitConnectionName\(event: Event\) \{[\s\S]*?\n\}/)?.[0];
  assert.ok(limiterSource);
  const limiterCompiled=ts.transpileModule(`const connectionNameLimit=32; const newConnection={value:{displayName:""}}; ${limiterSource}\nexport {limitConnectionName,newConnection};`,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
  const {limitConnectionName,newConnection}=await import(`data:text/javascript;base64,${Buffer.from(limiterCompiled).toString("base64")}`);
  for(const name of ["x".repeat(33),"中".repeat(33),"🧪".repeat(33)]) {
    const target={value:name};
    limitConnectionName({target,isComposing:false});
    assert.equal([...target.value].length,32);
    assert.equal(newConnection.value.displayName,target.value);
  }
  const composing={value:"中".repeat(33)};
  limitConnectionName({target:composing,isComposing:true});
  assert.equal([...composing.value].length,33);
  assert.match(page,/@compositionend="limitConnectionName"/);
  assert.match(page,/Number\.isInteger\(newConnection.value.port\)/);
  assert.match(page,/querySelector<HTMLElement>\('\[aria-invalid="true"\]'/);
  assert.match(page,/if \(onError\) onError\(cause\);\s+else error.value = cause/);
  const field=readFileSync("src/shared/components/FieldValidation.vue","utf8");
  assert.match(field,/<HelpTooltip[^>]*tone="error"/);
  assert.match(field,/:id="id" class="field-validation-description"/);
  const messagesSource=readFileSync("src/shared/i18n/remote-bridge.ts","utf8");
  const messagesCompiled=ts.transpileModule(messagesSource,{compilerOptions:{module:ts.ModuleKind.ESNext}}).outputText;
  const {remoteBridgeMessages,bridgeError}=await import(`data:text/javascript;base64,${Buffer.from(messagesCompiled).toString("base64")}`);
  for(const copy of Object.values(remoteBridgeMessages)) {
    assert.equal(bridgeError("Command remote_bridge_pick_identity_file not found",copy),copy.rbBackendRestartRequired);
    for(const [code,key] of Object.entries({invalidConnectionName:"rbConnectionNameInvalid",invalidConnectionDestination:"rbConnectionDestinationInvalid",invalidConnectionPort:"rbConnectionPortInvalid",connectionLimitReached:"rbConnectionLimitError"})) assert.equal(bridgeError({code,phase:"targetWrite"},copy),copy[key]);
  }
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
