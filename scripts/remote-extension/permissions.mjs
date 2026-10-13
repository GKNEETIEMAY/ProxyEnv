import * as fs from 'node:fs';
import { execFileSync } from 'node:child_process';

// A matching uid/gid or group name alone does not prove a private group.
// Unknown/directory-backed account sources keep the existing strict policy.
export function privateGroupFromRecords(uid, gid, passwd, groups, nsswitch) {
  if (!Number.isInteger(uid) || uid <= 0 || !Number.isInteger(gid) || gid <= 0) return null;
  for (const database of ['passwd', 'group']) {
    const rows = nsswitch.split(/\r?\n/).map(line => line.split('#')[0].trim()).filter(line => line.startsWith(database + ':'));
    if (rows.length !== 1) return null;
    const sources = rows[0].split(':')[1].trim().split(/\s+/);
    if (!sources.includes('files') || sources.some(source => !['files', 'systemd'].includes(source))) return null;
  }
  const accounts = passwd.trim().split(/\r?\n/).map(line => line.split(':'));
  const entries = groups.trim().split(/\r?\n/).map(line => line.split(':'));
  if (accounts.some(row => row.length !== 7 || !/^\d+$/.test(row[2]) || !/^\d+$/.test(row[3])) ||
      entries.some(row => row.length !== 4 || !/^\d+$/.test(row[2]))) return null;
  const users = accounts.filter(row => Number(row[2]) === uid);
  const matching = entries.filter(row => Number(row[2]) === gid);
  if (users.length !== 1 || matching.length !== 1) return null;
  const user = users[0][0], group = matching[0];
  if (accounts.filter(row => row[0] === user).length !== 1 || entries.filter(row => row[0] === group[0]).length !== 1 ||
      Number(users[0][3]) !== gid || group[0] !== user ||
      accounts.some(row => Number(row[3]) === gid && Number(row[2]) !== uid) ||
      group[3].split(',').filter(Boolean).some(member => member !== user)) return null;
  return gid;
}

export function networkPrivateGroup(uid) {
  if (process.platform !== 'linux') return null;
  try {
    const nsswitch = fs.readFileSync('/etc/nsswitch.conf', 'utf8');
    const options = { encoding: 'utf8', timeout: 4000, maxBuffer: 1024 * 1024, stdio: ['ignore', 'pipe', 'ignore'] };
    return privateGroupFromRecords(uid, process.getgid(),
      execFileSync('/usr/bin/getent', ['passwd'], options),
      execFileSync('/usr/bin/getent', ['group'], options), nsswitch);
  } catch { return null; }
}

export function unsafeWriteMode(stat, privateGid = null, platform = process.platform) {
  return platform !== 'win32' && !!((stat.mode & 0o002) ||
    ((stat.mode & 0o020) && (privateGid === null || stat.gid !== privateGid)));
}
