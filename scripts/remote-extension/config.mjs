import { parseTree, findNodeAtLocation, getNodeValue, modify, applyEdits } from 'jsonc-parser';
import { parseTOML, getStaticTOMLValue } from 'toml-eslint-parser';

export const fail = (code = 'configConflict') => { throw new Error(code); };
const own = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
export const routingName = name => /^(ANTHROPIC_(BASE_URL|AUTH_TOKEN|API_KEY|CUSTOM_HEADERS)|CLAUDE_CODE_(USE_.*|OAUTH_TOKEN|PROVIDER_MANAGED_BY_HOST))$/.test(name);
export function checkClaudeShared(text, environment = {}) {
  if (Object.keys(environment).some(name => routingName(name) && environment[name])) fail();
  if (!text) return;
  const tree = jsonTree(text);
  if (findNodeAtLocation(tree, ['apiKeyHelper'])) fail();
  const env = findNodeAtLocation(tree, ['env']);
  if (env && (env.type !== 'object' || env.children.some(property => routingName(property.children[0].value)))) fail();
}
export function jsonTree(text) {
  const errors = [];
  const tree = parseTree(text || '{}', errors, { allowTrailingComma: true, disallowComments: false });
  if (errors.length || tree?.type !== 'object') fail();
  function check(node) {
    if (node.type === 'object') {
      const keys = node.children.map(p => p.children[0].value);
      if (new Set(keys).size !== keys.length) fail();
    }
    for (const child of node.children || []) check(child);
  }
  check(tree);
  return tree;
}
export function claudeLoginPromptState(text) {
  const node = findNodeAtLocation(jsonTree(text), ['claudeCode.disableLoginPrompt']);
  if (!node) return 'absent';
  const value = getNodeValue(node);
  if (typeof value !== 'boolean') fail();
  return value ? 'enabled' : 'disabled';
}
export function patchClaude(text, port) {
  const tree = jsonTree(text);
  const key = 'claudeCode.environmentVariables';
  const loginPromptKey = 'claudeCode.disableLoginPrompt';
  const node = findNodeAtLocation(tree, [key]);
  const loginPromptNode = findNodeAtLocation(tree, [loginPromptKey]);
  const entries = node ? getNodeValue(node) : [];
  if (!Array.isArray(entries)) fail();
  if (loginPromptNode && typeof getNodeValue(loginPromptNode) !== 'boolean') fail();
  // Credentials already present are never inspected, displayed or overwritten.
  const names = new Set();
  for (const item of entries) {
    if (!item || typeof item.name !== 'string' || names.has(item.name) || routingName(item.name)) fail();
    names.add(item.name);
  }
  const extra = [
    { name: 'ANTHROPIC_BASE_URL', value: `http://127.0.0.1:${port}` },
    { name: 'ANTHROPIC_AUTH_TOKEN', value: 'PROXY_MANAGED' },
  ];
  let result = text || '{}';
  if (!node) {
    result = applyEdits(result, modify(result, [key], extra, {}));
  } else {
    // Insert only our entries. Existing array items/comments remain byte-identical.
    const at = node.offset + 1;
    result = result.slice(0, at) + extra.map(v => JSON.stringify(v)).join(',') +
      (entries.length ? ',' : '') + result.slice(at);
  }
  result = applyEdits(result, modify(result, [loginPromptKey], true, {}));
  jsonTree(result);
  return result;
}
export function patchCodex(text, port) {
  const ast = parseTOML(text, { tomlVersion: '1.0.0' });
  const config = getStaticTOMLValue(ast);
  if (own(config, 'profile') || own(config.model_providers || {}, 'proxyenv_bridge')) fail();
  if (own(config, 'model_provider') && typeof config.model_provider !== 'string') fail();
  const node = ast.body[0].body.find(n => n.type === 'TOMLKeyValue' &&
    n.key.keys.length === 1 && (n.key.keys[0].name ?? n.key.keys[0].value) === 'model_provider');
  const provider = 'model_provider = "proxyenv_bridge"\n';
  let result = node ? text.slice(0, node.value.range[0]) + '"proxyenv_bridge"' + text.slice(node.value.range[1]) : provider + text;
  result += `\n[model_providers.proxyenv_bridge]\nname = "ProxyEnv CC Switch"\nbase_url = "http://127.0.0.1:${port}/v1"\nwire_api = "responses"\nrequires_openai_auth = false\n`;
  // Reject incompatible inline/dotted table shapes instead of guessing how to rewrite them.
  const verified = getStaticTOMLValue(parseTOML(result, { tomlVersion: '1.0.0' }));
  if (verified.model_provider !== 'proxyenv_bridge' || verified.model_providers?.proxyenv_bridge?.base_url !== `http://127.0.0.1:${port}/v1`) fail();
  return result;
}
export function patch(text, tool, port) {
  if (!Number.isInteger(port) || port < 1024 || port > 65535) fail('invalidPort');
  if (tool === 'codex') return patchCodex(text, port);
  if (tool === 'claude') return patchClaude(text, port);
  fail('invalidRequest');
}
