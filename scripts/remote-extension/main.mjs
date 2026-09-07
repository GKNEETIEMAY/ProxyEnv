import * as fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { execFileSync } from 'node:child_process';
import { fail, checkClaudeShared } from './config.mjs';
import { safe, read, hash, digest, transaction } from './files.mjs';

const codes = new Set(['configConflict','unsafePath','invalidPort','invalidRequest','noBackup','verifyFailed','rollbackConflict','rollbackFailed','writeRolledBack','extensionMissing','extensionUnsupported','extensionContextChanged','customHome','remoteUnsupported']);
const version = value => typeof value === 'string' && /^\d+(?:\.\d+){1,3}(?:-[a-zA-Z0-9.-]+)?$/.test(value) && value.length < 48 ? value : '';
function run(request) {
  if (process.platform !== 'linux' || process.getuid() === 0 || Number(process.versions.node.split('.')[0]) < 20) fail('remoteUnsupported');
  const root = os.homedir(), uid = process.getuid();
  safe(root, root, uid);
  const server = path.join(root, '.vscode-server');
  safe(server, root, uid);
  const extensions = path.join(server, 'extensions');
  safe(extensions, root, uid);
  const installed = [];
  for (const tool of ['codex','claude']) {
    const id = tool === 'codex' ? 'openai.chatgpt' : 'anthropic.claude-code';
    const candidates = [];
    for (const folder of fs.existsSync(extensions) ? fs.readdirSync(extensions) : []) {
      if (!folder.startsWith(id + '-') || !/^[a-zA-Z0-9._-]+$/.test(folder)) continue;
      const manifest = path.join(extensions, folder, 'package.json');
      const raw = read(manifest, root, uid);
      if (!raw) continue;
      let pkg;
      try { pkg = JSON.parse(raw); } catch { continue; }
      if (`${pkg.publisher}.${pkg.name}`.toLowerCase() !== id || !version(pkg.version)) continue;
      let runtimeVersion = '';
      if (tool === 'codex') {
        const architecture = process.arch === 'arm64' ? 'aarch64' : 'x86_64';
        const executable = path.join(extensions, folder, 'bin', `linux-${architecture}`, 'codex');
        safe(path.dirname(executable), root, uid);
        if (fs.existsSync(executable)) {
          const stat = fs.lstatSync(executable);
          if (!stat.isFile() || stat.isSymbolicLink() || stat.uid !== uid || stat.mode & 0o022) fail('unsafePath');
          try {
            const text = execFileSync(executable, ['--version'], { timeout: 4000, maxBuffer: 1024, stdio: ['ignore','pipe','ignore'], env: { HOME: root, PATH: '/usr/bin:/bin' } }).toString().trim();
            runtimeVersion = version(text.replace(/^codex-cli /, ''));
          } catch { /* A missing/unknown runtime is a capability result, not a guessed version. */ }
        }
      }
      candidates.push({ tool, version: pkg.version, runtimeVersion, manifestHash: digest(raw) });
    }
    // Multiple installed versions need resolution in VS Code; do not pick arbitrarily.
    const candidate = candidates.length === 1 ? candidates[0] : null;
    const supported = candidate && (tool === 'claude' ? /^2\./.test(candidate.version) : /^0\.(\d+)\./.test(candidate.runtimeVersion) && Number(candidate.runtimeVersion.split('.')[1]) >= 134);
    let configuration = 'notConfigured';
    const file = tool === 'codex' ? path.join(root, '.codex/config.toml') : path.join(server, 'data/Machine/settings.json');
    try {
      transaction({ file, root, uid, tool, port: 25721, operation: 'restore-preview' });
      configuration = 'configured';
    } catch (error) { if (error.message !== 'noBackup') configuration = 'conflict'; }
    installed.push({ tool, detected: !!candidate, supported: !!supported, version: candidate?.version || '', runtimeVersion: candidate?.runtimeVersion || '', manifestHash: candidate?.manifestHash || '', configuration });
  }
  const user = os.userInfo().username;
  if (!/^[a-zA-Z0-9._-]{1,64}$/.test(user)) fail('remoteUnsupported');
  const contextHash = digest(JSON.stringify({ root, uid, user, node: process.execPath, nodeVersion: process.versions.node, installed: installed.map(({configuration, ...entry}) => entry) }));
  const details = { user, contextHash, extensions: installed.map(({manifestHash, ...entry}) => entry) };
  if (request.operation === 'inspect') return details;
  if (!['codex','claude'].includes(request.tool)) fail('invalidRequest');
  if (request.contextHash !== contextHash) fail('extensionContextChanged');
  const selected = installed.find(e => e.tool === request.tool);
  const restoring = request.operation.startsWith('restore');
  if (!restoring && !selected.supported) fail('extensionUnsupported');
  if (request.tool === 'codex' && process.env.CODEX_HOME && process.env.CODEX_HOME !== path.join(root, '.codex')) fail('customHome');
  if (request.tool === 'claude' && process.env.CLAUDE_CONFIG_DIR) fail('customHome');
  if (request.tool === 'claude' && !restoring) checkClaudeShared(read(path.join(root, '.claude/settings.json'), root, uid), process.env);
  const file = request.tool === 'codex' ? path.join(root, '.codex/config.toml') : path.join(server, 'data/Machine/settings.json');
  const result = transaction({ ...request, file, root, uid, contextHash });
  return { ...result, contextHash };
}
try {
  console.log(JSON.stringify(run(globalThis.bridgeExtensionRequest)));
} catch(error) {
  console.log(JSON.stringify({ error: codes.has(error.message) ? error.message : 'configConflict' }));
}
