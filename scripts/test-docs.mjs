import assert from "node:assert/strict";
import test from "node:test";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const subjects = ["GETTING_STARTED.md", "REMOTE_BRIDGE.md", "ARCHITECTURE.md",
  "ROADMAP.md", "INTERNAL_CONTRACTS.md", "RELEASE_QA_V0.2.md", "release-security.md"];
const localizedSubjects = subjects.map(name => name.replace(/\.md$/, ".zh-CN.md"));
const pairedDocs = ["SECURITY.md", "CONTRIBUTING.md", "assets/brand/README.md",
  "docs/assets/README.md", "docs/README.md", ...subjects.map(name => `docs/${name}`)];
const chinesePath = path => path.replace(/\.md$/, ".zh-CN.md");
const current = ["README.md", "README.zh-CN.md", ...pairedDocs, ...pairedDocs.map(chinesePath)];
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
  "docs/archive/readme-illustration-prompts.md",
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
    ["README.md", "README.zh-CN.md", ...subjects, ...localizedSubjects].sort());
  const allowed = new Set(["README.md", "README.zh-CN.md", "SECURITY.md", "CONTRIBUTING.md",
    "SECURITY.zh-CN.md", "CONTRIBUTING.zh-CN.md", "docs/README.md", "docs/README.zh-CN.md",
    "docs/archive/README.md", ...[...subjects, ...localizedSubjects].map(name => `docs/${name}`)]);
  for (const target of [...links(read("docs/README.md")), ...links(read("docs/README.zh-CN.md"))]) {
    const local = localTarget("docs/README.md", target);
    assert.ok(local && allowed.has(local), `Noncanonical index entry: ${target}`);
  }
  for (const file of current) {
    for (const target of links(read(file))) {
      const local = localTarget(file, target);
      if (local?.startsWith("docs/archive/")) {
        assert.ok(["docs/README.md", "docs/README.zh-CN.md", "docs/ARCHITECTURE.md", "docs/ARCHITECTURE.zh-CN.md"].includes(file) &&
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
    for (const version of ["v0.1.4", "v0.1.3", "v0.2.0", "v0.2.1", "X-ProxyEnv-Session"]) assert.ok(source.includes(version));
    assert.equal(source.split("\n").filter(line => line.startsWith("| ")).length, 8);
    assert.match(source, /<img src="assets\/brand\/routespan-mark\.png" width="88" alt="RouteSpan">/);
    // Illustration links/badges add markup, not reading length. Keep the prose budget.
    const proseLength = source.replace(/<[^>]*>/g, "").length;
    assert.ok(proseLength >= minimumLength && proseLength <= maximumLength,
      `README prose length ${proseLength}; expected ${minimumLength}–${maximumLength}`);
  }
  const docLinks = source => [...new Set(links(source).map(path => path.replace(/\.zh-CN\.md(?=$|#)/, ".md"))
    .filter(path => path.startsWith("docs/") || ["SECURITY.md", "CONTRIBUTING.md"].includes(path)))].sort();
  assert.deepEqual(docLinks(en), docLinks(zh));
  assert.match(en, /does not\s+replace CC Switch/);
  assert.match(zh, /不替代 CC Switch/);
  assert.match(en, /does not provide per-UID isolation/);
  assert.match(zh, /不能隔离共享 Linux/);
  assert.match(en, /not protect against root/);
  assert.match(zh, /不能防御服务器 root/);
  assert.match(en, /RouteSpan v0\.2\.2 is the latest public stable release/);
  assert.match(zh, /RouteSpan v0\.2\.2 是最新公开稳定版/);
  assert.match(en, /automatic external egress check/);
  assert.match(zh, /自动执行一次外网出口检查/);
});

test("published v0.2 status agrees across current EN/ZH docs without rewriting historical evidence", () => {
  const releaseUrl = "https://github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v0.2.2";
  for (const file of ["README.md", "README.zh-CN.md", "SECURITY.md", "SECURITY.zh-CN.md",
    ...["README", "GETTING_STARTED", "REMOTE_BRIDGE", "ROADMAP", "INTERNAL_CONTRACTS", "release-security"]
      .flatMap(name => [`docs/${name}.md`, `docs/${name}.zh-CN.md`])]) {
    const source = read(file);
    assert.match(source, /RouteSpan v0\.2\.2/, file);
    assert.doesNotMatch(source, /v0\.1\.4[^\n。]*?(?:remains a Draft|仍为 Draft)|Its GitHub Release remains Draft/, file);
    assert.doesNotMatch(source, /not yet (?:publicly )?released|is unreleased|v0\.2\.0[^\n。]*尚未|v0\.2\.0[^\n。]*未发布/i, file);
    assert.doesNotMatch(source, /latest public release is v0\.1\.3|最新公开(?: Release|版)为 v0\.1\.3/, file);
  }
  for (const file of ["README.md", "README.zh-CN.md", "docs/GETTING_STARTED.md", "docs/GETTING_STARTED.zh-CN.md",
    "docs/RELEASE_QA_V0.2.md", "docs/RELEASE_QA_V0.2.zh-CN.md"]) {
    assert.ok(links(read(file)).includes(releaseUrl), `Missing stable download: ${file}`);
  }
  for (const file of ["docs/RELEASE_QA_V0.2.md", "docs/RELEASE_QA_V0.2.zh-CN.md"]) {
    const source = read(file);
    assert.ok(source.includes("0a731b47d17bd5d2f8a4885cbf926a1bac68d35f"));
    assert.ok(source.includes("38335015721"));
    assert.ok(source.includes("85be2e17d185ccff526936e0de3321ca99463789"));
    assert.ok(source.includes("39395982821"));
    assert.match(source, /historical snapshots|历史快照/);
    assert.match(source, /kept privately|私下保存/);
    assert.match(source, /^\|.*\| BLOCKED \|$/m, "Preserve earlier failed-build results");
  }
});

test("v0.2.1 publication retains its source branch and private device acceptance", () => {
  for (const file of ["CONTRIBUTING.md", "CONTRIBUTING.zh-CN.md", "docs/ROADMAP.md", "docs/ROADMAP.zh-CN.md"]) {
    const source = read(file);
    assert.ok(source.includes("v0.2.1dev"), file);
    assert.ok(source.includes("v0.2.0"), file);
    assert.match(source, /private|私下保存/, file);
    assert.doesNotMatch(source, /v0\.2\.0 is unreleased|v0\.2\.0 尚未发布/, file);
    if (file.startsWith("docs/ROADMAP")) {
      assert.match(source, /Released: RouteSpan v0\.2\.1|已发布：RouteSpan v0\.2\.1/, file);
      assert.ok(links(source).includes("../.github/release-notes/v0.2.1.md"), file);
      assert.match(source, /separate authorization|分别需要授权/, file);
      assert.match(source, /contrast exceptions|对比度例外/, file);
    }
  }
});

test("every current document has a navigable Chinese mirror with matching structure and commands", () => {
  for (const path of pairedDocs) {
    const translatedPath = chinesePath(path);
    const en = read(path), zh = read(translatedPath);
    assert.ok(links(en).includes(translatedPath.split("/").at(-1)), `Missing Chinese entry: ${path}`);
    assert.ok(links(zh).includes(path.split("/").at(-1)), `Missing English entry: ${translatedPath}`);
    assert.match(zh, /[\u4e00-\u9fff]/);
    const headings = text => [...text.matchAll(/^## /gm)].length;
    assert.equal(headings(zh), headings(en), `Section count drift: ${translatedPath}`);
    const commands = text => [...text.matchAll(/```powershell\n([\s\S]*?)```/g)].map(match => match[1].trim());
    assert.deepEqual(commands(zh), commands(en), `Command drift: ${translatedPath}`);
    for (const target of links(zh)) {
      const local = localTarget(translatedPath, target);
      if (local && pairedDocs.includes(local)) {
        // Only language switching may point at the English edition.
        assert.ok(local === path, `Chinese navigation points to English: ${translatedPath} -> ${local}`);
      }
    }
  }
  const statuses = text => [...text.matchAll(/^\|.*\| (PASS|PARTIAL|PENDING|BLOCKED) \|$/gm)].map(match => match[1]);
  assert.deepEqual(statuses(read("docs/RELEASE_QA_V0.2.zh-CN.md")), statuses(read("docs/RELEASE_QA_V0.2.md")));
  assert.match(read("docs/GETTING_STARTED.zh-CN.md"), /不.*自动安装|不会自动安装/);
  assert.match(read("docs/REMOTE_BRIDGE.zh-CN.md"), /不设用户名\/密码认证/);
  assert.match(read("SECURITY.zh-CN.md"), /不能按 UID 隔离/);
  assert.match(read("SECURITY.zh-CN.md"), /root 与同账户恶意进程/);
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
