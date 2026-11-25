import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createFixtureEnvironment, posix } from "./shell-environment.mjs";

export { posix };

export function createShellFixtureFactory({ root, shell, python, script, probeScript }) {
  return function fixture({jsonEngine="python3",claudeLocation="path",codexLocation="path",codexBody,versionBody,privateGroup=false,sharedGroup=false}={}) {
    const directory=mkdtempSync(join(root,"bridge-test-"));
    const home=join(directory,"home"), bin=join(directory,"bin"), temp=join(directory,"tmp");
    mkdirSync(home);mkdirSync(bin);mkdirSync(temp,{mode:0o700});
    const environment = (overrides = {}) => createFixtureEnvironment({ home, bin, temp }, overrides);
    const mock=(name,body)=>writeFileSync(join(bin,name),`#!/bin/sh\n${body}\n`,{mode:0o755});
    mock("uname",'printf "%s" "${TEST_TARGET_OS-Linux}"');
    mock("ss",'printf "%s" "${TEST_LISTENERS:-}"');
    // MSYS has no flock or Unix permission model. Only these platform adapters
    // are mocked; file content, hashing, writes, readback and restore are real.
    mock("flock","exit 0");
    if(privateGroup) mock("id",'[ "$1" != -gn ] || { /usr/bin/id -un; exit; }; /usr/bin/id "$@"');
    if(sharedGroup) mock("id",'[ "$1" != -gn ] || { printf lab-users; exit; }; /usr/bin/id "$@"');
    if (python) mock(jsonEngine,`exec '${posix(python).replaceAll("'", "'\\''")}' "$@"`);
    if(process.platform==="win32") mock("stat",'[ "$2" != %a ] || { printf 700; exit; }; exec /usr/bin/stat "$@"');
    mock("mv",'for target do :; done; if [ "${TEST_FAIL_REPLACE:-}" = 1 ] && [ ! -e "$HOME/.replace-failed" ]; then case "$target" in *config.toml|*settings.json) touch "$HOME/.replace-failed"; exit 1;; esac; fi; exec /usr/bin/mv "$@"');
    const codexVersionBody=codexBody ?? 'printf "%s\\n" "${TEST_CODEX_VERSION:-codex-cli 0.134.0}"';
    if(codexLocation==="path") mock("codex",codexVersionBody);
    else if(codexLocation!=="missing") {
      const userBin=join(home,codexLocation); mkdirSync(userBin,{recursive:true,mode:0o700});
      writeFileSync(join(userBin,"codex"),`#!/bin/sh\n${codexVersionBody}\n`,{mode:0o755});
    }
    const claudeBody='if [ "$1" = --version ]; then printf "2.1.227 (Claude Code)\\n"; exit; fi; case "${TEST_CLAUDE_VERIFY:-verified}" in verified) printf \'{"result":"PROXYENV_VERIFY_OK"}\\n\';; auth) printf \'login required secret-fixture\\n\' >&2; exit 1;; route) printf \'gateway connection refused secret-fixture\\n\' >&2; exit 1;; timeout) exit 124;; *) printf \'unexpected secret-fixture\\n\' >&2; exit 1;; esac';
    if(["native","nvm"].includes(claudeLocation)) {
      const userBin=claudeLocation==="native" ? join(home,".local/bin") : join(home,".nvm/versions/node/v22.0.0/bin");
      mkdirSync(userBin,{recursive:true,mode:claudeLocation==="nvm" ? 0o775 : 0o700});
      writeFileSync(join(userBin,"claude"),`#!/bin/sh\n${versionBody ?? claudeBody}\n`,{mode:0o755});
    } else if(claudeLocation==="path") mock("claude",versionBody ?? claudeBody);
    mock("curl",'[ "${TEST_CURL_RESULT:-ok}" = ok ]');
    const run=(operation,tool="codex",port=25721,expected="absent",env={})=>{
      let backupHash="absent";
      if(operation==="restore") { const reviewed=run("restore-preview",tool,port); if(reviewed.error) return reviewed; if(expected==="absent") expected=reviewed.expectedHash; backupHash=reviewed.backupHash; }
      const profileModel=env.TEST_PROFILE_MODEL ?? "fixture-model";
      const profileCatalog=env.TEST_PROFILE_CATALOG ?? JSON.stringify({models:[{slug:profileModel,future:{preserved:true}}],futureRoot:[1,2]});
      const profileHash=createHash("sha256").update(profileModel).update(Buffer.from([0])).update(profileCatalog).digest("hex");
      const needsProfile=tool==="codex" && ["preview","apply"].includes(operation);
      const claudeProfile=env.TEST_CLAUDE_PROFILE || '{"model":"fixture-claude"}';
      const claudeProfileHash=createHash("sha256").update(claudeProfile).digest("hex");
      const needsClaudeProfile=tool==="claude" && ["preview","apply"].includes(operation);
      const sessionToken=env.TEST_SESSION_TOKEN || "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
      const input=`export HOME='${posix(home)}'\nexport PATH='${posix(bin)}:/usr/bin:/bin'\noperation='${operation}'\ntool='${tool}'\nport=${port}\nports='${env.TEST_REMOTE_PORT ?? 17897}'\nexpected='${expected}'\nexpected_backup='${backupHash}'\nrepair_permissions='${env.TEST_REPAIR_PERMISSIONS === "1" ? "true" : "false"}'\nprotocol='http'\nscheme='http'\nsession_id='0123456789abcdef0123456789abcdef'\nsession_token='${sessionToken}'\nPROXYENV_SESSION_TOKEN="$session_token"\nexport PROXYENV_SESSION_TOKEN\nprofile_model_b64='${needsProfile ? Buffer.from(profileModel).toString("base64") : ""}'\nprofile_catalog_b64='${needsProfile ? Buffer.from(profileCatalog).toString("base64") : ""}'\nprofile_settings_b64='${needsClaudeProfile ? Buffer.from(claudeProfile).toString("base64") : ""}'\nprofile_hash='${needsProfile ? profileHash : needsClaudeProfile ? claudeProfileHash : ""}'\n${env.TEST_COMPACT_PROBE === "1" ? probeScript : script}`;
      const started=performance.now();
      const result=spawnSync(shell,["-s"],{input,encoding:"utf8",timeout:60000,env:environment(env)});
      assert.equal(result.status,0,`${tool}/${operation}: ${Math.round(performance.now()-started)}ms; ${result.error?.code ?? result.signal ?? "exit"}; ${result.stderr || ""}`);
      return JSON.parse(result.stdout.trim());
    };
    const runSkill=(operation,tool,content,env={})=>{
      if(process.platform==="win32") {
        // MSYS cannot apply POSIX modes to Windows ACLs. Ownership and hashing
        // remain real; Linux CI exercises the actual mode checks.
        mock("mkdir",'if [ "$1" = -m ]; then shift 2; fi; exec /usr/bin/mkdir "$@"');
        mock("chmod",'exit 0');
        mock("find",'for argument do if [ "$argument" = -perm ]; then exit 0; fi; done; exec /usr/bin/find "$@"');
      }
      const skillHash=createHash("sha256").update(content).digest("hex");
      const manifest=`${skillHash}  SKILL.md\n`;
      const input=`export HOME='${posix(home)}'\nexport PATH='${posix(bin)}:/usr/bin:/bin'\noperation='${operation}'\ntool='${tool}'\nskill_name='fixture-skill'\nskill_hash='${skillHash}'\nfile_count=1\ntotal_size=${Buffer.byteLength(content)}\nmanifest_b64='${Buffer.from(manifest).toString("base64")}'\ndirectories_b64=''\n${readFileSync("src-tauri/src/features/remote_bridge/skill-remote.sh","utf8")}`;
      const result=spawnSync(shell,["-s"],{input,encoding:"utf8",timeout:60000,env:environment(env)});
      assert.equal(result.status,0,result.stderr || result.error?.message);
      return JSON.parse(result.stdout.trim());
    };
    return {home,run,runSkill,cleanup:()=>{ assert.ok(resolve(directory).startsWith(root + (process.platform==="win32"?"\\":"/"))); rmSync(directory,{recursive:true,force:true}); }};
  }
}
