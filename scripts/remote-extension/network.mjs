// Session-scoped remote VS Code settings. Never writes the local user's settings.
import * as fs from 'node:fs';
import path from 'node:path';
import { getNodeValue, modify, applyEdits } from 'jsonc-parser';
import { fail, jsonTree } from './config.mjs';
import { safe, read, atomic, hash } from './files.mjs';

const legacyKeys = ['http.useLocalProxyConfiguration', 'http.proxy', 'http.proxyAuthorization', 'http.noProxy'];
const terminalKey = 'terminal.integrated.env.linux';
const keys = [...legacyKeys, terminalKey];
// Network settings do not depend on an AI extension or its installed versions.
export function networkSettingsPath(root, uid, agentFolder, privateGid = null) {
  const roots = agentFolder ? [agentFolder] : ['.vscode-server', '.vscode-server-insiders', '.vscode-remote'].map(name => path.join(root, name));
  if (agentFolder && (!path.isAbsolute(agentFolder) || agentFolder === root || !agentFolder.startsWith(root + path.sep))) fail('customHome');
  const existing = roots.filter(candidate => fs.existsSync(candidate));
  if (!existing.length) fail('vscodeServerMissing');
  if (existing.length !== 1) fail('vscodeServerAmbiguous');
  const selected = existing[0];
  safe(selected, root, uid, privateGid);
  if (!fs.lstatSync(selected).isDirectory()) fail('unsafePath');
  return path.join(selected, 'data/Machine/settings.json');
}
const own = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
// JSON object order is not a configuration change. Array order and the
// distinction between absent settings and explicit null values still matter.
function equal(a, b) {
  if (a === b) return true;
  if (!a || !b || typeof a !== 'object' || typeof b !== 'object' || Array.isArray(a) !== Array.isArray(b)) return false;
  const names = Object.keys(a);
  return names.length === Object.keys(b).length && names.every(name => own(b, name) && equal(a[name], b[name]));
}
function snapshot(text) {
  const values = getNodeValue(jsonTree(text || '{}'));
  return Object.fromEntries(keys.map(key => [key, own(values, key) ? { value: values[key] } : {}]));
}
function patch(text, values) {
  let result = text || '{}';
  for (const key of keys) result = applyEdits(result, modify(result, [key], values[key].value, {}));
  jsonTree(result);
  return result;
}
function validSnapshot(value, expectedKeys = keys) {
  return value && Object.keys(value).length === expectedKeys.length && expectedKeys.every(key =>
    own(value, key) && value[key] && typeof value[key] === 'object' && !Array.isArray(value[key]) &&
    Object.keys(value[key]).every(name => name === 'value'));
}

function terminalEnvironment(original, proxyPort, proxyProtocol) {
  if (proxyPort == null && proxyProtocol == null) return original;
  if (!Number.isInteger(proxyPort) || proxyPort < 1024 || proxyPort > 65535 || !['http', 'mixed', 'socks5'].includes(proxyProtocol)) fail('invalidRequest');
  const previous = original.value ?? {};
  if (!previous || typeof previous !== 'object' || Array.isArray(previous) || Object.values(previous).some(value => value !== null && typeof value !== 'string')) fail();
  const env = { ...previous };
  const http = proxyProtocol === 'socks5' ? null : `http://127.0.0.1:${proxyPort}`;
  const socks = proxyProtocol === 'http' ? null : `socks5h://127.0.0.1:${proxyPort}`;
  for (const [upper, value] of [['HTTP_PROXY', http], ['HTTPS_PROXY', http], ['ALL_PROXY', socks]]) {
    env[upper] = value;
    env[upper.toLowerCase()] = value;
  }
  const bypass = ['localhost', '127.0.0.1', '::1', ...(previous.NO_PROXY || '').split(','), ...(previous.no_proxy || '').split(',')];
  env.NO_PROXY = [...new Set(bypass.map(value => value.trim()).filter(Boolean))].join(',');
  env.no_proxy = env.NO_PROXY;
  return { value: env };
}

export function networkTransaction({ file, root, uid, operation, sessionId, proxyPort, proxyProtocol }, inject = () => {}, privateGid = null) {
  // This policy is computed by the helper, never accepted from an IPC request.
  const readNetwork = file => read(file, root, uid, privateGid);
  const writeNetwork = (file, text) => atomic(file, text, root, uid, privateGid);
  if (!['network-apply', 'network-restore'].includes(operation) || !/^[a-f0-9]{32}$/.test(sessionId || '')) fail('invalidRequest');
  const restoring = operation === 'network-restore';
  if (!restoring) terminalEnvironment({}, proxyPort, proxyProtocol);
  const journal = file + '.proxyenv-network-state';
  const lock = file + '.proxyenv-network-lock';
  safe(path.dirname(file), root, uid, privateGid);
  if (!fs.existsSync(path.dirname(file))) {
    if (restoring) return { restored: true };
    fs.mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 });
  }
  safe(lock, root, uid, privateGid);
  try { fs.mkdirSync(lock, { mode: 0o700 }); } catch (error) {
    fail(error.code === 'EEXIST' ? 'vscodeNetworkBusy' : 'unsafePath');
  }
  try {
    const current = readNetwork(file), raw = readNetwork(journal);
    const before = snapshot(current);
    let record = null;
    if (raw !== null) {
      try { record = JSON.parse(raw); } catch { fail(); }
      const expectedKeys = record.schema === 1 ? legacyKeys : keys;
      if (![1, 2].includes(record.schema) || !['prepared', 'applied'].includes(record.state) ||
          !validSnapshot(record.original, expectedKeys) || !validSnapshot(record.applied, expectedKeys) || !validSnapshot(record.before, expectedKeys) ||
          typeof record.originalAbsent !== 'boolean' || typeof record.preserveFile !== 'boolean' ||
          !/^(absent|[a-f0-9]{64})$/.test(record.appliedFileHash || '')) fail();
      // Do not steal another bridge session or overwrite a user's proxy edits.
      if (record.sessionId !== sessionId) fail('extensionContextChanged');
      if (record.schema === 1) {
        // Previous releases did not own terminal settings. Preserve their current
        // value as the baseline, including during legacy restoration.
        for (const values of [record.original, record.applied, record.before]) values[terminalKey] = before[terminalKey];
      }
      if (!equal(before, record.applied) && !(record.state === 'prepared' && equal(before, record.before))) fail();
    }
    if (restoring && !record) return { restored: true };
    const original = record?.original || before;
    const originalAbsent = record ? record.originalAbsent : current === null;
    const preserveFile = !!record && (record.preserveFile || hash(current) !== record.appliedFileHash);
    let applied = original;
    if (!restoring) {
      const bypass = original['http.noProxy'].value ?? [];
      if (!Array.isArray(bypass) || bypass.some(value => typeof value !== 'string')) fail();
      applied = {
        'http.useLocalProxyConfiguration': { value: false },
        // Never route the extension host through the General Proxy. Keeping
        // loopback direct avoids stale inherited settings and proxy-dialog loops.
        // Codex and Claude use the separately authenticated AI Route directly.
        'http.proxy': {},
        'http.proxyAuthorization': {},
        'http.noProxy': { value: [...new Set([...bypass, 'localhost', '127.0.0.1', '::1'])] },
        [terminalKey]: terminalEnvironment(original[terminalKey], proxyPort, proxyProtocol),
      };
    }
    // Reopening VS Code or refreshing the same bridge is a read-only check.
    // A prepared transaction still needs recovery; endpoint changes still write.
    if (!restoring && record?.schema === 2 && record.state === 'applied' && equal(before, applied)) return { configured: true };
    let next = patch(current, applied);
    if (restoring && originalAbsent && !preserveFile && Object.keys(getNodeValue(jsonTree(next))).length === 0) next = null;
    const pending = { schema: 2, state: 'prepared', sessionId, original, originalAbsent, preserveFile, before, applied, appliedFileHash: hash(next) };
    writeNetwork(journal, JSON.stringify(pending));
    try {
      inject('prepared');
      if (readNetwork(file) !== current) fail();
      writeNetwork(file, next);
      inject('replaced');
      if (readNetwork(file) !== next) fail('verifyFailed');
      writeNetwork(journal, restoring ? null : JSON.stringify({ ...pending, state: 'applied' }));
    } catch {
      const now = readNetwork(file);
      if (now !== current && now !== next) fail('rollbackConflict');
      writeNetwork(file, current);
      writeNetwork(journal, raw);
      fail('writeRolledBack');
    }
    // Never serialize credentials, configuration values or backups to stdout.
    return restoring ? { restored: true } : { configured: true };
  } finally { fs.rmdirSync(lock); }
}
