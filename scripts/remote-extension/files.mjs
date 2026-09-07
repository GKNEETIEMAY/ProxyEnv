import * as fs from 'node:fs';
import path from 'node:path';
import { createHash, randomBytes } from 'node:crypto';
import { fail, patch } from './config.mjs';

export const digest = text => createHash('sha256').update(text).digest('hex');
export const hash = text => text === null ? 'absent' : digest(text);
export function safe(file, root, uid) {
  if (file !== root && !file.startsWith(root + path.sep)) fail('unsafePath');
  const relative = path.relative(root, file);
  let current = root;
  for (const part of ['', ...relative.split(path.sep).filter(Boolean)]) {
    if (part) current = path.join(current, part);
    let s;
    try { s = fs.lstatSync(current); } catch (e) { if (e.code === 'ENOENT') continue; throw e; }
    if (s.isSymbolicLink() || s.uid !== uid || (process.platform !== 'win32' && s.mode & 0o022) || (!s.isDirectory() && (!s.isFile() || s.nlink !== 1 || s.size > 1024 * 1024))) fail('unsafePath');
  }
}
export function read(file, root, uid) {
  safe(file, root, uid);
  try {
    const fd = fs.openSync(file, fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW || 0));
    try {
      const s = fs.fstatSync(fd);
      if (!s.isFile() || s.uid !== uid || s.nlink !== 1 || s.size > 1024 * 1024 || (process.platform !== 'win32' && s.mode & 0o022)) fail('unsafePath');
      // Bound the actual read as well as fstat: another editor may grow the file.
      const buffer = Buffer.alloc(1024 * 1024 + 1);
      let length = 0;
      while (length < buffer.length) {
        const count = fs.readSync(fd, buffer, length, buffer.length - length, null);
        if (!count) break;
        length += count;
      }
      if (length > 1024 * 1024) fail('unsafePath');
      const bytes = buffer.subarray(0, length);
      const text = bytes.toString('utf8');
      if (!Buffer.from(text).equals(bytes)) fail();
      return text;
    } finally { fs.closeSync(fd); }
  } catch(e) { if(e.code === 'ENOENT') return null; throw e; }
}
function atomic(file, text, root, uid) {
  safe(file, root, uid);
  if (text === null) { if (fs.existsSync(file)) fs.unlinkSync(file); return; }
  const temporary = file + '.tmp-' + randomBytes(12).toString('hex');
  const fd = fs.openSync(temporary, 'wx', 0o600);
  try { fs.writeFileSync(fd, text); fs.fsyncSync(fd); }
  finally { fs.closeSync(fd); }
  try { safe(file, root, uid); fs.renameSync(temporary, file); }
  finally { if (fs.existsSync(temporary)) fs.unlinkSync(temporary); }
  if (process.platform !== 'win32') {
    const directory = fs.openSync(path.dirname(file), 'r');
    try { fs.fsyncSync(directory); } finally { fs.closeSync(directory); }
  }
}
export function transaction({ file, root, uid, tool, port, operation, expectedHash, journalHash, contextHash }, inject = () => {}) {
  const backup = file + '.proxyenv-extension-original';
  const journal = file + '.proxyenv-extension-state';
  const lock = file + '.proxyenv-extension-lock';
  const load = () => {
    const current = read(file, root, uid), raw = read(journal, root, uid), original = read(backup, root, uid);
    let record;
    if (raw !== null) {
      try { record = JSON.parse(raw); } catch { fail(); }
      if (record.schema !== 1 || record.tool !== tool || record.state !== 'applied' ||
          record.appliedHash !== hash(current) || record.originalHash !== hash(original) ||
          !Number.isInteger(record.port) || record.port < 1024 || record.port > 65535) fail();
    } else if (original !== null) fail();
    return { current, raw, original, record };
  };
  const restoring = operation.startsWith('restore');
  function preview(data) {
    if (restoring && !data.record) fail('noBackup');
    if (!restoring) patch(data.record ? data.original || '' : data.current || '', tool, port);
    return { expectedHash: hash(data.current), journalHash: hash(data.raw), previousPort: data.record?.port ?? null, originalExists: data.record ? data.original !== null : data.current !== null };
  }
  const initial = load();
  if (operation === 'preview' || operation === 'restore-preview') return preview(initial);
  if (!['apply', 'restore'].includes(operation)) fail('invalidRequest');
  safe(path.dirname(file), root, uid);
  fs.mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 });
  safe(lock, root, uid);
  try { fs.mkdirSync(lock, { mode: 0o700 }); } catch { fail('configConflict'); }
  try {
    const data = load();
    preview(data);
    if (hash(data.current) !== expectedHash || hash(data.raw) !== journalHash) fail();
    const original = data.record ? data.original : data.current;
    const next = restoring ? original : patch(original || '', tool, port);
    const record = JSON.stringify({ schema: 1, tool, port, contextHash, state: 'prepared', originalHash: hash(original), appliedHash: hash(next) });
    try {
      if (!data.record && original !== null) atomic(backup, original, root, uid);
      atomic(journal, record, root, uid);
      inject('prepared');
      if (hash(read(file, root, uid)) !== expectedHash) fail();
      atomic(file, next, root, uid);
      inject('replaced');
      if (hash(read(file, root, uid)) !== hash(next)) fail('verifyFailed');
      if (restoring) {
        atomic(journal, null, root, uid);
        atomic(backup, null, root, uid);
      } else atomic(journal, JSON.stringify({ ...JSON.parse(record), state: 'applied' }), root, uid);
    } catch {
      const now = hash(read(file, root, uid));
      if (now !== expectedHash && now !== hash(next)) fail('rollbackConflict');
      try {
        atomic(file, data.current, root, uid);
        atomic(backup, data.original, root, uid);
        atomic(journal, data.raw, root, uid);
        if (hash(read(file, root, uid)) !== expectedHash) fail();
      } catch { fail('rollbackFailed'); }
      fail('writeRolledBack');
    }
    return { configured: !restoring };
  } finally { fs.rmdirSync(lock); }
}
