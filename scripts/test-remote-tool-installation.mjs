import assert from "node:assert/strict";
import test from "node:test";
import {readFileSync} from "node:fs";
import {adapterModuleUrl,presentationModuleUrl,guidanceModuleUrl,copyModuleUrl} from "./remote-tool-test-modules.mjs";
const {canToggleRemoteTool,getRemoteToolAdapter}=await import(adapterModuleUrl);
const {overviewToolLabel,overviewToolChecking,toolInstallationLabel,toolInstallationLines,toolInstallationRows}=await import(presentationModuleUrl);
const {installMethods,installRequirementAvailable,installMissingRequirements}=await import(guidanceModuleUrl);
const {remoteBridgeMessages}=await import(copyModuleUrl);
const summary=(state,configured=false) => ({status:"connected",cc:{remotePort:15721},ccStatus:"connected",codexState:"ready",claudeState:"ready",codexConfigured:configured,claudeConfigured:false,toolInstallations:[{tool:"codex",state,version:state==="unsupported"?"0.133.9":state==="installed"?"0.134.0":null},{tool:"claude",state:"installed",version:"2.1.227"}]});
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
