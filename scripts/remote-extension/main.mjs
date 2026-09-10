import * as fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { execFileSync } from 'node:child_process';
import { fail, checkClaudeShared } from './config.mjs';
import { safe, read, hash, digest, transaction } from './files.mjs';

const codes = new Set(['configConflict','unsafePath','invalidPort','invalidRequest','noBackup','verifyFailed','rollbackConflict','rollbackFailed','writeRolledBack','extensionMissing','extensionUnsupported','extensionContextChanged','customHome','remoteUnsupported']);
const version = value => typeof value === 'string' && /^\d+(?:\.\d+){1,3}(?:-[a-zA-Z0-9.-]+)?$/.test(value) && value.length < 48 ? value : '';
const safeSegment = value => typeof value === 'string' && /^[a-zA-Z0-9._-]{1,96}$/.test(value);
const displayPath = (root, value) => value === root ? '~' : `~/${path.relative(root, value).split(path.sep).join('/')}`;

function list(root, directory, uid) {
  safe(directory, root, uid);
  if (!fs.existsSync(directory)) return [];
  const stat = fs.lstatSync(directory);
  if (!stat.isDirectory() || stat.isSymbolicLink() || stat.uid !== uid || stat.mode & 0o022) fail('unsafePath');
  return fs.readdirSync(directory);
}

function serverVersions(root, serverRoot, uid) {
  const values = new Set();
  for (const folder of list(root, path.join(serverRoot, 'bin'), uid)) {
    if (safeSegment(folder)) values.add(folder);
  }
  for (const folder of list(root, path.join(serverRoot, 'cli/servers'), uid)) {
    const match = /^(?:Stable|Insiders)-([a-zA-Z0-9._-]{1,96})$/.exec(folder);
    if (match) values.add(match[1]);
  }
  return [...values].sort();
}

function vscodeContexts(root, uid) {
  const definitions = [
    { edition: 'stable', actualRoot: path.join(root, '.vscode-server'), evidence: 'defaultStableRoot' },
    { edition: 'insiders', actualRoot: path.join(root, '.vscode-server-insiders'), evidence: 'defaultInsidersRoot' },
    { edition: 'legacy', actualRoot: path.join(root, '.vscode-remote'), evidence: 'legacyRoot' },
  ];
  const custom = process.env.VSCODE_AGENT_FOLDER;
  if (custom) {
    if (!path.isAbsolute(custom) || (custom !== root && !custom.startsWith(root + path.sep))) fail('customHome');
    definitions.push({ edition: 'custom', actualRoot: custom, evidence: 'agentFolderEnvironment' });
  }
  const unique = new Map();
  for (const definition of definitions) {
    if (unique.has(definition.actualRoot) || !fs.existsSync(definition.actualRoot)) continue;
    safe(definition.actualRoot, root, uid);
    const stat = fs.lstatSync(definition.actualRoot);
    if (!stat.isDirectory() || stat.isSymbolicLink() || stat.uid !== uid || stat.mode & 0o022) fail('unsafePath');
    unique.set(definition.actualRoot, { ...definition, versions: serverVersions(root, definition.actualRoot, uid) });
  }
  const contexts = [...unique.values()];
  const selected = contexts.length === 1 ? contexts[0] : null;
  const status = selected ? 'detected' : contexts.length ? 'ambiguous' : 'unsupported';
  return {
    status,
    selected,
    contexts,
    details: {
      status,
      edition: selected?.edition || 'unknown',
      serverRoot: selected ? displayPath(root, selected.actualRoot) : '',
      serverVersion: selected?.versions.length === 1 ? selected.versions[0] : '',
      serverVersions: selected?.versions || [],
      dataPath: selected ? displayPath(root, path.join(selected.actualRoot, 'data')) : '',
      remoteSettingsPath: selected ? displayPath(root, path.join(selected.actualRoot, 'data/Machine/settings.json')) : '',
      extensionRoot: selected ? displayPath(root, path.join(selected.actualRoot, 'extensions')) : '',
      evidence: selected?.evidence || (contexts.length ? 'multipleServerRoots' : 'noServerRoot'),
      confidence: selected ? (selected.versions.length === 1 ? 'high' : 'medium') : 'low',
      candidateCount: contexts.length,
    },
  };
}

function extensionCandidates(root, uid, contexts, tool) {
  const id = tool === 'codex' ? 'openai.chatgpt' : 'anthropic.claude-code';
  const candidates = [];
  for (const context of contexts) {
    const extensions = path.join(context.actualRoot, 'extensions');
    for (const folder of list(root, extensions, uid)) {
      if (!folder.startsWith(id + '-') || !safeSegment(folder)) continue;
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
          } catch { /* Runtime availability is evidence, never a guessed version. */ }
        }
      }
      const compatible = tool === 'claude'
        ? /^2\./.test(pkg.version)
        : /^0\.(\d+)\./.test(runtimeVersion) && Number(runtimeVersion.split('.')[1]) >= 134;
      candidates.push({ tool, version: pkg.version, runtimeVersion, manifestHash: digest(raw), compatible, contextRoot: displayPath(root, context.actualRoot) });
    }
  }
  return candidates;
}

function configurationState({ tool, root, uid, vscode }) {
  const file = tool === 'codex'
    ? path.join(root, '.codex/config.toml')
    : vscode.selected ? path.join(vscode.selected.actualRoot, 'data/Machine/settings.json') : null;
  if (!file) return 'unknown';
  try {
    transaction({ file, root, uid, tool, port: 25721, operation: 'restore-preview' });
    return 'configured';
  } catch (error) {
    return error.message === 'noBackup' ? 'notConfigured' : 'conflict';
  }
}

function run(request) {
  if (process.platform !== 'linux' || process.getuid() === 0 || Number(process.versions.node.split('.')[0]) < 20) fail('remoteUnsupported');
  const root = os.homedir(), uid = process.getuid();
  safe(root, root, uid);
  const vscode = vscodeContexts(root, uid);
  const installed = [];
  for (const tool of ['codex','claude']) {
    const candidates = extensionCandidates(root, uid, vscode.contexts, tool);
    const versions = [...new Set(candidates.map(candidate => candidate.version))].sort();
    const runtimeVersions = [...new Set(candidates.map(candidate => candidate.runtimeVersion).filter(Boolean))].sort();
    installed.push({
      tool,
      detected: candidates.length > 0,
      supported: candidates.length > 0 && candidates.every(candidate => candidate.compatible),
      version: versions.length === 1 ? versions[0] : '',
      versions,
      runtimeVersion: runtimeVersions.length === 1 ? runtimeVersions[0] : '',
      runtimeVersions,
      candidateCount: candidates.length,
      location: candidates.length ? (vscode.status === 'detected' ? 'activeUnknown' : 'locationUnknown') : 'locationUnknown',
      manifestHashes: candidates.map(candidate => candidate.manifestHash),
      contextRoots: [...new Set(candidates.map(candidate => candidate.contextRoot))],
      configuration: configurationState({ tool, root, uid, vscode }),
    });
  }
  const user = os.userInfo().username;
  if (!/^[a-zA-Z0-9._-]{1,64}$/.test(user)) fail('remoteUnsupported');
  const contextHash = digest(JSON.stringify({ root, uid, user, node: process.execPath, nodeVersion: process.versions.node, vscode: vscode.details, installed: installed.map(({configuration, ...entry}) => entry) }));
  const details = { user, contextHash, vscode: vscode.details, extensions: installed.map(({manifestHashes, contextRoots, ...entry}) => entry) };
  if (request.operation === 'inspect') return details;
  if (!['codex','claude'].includes(request.tool)) fail('invalidRequest');
  if (request.contextHash !== contextHash) fail('extensionContextChanged');
  const selected = installed.find(e => e.tool === request.tool);
  const restoring = request.operation.startsWith('restore');
  if ((!restoring || request.tool === 'claude') && (vscode.status !== 'detected' || !vscode.selected)) fail('extensionContextChanged');
  if (!restoring && !selected.detected) fail('extensionMissing');
  if (!restoring && !selected.supported) fail('extensionUnsupported');
  if (request.tool === 'codex' && process.env.CODEX_HOME && process.env.CODEX_HOME !== path.join(root, '.codex')) fail('customHome');
  if (request.tool === 'claude' && process.env.CLAUDE_CONFIG_DIR) fail('customHome');
  if (request.tool === 'claude' && !restoring) checkClaudeShared(read(path.join(root, '.claude/settings.json'), root, uid), process.env);
  const file = request.tool === 'codex' ? path.join(root, '.codex/config.toml') : path.join(vscode.selected.actualRoot, 'data/Machine/settings.json');
  const result = transaction({ ...request, file, root, uid, contextHash });
  return { ...result, contextHash, path: request.tool === 'codex' ? '~/.codex/config.toml' : vscode.details.remoteSettingsPath };
}
try {
  console.log(JSON.stringify(run(globalThis.bridgeExtensionRequest)));
} catch(error) {
  console.log(JSON.stringify({ error: codes.has(error.message) ? error.message : 'configConflict' }));
}
