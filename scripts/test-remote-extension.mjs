import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { patch, jsonTree, checkClaudeShared } from './remote-extension/config.mjs';
import { transaction, hash, safe } from './remote-extension/files.mjs';

test('Codex preserves comments, unknown settings, secrets and quoted selector formatting', () => {
  const text = '# user comment\r\n"model_provider" = \'openai\' # keep\r\nmodel="custom"\r\n[mcp_servers.x]\r\ncommand="test"\r\n[model_providers.other]\r\nhttp_headers={Authorization="secret-fixture"}\r\n';
  const result = patch(text, 'codex', 25721);
  assert.ok(result.startsWith(text.replace("'openai'", '"proxyenv_bridge"')));
  assert.match(result, /base_url = "http:\/\/127.0.0.1:25721\/v1"/);
});
test('Codex never reuses existing provider names, legacy selectors or invalid TOML', () => {
  for (const text of ['profile="x"', '[model_providers.proxyenv_bridge]\nauth="secret"', 'model_provider="x"\nmodel_provider="y"', 'model_provider=1', 'a="broken']) {
    assert.throws(() => patch(text, 'codex', 25721));
  }
  assert.throws(() => patch('', 'codex', 0));
  assert.throws(() => patch('', 'anything', 25721));
});
test('Claude inserts only its two entries and preserves JSONC comments and unrelated credentials', () => {
  const text = '{ // before\r\n"claudeCode.environmentVariables": [/* keep */{"name":"OTHER_SECRET", "value":"secret-fixture"},],\r\n"other":{"unknown":true,},\r\n}';
  const result = patch(text, 'claude', 25721);
  assert.ok(result.includes('/* keep */{"name":"OTHER_SECRET", "value":"secret-fixture"},]'));
  assert.ok(result.endsWith('"other":{"unknown":true,},\r\n}'));
  assert.match(result, /http:\/\/127.0.0.1:25721"/);
  jsonTree(result);
  for (const empty of ['{}', '{"claudeCode.environmentVariables":[/* empty */]}']) jsonTree(patch(empty, 'claude', 25721));
});
test('Claude rejects duplicate keys, routing overrides and existing credentials without echoing values', () => {
  for (const text of ['{"a":1,"a":2}', '{"claudeCode.environmentVariables":{}}', '{"claudeCode.environmentVariables":[{"name":"ANTHROPIC_AUTH_TOKEN","value":"secret-fixture"}]}', '{"claudeCode.environmentVariables":[{"name":"CLAUDE_CODE_USE_BEDROCK","value":"1"}]}']) {
    assert.throws(() => patch(text, 'claude', 25721), e => e.message === 'configConflict');
  }
});
function fixture(tool) {
  const base = path.resolve('.debug-tmp'); fs.mkdirSync(base,{recursive:true});
  const root = fs.mkdtempSync(path.join(base, 'extension-test-'));
  const file = path.join(root, tool === 'codex' ? 'config.toml' : 'settings.json');
  const uid = fs.statSync(root).uid;
  const run = (operation, extra = {}, inject) => transaction({file,root,uid,tool,port:25721,contextHash:'context',operation,...extra},inject);
  return {root,file,run,uid,cleanup(){ assert.ok(root.startsWith(base+path.sep));fs.rmSync(root,{recursive:true,force:true}); }};
}
for (const tool of ['codex','claude']) {
  test(`${tool}: preview is read-only, apply/update/restore preserve original bytes`, () => {
    const f=fixture(tool);
    try {
      const original=tool==='codex'?'# exact\nmodel="mine"\n':'{/* exact */"other":"secret-fixture"}';
      fs.writeFileSync(f.file,original,{mode:0o600});
      let preview=f.run('preview'); assert.equal(fs.readdirSync(f.root).length,1);
      assert.ok(!JSON.stringify(preview).includes('secret-fixture'));
      assert.deepEqual(f.run('apply',preview),{configured:true});
      assert.throws(()=>f.run('apply',preview),/configConflict/);
      preview=f.run('preview'); f.run('apply',{...preview,port:25722});
      assert.match(fs.readFileSync(f.file,'utf8'),/25722/);
      const restore=f.run('restore-preview'); f.run('restore',restore);
      assert.equal(fs.readFileSync(f.file,'utf8'),original);
      assert.equal(fs.readdirSync(f.root).length,1);
    } finally {f.cleanup();}
  });
  test(`${tool}: absent file restores to absence and external edits block restore`, () => {
    const f=fixture(tool);
    try {
      f.run('apply',f.run('preview')); f.run('restore',f.run('restore-preview'));
      assert.equal(fs.existsSync(f.file),false);
      f.run('apply',f.run('preview'));
      fs.appendFileSync(f.file,'\n# external');
      assert.throws(()=>f.run('restore-preview'),/configConflict/);
      assert.ok(fs.readFileSync(f.file,'utf8').endsWith('# external'));
    } finally {f.cleanup();}
  });
  test(`${tool}: write failure rolls back file and journal; third-party races are retained`, () => {
    const f=fixture(tool);
    try {
      let preview=f.run('preview');
      assert.throws(()=>f.run('apply',preview,stage=>{if(stage==='replaced')throw Error('fault');}),/writeRolledBack/);
      assert.equal(fs.existsSync(f.file),false);
      assert.equal(fs.readdirSync(f.root).length,0);
      preview=f.run('preview');
      assert.throws(()=>f.run('apply',preview,stage=>{if(stage==='replaced'){fs.writeFileSync(f.file,'external');throw Error('fault');}}),/rollbackConflict/);
      assert.equal(fs.readFileSync(f.file,'utf8'),'external');
      assert.throws(()=>f.run('restore-preview'),/configConflict/);
    } finally {f.cleanup();}
  });
}
test('tampered backup and abandoned transaction lock fail closed', () => {
  const f=fixture('codex');
  try {
    fs.writeFileSync(f.file,'model="mine"',{mode:0o600});f.run('apply',f.run('preview'));
    const preview=f.run('restore-preview');fs.writeFileSync(f.file+'.proxyenv-extension-original','model="changed"');
    assert.throws(()=>f.run('restore',preview),/configConflict/);
  } finally {f.cleanup();}
  const g=fixture('codex');
  try {
    const preview=g.run('preview');fs.mkdirSync(g.file+'.proxyenv-extension-lock');
    assert.throws(()=>g.run('apply',preview),/configConflict/);assert.equal(fs.existsSync(g.file),false);
    assert.throws(()=>safe(path.resolve(g.root,'../outside'),g.root,g.uid),/unsafePath/);
  } finally {g.cleanup();}
});
test('hardlinks are refused', () => {
  const f=fixture('codex');
  try { fs.writeFileSync(f.file,'');fs.linkSync(f.file,f.file+'.link'); assert.throws(()=>f.run('preview'),/unsafePath/); }
  finally {f.cleanup();}
});
test('shared Claude configuration and SSH environment cannot silently override extension routing', () => {
  for (const text of ['{"env":{"ANTHROPIC_BASE_URL":"https://other"}}', '{"apiKeyHelper":"private-command"}', '{"env":{"ANTHROPIC_API_KEY":"secret"}}']) assert.throws(()=>checkClaudeShared(text),/configConflict/);
  assert.throws(()=>checkClaudeShared('{}',{CLAUDE_CODE_USE_BEDROCK:'1'}),/configConflict/);
  checkClaudeShared('{"permissions":{"allow":[]},"env":{"OTHER":"preserved"}}');
});
test('invalid UTF-8 and oversized configuration are rejected without mutation', () => {
  const f=fixture('codex');
  try {
    fs.writeFileSync(f.file, Buffer.from([0xff,0xfe]));
    assert.throws(()=>f.run('preview'),/configConflict/);
    fs.writeFileSync(f.file, Buffer.alloc(1024*1024+1));
    assert.throws(()=>f.run('preview'),/unsafePath/);
    assert.equal(fs.readdirSync(f.root).length,1);
  } finally {f.cleanup();}
});
test('Linux permissions and symlink checks use the actual filesystem', {skip:process.platform!=='linux'}, () => {
  const f=fixture('codex');
  try {
    fs.writeFileSync(f.file,'',{mode:0o600});fs.chmodSync(f.file,0o666);
    assert.throws(()=>f.run('preview'),/unsafePath/);
    fs.chmodSync(f.file,0o600);fs.renameSync(f.file,f.file+'.real');fs.symlinkSync(f.file+'.real',f.file);
    assert.throws(()=>f.run('preview'),/unsafePath/);
  } finally {f.cleanup();}
});
test('Linux bundled helper inspects separate extension runtimes and completes preview/apply/restore', {skip:process.platform!=='linux' || process.getuid?.()===0}, () => {
  const f=fixture('codex');
  try {
    for(const [id,version] of [['openai.chatgpt','26.825.51511'],['anthropic.claude-code','2.1.252']]) {
      const folder=path.join(f.root,'.vscode-server/extensions',id+'-'+version);
      fs.mkdirSync(folder,{recursive:true,mode:0o700});
      fs.writeFileSync(path.join(folder,'package.json'),JSON.stringify({publisher:id.split('.')[0],name:id.split('.')[1],version}),{mode:0o600});
      if(id==='openai.chatgpt') {
        const bin=path.join(folder,'bin',process.arch==='arm64'?'linux-aarch64':'linux-x86_64');fs.mkdirSync(bin,{recursive:true,mode:0o700});
        fs.writeFileSync(path.join(bin,'codex'),'#!/bin/sh\nprintf "codex-cli 0.151.0-alpha.7.2\\n"\n',{mode:0o700});
      }
    }
    const source=fs.readFileSync('src-tauri/src/features/remote_bridge/extension-helper.cjs','utf8');
    const run=request=>{
      const result=spawnSync(process.execPath,['-'],{input:`globalThis.bridgeExtensionRequest=${JSON.stringify(request)};\n${source}`,encoding:'utf8',timeout:10000,env:{HOME:f.root,PATH:'/usr/bin:/bin'}});
      assert.equal(result.status,0,result.stderr);
      return JSON.parse(result.stdout);
    };
    const inspection=run({operation:'inspect'});
    assert.equal(inspection.extensions.length,2);
    assert.equal(inspection.extensions[0].runtimeVersion,'0.151.0-alpha.7.2');
    for (const tool of ['codex','claude']) {
      const request={tool,port:25721,contextHash:inspection.contextHash};
      const preview=run({...request,operation:'preview'});assert.equal(preview.error,undefined);
      assert.equal(run({...request,...preview,operation:'apply'}).configured,true);
      const restore=run({...request,operation:'restore-preview'});
      assert.equal(run({...request,...restore,operation:'restore'}).configured,false);
    }
    assert.equal(run({operation:'preview',tool:'codex',contextHash:'wrong',port:25721}).error,'extensionContextChanged');
    const request={tool:'codex',port:25721,contextHash:inspection.contextHash};
    run({...request,...run({...request,operation:'preview'}),operation:'apply'});
    fs.renameSync(path.join(f.root,'.vscode-server'),path.join(f.root,'server-removed'));
    const removed=run({operation:'inspect'});
    assert.equal(removed.extensions[0].detected,false);
    const recovery={tool:'codex',port:25721,contextHash:removed.contextHash};
    assert.equal(run({...recovery,...run({...recovery,operation:'restore-preview'}),operation:'restore'}).configured,false);
  } finally {f.cleanup();}
});
