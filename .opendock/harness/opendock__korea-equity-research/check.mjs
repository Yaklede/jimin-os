#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const title = "Korea Equity Research";
const runDir = ".opendock/runs/korea-equity-research";
const templateFile = ".opendock/templates/korea-equity-research/EQUITY_RESEARCH_RUN.md";
const requiredFiles = ["KOREA_EQUITY_RESEARCH.md", "HARNESS.md"];
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
const forbiddenAdvicePattern = /(매수\s*추천|매도\s*추천|상한가\s*간다|무조건\s*오른다|확정\s*수익|목표가\s*보장|지금\s*사라)/i;
const sourcePattern = /(출처|source|KRX|OpenDART|DART|KIND|data\.go\.kr|한국거래소|금융감독원|금융위원회|ECOS|한국은행)/i;
const scopePattern = /(종목|단축코드|ISIN|시장|기준일|조회일)/i;
const disclosurePattern = /(공시|사업보고서|분기보고서|반기보고서|주요사항|정정공시|OpenDART|KIND)/i;
const riskPattern = /(리스크|하락\s*시나리오|반대\s*시나리오|한계|비추천|투자\s*추천이\s*아니)/i;

for (const file of walk(runDir)) {
  const name = rel(file);
  const text = fs.readFileSync(file, "utf8");
  if (/[ \t]+$/m.test(text)) fail("trailing-whitespace", name, "Remove trailing whitespace.");
  if (/^\t+/m.test(text)) fail("tab-indentation", name, "Use spaces for indentation.");
  if (secretPattern.test(text)) fail("secret-like-value", name, "Do not store real-looking secrets in research docs.");
  if (forbiddenAdvicePattern.test(text)) fail("investment-advice", name, "Do not write direct equity investment recommendations.");
  if (!sourcePattern.test(text)) fail("missing-source", name, "Research run needs official source notes or source URLs.");
  if (!scopePattern.test(text)) fail("missing-scope", name, "Research run needs ticker, market, base date, or lookup date.");
  if (!disclosurePattern.test(text)) fail("missing-disclosure-check", name, "Research run needs disclosure check notes.");
  if (!riskPattern.test(text)) fail("missing-risk", name, "Research run needs risk, counter-scenario, limits, or non-advice note.");
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
