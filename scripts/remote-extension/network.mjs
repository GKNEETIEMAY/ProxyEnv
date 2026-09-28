// Session-scoped remote VS Code settings. Never writes the local user's settings.
import * as fs from 'node:fs';
import path from 'node:path';
import { getNodeValue, modify, applyEdits } from 'jsonc-parser';
import { fail, jsonTree } from './config.mjs';
import { safe, read, atomic, hash } from './files.mjs';

const keys = ['http.useLocalProxyConfiguration', 'http.proxy', 'http.proxyAuthorization', 'http.noProxy'];
// Network settings do not depend on an AI extension or its installed versions.
export function networkSettingsPath(root, uid, agentFolder) {
  const roots = agentFolder ? [agentFolder] : ['.vscode-server', '.vscode-server-insiders', '.vscode-remote'].map(name => path.join(root, name));
  if (agentFolder && (!path.isAbsolute(agentFolder) || agentFolder === root || !agentFolder.startsWith(root + path.sep))) fail('customHome');
  const existing = roots.filter(candidate => fs.existsSync(candidate));
  if (!existing.length) fail('vscodeServerMissing');
  if (existing.length !== 1) fail('vscodeServerAmbiguous');
  const selected = existing[0];
  safe(selected, root, uid);
  if (!fs.lstatSync(selected).isDirectory()) fail('unsafePath');
  return path.join(selected, 'data/Machine/settings.json');
}
const own = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
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
function validSnapshot(value) {
  return value && Object.keys(value).length === keys.length && keys.every(key =>
    own(value, key) && value[key] && typeof value[key] === 'object' && !Array.isArray(value[key]) &&
    Object.keys(value[key]).every(name => name === 'value'));
}

export function networkTransaction({ file, root, uid, operation, sessionId }, inject = () => {}) {
  if (!['network-apply', 'network-restore'].includes(operation) || !/^[a-f0-9]{32}$/.test(sessionId || '')) fail('invalidRequest');
  const restoring = operation === 'network-restore';
  const journal = file + '.proxyenv-network-state';
  const lock = file + '.proxyenv-network-lock';
  safe(path.dirname(file), root, uid);
  if (!fs.existsSync(path.dirname(file))) {
    if (restoring) return { restored: true };
    fs.mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 });
  }
  safe(lock, root, uid);
  try { fs.mkdirSync(lock, { mode: 0o700 }); } catch { fail('configConflict'); }
  try {
    const current = read(file, root, uid), raw = read(journal, root, uid);
    const before = snapshot(current);
    let record = null;
    if (raw !== null) {
      try { record = JSON.parse(raw); } catch { fail(); }
      if (record.schema !== 1 || !['prepared', 'applied'].includes(record.state) ||
          !validSnapshot(record.original) || !validSnapshot(record.applied) || !validSnapshot(record.before) ||
          typeof record.originalAbsent !== 'boolean' || typeof record.preserveFile !== 'boolean' ||
          !/^(absent|[a-f0-9]{64})$/.test(record.appliedFileHash || '')) fail();
      // Do not steal another bridge session or overwrite a user's proxy edits.
      if (record.sessionId !== sessionId) fail('extensionContextChanged');
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
        // Never route the extension host through the authenticated General Proxy.
        // Chromium and extension processes do not consume one shared credential
        // source reliably, which otherwise causes login prompts and 407 loops.
        // Codex and Claude use the separately authenticated AI Route directly.
        'http.proxy': {},
        'http.proxyAuthorization': {},
        'http.noProxy': { value: [...new Set([...bypass, 'localhost', '127.0.0.1', '::1'])] },
      };
    }
    let next = patch(current, applied);
    if (restoring && originalAbsent && !preserveFile && Object.keys(getNodeValue(jsonTree(next))).length === 0) next = null;
    const pending = { schema: 1, state: 'prepared', sessionId, original, originalAbsent, preserveFile, before, applied, appliedFileHash: hash(next) };
    atomic(journal, JSON.stringify(pending), root, uid);
    try {
      inject('prepared');
      if (read(file, root, uid) !== current) fail();
      atomic(file, next, root, uid);
      inject('replaced');
      if (read(file, root, uid) !== next) fail('verifyFailed');
      atomic(journal, restoring ? null : JSON.stringify({ ...pending, state: 'applied' }), root, uid);
    } catch {
      const now = read(file, root, uid);
      if (now !== current && now !== next) fail('rollbackConflict');
      atomic(file, current, root, uid);
      atomic(journal, raw, root, uid);
      fail('writeRolledBack');
    }
    // Never serialize credentials, configuration values or backups to stdout.
    return restoring ? { restored: true } : { configured: true };
  } finally { fs.rmdirSync(lock); }
}
