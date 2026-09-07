import { build } from 'esbuild';
import { readFileSync } from 'node:fs';
const checking = process.argv.includes('--check');
const licenses = ['node_modules/jsonc-parser/LICENSE.md', 'node_modules/toml-eslint-parser/LICENSE'].map(file => readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).join('\n\n');
const result = await build({
  entryPoints: ['scripts/remote-extension/main.mjs'],
  outfile: 'src-tauri/src/features/remote_bridge/extension-helper.cjs',
  bundle: true, platform: 'node', target: 'node20', format: 'cjs',
  minify: true, legalComments: 'external',
  write: !checking,
  banner: { js: '/*! Bundled parser licenses:\n' + licenses + '\n*/' },
});
if (checking) for (const file of result.outputFiles) {
  if (!readFileSync(file.path).equals(file.contents)) throw new Error('Remote extension bundle is stale. Run pnpm build:remote.');
}
