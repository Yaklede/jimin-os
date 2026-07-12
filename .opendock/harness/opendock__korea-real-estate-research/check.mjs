#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const title = "Korea Real Estate Research";
const runDir = ".opendock/runs/korea-real-estate-research";
const templateFile = ".opendock/templates/korea-real-estate-research/REAL_ESTATE_RESEARCH_RUN.md";
const requiredFiles = ["KOREA_REAL_ESTATE_RESEARCH.md", "HARNESS.md"];
const failures = [];

function full(file) {
  return path.join(root, file);
}

function exists(file) {
  return fs.existsSync(full(file));
}

function rel(file) {
  return path.relative(root, file).split(path.sep).join("/");
}

function fail(rule, file, detail) {
  failures.push({ rule, file, detail });
}

function walk(dir) {
  const start = full(dir);
  if (!fs.existsSync(start)) return [];
  const out = [];
  for (const entry of fs.readdirSync(start, { withFileTypes: true })) {
    const next = path.join(start, entry.name);
    if (entry.isDirectory()) out.push(...walk(rel(next)));
    else if (entry.isFile() && /\.md$/i.test(entry.name)) out.push(next);
  }
  return out;
}

for (const file of requiredFiles) {
  if (!exists(file)) fail("missing-required-file", file, `${file} is required.`);
}
if (!exists(templateFile)) fail("missing-template", templateFile, "Run template is required.");

const secretPattern = /\b(api[_ -]?key|access[_ -]?token|refresh[_ -]?token|password|private[_ -]?key|secret)\b\s*[:=]\s*['"]?[A-Za-z0-9_./+=-]{12,}/i;
const forbiddenAdvicePattern = /(지금\s*사라|무조건\s*오른다|확실한\s*투자처|손해\s*볼\s*수\s*없다|급등\s*보장|매수\s*추천)/i;
const sourcePattern = /(출처|source|data\.go\.kr|rt\.molit\.go\.kr|국토교통부|한국부동산원|R-ONE|ECOS|KOSIS|한국은행)/i;
const scopePattern = /(지역|주택유형|거래유형|면적|기간|기준일|조회일)/i;
const limitationPattern = /(한계|주의|반대\s*시나리오|추가\s*확인|비추천|투자\s*추천이\s*아니)/i;

for (const file of walk(runDir)) {
  const name = rel(file);
  const text = fs.readFileSync(file, "utf8");
  if (/[ \t]+$/m.test(text)) fail("trailing-whitespace", name, "Remove trailing whitespace.");
  if (/^\t+/m.test(text)) fail("tab-indentation", name, "Use spaces for indentation.");
  if (secretPattern.test(text)) fail("secret-like-value", name, "Do not store real-looking secrets in research docs.");
  if (forbiddenAdvicePattern.test(text)) fail("investment-advice", name, "Do not write direct real-estate investment recommendations.");
  if (!sourcePattern.test(text)) fail("missing-source", name, "Research run needs official source notes or source URLs.");
  if (!scopePattern.test(text)) fail("missing-scope", name, "Research run needs region, period, type, area, or base-date scope.");
  if (!limitationPattern.test(text)) fail("missing-limitations", name, "Research run needs limits, counter-scenario, or non-advice note.");
}

if (failures.length) {
  console.error(`OpenDock harness: ${title}`);
  console.error(`Failures: ${failures.length}`);
  for (const failure of failures) console.error(`- [${failure.rule}] ${failure.file}: ${failure.detail}`);
  process.exit(1);
}

console.log(`OpenDock harness: ${title}`);
console.log(`Run files scanned: ${walk(runDir).length}`);
console.log("Capability check passed.");
