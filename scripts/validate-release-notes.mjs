import { readFile } from "node:fs/promises";

const filePath = process.argv[2];
if (!filePath) throw new Error("Usage: node scripts/validate-release-notes.mjs <release-notes.md>");

const releaseNotes = await readFile(filePath, "utf8");
const requiredHeadings = {
  "zh-CN": ["新功能", "改进", "增强", "修复"],
  en: ["New Features", "Improvements", "Enhancements", "Fixes"],
  ja: ["新機能", "改善", "強化", "修正"],
  ko: ["새로운 기능", "개선", "강화", "수정"]
};

for (const [locale, headings] of Object.entries(requiredHeadings)) {
  const start = `<!-- proxyenv-release:${locale} -->`;
  const end = "<!-- proxyenv-release:end -->";
  const startIndex = releaseNotes.indexOf(start);
  if (startIndex < 0) throw new Error(`Missing ${locale} release-note block`);
  const endIndex = releaseNotes.indexOf(end, startIndex + start.length);
  if (endIndex < 0) throw new Error(`Missing end marker for ${locale} release-note block`);
  const block = releaseNotes.slice(startIndex + start.length, endIndex);
  for (const heading of headings) {
    if (!block.includes(`## ${heading}`)) throw new Error(`Missing ${locale} heading: ${heading}`);
  }
  if (!/^\s*-\s+\S/m.test(block)) throw new Error(`${locale} release-note block has no change items`);
}

if (/full\s+changelog\s*:/i.test(releaseNotes)) {
  throw new Error("Release notes must contain reviewed change content, not a Full Changelog placeholder");
}

console.log(`Validated localized release notes: ${filePath}`);
