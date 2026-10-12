import assert from "node:assert/strict";
import test from "node:test";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const subjects = ["GETTING_STARTED.md", "REMOTE_BRIDGE.md", "ARCHITECTURE.md",
  "ROADMAP.md", "INTERNAL_CONTRACTS.md", "RELEASE_QA_V0.2.md", "release-security.md"];
const current = ["README.md", "README.zh-CN.md", "SECURITY.md", "CONTRIBUTING.md",
  "assets/brand/README.md", "docs/assets/README.md", "docs/README.md", ...subjects.map(name => `docs/${name}`)];
const readmeArtwork = ["routespan-workflow.png", "routespan-architecture.png",
  "routespan-app-launch.png", "routespan-tool-readiness.png"];
const read = path => readFileSync(resolve(root, path), "utf8").replace(/\r\n/g, "\n");
const withoutFences = source => source.replace(/^(?:```|~~~)[\s\S]*?^(?:```|~~~).*$/gm, "");

// Repository Markdown subset: inline/ref links, images and quoted HTML src/href.
// Code fences are examples, not navigation. No network requests are made.
function links(source) {
  const text = withoutFences(source);
  const targets = [];
  const destination = raw => {
    const match = raw.trim().match(/^<([^>]+)>|^(\S+)/);
    if (match) targets.push(match[1] ?? match[2]);
  };
  for (const match of text.matchAll(/!?\[[^\]\n]*\]\((<[^>]+>|[^)\n]+)\)/g)) destination(match[1]);
  for (const match of text.matchAll(/\b(?:src|href)\s*=\s*["']([^"']+)["']/gi)) targets.push(match[1]);
  const definitions = new Map([...text.matchAll(/^\s{0,3}\[([^\]]+)\]:\s*(<[^>]+>|\S+)/gm)]
    .map(match => [match[1].toLowerCase(), match[2]]));
  for (const match of text.matchAll(/!?\[([^\]]+)\]\[([^\]]*)\]/g)) {
    const key = (match[2] || match[1]).toLowerCase();
    assert.ok(definitions.has(key), `Undefined reference: ${key}`);
    destination(definitions.get(key));
  }
  return targets;
}
function localTarget(file, target) {
  if (/^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(target)) return null;
  const path = decodeURIComponent(target.split(/[?#]/, 1)[0]);
  const absolute = resolve(root, dirname(file), path || file.split("/").at(-1));
  const rel = relative(root, absolute);
  assert.ok(!rel.startsWith(`..${sep}`) && rel !== ".." && !path.startsWith("/"),
    `Out-of-repository link: ${file} -> ${target}`);
  return rel.split(sep).join("/");
}
const archiveNames = [
  ...["README", "V0.2_SCOPE", "CLOSEOUT_STATUS_V0.2", "FIRST_CONNECTION",
    "REMOTE_TOOL_READINESS", "UI_UX", "SSH_CONNECTION_MANAGER",
    "MANAGED_REMOTE_ENVIRONMENT", "SKILLS_REMOTE_PROJECTION",
    "AUTHENTICATED_RELAY", "MCP_V0.3", "ACCEPTANCE_V0.2"]
    .map(name => `docs/archive/remote-bridge/${name}.md`),
  ...["BRAND_MIGRATION", "INFORMATION_ARCHITECTURE"]
    .map(name => `docs/archive/product/${name}.md`),
  "docs/archive/REMOTE_BRIDGE_EXTENSION_AUDIT_2026-09-06.md",
  "docs/archive/REMOTE_BRIDGE_IMPLEMENTATION_2026-09-22.md",
];

test("link extraction covers Markdown, images, HTML and references without parsing code fences", () => {
  assert.deepEqual(links('[A](a.md) ![B](<b c.png>) <a href="d.md">D</a>\n[E][ref]\n[ref]: e.md\n```text\n[X](missing.md)\n```'),
    ["a.md", "b c.png", "d.md", "e.md"]);
  assert.throws(() => links("[A][missing]"), /Undefined reference/);
  assert.throws(() => localTarget("docs/README.md", "../../outside.md"), /Out-of-repository/);
});

test("current and archived relative links and images resolve to repository files", () => {
  for (const file of [...current, "docs/archive/README.md", ...archiveNames]) {
    for (const target of links(read(file))) {
      const local = localTarget(file, target);
      if (local !== null) assert.ok(existsSync(resolve(root, local)), `${file} -> ${target}`);
    }
  }
});

test("canonical current docs are bounded and the index links only authoritative entries", () => {
  assert.deepEqual(readdirSync(resolve(root, "docs")).filter(name => name.endsWith(".md")).sort(),
    ["README.md", ...subjects].sort());
  const allowed = new Set(["README.md", "README.zh-CN.md", "SECURITY.md", "CONTRIBUTING.md",
    "docs/archive/README.md", ...subjects.map(name => `docs/${name}`)]);
  for (const target of links(read("docs/README.md"))) {
    const local = localTarget("docs/README.md", target);
    assert.ok(local && allowed.has(local), `Noncanonical index entry: ${target}`);
  }
  for (const file of current) {
    for (const target of links(read(file))) {
      const local = localTarget(file, target);
      if (local?.startsWith("docs/archive/")) {
        assert.ok(["docs/README.md", "docs/ARCHITECTURE.md"].includes(file) &&
          local === "docs/archive/README.md", `Archive used as current source: ${file}`);
      }
    }
  }
  for (const dir of ["docs/remote-bridge", "docs/product"]) {
    assert.ok(!existsSync(resolve(root, dir)) || readdirSync(resolve(root, dir)).length === 0,
      `Superseded current directory still populated: ${dir}`);
  }
});

test("all retained historical documents have a stale marker and appear in the archive index", () => {
  const listed = new Set(links(read("docs/archive/README.md"))
    .map(target => localTarget("docs/archive/README.md", target)));
  for (const path of archiveNames) {
    assert.match(read(path), /^> \[!WARNING\]\n> Historical document\./);
    assert.match(read(path).slice(0, 250), /not a source of truth for current RouteSpan behavior/);
    assert.ok(listed.has(path), path);
  }
});

test("display branding is RouteSpan; legacy identifiers require compatibility context", () => {
  for (const file of current) {
    const source = read(file);
    assert.equal(/EnvHub|境启(?=\s|[·/—]|ProxyEnv|$)|assets\/icon\.svg|codex-clipboard-/m.test(source), false, file);
    assert.doesNotMatch(source, /^#{1,6}\s+.*ProxyEnv/gm);
    assert.doesNotMatch(source, /\b(?:Stage [1-5]|M8|M10)\b/);
    const paragraphs = source.split(/\n\s*\n/);
    for (const paragraph of paragraphs.filter(value => value.includes("ProxyEnv"))) {
      // Paths, headers and repository names are compatibility identifiers, not display names.
      const prose = paragraph.replace(/`[^`]*`/g, "").replace(/https?:\/\/\S+/g, "");
      if (prose.includes("ProxyEnv")) {
        assert.match(paragraph, /legacy|compatib|baseline|predecessor|retained|previous stable|旧品牌|兼容|稳定基线|名称/i,
          `Unexplained old display brand in ${file}: ${paragraph}`);
      }
    }
  }
});

test("EN/ZH README section order, capability count, entries and documentation links agree", () => {
  const en = read("README.md"), zh = read("README.zh-CN.md");
  const headings = source => [...source.matchAll(/^## (.+)$/gm)].map(match => match[1]);
  assert.deepEqual(headings(en), ["Release status", "What is RouteSpan?", "Why RouteSpan?",
    "How it works", "Core capabilities", "Quick start", "Supported remote development tools",
    "Security and boundaries", "Current v0.2 status", "Documentation", "Development", "License"]);
  assert.deepEqual(headings(zh), ["发布状态", "RouteSpan 是什么？", "为什么使用 RouteSpan？",
    "工作方式", "核心能力", "快速开始", "受支持的远程开发工具", "安全与边界",
    "当前 v0.2 状态", "文档", "开发", "许可"]);
  const sections = source => [...source.matchAll(/^### (.+)$/gm)].map(match => match[1]);
  assert.equal(sections(en).length, 3);
  assert.equal(sections(zh).length, 3);
  const featureCounts = source => source.split(/^### /m).slice(1)
    .map(section => (section.split(/^## /m)[0].match(/^- /gm) ?? []).length);
  assert.deepEqual(featureCounts(en), featureCounts(zh));
  for (const [source, minimumLength, maximumLength] of [[en, 6000, 10000], [zh, 5000, 9000]]) {
    for (const label of ["Codex CLI", "Claude Code CLI", "VS Code Remote", "MobaXterm", "Skills"]) assert.ok(source.includes(label));
    for (const version of ["v0.1.4", "v0.1.3", "v0.2.0", "Draft", "X-ProxyEnv-Session"]) assert.ok(source.includes(version));
    assert.equal(source.split("\n").filter(line => line.startsWith("| ")).length, 8);
    assert.match(source, /<img src="assets\/brand\/routespan-mark\.png" width="88" alt="RouteSpan">/);
    // Illustration links/badges add markup, not reading length. Keep the prose budget.
    const proseLength = source.replace(/<[^>]*>/g, "").length;
    assert.ok(proseLength >= minimumLength && proseLength <= maximumLength,
      `README prose length ${proseLength}; expected ${minimumLength}–${maximumLength}`);
  }
  const docLinks = source => [...new Set(links(source).filter(path => path.startsWith("docs/") || ["SECURITY.md", "CONTRIBUTING.md"].includes(path)))].sort();
  assert.deepEqual(docLinks(en), docLinks(zh));
  assert.match(en, /does not\s+replace CC Switch/);
  assert.match(zh, /不替代 CC Switch/);
  assert.match(en, /does not provide per-UID isolation/);
  assert.match(zh, /不能隔离共享 Linux/);
  assert.match(en, /not protect against root/);
  assert.match(zh, /不能防御服务器 root/);
  assert.match(en, /not yet released/);
  assert.match(zh, /尚未正式发布/);
  assert.match(en, /automatic external egress check/);
  assert.match(zh, /自动执行一次外网出口检查/);
});

test("remote guide is substantive and preserves independent readiness and safety boundaries", () => {
  const guide = read("docs/REMOTE_BRIDGE.md");
  for (const section of ["Target discovery", "Connect", "Bridge local network", "CC Switch AI Route",
    "Codex / Claude states", "Skills", "Launch actions", "Overview and Advanced",
    "Reconnect and disconnect", "Troubleshooting"]) assert.ok(guide.includes(`## ${section}`));
  assert.ok(guide.length > 6000);
  assert.doesNotMatch(guide, /^#.*moved|documentation has moved/i);
  for (const state of ["Installed", "Configured", "Verified"]) assert.ok(guide.includes(state));
  assert.match(guide, /two-second metadata checks/);
  assert.match(guide, /no username\/password authentication/);
  assert.match(guide, /automatically/);
});

test("documentation tests are wired into local checks and CI without removing the audit gate", () => {
  const pkg = JSON.parse(read("package.json"));
  assert.equal(pkg.scripts["test:docs"], "node --test scripts/test-docs.mjs");
  assert.ok(pkg.scripts.check.includes("pnpm test:docs"));
  const ci = read(".github/workflows/ci.yml");
  assert.match(ci, /name: Test documentation contracts\s+run: pnpm test:docs/);
  assert.ok(ci.indexOf("name: Test documentation contracts") > ci.indexOf("name: Build frontend"));
  assert.match(ci, /pnpm audit --audit-level high/);
  for (const name of ["brand", "docs", "report", "remote", "extensions"])
    assert.ok(read("CONTRIBUTING.md").includes(`pnpm test:${name}`));
});

test("README illustrations are valid PNGs, not interface screenshots; platforms and license remain visible", () => {
  for (const name of readmeArtwork) {
    const path = `docs/assets/${name}`;
    const bytes = readFileSync(resolve(root, path));
    assert.equal(bytes.toString("hex", 0, 8), "89504e470d0a1a0a");
    assert.equal(bytes.toString("ascii", 12, 16), "IHDR");
    assert.ok(bytes.readUInt32BE(16) > bytes.readUInt32BE(20), "README art must be landscape");
  }
  for (const file of ["README.md", "README.zh-CN.md"]) {
    const source = read(file);
    assert.match(source, /href="LICENSE"><img alt="MIT/);
    assert.match(source, /platform-Windows%2010%20%7C%2011%20x64/);
    for (const name of readmeArtwork) assert.ok(source.includes(name));
    assert.doesNotMatch(source, /routespan-local\.png|```text/);
  }
  assert.match(read("README.md"), /checkmarks show the intended setup, not verified network/);
  assert.match(read("README.md"), /Arrows show capability flow, not packet\s+or SSH-establishment direction/);
  assert.match(read("README.zh-CN.md"), /绿色勾选.*不代表网络访问与模型请求已经验证通过/);
  assert.match(read("README.zh-CN.md"), /不是逐包传输或 SSH 建立方向图/);
  assert.match(read("README.md"), /Running applications stay\s+unchanged/);
  assert.match(read("README.md"), /Green checks illustrate prepared states, not successful model requests/);
  assert.match(read("README.zh-CN.md"), /已有应用保持不变/);
  assert.match(read("README.zh-CN.md"), /绿色勾选仅示意准备状态/);
});

test("subject documents contain no leaked user-machine paths", () => {
  assert.ok(statSync(resolve(root, "assets/brand/routespan-mark.png")).size > 0);
  for (const file of current) {
    assert.doesNotMatch(read(file), /[CD]:[\\/]Users[\\/]|iZ7x|sk-[a-z0-9]{20,}/i);
  }
});
