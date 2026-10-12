import assert from "node:assert/strict";
import test from "node:test";
import {readFileSync,existsSync,mkdirSync,mkdtempSync,writeFileSync,chmodSync,rmSync} from "node:fs";
import {dirname,join,resolve} from "node:path";
import {spawnSync} from "node:child_process";
import {adapterModuleUrl,presentationModuleUrl,guidanceModuleUrl,copyModuleUrl} from "./remote-tool-test-modules.mjs";
const {canToggleRemoteTool,getRemoteToolAdapter}=await import(adapterModuleUrl);
const {overviewToolLabel,overviewToolChecking,overviewToolConfigurationState,toolInstallationLabel,toolInstallationLines,toolInstallationRows}=await import(presentationModuleUrl);
const {installMethods,installRequirementAvailable,installMissingRequirements,installPathCommands}=await import(guidanceModuleUrl);
const {remoteBridgeMessages}=await import(copyModuleUrl);
const summary=(state,configured=false) => ({status:"connected",cc:{remotePort:15721},ccStatus:"connected",codexState:"ready",claudeState:"ready",codexConfigured:configured,claudeConfigured:false,toolInstallations:[{tool:"codex",state,version:state==="unsupported"?"0.133.9":state==="installed"?"0.134.0":null},{tool:"claude",state:"installed",version:"2.1.227"}]});

test("Overview configuration badges distinguish configuration from installation and preserve warning states",()=>{
  assert.equal(overviewToolConfigurationState(summary("installed"),"codex",false),"warning");
  assert.equal(overviewToolConfigurationState(summary("installed",true),"codex",true),"healthy");
  assert.equal(overviewToolConfigurationState(summary("missing",true),"codex",true),"warning");
  assert.equal(overviewToolConfigurationState({...summary("installed",true),ccStatus:"unavailable"},"codex",true),"warning");
  assert.equal(overviewToolConfigurationState(summary("checking"),"codex",false),"checking");
  for(const profile of ["synced","restartRequired","conflict","remoteChanged","localChanged","invalidLocalProfile","remoteUnavailable"]) {
    assert.equal(overviewToolConfigurationState({...summary("installed"),claudeConfigured:true,claudeProfileState:profile},"claude",true),profile==="synced" ? "healthy" : "warning");
  }
  const overview=readFileSync("src/features/remote-bridge/components/RemoteBridgeOverview.vue","utf8");
  assert.ok(overview.indexOf('class="bridge-tool-status"') < overview.indexOf('class="bridge-tool-installation"'));
});
test("PATH recovery follows the installation method without treating installed CLI as missing",()=>{
  assert.deepEqual(installPathCommands("claude").map(recipe=>recipe.id),["nativeSession","nativeBash","npmSession"]);
  assert.deepEqual(installPathCommands("codex").map(recipe=>recipe.id),["npmSession"]);
  for(const tool of ["claude","codex"]) for(const recipe of installPathCommands(tool)) {
    assert.match(recipe.command,new RegExp(`${tool} --version`));
    assert.match(recipe.command,/\[ -x /);
    assert.doesNotMatch(recipe.command,/sudo|curl|source |\.profile|settings\.json|config\.toml/);
    if(recipe.id==="npmSession") {
      assert.match(recipe.command,/npm prefix -g/);
      assert.match(recipe.command,/\[ -n "\$proxyenv_npm_prefix" \]/);
      assert.doesNotMatch(recipe.command,/\.bashrc|\.local\/bin/);
    }
  }
  const dialog=readFileSync("src/features/remote-bridge/components/RemoteToolInstallDialog.vue","utf8");
  assert.match(dialog,/inspection.installation === 'installed' \? copy.rbInstallAlreadyInstalled/);
  assert.match(dialog,/<details class="install-path-help"/);
  assert.match(dialog,/copyCommand\(recipe.command,recipe.id\)/);
  assert.match(dialog,/pathHelpOpen.value=false/);
  for(const copy of Object.values(remoteBridgeMessages)) for(const key of ["rbInstallAlreadyInstalled","rbInstallPathTitle","rbInstallPathNpmHint","rbInstallPathCodex","rbInstallPathShell"]) assert.ok(copy[key]);
});

const gitPath=process.platform==="win32" ? spawnSync("where.exe",["git"],{encoding:"utf8"}).stdout?.trim().split(/\r?\n/)[0] : undefined;
const shell=process.env.PROXYENV_TEST_SHELL || (gitPath ? resolve(dirname(gitPath),"../bin/bash.exe") : "/bin/bash");
test("copyable PATH recipes repair a fixture terminal and persist Bash once, without touching the real HOME",{skip:!existsSync(shell)},()=>{
  const root=resolve(".debug-tmp");mkdirSync(root,{recursive:true});
  const fixture=mkdtempSync(join(root,"install-path-"));
  const posix=path=>path.replaceAll("\\","/").replace(/^([A-Za-z]):/,(_,drive)=>`/${drive.toLowerCase()}`);
  const executable=(path,body)=>{mkdirSync(dirname(path),{recursive:true});writeFileSync(path,`#!/bin/sh\n${body}\n`);chmodSync(path,0o755);};
  const run=(command,extra={})=>spawnSync(shell,["-c",command],{encoding:"utf8",timeout:10000,env:{...process.env,HOME:posix(fixture),PATH:"/usr/bin:/bin",...extra}});
  try {
    const recipes=installPathCommands("claude");
    assert.notEqual(run(recipes[1].command).status,0,"Missing executable cannot write .bashrc");
    assert.equal(existsSync(join(fixture,".bashrc")),false);
    executable(join(fixture,".local/bin/claude"),'printf "2.1.295\\n"');
    assert.equal(run(recipes[0].command).stdout.trim(),"2.1.295");
    assert.equal(existsSync(join(fixture,".bashrc")),false,"Session fix never persists");
    for(let i=0;i<2;i++) assert.equal(run(recipes[1].command).status,0);
    assert.equal(readFileSync(join(fixture,".bashrc"),"utf8").trim(),'export PATH="$HOME/.local/bin:$PATH"');
    assert.equal(run('. "$HOME/.bashrc"; claude --version').stdout.trim(),"2.1.295");
    const prefix=join(fixture,"npm prefix with spaces"),npmBin=join(fixture,"node/bin");
    executable(join(npmBin,"npm"),`printf '%s\\n' '${posix(prefix)}'`);
    const env={PATH:`${posix(npmBin)}:/usr/bin:/bin`};
    for(const tool of ["claude","codex"]) {
      executable(join(prefix,"bin",tool),`printf '${tool}-version\\n'`);
      const recipe=installPathCommands(tool).find(recipe=>recipe.id==="npmSession");
      assert.equal(run(recipe.command,env).stdout.trim(),`${tool}-version`);
      executable(join(npmBin,"npm"),'exit 1');
      assert.notEqual(run(recipe.command,env).status,0,"Failed npm prefix must stop recovery");
      executable(join(npmBin,"npm"),`printf '%s\\n' '${posix(prefix)}'`);
    }
  } finally {rmSync(fixture,{recursive:true,force:true});}
});
test("installation gating preserves disabling existing routes and tool independence",()=>{
  for(const state of ["unknown","checking","installed","missing","unsupported","broken"]) {
    assert.equal(canToggleRemoteTool(summary(state),"codex"),state==="installed");
    assert.equal(canToggleRemoteTool(summary(state,true),"codex"),state!=="checking");
    assert.equal(canToggleRemoteTool(summary(state),"claude"),true);
    const off={...summary(state,true),cc:null,ccStatus:null};
    assert.equal(getRemoteToolAdapter("codex").inspect(off).installation,state);
    assert.equal(canToggleRemoteTool(off,"codex"),state!=="checking");
  }
});
test("all locales distinguish installation, configuration and verification without changing data",()=>{
  for(const copy of Object.values(remoteBridgeMessages)) {
    assert.ok(toolInstallationLabel(copy,summary("installed"),"codex").includes("0.134.0"));
    assert.ok(overviewToolLabel(copy,summary("missing",true),"codex",true).includes(copy.rbInstallConfigured));
    assert.equal(overviewToolLabel(copy,summary("missing"),"codex",false),`CLI ${copy.rbInstallMissing}`);
    assert.ok(toolInstallationLabel(copy,summary("unsupported"),"codex").includes("0.133.9"));
    assert.equal(overviewToolChecking({...summary("checking"),cc:null},"codex"),true);
    for(const key of Object.keys(remoteBridgeMessages.en).filter(key=>key.startsWith("rbInstall"))) assert.equal(typeof copy[key],"string",key);
  }
});
test("compatible Remote SSH extensions enable their shared profile without a standalone CLI",()=>{
  for(const id of ["codex","claude"]) {
    const value=summary("missing");
    value.claudeConfigured=false;
    const observation=value.toolInstallations.find(tool=>tool.tool===id);
    observation.state="missing"; observation.version=null;
    for(const extensionState of ["unknown","checking","missing","unsupported","broken","installed"]) {
      observation.extensionState=extensionState;
      observation.extensionVersion=extensionState==="installed" ? "2.1.227" : null;
      assert.equal(canToggleRemoteTool(value,id),extensionState==="installed",`${id}/${extensionState}`);
      assert.equal(overviewToolChecking(value,id),extensionState==="checking");
    }
    assert.equal(getRemoteToolAdapter(id).inspect(value).verificationSupported,false,"CLI model probe cannot execute a plugin");
    for(const copy of Object.values(remoteBridgeMessages)) {
      assert.ok(toolInstallationLabel(copy,value,id).includes(copy.rbInstallExtension));
      assert.equal(overviewToolLabel(copy,value,id,false),copy.rbNotConfigured);
    }
    value.ccStatus="unavailable";
    assert.equal(canToggleRemoteTool(value,id),false,"Plugin discovery alone is not a working bridge");
    value[id==="codex"?"codexConfigured":"claudeConfigured"]=true;
    observation.extensionState="missing";
    assert.equal(canToggleRemoteTool(value,id),true,"Removing the plugin cannot prevent restoration");
  }
});
test("official catalog is static, has no sudo and flags missing dependencies rather than hiding commands",()=>{
  for(const tool of ["codex","claude"]) {
    const methods=installMethods(tool);
    assert.deepEqual(methods.map(method=>method.id),["native","npm","homebrew"]);
    assert.equal(methods[0].recommended,true);
    for(const method of methods) {
      assert.ok(method.officialUrl.startsWith("https://"));
      assert.doesNotMatch(method.command,/sudo|apt|yum|PATH=|bashrc/);
      assert.equal(installRequirementAvailable(tool,method),false);
      assert.equal(installRequirementAvailable(tool,method,{curl:true,npm:true,brew:true,nodeVersion:"22.18.0"}),true);
    }
  }
  assert.equal(installRequirementAvailable("claude",installMethods("claude")[1],{npm:true,nodeVersion:"17.9.0"}),false);
  assert.equal(installRequirementAvailable("claude",installMethods("claude")[1],{npm:true,nodeVersion:"18.0.0"}),true);
  assert.equal(installRequirementAvailable("claude",installMethods("claude")[1],{npm:true,nodeVersion:"18.20.0"}),true);
  assert.equal(installRequirementAvailable("claude",installMethods("claude")[1],{npm:false,nodeVersion:"22.0.0"}),false);
  assert.equal(installRequirementAvailable("claude",installMethods("claude")[1],{npm:true,nodeVersion:"22.0.0"}),true);
  for(const copy of Object.values(remoteBridgeMessages)) {
    assert.match(copy.rbInstallNode,/18\+/);
    assert.match(copy.rbInstallNodeRecommended,/22\+/);
  }
  const dialog=readFileSync("src/features/remote-bridge/components/RemoteToolInstallDialog.vue","utf8");
  assert.match(dialog,/<template v-if="tool==='claude'"> \{\{ copy.rbInstallNodeRecommended \}\}<\/template>/);
});
test("Codex and Claude share npm detection while Claude Node checks stay independent",()=>{
  const codex=installMethods("codex")[1],claude=installMethods("claude")[1];
  for(const npm of [false,true]) for(const nodeVersion of [null,"17.9.0","18.0.0","22.0.0"]) {
    const capabilities={curl:true,npm,brew:false,nodeVersion};
    const codexMissing=installMissingRequirements("codex",codex,capabilities);
    const claudeMissing=installMissingRequirements("claude",claude,capabilities);
    assert.equal(codexMissing.includes("npm"),!npm);
    assert.equal(claudeMissing.includes("npm"),!npm);
    assert.equal(claudeMissing.includes("node"),Number(nodeVersion?.split(".")[0] ?? 0)<18);
    assert.equal(codexMissing.includes("node"),false);
  }
  assert.deepEqual(installMissingRequirements("claude",claude,{npm:true,nodeVersion:null}),["node"]);
  assert.deepEqual(installMissingRequirements("claude",claude,{npm:false,nodeVersion:"22.0.0"}),["npm"]);
});

test("Advanced reuses installation evidence and Overview icons above terminal environment",()=>{
  const advanced=readFileSync("src/features/remote-bridge/components/RemoteBridgeAdvancedView.vue","utf8");
  assert.match(advanced,/toolInstallationRows\(props.copy,props.summary,adapter.id\)/);
  assert.match(advanced,/<RemoteToolIcon :tool="tool.id"/);
  assert.match(advanced,/<tbody><tr v-for="tool in installationTools"/);
  assert.match(advanced,/<td v-for="row in tool.rows"/);
  assert.match(advanced,/<th scope="col" :title="copy.rbOverviewAiTools">\{\{ copy.rbOverviewAiTools \}\}<\/th><th scope="col">CLI<\/th>/);
  assert.match(advanced,/\.advanced-installations thead th \{[^}]*height:22px; font-size:10px;/);
  assert.match(advanced,/:label="row.state === 'healthy' \? row.version \?\? '—' : row.status"/);
  assert.doesNotMatch(advanced,/advanced-links > \.advanced-group \{ flex:1/);
  assert.ok(advanced.indexOf('class="advanced-group advanced-installations"')<advanced.indexOf('<h3>{{ copy.rbAdvTerminal }}'));
  assert.doesNotMatch(advanced,/remoteBackend\.discoverTools|overviewToolLabel\(/);
  const overview=readFileSync("src/features/remote-bridge/components/RemoteBridgeOverview.vue","utf8");
  assert.match(overview,/toolInstallationLines/);
});

test("installation rows keep CLI and extension states independent of route configuration",()=>{
  const states={unknown:"idle",checking:"checking",installed:"healthy",missing:"disabled",unsupported:"warning",broken:"failed"};
  for(const copy of Object.values(remoteBridgeMessages)) for(const [state,expected] of Object.entries(states)) {
    const value=summary(state,true);
    value.toolInstallations[0].extensionState="installed";
    value.toolInstallations[0].extensionVersion="26.825.2";
    const rows=toolInstallationRows(copy,value,"codex");
    assert.equal(rows.length,2);
    assert.equal(rows[0].state,expected);
    assert.equal(rows[1].state,"healthy");
    assert.equal(rows[1].label,copy.rbInstallExtension);
    assert.equal(rows[1].version,"26.825.2");
    assert.match(rows[1].status,/26\.825\.2/);
  }
  const value=summary("installed");
  delete value.toolInstallations[0].extensionState;
  const rows=toolInstallationRows(remoteBridgeMessages.en,value,"codex");
  assert.equal(rows[1].state,"idle");
  assert.equal(rows[1].status,remoteBridgeMessages.en.rbInstallUnknown);
});

test("CLI and extension versions occupy independent localized overview lines",()=>{
  const value=summary("installed");
  value.toolInstallations[0].extensionState="installed";
  value.toolInstallations[0].extensionVersion="26.825.2";
  for(const copy of Object.values(remoteBridgeMessages)) {
    const lines=toolInstallationLines(copy,value,"codex");
    assert.equal(lines.length,2);
    assert.match(lines[0],/^CLI/);
    assert.ok(lines[1].startsWith(copy.rbInstallExtension));
    assert.ok(lines.every(line=>!line.includes(" / ")));
    assert.match(lines[0],/0\.134\.0/);
    assert.match(lines[1],/26\.825\.2/);
  }
  delete value.toolInstallations[0].extensionState;
  assert.equal(toolInstallationLines(remoteBridgeMessages.en,value,"codex").length,1);
});

test("installation guidance never applies routes or executes commands and protects stale dialog responses",()=>{
  const source=readFileSync("src/features/remote-bridge/components/RemoteToolInstallDialog.vue","utf8");
  assert.doesNotMatch(source,/configApply|configPreview|toolVerify|invoke\(|execCommand|paste/);
  assert.match(source,/proxyOn.value \? remoteBackend.launchProxyTerminal\(\) : remoteBackend.launchManualTerminal\(\)/);
  assert.match(source,/remoteBackend.discoverTools\(targetId.value\)/);
  assert.match(source,/revision===generation/);
  assert.match(source,/clearTimeout\(timer\); timer=setTimeout[\s\S]*2000/);
  assert.match(source,/@cancel.prevent="close"/);
  assert.match(source,/@click="close">\{\{ copy.rbClose \}\}/);
  assert.doesNotMatch(source,/rbInstallLater/);
  assert.match(source,/previousFocus\?\.focus\(\)/);
});
