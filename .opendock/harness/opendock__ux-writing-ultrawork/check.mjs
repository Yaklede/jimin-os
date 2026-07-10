#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const title = "UX Writing Ultrawork";
const focus = "Korean and English UX writing, terminology, naming, and user-facing copy quality";
const runRoot = ".opendock/runs/ux-writing";
const maxTextFileBytes = 1024 * 1024;
const readFailures = [];
const traversalFailures = [];
const maxWalkEntries = 20000;
const maxWalkDepth = 32;

const ignoredSegments = new Set([
  ".git",
  "node_modules",
  ".opendock",
  ".agents",
  ".claude",
  ".codex",
  ".cursor",
  "dist",
  "build",
  "coverage",
  ".next",
  ".turbo",
  ".gradle",
  "target",
  ".venv",
  "venv",
]);
const ignoredRootFiles = new Set(["AGENTS.md", "CLAUDE.md", "GEMINI.md", "HARNESS.md", "README.md", "WRITING.md", "TERMS.md"]);
const textExtensions = new Set([
  ".md",
  ".mdx",
  ".txt",
  ".json",
  ".yml",
  ".yaml",
  ".toml",
  ".js",
  ".jsx",
  ".ts",
  ".tsx",
  ".css",
  ".scss",
  ".html",
  ".vue",
  ".svelte",
  ".sh",
  ".ps1",
  ".xml",
  ".svg",
  "",
]);
const targetExtensions = new Set([
  ".md",
  ".mdx",
  ".txt",
  ".json",
  ".yml",
  ".yaml",
  ".toml",
  ".js",
  ".jsx",
  ".ts",
  ".tsx",
  ".html",
  ".vue",
  ".svelte",
  ".css",
  ".scss",
]);
const activeStatuses = new Set(["active", "review", "ready", "ready-for-review", "handoff"]);
const inactiveStatuses = new Set(["draft", "none", "paused", "backlog"]);

const defaultDeveloperTerms = [
  "payload",
  "endpoint",
  "schema",
  "token",
  "null",
  "undefined",
  "forbidden",
  "permission denied",
  "bad request",
  "internal server error",
  "stack trace",
  "namespace",
  "tenant",
  "webhook",
];

const koreanDeveloperTerms = [
  "인증 토큰",
  "토큰이 만료",
  "유효하지 않은 payload",
  "페이로드",
  "엔드포인트",
  "스키마",
  "밸리데이션",
  "검증 실패",
  "요청 실패",
  "권한 없음",
  "서버 에러",
  "서버 오류",
];

const koreanRecoveryWords = ["다시", "확인", "시도", "입력", "로그인", "문의", "새로고침", "기다", "선택", "수정"];
const englishRecoveryWords = ["try", "check", "sign in", "log in", "contact", "refresh", "update", "choose", "enter", "fix", "wait"];
const errorSignals = [
  "error",
  "failed",
  "failure",
  "invalid",
  "denied",
  "unavailable",
  "오류",
  "에러",
  "실패",
  "유효하지",
  "권한",
  "문제",
  "만료",
];

function resolve(rel) {
  return path.join(root, rel);
}

function exists(rel) {
  return fs.existsSync(resolve(rel));
}

function normalize(file) {
  return path.relative(root, file).split(path.sep).join("/");
}

function escapeTerminal(value) {
  return String(value)
    .replace(/\x1B\[[0-?]*[ -/]*[@-~]/g, "")
    .replace(/[\r\n\t]/g, " ");
}

function recordTraversalFailure(rule, file, detail) {
  if (traversalFailures.some((failure) => failure.rule === rule && failure.file === file)) return;
  traversalFailures.push({ rule, file, detail });
}

function walk(dir, depth = 0, state = { entries: 0, stopped: false }) {
  const entries = [];
  if (state.stopped || !fs.existsSync(dir)) return entries;
  if (depth > maxWalkDepth) {
    recordTraversalFailure("walk-depth-budget", normalize(dir), `Directory traversal exceeded ${maxWalkDepth} levels.`);
    state.stopped = true;
    return entries;
  }
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (ignoredSegments.has(entry.name)) continue;
    const full = path.join(dir, entry.name);
    state.entries += 1;
    if (state.entries > maxWalkEntries) {
      recordTraversalFailure("walk-entry-budget", normalize(full), `Directory traversal exceeded ${maxWalkEntries} entries.`);
      state.stopped = true;
      return entries;
    }
    if (entry.isDirectory()) entries.push(...walk(full, depth + 1, state));
    else if (entry.isFile() && !(dir === root && ignoredRootFiles.has(entry.name))) entries.push(full);
    if (state.stopped) break;
  }
  return entries;
}

function safeRelativePath(value) {
  const trimmed = String(value).trim().replace(/^["'`]+|["'`.,)]+$/g, "");
  if (!trimmed || trimmed.includes("://") || path.isAbsolute(trimmed)) return null;
  const normalized = path.normalize(trimmed).split(path.sep).join("/");
  if (normalized.startsWith("../") || normalized === "..") return null;
  if (normalized.split("/").some((segment) => ignoredSegments.has(segment))) return null;
  return normalized;
}

function isTargetLike(rel) {
  return targetExtensions.has(path.extname(rel).toLowerCase());
}

function parseField(text, name) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = text.match(new RegExp(`^${escaped}\\s*:\\s*(.+)$`, "im"));
  return match ? match[1].trim() : "";
}

function extractTargetPaths(text) {
  const targets = new Set();
  const addCandidate = (candidate) => {
    const rel = safeRelativePath(candidate);
    if (rel && isTargetLike(rel)) targets.add(rel);
  };

  const lines = text.split(/\r?\n/);
  let inTargetFiles = false;
  for (const line of lines) {
    if (/^#{1,6}\s+Target Files\s*$/i.test(line.trim())) {
      inTargetFiles = true;
      continue;
    }
    if (inTargetFiles && /^#{1,6}\s+/.test(line.trim())) break;
    if (!inTargetFiles) continue;
    for (const match of line.matchAll(/`([^`]+)`/g)) addCandidate(match[1]);
    if (!/^\s*[-*]\s+/.test(line) && !/\b(Target|Output|Changed|File|Path)s?\b/i.test(line)) continue;
    for (const match of line.matchAll(/[A-Za-z0-9._@+/-]+\.(?:mdx?|txt|json|ya?ml|toml|jsx?|tsx?|html|vue|svelte|css|scss)\b/gi)) {
      addCandidate(match[0]);
    }
  }

  return [...targets];
}

function fileMtime(rel) {
  try {
    return fs.statSync(resolve(rel)).mtimeMs;
  } catch {
    return 0;
  }
}

function findRunDocuments() {
  const docs = [];
  const fullRunRoot = resolve(runRoot);
  if (!fs.existsSync(fullRunRoot)) return docs;

  for (const entry of fs.readdirSync(fullRunRoot, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const runRel = `${runRoot}/${entry.name}`;
    const manifestRel = `${runRel}/manifest.md`;
    if (!exists(manifestRel)) continue;
    const text = fs.readFileSync(resolve(manifestRel), "utf8");
    docs.push({
      label: runRel,
      manifestRel,
      manifestText: text,
      status: parseField(text, "Status").toLowerCase(),
      targets: extractTargetPaths(text),
      mtime: fileMtime(manifestRel),
    });
  }

  return docs.sort((a, b) => b.mtime - a.mtime || b.label.localeCompare(a.label));
}

function resolveTargetScope(failures) {
  const argvTargets = process.argv.slice(2).map(safeRelativePath).filter(Boolean).filter(isTargetLike);
  if (argvTargets.length > 0) {
    return { label: "argv", active: true, targets: [...new Set(argvTargets)] };
  }

  const runs = findRunDocuments();
  if (runs.length === 0) return { label: "none", active: false, targets: [] };

  const activeRun = runs.find((run) => activeStatuses.has(run.status)) ?? runs.find((run) => run.targets.length > 0) ?? runs[0];
  const active = activeStatuses.has(activeRun.status) || (!inactiveStatuses.has(activeRun.status) && activeRun.targets.length > 0);
  if (!active && activeRun.targets.length === 0) {
    return { label: activeRun.label, active: false, targets: [] };
  }
  if (activeRun.targets.length === 0) {
    failures.push({
      rule: "missing-target-files",
      file: activeRun.manifestRel,
      detail: "Active UX writing run must list target files to validate.",
    });
  }
  return { label: activeRun.label, active: true, targets: activeRun.targets };
}

function readText(file) {
  const ext = path.extname(file);
  const base = path.basename(file);
  if (!textExtensions.has(ext) && !["Dockerfile", "Makefile"].includes(base)) return null;
  try {
    const stats = fs.statSync(file);
    if (stats.size > maxTextFileBytes) {
      readFailures.push({
        rule: "file-too-large",
        file: normalize(file),
        detail: `File exceeds ${maxTextFileBytes} bytes and was not scanned.`,
      });
      return null;
    }
    const buffer = fs.readFileSync(file);
    if (buffer.includes(0)) return null;
    return buffer.toString("utf8");
  } catch {
    return null;
  }
}

function readManagedDoc(rel) {
  const full = resolve(rel);
  if (!fs.existsSync(full)) return { exists: false, text: "", lower: "" };
  if (fs.statSync(full).size > maxTextFileBytes) return { exists: true, tooLarge: true, text: "", lower: "" };
  const text = fs.readFileSync(full, "utf8");
  return { exists: true, text, lower: text.toLowerCase() };
}

function lineIsInstructionOrExample(line) {
  return /^\s*(#|[-*]\s|>\s|\||```)/.test(line) || /\b(Avoid|Prefer|Examples?|Terms?|Rules?)\b/i.test(line);
}

function extractAllowedTerms(writing, terms) {
  const combined = `${writing.text}\n${terms.text}`;
  const allowed = new Set(["api"]);
  let capture = false;
  for (const rawLine of combined.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (/^#{1,6}\s+Allowed Developer Terms/i.test(line)) {
      capture = true;
      continue;
    }
    if (capture && /^#{1,6}\s+/.test(line)) break;
    if (capture) {
      const item = line.match(/^[-*]\s+(.+)$/);
      if (item) allowed.add(item[1].trim().toLowerCase());
    }
  }
  return allowed;
}

function extractAvoidTerms(terms) {
  const avoid = new Set([...defaultDeveloperTerms, ...koreanDeveloperTerms]);
  if (!terms.exists) return avoid;

  let inAvoid = false;
  for (const rawLine of terms.text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (/^#{1,6}\s+Allowed Developer Terms/i.test(line)) {
      inAvoid = false;
      continue;
    }
    if (/^#{1,6}\s+(Avoid|피해야 할|금지)/i.test(line)) {
      inAvoid = true;
      continue;
    }
    if (inAvoid && /^#{1,6}\s+/.test(line)) inAvoid = false;
    if (inAvoid) {
      const item = line.match(/^[-*]\s+(.+)$/);
      if (item) avoid.add(item[1].trim().toLowerCase());
    }
    if (line.includes("|") && !/^[-| :]+$/.test(line) && /avoid/i.test(line)) continue;
    if (line.includes("|") && !/^[-| :]+$/.test(line) && !/^Concept\s*\|/i.test(line)) {
      const cols = line.split("|").map((part) => part.trim()).filter(Boolean);
      const maybeAvoid = cols[cols.length - 1];
      if (maybeAvoid && !/^avoid$/i.test(maybeAvoid)) {
        for (const term of maybeAvoid.split(",")) {
          const value = term.trim();
          if (value) avoid.add(value.toLowerCase());
        }
      }
    }
  }
  return avoid;
}

function inferKoreanEnding(writing) {
  const ending = parseField(writing.text, "Default Korean ending").toLowerCase();
  if (ending.includes("합니다")) return "formal";
  if (ending.includes("해요")) return "haeyo";
  if (writing.lower.includes("합니다체")) return "formal";
  if (writing.lower.includes("해요체")) return "haeyo";
  return "haeyo";
}

function inferEnglishStyle(writing) {
  return {
    sentenceCase: !/all caps|uppercase/i.test(writing.text),
    concise: !/long form|editorial/i.test(writing.text),
  };
}

function push(failures, rule, file, detail) {
  failures.push({ rule, file, detail });
}

function targetLines(file) {
  return file.text
    .split(/\r?\n/)
    .map((line, index) => ({ text: line, index: index + 1 }))
    .filter(({ text }) => text.trim() && !/^\s*(import|export|const|let|var|function|class|type|interface)\b/.test(text));
}

function hasKorean(text) {
  return /[가-힣]/.test(text);
}

function hasEnglishWords(text) {
  return /[A-Za-z]{3,}/.test(text);
}

function includesAny(lower, words) {
  return words.some((word) => lower.includes(word.toLowerCase()));
}

function checkAvoidTerms(files, allowedTerms, avoidTerms, failures) {
  for (const file of files) {
    for (const { text, index } of targetLines(file)) {
      if (lineIsInstructionOrExample(text)) continue;
      const lower = text.toLowerCase();
      for (const term of avoidTerms) {
        if (!term || allowedTerms.has(term)) continue;
        if (term.length < 3) continue;
        const isAscii = /^[a-z0-9 _-]+$/i.test(term);
        const found = isAscii
          ? new RegExp(`(^|[^a-z0-9])${term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}([^a-z0-9]|$)`, "i").test(lower)
          : lower.includes(term.toLowerCase());
        if (found) {
          push(failures, "avoid-term", `${file.rel}:${index}`, `Avoid user-facing internal term "${term}". Prefer the public term from TERMS.md or WRITING.md.`);
        }
      }
    }
  }
}

function checkPlaceholders(files, failures) {
  const placeholderRe = /\b(TODO|FIXME|Lorem ipsum|lorem|임시|테스트 문구|샘플 문구|sample text|placeholder)\b/i;
  for (const file of files) {
    for (const { text, index } of targetLines(file)) {
      if (placeholderRe.test(text)) {
        push(failures, "placeholder-copy", `${file.rel}:${index}`, "Remove placeholder copy before handoff.");
      }
    }
  }
}

function checkKoreanTone(files, writing, failures) {
  const ending = inferKoreanEnding(writing);
  for (const file of files) {
    const koLines = targetLines(file).filter(({ text }) => hasKorean(text) && !lineIsInstructionOrExample(text));
    const hasFormal = koLines.some(({ text }) => /(합니다|됩니다|되었습니다|필요합니다|바랍니다)[.!?。"]?\s*$/.test(text.trim()));
    const hasHaeyo = koLines.some(({ text }) => /(해요|세요|돼요|예요|이에요|주세요|있어요|없어요)[.!?。"]?\s*$/.test(text.trim()));
    if (hasFormal && hasHaeyo) {
      push(failures, "mixed-korean-ending", file.rel, "Do not mix 합니다체 and 해요체 in the same user-facing scope unless WRITING.md explicitly allows it.");
    }
    if (ending === "haeyo") {
      for (const { text, index } of koLines) {
        if (/(되었습니다|됩니다|처리되었습니다|요청되었습니다|실행됩니다|필요합니다)[.!?。"]?\s*$/.test(text.trim())) {
          push(failures, "korean-passive-formal", `${file.rel}:${index}`, "Default Korean tone is 해요체. Prefer active, plain wording unless WRITING.md allows formal/system wording.");
        }
      }
    }
  }
}

function checkEnglishStyle(files, writing, failures) {
  const style = inferEnglishStyle(writing);
  for (const file of files) {
    for (const { text, index } of targetLines(file)) {
      if (!hasEnglishWords(text) || lineIsInstructionOrExample(text)) continue;
      const cleaned = text.replace(/[`"'{}[\](),:;<>/=._-]/g, " ").trim();
      if (!cleaned) continue;
      const words = cleaned.split(/\s+/).filter(Boolean);
      const alphaWords = words.filter((word) => /[A-Za-z]/.test(word));
      if (style.concise && alphaWords.length > 22 && /(?:button|label|toast|error|title|cta|message|copy|text)/i.test(text)) {
        push(failures, "long-english-ui-copy", `${file.rel}:${index}`, "English UI copy should be short and scannable.");
      }
      if (style.sentenceCase && alphaWords.length >= 2) {
        const upperWords = alphaWords.filter((word) => /^[A-Z]{2,}$/.test(word) && !["API", "URL", "ID", "UI", "UX"].includes(word));
        if (upperWords.length > 0) {
          push(failures, "english-all-caps", `${file.rel}:${index}`, "Avoid all-caps UI copy unless WRITING.md explicitly allows it.");
        }
      }
    }
  }
}

function checkErrorsHaveRecovery(files, failures) {
  for (const file of files) {
    for (const { text, index } of targetLines(file)) {
      const lower = text.toLowerCase();
      if (!includesAny(lower, errorSignals)) continue;
      if (lineIsInstructionOrExample(text)) continue;
      const hasRecovery = includesAny(lower, englishRecoveryWords) || koreanRecoveryWords.some((word) => text.includes(word));
      if (!hasRecovery) {
        push(failures, "missing-recovery-action", `${file.rel}:${index}`, "Error copy should include what the user can do next.");
      }
    }
  }
}

function checkButtonsAndCtas(files, failures) {
  const nounishKorean = /(확인|취소|삭제|저장|생성|수정|설정|관리|전송|완료)$/;
  const weakEnglish = /^(submit|confirm|cancel|ok|manage|process)$/i;
  const buttonPatterns = [
    /<button[^>]*>([^<]{1,80})<\/button>/gi,
    /\b(?:button|cta|label|title|aria-label|text)\s*[:=]\s*["'`]([^"'`]{1,80})["'`]/gi,
  ];

  for (const file of files) {
    for (const pattern of buttonPatterns) {
      for (const match of file.text.matchAll(pattern)) {
        const label = match[1].trim();
        if (!label || lineIsInstructionOrExample(label)) continue;
        const approxLine = file.text.slice(0, match.index ?? 0).split(/\r?\n/).length;
        if (hasKorean(label) && nounishKorean.test(label) && !/(하기|하기|해요|세요|주세요|하기|하기)$/.test(label)) {
          push(failures, "noun-button-label", `${file.rel}:${approxLine}`, `Button/CTA "${label}" should describe the user's action, not only a noun.`);
        }
        if (!hasKorean(label) && weakEnglish.test(label)) {
          push(failures, "weak-english-button", `${file.rel}:${approxLine}`, `Button/CTA "${label}" is vague. Prefer a specific user action.`);
        }
      }
    }
  }
}

function checkNaming(files, failures) {
  const internalNameRe = /\b(?:admin|manager|dashboard|resource|entity|object|service|controller|handler|util|helper|crud|namespace|tenant)(?:[-_ ](?:tool|page|screen|menu|module|feature))?\b/i;
  const nameSignals = /\b(?:feature name|menu name|plan name|product name|service name|기능명|메뉴명|플랜명|서비스명|상품명|이름|작명)\b/i;
  for (const file of files) {
    for (const { text, index } of targetLines(file)) {
      if (!nameSignals.test(text)) continue;
      if (internalNameRe.test(text)) {
        push(failures, "internal-naming", `${file.rel}:${index}`, "Naming should fit the product concept and avoid internal implementation labels.");
      }
    }
  }
}

function runChecks() {
  const failures = [];
  const writing = readManagedDoc("WRITING.md");
  const terms = readManagedDoc("TERMS.md");
  const scope = resolveTargetScope(failures);

  if (!writing.exists) push(failures, "missing-writing-contract", "WRITING.md", "WRITING.md is required before UX writing output can be validated.");
  if (writing.tooLarge) push(failures, "file-too-large", "WRITING.md", `WRITING.md exceeds ${maxTextFileBytes} bytes and was not scanned.`);
  if (!terms.exists) push(failures, "missing-terms", "TERMS.md", "TERMS.md is required for terminology checks.");
  if (terms.tooLarge) push(failures, "file-too-large", "TERMS.md", `TERMS.md exceeds ${maxTextFileBytes} bytes and was not scanned.`);

  const files = scope.targets
    .map((rel) => {
      const full = resolve(rel);
      if (!fs.existsSync(full)) {
        push(failures, "missing-target-file", rel, "Target file listed in the UX writing run does not exist.");
        return null;
      }
      return { full, rel, text: readText(full) };
    })
    .filter((item) => item !== null)
    .filter((item) => item.text !== null);
  failures.push(...readFailures, ...traversalFailures);

  if (!scope.active && scope.targets.length === 0) {
    return { filesScanned: 0, failures, writing, terms, scope };
  }

  const allowedTerms = extractAllowedTerms(writing, terms);
  const avoidTerms = extractAvoidTerms(terms);
  checkAvoidTerms(files, allowedTerms, avoidTerms, failures);
  checkPlaceholders(files, failures);
  checkKoreanTone(files, writing, failures);
  checkEnglishStyle(files, writing, failures);
  checkErrorsHaveRecovery(files, failures);
  checkButtonsAndCtas(files, failures);
  checkNaming(files, failures);

  return { filesScanned: files.length, failures, writing, terms, scope };
}

function printResult(result) {
  if (result.failures.length > 0) {
    console.error(`OpenDock harness: ${title}`);
    console.error(`Focus: ${focus}`);
    console.error(`Scope: ${result.scope.label}`);
    console.error(`WRITING.md: ${result.writing.exists ? "loaded" : "missing"}`);
    console.error(`TERMS.md: ${result.terms.exists ? "loaded" : "missing"}`);
    console.error(`Files scanned: ${result.filesScanned}`);
    console.error(`Failures: ${result.failures.length}`);
    for (const failure of result.failures.slice(0, 160)) {
      console.error(`- [${escapeTerminal(failure.rule)}] ${escapeTerminal(failure.file)}: ${escapeTerminal(failure.detail)}`);
    }
    if (result.failures.length > 160) console.error(`... ${result.failures.length - 160} more failures omitted`);
    process.exit(1);
  }
  console.log(`OpenDock harness: ${title}`);
  console.log(`Focus: ${focus}`);
  console.log(`Scope: ${result.scope.label}`);
  console.log(`WRITING.md: ${result.writing.exists ? "loaded" : "missing"}`);
  console.log(`TERMS.md: ${result.terms.exists ? "loaded" : "missing"}`);
  console.log(`Files scanned: ${result.filesScanned}`);
  if (!result.scope.active && result.scope.targets.length === 0) console.log("No active UX writing run detected.");
  console.log("Ultrawork passed.");
}

printResult(runChecks());
