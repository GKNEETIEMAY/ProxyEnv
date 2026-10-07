import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { getNodeValue } from 'jsonc-parser';
import { jsonTree } from './remote-extension/config.mjs';
import { networkTransaction, networkSettingsPath } from './remote-extension/network.mjs';

test('network setup does not inspect extension installations or stale server version directories', t => {
  const f = fixture(t);
  const server = path.join(path.dirname(f.file), '.vscode-server');
  fs.mkdirSync(server, {mode:0o700});
  // Deliberately invalid version-scanning entries: irrelevant to Machine settings.
  fs.writeFileSync(path.join(server, 'bin'), 'not a directory');
  fs.writeFileSync(path.join(server, 'extensions'), 'not a directory');
  assert.equal(networkSettingsPath(path.dirname(f.file), fs.statSync(server).uid), path.join(server, 'data/Machine/settings.json'));
});

test('network setup distinguishes absent and ambiguous servers and honors explicit agent root', t => {
  const f = fixture(t), root = path.dirname(f.file), uid = fs.statSync(root).uid;
  assert.throws(() => networkSettingsPath(root, uid), /vscodeServerMissing/);
  const stable = path.join(root, '.vscode-server');
  fs.mkdirSync(stable, {mode:0o700});
  fs.mkdirSync(path.join(root, '.vscode-server-insiders'), {mode:0o700});
  assert.throws(() => networkSettingsPath(root, uid), /vscodeServerAmbiguous/);
  assert.equal(networkSettingsPath(root, uid, stable), path.join(stable, 'data/Machine/settings.json'));
  assert.throws(() => networkSettingsPath(root, uid, path.dirname(root)), /customHome/);
});

const sessionId = 'b'.repeat(32);

test('new VS Code terminals inherit the actual bridge port and restore original environment', t => {
  const initial = { 'terminal.integrated.env.linux': { CUSTOM: 'keep', HTTP_PROXY: 'http://old:8080', NO_PROXY: 'lab.test' } };
  const f = fixture(t, JSON.stringify(initial));
  f.apply({ proxyPort: 17897, proxyProtocol: 'mixed' });
  const env = f.read()['terminal.integrated.env.linux'];
  assert.equal(env.CUSTOM, 'keep');
  assert.equal(env.HTTP_PROXY, 'http://127.0.0.1:17897');
  assert.equal(env.http_proxy, env.HTTP_PROXY);
  assert.equal(env.HTTPS_PROXY, env.HTTP_PROXY);
  assert.equal(env.ALL_PROXY, 'socks5h://127.0.0.1:17897');
  assert.equal(env.all_proxy, env.ALL_PROXY);
  assert.equal(env.NO_PROXY, 'localhost,127.0.0.1,::1,lab.test');
  assert.equal(env.no_proxy, env.NO_PROXY);
  f.apply({ proxyPort: 17898, proxyProtocol: 'http' });
  assert.equal(f.read()['terminal.integrated.env.linux'].HTTP_PROXY, 'http://127.0.0.1:17898');
  assert.equal(f.read()['terminal.integrated.env.linux'].ALL_PROXY, null);
  f.restore();
  assert.deepEqual(JSON.parse(fs.readFileSync(f.file, 'utf8')), initial);
});

test('terminal bridge supports SOCKS and never replaces concurrent user environment edits', t => {
  const f = fixture(t);
  f.apply({ proxyPort: 10809, proxyProtocol: 'socks5' });
  const edited = f.read();
  assert.equal(edited['terminal.integrated.env.linux'].HTTP_PROXY, null);
  assert.equal(edited['terminal.integrated.env.linux'].ALL_PROXY, 'socks5h://127.0.0.1:10809');
  edited['terminal.integrated.env.linux'].CUSTOM = 'user changed';
  fs.writeFileSync(f.file, JSON.stringify(edited));
  assert.throws(() => f.restore(), /configConflict/);
  assert.deepEqual(f.read(), edited);
});

test('remote network accepts reordered settings and journals without treating formatting as a conflict', t => {
  const initial = { 'terminal.integrated.env.linux': { CUSTOM: 'keep', NO_PROXY: 'lab.test' } };
  const f = fixture(t, JSON.stringify(initial));
  const reorder = value => {
    if (Array.isArray(value)) return value.map(reorder);
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).reverse().map(([key, item]) => [key, reorder(item)]));
    return value;
  };
  const reformat = () => {
    fs.writeFileSync(f.file, JSON.stringify(reorder(f.read()), null, 2));
    fs.writeFileSync(f.journal, JSON.stringify(reorder(JSON.parse(fs.readFileSync(f.journal, 'utf8'))), null, 2));
  };
  f.apply({ proxyPort: 7897, proxyProtocol: 'mixed' });
  reformat();
  assert.deepEqual(f.apply({ proxyPort: 17897, proxyProtocol: 'http' }), { configured: true });
  assert.equal(f.read()['terminal.integrated.env.linux'].HTTP_PROXY, 'http://127.0.0.1:17897');
  reformat();
  assert.deepEqual(f.restore(), { restored: true });
  assert.deepEqual(JSON.parse(fs.readFileSync(f.file, 'utf8')), initial);
});

test('reordered snapshots still reject changed managed values, arrays, and absent settings', t => {
  for (const change of [
    values => { values['terminal.integrated.env.linux'].HTTP_PROXY = 'http://user:9000'; },
    values => { values['http.noProxy'].reverse(); },
    values => { values['http.proxy'] = null; },
  ]) {
    const f = fixture(t);
    f.apply({ proxyPort: 7897, proxyProtocol: 'mixed' });
    const edited = f.read();
    edited['terminal.integrated.env.linux'] = Object.fromEntries(Object.entries(edited['terminal.integrated.env.linux']).reverse());
    change(edited);
    fs.writeFileSync(f.file, JSON.stringify(edited));
    const current = fs.readFileSync(f.file, 'utf8'), journal = fs.readFileSync(f.journal, 'utf8');
    assert.throws(() => f.apply({ proxyPort: 17897, proxyProtocol: 'http' }), /configConflict/);
    assert.throws(() => f.restore(), /configConflict/);
    assert.equal(fs.readFileSync(f.file, 'utf8'), current);
    assert.equal(fs.readFileSync(f.journal, 'utf8'), journal);
  }
});

test('legacy network journals preserve previously unmanaged terminal settings', t => {
  const f = fixture(t, '{}');
  f.apply();
  const record = JSON.parse(fs.readFileSync(f.journal, 'utf8'));
  record.schema = 1;
  for (const values of [record.original, record.applied, record.before]) delete values['terminal.integrated.env.linux'];
  fs.writeFileSync(f.journal, JSON.stringify(record));
  const edited = f.read();
  edited['terminal.integrated.env.linux'] = { CUSTOM: 'not previously managed' };
  fs.writeFileSync(f.file, JSON.stringify(edited));
  f.apply({ proxyPort: 7897, proxyProtocol: 'http' });
  f.restore();
  assert.deepEqual(JSON.parse(fs.readFileSync(f.file, 'utf8')), { 'terminal.integrated.env.linux': { CUSTOM: 'not previously managed' } });
});

test('invalid terminal bridge endpoints fail before any write', t => {
  const f = fixture(t, '{}');
  for (const extra of [{proxyPort:80,proxyProtocol:'http'}, {proxyPort:7897,proxyProtocol:'unknown'}, {proxyPort:7897}, {proxyProtocol:'http'}]) {
    assert.throws(() => f.apply(extra), /invalidRequest/);
    assert.equal(fs.readFileSync(f.file, 'utf8'), '{}');
    assert.equal(fs.existsSync(f.journal), false);
  }
});

test('unchanged VS Code setup is read-only while a changed bridge port updates its transaction', t => {
  const f = fixture(t, '{ // user comment\n }');
  f.apply({ proxyPort: 7897, proxyProtocol: 'mixed' });
  const current = fs.readFileSync(f.file, 'utf8'), journal = fs.readFileSync(f.journal, 'utf8');
  const phases = [];
  assert.deepEqual(f.apply({ proxyPort: 7897, proxyProtocol: 'mixed' }, phase => phases.push(phase)), { configured: true });
  assert.deepEqual(phases, []);
  assert.equal(fs.readFileSync(f.file, 'utf8'), current);
  assert.equal(fs.readFileSync(f.journal, 'utf8'), journal);
  f.apply({ proxyPort: 17897, proxyProtocol: 'http' }, phase => phases.push(phase));
  assert.deepEqual(phases, ['prepared', 'replaced']);
  assert.equal(f.read()['terminal.integrated.env.linux'].HTTP_PROXY, 'http://127.0.0.1:17897');
  f.restore();
  assert.deepEqual(JSON.parse(fs.readFileSync(f.file, 'utf8').replace('// user comment', '')), {});
});

test('runtime launcher resolves installed executable paths and rejects unsafe or old runtimes', t => {
  const git = process.platform === 'win32' ? spawnSync('where.exe', ['git'], {encoding:'utf8'}).stdout?.trim().split(/\r?\n/)[0] : null;
  const shell = git ? path.resolve(path.dirname(git), '../bin/bash.exe') : '/bin/sh';
  if (!fs.existsSync(shell)) return t.skip('POSIX shell unavailable');
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'proxyenv-runtime-'));
  t.after(() => fs.rmSync(directory, {recursive:true, force:true}));
  const executable = path.join(directory, 'node');
  fs.writeFileSync(executable, '#!/bin/sh\nprintf "%s\\n" "${TEST_NODE_VERSION:-v22.0.0}"\n', {mode:0o700});
  const posix = executable.replaceAll('\\', '/').replace(/^([A-Za-z]):/, (_, drive) => `/${drive.toLowerCase()}`);
  const source = fs.readFileSync('src-tauri/src/features/remote_bridge/extension-launch.sh', 'utf8');
  assert.match(source, /command -v node/);
  assert.match(source, /\.nvm\/versions\/node\/\*\/bin\/node/);
  // Mock only POSIX ownership/modes on Windows; run the real selection and
  // canonicalization control flow and a real executable with a version result.
  const script = `stat() { case "$2" in '%a') printf '%s' "${'$'}{TEST_NODE_MODE:-755}";; '%u') printf '%s' "${'$'}{TEST_NODE_OWNER:-0}";; '%g') printf '%s' "${'$'}{TEST_NODE_GROUP:-0}";; esac; }\n` +
    `id() { case "$1" in -u) printf '%s' "${'$'}{TEST_UID:-1000}";; -g) printf '%s' "${'$'}{TEST_GID:-1000}";; -un) printf '%s' "${'$'}{TEST_USER:-user}";; -gn) printf '%s' "${'$'}{TEST_GROUP_NAME:-user}";; esac; }\n` +
    source.replace(/^for candidate in .*; do$/m, `for candidate in '${posix}'; do`) + '\nprintf "runtime-ready\\n"\n';
  const run = env => spawnSync(shell, ['-s'], {input:script, encoding:'utf8', timeout:10000, env:{...process.env,...env}});
  assert.match(run({}).stdout, /runtime-ready/);
  assert.match(run({TEST_NODE_MODE:'775',TEST_NODE_OWNER:'1000',TEST_NODE_GROUP:'1000'}).stdout, /runtime-ready/);
  assert.match(run({TEST_NODE_MODE:'775',TEST_NODE_OWNER:'1000',TEST_NODE_GROUP:'2000',TEST_GROUP_NAME:'lab'}).stdout, /remoteNodeUnsafe/);
  assert.match(run({TEST_NODE_MODE:'777'}).stdout, /remoteNodeUnsafe/);
  assert.match(run({TEST_NODE_VERSION:'v18.0.0'}).stdout, /remoteNodeUnsupported/);
});
function fixture(t, initial = null) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'proxyenv-network-'));
  const file = path.join(root, 'settings.json');
  if (initial !== null) fs.writeFileSync(file, initial, { mode: 0o600 });
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const options = { root, file, uid: fs.statSync(root).uid, sessionId };
  return {
    file, journal: file + '.proxyenv-network-state',
    apply: (extra = {}, inject) => networkTransaction({ ...options, operation: 'network-apply', ...extra }, inject),
    restore: (extra = {}, inject) => networkTransaction({ ...options, operation: 'network-restore', ...extra }, inject),
    read: () => ({ ...getNodeValue(jsonTree(fs.readFileSync(file, 'utf8'))) }),
  };
}
test('remote network isolates VS Code from inherited proxies and bypasses AI loopback', t => {
  const f = fixture(t, '{ // preserved comment\n "editor.fontSize":14, "http.proxy":"http://old.example:8080", "http.noProxy":["*.lab.test"], "http.proxyStrictSSL":true, }');
  assert.deepEqual(f.apply(), { configured: true });
  const current = f.read();
  assert.equal(Object.hasOwn(current, 'http.proxy'), false);
  assert.equal(Object.hasOwn(current, 'http.proxyAuthorization'), false);
  assert.equal(current['http.useLocalProxyConfiguration'], false);
  assert.deepEqual(current['http.noProxy'], ['*.lab.test', 'localhost', '127.0.0.1', '::1']);
  assert.equal(current['http.proxyStrictSSL'], true);
  assert.equal(current['editor.fontSize'], 14);
  assert.match(fs.readFileSync(f.file, 'utf8'), /preserved comment/);
  if (process.platform !== 'win32') {
    assert.equal(fs.statSync(f.file).mode & 0o777, 0o600);
    assert.equal(fs.statSync(f.journal).mode & 0o777, 0o600);
  }
});
test('remote network updates and restores its keys without removing unrelated edits', t => {
  const f = fixture(t, '{"http.proxy":"http://original:8080","http.proxyAuthorization":null,"http.noProxy":["lab.test"],"theme":"dark"}');
  f.apply();
  const edited = f.read(); edited.theme = 'light'; edited['new.setting'] = true;
  fs.writeFileSync(f.file, JSON.stringify(edited));
  f.apply();
  assert.equal(Object.hasOwn(f.read(), 'http.proxy'), false);
  assert.deepEqual(f.restore(), { restored: true });
  assert.deepEqual(f.read(), { 'http.proxy':'http://original:8080','http.proxyAuthorization':null,'http.noProxy':['lab.test'],theme:'light','new.setting':true });
  assert.equal(fs.existsSync(f.journal), false);
  assert.deepEqual(f.restore(), { restored: true });
});
test('remote network rejects concurrent sessions and never overwrites user proxy changes', t => {
  const f = fixture(t); f.apply();
  assert.throws(() => f.apply({sessionId:'c'.repeat(32)}), /extensionContextChanged/);
  assert.throws(() => f.restore({sessionId:'c'.repeat(32)}), /extensionContextChanged/);
  const edited = f.read(); edited['http.proxy'] = 'http://user:9000';
  fs.writeFileSync(f.file, JSON.stringify(edited));
  assert.throws(() => f.apply(), /configConflict/);
  assert.throws(() => f.restore(), /configConflict/);
  assert.equal(f.read()['http.proxy'], 'http://user:9000');
});
test('remote network rejects malformed settings and invalid ownership without writing', t => {
  const f = fixture(t, '{"http.proxy":"a","http.proxy":"b"}');
  assert.throws(() => f.apply(), /configConflict/);
  assert.throws(() => f.apply({sessionId:'not-a-session'}), /invalidRequest/);
  assert.equal(fs.existsSync(f.journal), false);
});
test('remote network removes only its newly created empty settings file', t => {
  const f = fixture(t); f.apply(); f.restore();
  assert.equal(fs.existsSync(f.file), false);
  f.apply();
  const edited = f.read(); edited['editor.fontSize'] = 15;
  fs.writeFileSync(f.file, JSON.stringify(edited));
  f.restore(); assert.deepEqual(f.read(), { 'editor.fontSize':15 });
});
test('remote network rolls back a failed write and preserves the original settings', t => {
  const initial = '{"theme":"dark"}';
  const f = fixture(t, initial);
  for (const stage of ['prepared', 'replaced']) {
    assert.throws(() => f.apply({}, phase => { if (phase === stage) throw Error('fault'); }), /writeRolledBack/);
    assert.equal(fs.readFileSync(f.file, 'utf8'), initial);
    assert.equal(fs.existsSync(f.journal), false);
  }
});

test('remote network preserves user comments even when no unrelated settings remain', t => {
  const f = fixture(t); f.apply();
  fs.writeFileSync(f.file, '// user note\n' + fs.readFileSync(f.file, 'utf8'));
  f.apply(); f.restore();
  assert.match(fs.readFileSync(f.file, 'utf8'), /user note/);
  assert.deepEqual(f.read(), {});
});
test('remote network never rolls back over a concurrent file modification', t => {
  const f = fixture(t, '{}');
  assert.throws(() => f.apply({}, phase => {
    if (phase === 'prepared') fs.writeFileSync(f.file, '{"concurrent":true}');
  }), /rollbackConflict/);
  assert.deepEqual(f.read(), {concurrent:true});
});

test('remote network recovers a prepared journal without losing the original values', t => {
  const f = fixture(t, '{"http.proxy":"http://original:8080"}');
  f.apply();
  const record = JSON.parse(fs.readFileSync(f.journal, 'utf8'));
  record.state = 'prepared';
  fs.writeFileSync(f.journal, JSON.stringify(record));
  f.restore();
  assert.deepEqual(f.read(), { 'http.proxy':'http://original:8080' });
});

test('remote network restoration precedes a new session and never stores proxy credentials', t => {
  const f = fixture(t, '{"http.proxy":"http://original:8080"}');
  f.apply(); f.restore();
  const result = f.apply({sessionId:'c'.repeat(32)});
  assert.deepEqual(result, {configured:true});
  assert.equal(Object.hasOwn(f.read(), 'http.proxy'), false);
  const journal = fs.readFileSync(f.journal, 'utf8');
  assert.doesNotMatch(journal, /sessionToken|Basic |proxyenv:/i);
  assert.throws(() => f.restore(), /extensionContextChanged/);
  f.restore({sessionId:'c'.repeat(32)});
  assert.deepEqual(f.read(), { 'http.proxy':'http://original:8080' });
});

test('general proxy credentials are absent and VS Code uses managed setup', () => {
  const page = fs.readFileSync('src/features/remote-bridge/components/RemoteBridgePage.vue', 'utf8');
  assert.doesNotMatch(page, /remote-auth-fallback|copyProxyPassword|rbProxyAuth/);
  const commands = fs.readFileSync('src-tauri/src/commands/remote_bridge.rs', 'utf8');
  assert.doesNotMatch(commands, /remote_bridge_proxy_password|proxyCredentialCopy/);
  assert.match(commands, /bridge::open_vscode\(target_id\)/);
  const bridge = fs.readFileSync('src-tauri/src/features/remote_bridge/mod.rs', 'utf8');
  assert.match(bridge, /vscode_network::apply\(/);
  assert.match(bridge, /ai_relay[\s\S]*\.or\(state\.proxy_relay\.as_ref\(\)\)/);
  assert.match(bridge, /state\.proxy_relay = None;\s*state\.ai_relay = None;\s*let vscode_restore_error = restore_vscode_network/);
  const network = fs.readFileSync('src-tauri/src/features/remote_bridge/vscode_network.rs', 'utf8');
  assert.doesNotMatch(network, /sessionToken|proxyAuthorization|relay\.token/);
});
