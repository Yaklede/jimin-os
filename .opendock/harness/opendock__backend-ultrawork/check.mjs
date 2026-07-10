#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const maxTextFileBytes = 1024 * 1024;
const readFailures = [];
const traversalFailures = [];
const maxWalkEntries = 20000;
const maxWalkDepth = 32;
const config = {
  "title": "Backend Ultrawork",
  "focus": "backend API and service quality gates",
  "patterns": [
    {
      "id": "trailing-whitespace",
      "globs": [
        "**/*"
      ],
      "pattern": "[ \\t]+$",
      "message": "Remove trailing whitespace."
    },
    {
      "id": "tab-indentation",
      "globs": [
        "**/*.md",
        "**/*.ts",
        "**/*.tsx",
        "**/*.js",
        "**/*.jsx",
        "**/*.css",
        "**/*.scss",
        "**/*.kt",
        "**/*.java",
        "**/*.yml",
        "**/*.yaml",
        "**/*.sql",
        "**/*.tf"
      ],
      "pattern": "^\\t+",
      "message": "Use spaces for indentation unless the project explicitly requires tabs."
    },
    {
      "id": "hardcoded-secret",
      "globs": [
        "**/*.ts",
        "**/*.js",
        "**/*.kt",
        "**/*.java",
        "**/*.yml",
        "**/*.yaml",
        "**/*.properties"
      ],
      "pattern": "(?i)(password|secret|token|api[_-]?key)\\s*[:=]\\s*[\"\\']?[^\"\\'\\n]{8,}",
      "message": "Hardcoded secret-like value."
    },
    {
      "id": "log-sensitive-data",
      "globs": [
        "**/*.ts",
        "**/*.js",
        "**/*.kt",
        "**/*.java"
      ],
      "pattern": "(?i)(console\\.log|logger\\.|log\\.)[^\\n]*(password|secret|token|ssn|email)",
      "message": "Logs should not include sensitive data."
    },
    {
      "id": "sql-concat",
      "globs": [
        "**/*.ts",
        "**/*.js",
        "**/*.kt",
        "**/*.java"
      ],
      "pattern": "(?i)(select|insert|update|delete)[^\\n]*(\\+|\\$\\{)",
      "message": "SQL string interpolation or concatenation needs review."
    },
    {
      "id": "request-body-without-validation",
      "globs": [
        "**/*.ts",
        "**/*.js",
        "**/*.kt",
        "**/*.java"
      ],
      "pattern": "(?i)(req\\.body|@RequestBody)(?![\\s\\S]{0,200}(validate|schema|@Valid|zod|joi|class-validator))",
      "message": "Request bodies need validation."
    },
    {
      "id": "unguarded-admin-route",
      "globs": [
        "**/*.ts",
        "**/*.js",
        "**/*.kt",
        "**/*.java"
      ],
      "pattern": "(?i)(admin|internal|private)[^\\n]*(route|controller|endpoint)(?![\\s\\S]{0,300}(auth|guard|authorize|permission|role))",
      "message": "Sensitive routes need auth or permission guards."
    },
    {
      "id": "pii-logging",
      "globs": [
        "**/*.ts",
        "**/*.js",
        "**/*.kt",
        "**/*.java"
      ],
      "pattern": "(?i)(logger|console\\.log|log\\.)[^\\n]*(email|phone|address|ssn|resident|passport)",
      "message": "PII must not be logged."
    }
  ],
  "packageScripts": [
    "lint",
    "test",
    "build"
  ]
};
const ignoredSegments = new Set([".git", "node_modules", ".opendock", ".agents", ".claude", ".codex", ".cursor", "dist", "build", "coverage", ".next", ".turbo", ".gradle", "target", ".venv", "venv"]);
const ignoredRootFiles = new Set(["AGENTS.md", "CLAUDE.md", "GEMINI.md", "HARNESS.md", "README.md"]);
const textExtensions = new Set([".md", ".mdx", ".txt", ".json", ".yml", ".yaml", ".toml", ".js", ".jsx", ".ts", ".tsx", ".css", ".scss", ".html", ".kt", ".kts", ".java", ".sql", ".sh", ".ps1", ".plist", ".xml", ".tf", ".tfvars", ".dart", ".properties", ".py", ".dbt", ""]);

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

function normalize(file) {
  return path.relative(root, file).split(path.sep).join("/");
}

function globToRegExp(glob) {
  let out = "^";
  for (let i = 0; i < glob.length; i += 1) {
    const ch = glob[i];
    const next = glob[i + 1];
    if (ch === "*" && next === "*") { out += ".*"; i += 1; }
    else if (ch === "*") out += "[^/]*";
    else if (ch === ".") out += "\\.";
    else if ("+?^${}()|[]\\".includes(ch)) out += `\\${ch}`;
    else out += ch;
  }
  return new RegExp(`${out}$`);
}

function matchesAny(file, globs) {
  return globs.some((glob) => {
    if (glob === "**/*") return true;
    if (glob.startsWith("**/")) {
      const tail = glob.slice(3);
      if (!tail.includes("*")) return file === tail || file.endsWith(`/${tail}`);
      return globToRegExp(glob).test(file) || globToRegExp(tail).test(file);
    }
    if (glob.startsWith("**/*.")) return file.endsWith(glob.slice(4));
    if (!glob.includes("*")) return file === glob || file.endsWith(`/${glob}`);
    return globToRegExp(glob).test(file);
  });
}

function readText(file) {
  const ext = path.extname(file);
  const base = path.basename(file);
  if (!textExtensions.has(ext) && !["Dockerfile", "Makefile", "AndroidManifest.xml", "gradlew"].includes(base)) return null;
  try {
    const stats = fs.statSync(file);
    if (stats.size > maxTextFileBytes) {
      readFailures.push({
        rule: "file-too-large",
        file: path.relative(root, file).split(path.sep).join("/"),
        detail: `File exceeds ${maxTextFileBytes} bytes and was not scanned.`
      });
      return null;
    }

    const buffer = fs.readFileSync(file);
    if (buffer.includes(0)) return null;
    return buffer.toString("utf8");
  } catch { return null; }
}

function compilePattern(pattern) {
  let source = pattern;
  let flags = "m";
  while (source.startsWith("(?i)") || source.startsWith("(?s)")) {
    if (source.startsWith("(?i)")) { flags += "i"; source = source.slice(4); }
    if (source.startsWith("(?s)")) { flags += "s"; source = source.slice(4); }
  }
  return new RegExp(source, flags);
}

function readPackageJson() {
  const file = path.join(root, "package.json");
  if (!fs.existsSync(file)) return null;
  try { return JSON.parse(fs.readFileSync(file, "utf8")); }
  catch { return { scripts: {} }; }
}

function hasGradleProject(files) {
  return fs.existsSync(path.join(root, "build.gradle")) || fs.existsSync(path.join(root, "build.gradle.kts")) || files.some((file) => file.rel.endsWith("/build.gradle") || file.rel.endsWith("/build.gradle.kts"));
}

function hasOpeningCodeFenceWithoutLanguage(text) {
  let inFence = false;
  for (const line of text.split(/\r?\n/)) {
    if (!line.startsWith("```")) continue;
    const marker = line.trim();
    if (!inFence) {
      if (marker === "```") return true;
      inFence = true;
    } else if (marker === "```") {
      inFence = false;
    }
  }
  return false;
}

function escapeTerminal(value) {
  return String(value).replace(/[\x00-\x1f\x7f-\x9f]/g, (char) => {
    const code = char.charCodeAt(0).toString(16).padStart(2, "0");
    return `\\x${code}`;
  });
}

function run() {
  const files = walk(root).map((full) => ({ full, rel: normalize(full), text: readText(full) })).filter((item) => item.text !== null);
  const failures = [];
  failures.push(...readFailures, ...traversalFailures);

  for (const rule of config.patterns) {
    if (rule.id === "missing-code-fence-language") {
      for (const file of files) {
        if (!matchesAny(file.rel, rule.globs)) continue;
        if (hasOpeningCodeFenceWithoutLanguage(file.text)) failures.push({ rule: rule.id, file: file.rel, detail: rule.message });
      }
      continue;
    }
    const re = compilePattern(rule.pattern);
    for (const file of files) {
      if (!matchesAny(file.rel, rule.globs)) continue;
      if (re.test(file.text)) failures.push({ rule: rule.id, file: file.rel, detail: rule.message });
    }
  }

  const pkg = readPackageJson();
  if (pkg && config.packageScripts?.length) {
    const scripts = pkg.scripts || {};
    for (const script of config.packageScripts) {
      if (!scripts[script]) failures.push({ rule: "missing-package-script", file: "package.json", detail: `Missing package script: ${script}` });
    }
  }

  if (hasGradleProject(files)) {
    for (const required of config.gradleRequired || []) {
      if (!fs.existsSync(path.join(root, required))) failures.push({ rule: "missing-gradle-file", file: required, detail: `Gradle project should include ${required}` });
    }
    const gradleText = files.filter((file) => file.rel.endsWith("build.gradle") || file.rel.endsWith("build.gradle.kts")).map((file) => file.text).join("\n");
    for (const task of config.gradleTasks || []) {
      if (!gradleText.includes(task)) failures.push({ rule: "missing-gradle-task", file: "build.gradle(.kts)", detail: `Expected Gradle task or plugin reference: ${task}` });
    }
  }

  if (failures.length > 0) {
    console.error(`OpenDock harness: ${config.title}`);
    console.error(`Focus: ${config.focus}`);
    console.error(`Files scanned: ${files.length}`);
    console.error(`Failures: ${failures.length}`);
    for (const failure of failures.slice(0, 120)) console.error(`- [${escapeTerminal(failure.rule)}] ${escapeTerminal(failure.file)}: ${escapeTerminal(failure.detail)}`);
    if (failures.length > 120) console.error(`... ${failures.length - 120} more failures omitted`);
    process.exit(1);
  }
  console.log(`OpenDock harness: ${config.title}`);
  console.log(`Focus: ${config.focus}`);
  console.log(`Files scanned: ${files.length}`);
  console.log("Ultrawork passed.");
}

run();
