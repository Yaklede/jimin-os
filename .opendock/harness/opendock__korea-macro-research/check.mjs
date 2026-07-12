#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const title = "Korea Macro Research";
const runDir = ".opendock/runs/korea-macro-research";
const templateFile = ".opendock/templates/korea-macro-research/MACRO_RESEARCH_RUN.md";
const requiredFiles = ["KOREA_MACRO_RESEARCH.md", "HARNESS.md"];
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
const sourcePattern = /(출처|source|ECOS|KOSIS|한국은행|통계청|data\.go\.kr|R-ONE|한국부동산원)/i;
const definitionPattern = /(지표|단위|공표\s*주기|기준일|조회일|통계표|항목|계절조정|전년동월비|전월비)/i;
const limitationPattern = /(한계|주의|반대\s*시나리오|가능\s*경로|보조\s*지표|가설)/i;
const vagueCurrentPattern = /(최근|현재|요즘)(?![^\n]{0,30}(기준일|조회일|202[0-9]|20[0-9]{2}))/i;

for (const file of walk(runDir)) {
  const name = rel(file);
  const text = fs.readFileSync(file, "utf8");
  if (/[ \t]+$/m.test(text)) fail("trailing-whitespace", name, "Remove trailing whitespace.");
  if (/^\t+/m.test(text)) fail("tab-indentation", name, "Use spaces for indentation.");
  if (secretPattern.test(text)) fail("secret-like-value", name, "Do not store real-looking secrets in research docs.");
  if (vagueCurrentPattern.test(text)) fail("vague-current-date", name, "Recent/current statements need an explicit base date.");
  if (!sourcePattern.test(text)) fail("missing-source", name, "Research run needs official source notes or source URLs.");
  if (!definitionPattern.test(text)) fail("missing-definition", name, "Research run needs indicator definition, unit, period, or base-date notes.");
  if (!limitationPattern.test(text)) fail("missing-limitations", name, "Research run needs limits, counter-scenario, or hypothesis notes.");
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
