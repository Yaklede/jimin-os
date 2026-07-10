#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const runRoot = ".opendock/runs/design";
const maxTextFileBytes = 1024 * 1024;
const readFailures = [];
const traversalFailures = [];
const maxWalkEntries = 20000;
const maxWalkDepth = 32;
const mode = "design";
const title = mode === "figma" ? "Figma Ultrawork" : "Design Ultrawork";
const focus =
  mode === "figma"
    ? "Figma canvas quality, DESIGN.md contract alignment, and handoff readiness"
    : "design implementation quality, DESIGN.md contract alignment, and UI handoff readiness";

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
const ignoredRootFiles = new Set(["AGENTS.md", "CLAUDE.md", "GEMINI.md", "HARNESS.md", "README.md", "DESIGN.md"]);
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
  ".sh",
  ".ps1",
  ".xml",
  ".svg",
  "",
]);
const activeStatuses = new Set(["active", "review", "ready", "ready-for-review", "handoff"]);
const inactiveStatuses = new Set(["draft", "none", "paused", "backlog"]);
const targetExtensions = new Set([
  ".css",
  ".scss",
  ".html",
  ".js",
  ".jsx",
  ".ts",
  ".tsx",
  ".svg",
  ".md",
  ".mdx",
  ".json",
  ".yml",
  ".yaml",
  ".png",
  ".jpg",
  ".jpeg",
  ".webp",
  ".avif",
]);

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

function normalizePath(file) {
  return path.relative(root, file).split(path.sep).join("/");
}

function resolve(rel) {
  return path.join(root, rel);
}

function exists(rel) {
  return fs.existsSync(resolve(rel));
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

function hasMeaningfulField(text, name) {
  const value = parseField(text, name)
    .replace(/^[-_*`"']+|[-_*`"'.]+$/g, "")
    .trim();
  return value.length > 0 && !/^(todo|tbd|n\/a|none|미정|없음)$/i.test(value);
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
    for (const match of line.matchAll(/[A-Za-z0-9._@+/-]+\.(?:css|scss|html|js|jsx|ts|tsx|svg|md|mdx|json|ya?ml|png|jpe?g|webp|avif)\b/gi)) {
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
    return { label: activeRun.label, active: false, targets: [], manifestRel: activeRun.manifestRel, manifestText: activeRun.manifestText };
  }
  if (activeRun.targets.length === 0) {
    failures.push({
      rule: "missing-target-files",
      file: activeRun.manifestRel,
      detail: "Active design run must list target files to validate.",
    });
  }
  return {
    label: activeRun.label,
    active: true,
    targets: activeRun.targets,
    manifestRel: activeRun.manifestRel,
    manifestText: activeRun.manifestText,
  };
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
        file: path.relative(root, file).split(path.sep).join("/"),
        detail: `File exceeds ${maxTextFileBytes} bytes and was not scanned.`
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

function normalizeHex(hex) {
  let value = hex.toLowerCase();
  if (value.length === 4) value = `#${value[1]}${value[1]}${value[2]}${value[2]}${value[3]}${value[3]}`;
  if (value.length === 5) {
    value = `#${value[1]}${value[1]}${value[2]}${value[2]}${value[3]}${value[3]}${value[4]}${value[4]}`;
  }
  return value;
}

function compactLength(value) {
  return String(value).toLowerCase().replace(/\s+/g, "");
}

function readDesignContract() {
  const file = path.join(root, "DESIGN.md");
  if (!fs.existsSync(file)) {
    return {
      exists: false,
      file: "DESIGN.md",
      text: "",
      lower: "",
      colors: new Set(),
      accentColors: new Set(),
      lengths: new Set(),
      allowsNegativeLetterSpacing: false,
      maxFontWeight: null,
      forbidPureWhiteBackground: false,
      forbidPillCtas: false,
      requirePillButtons: false,
      forbidAccentButtons: false,
      forbidDropShadows: false,
    };
  }

  if (fs.statSync(file).size > maxTextFileBytes) {
    return {
      exists: true,
      file: "DESIGN.md",
      text: "",
      lower: "",
      colors: new Set(),
      accentColors: new Set(),
      lengths: new Set(),
      allowsNegativeLetterSpacing: false,
      maxFontWeight: null,
      forbidPureWhiteBackground: false,
      forbidPillCtas: false,
      requirePillButtons: false,
      forbidAccentButtons: false,
      forbidDropShadows: false,
      tooLarge: true,
    };
  }

  const text = fs.readFileSync(file, "utf8");
  const lower = text.toLowerCase();
  const colors = new Set();
  const accentColors = new Set();
  for (const match of text.matchAll(/#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/g)) {
    colors.add(normalizeHex(match[0]));
  }
  for (const line of text.split(/\r?\n/)) {
    for (const match of line.matchAll(/\{colors\.accent[^}]*\}[^#]*(#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b)/g)) {
      accentColors.add(normalizeHex(match[1]));
    }
    if (!/^\s*-\s*\*\*Accent\b|^\s*-\s*Accent\b/.test(line)) continue;
    for (const match of line.matchAll(/#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/g)) {
      accentColors.add(normalizeHex(match[0]));
    }
  }

  const lengths = new Set();
  for (const match of text.matchAll(/-?\d+(?:\.\d+)?\s*(?:px|rem|em|%)/gi)) {
    lengths.add(compactLength(match[0]));
  }

  let maxFontWeight = null;
  if (
    /weight ceiling (?:is |at )?600/.test(lower) ||
    /600 (?:is|as) the maximum weight/.test(lower) ||
    /maximum weight (?:is |at )?600/.test(lower) ||
    /maximum weight in the system/.test(lower) ||
    /don't use weight 700/.test(lower) ||
    /never uses? 700/.test(lower) ||
    /never uses? 700\+/.test(lower)
  ) {
    maxFontWeight = 600;
  }

  return {
    exists: true,
    file: "DESIGN.md",
    text,
    lower,
    colors,
    accentColors,
    lengths,
    allowsNegativeLetterSpacing: /negative (?:letter-spacing|tracking)|letter-spacing[^.\n]*-\d|tracking[^.\n]*-\d/.test(lower),
    maxFontWeight,
    forbidPureWhiteBackground: /don't use pure white|do not use pure white|not pure white/.test(lower),
    forbidPillCtas: /don't render ctas as pills|do not render ctas as pills|never uses? pill ctas|brand never uses pill ctas/.test(
      lower,
    ),
    requirePillButtons: /all buttons are pill|every cta is a pill|all buttons are pill-shaped|compose every cta as a pill/.test(
      lower,
    ),
    forbidAccentButtons: /accent colors?.*(?:never|not).*button|chromatic accents?.*never.*button|not as button backgrounds|not as button colours|not as button colors/.test(
      lower,
    ),
    forbidDropShadows: /no drop shadows|don't add drop shadows|do not add drop shadows/.test(lower) && !/layered drop/.test(lower),
  };
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

function push(failures, rule, file, detail) {
  failures.push({ rule, file, detail });
}

function checkLayoutPlanning(scope, failures) {
  if (!scope.active || !scope.manifestText || scope.label === "argv") return;
  const requiredFields = [
    ["Layout Type", "Record the screen type before building: ecommerce, blog, portfolio, landing, saas, dashboard, mobile, brand, or component."],
    ["First Gaze", "Record where the user's first attention should go."],
    ["Primary Action", "Record the primary user action or CTA."],
    ["Section Architecture", "Record the planned section order or layout structure."],
    ["Reference Categories", "Record the reference categories reviewed for this task."],
    ["Reference Notes", "Record what pattern was extracted without copying source assets."],
  ];
  for (const [field, detail] of requiredFields) {
    if (!hasMeaningfulField(scope.manifestText, field)) {
      push(failures, "missing-layout-planning", scope.manifestRel ?? "design run manifest", `${field}: ${detail}`);
    }
  }
  const notes = parseField(scope.manifestText, "Reference Notes").toLowerCase();
  if (/\b(copy|clone|screenshot|exact|rip|scrape)\b/.test(notes)) {
    push(
      failures,
      "unsafe-reference-use",
      scope.manifestRel ?? "design run manifest",
      "Reference notes must describe extracted intent/patterns, not copying, cloning, screenshots, or scraping.",
    );
  }
}

function checkPalettePlanning(scope, failures) {
  if (!scope.active || !scope.manifestText || scope.label === "argv") return;
  const requiredFields = [
    ["Palette Source", "Record whether the palette comes from Coolors, Color Hunt, Adobe Color, existing brand, image extraction, or custom exploration."],
    ["Palette Mood", "Record the intended color mood and what mood should be avoided."],
    ["Palette Role Map", "Map colors to canvas, surface, text, border, primary, secondary, focus, and semantic roles."],
    ["Contrast Plan", "Record how text, CTA, disabled state, and focus ring contrast will be checked."],
    ["Color Risks", "Record palette risks such as muddy warmth, extra accents, low contrast, or semantic color confusion."],
  ];
  for (const [field, detail] of requiredFields) {
    if (!hasMeaningfulField(scope.manifestText, field)) {
      push(failures, "missing-palette-planning", scope.manifestRel ?? "design run manifest", `${field}: ${detail}`);
    }
  }

  const source = parseField(scope.manifestText, "Palette Source").toLowerCase();
  if (source) {
    const knownSource = /\b(coolors?|color\s*hunt|adobe|brand|image|photo|custom|existing|design\.md)\b/.test(source);
    if (!knownSource) {
      push(
        failures,
        "unclear-palette-source",
        scope.manifestRel ?? "design run manifest",
        "Palette Source should name a concrete basis such as Coolors, Color Hunt, Adobe Color, existing brand, image extraction, custom exploration, or DESIGN.md.",
      );
    }
  }

  const roleMap = parseField(scope.manifestText, "Palette Role Map").toLowerCase();
  if (roleMap) {
    const roleGroups = [
      /\b(canvas|background|bg)\b/,
      /\b(surface|card|panel|menu)\b/,
      /\b(text|ink|foreground)\b/,
      /\b(border|line|divider)\b/,
      /\b(primary|accent|cta|action)\b/,
      /\b(focus|ring)\b/,
      /\b(success|warning|error|danger|semantic)\b/,
    ];
    const covered = roleGroups.filter((group) => group.test(roleMap)).length;
    if (covered < 5) {
      push(
        failures,
        "incomplete-palette-role-map",
        scope.manifestRel ?? "design run manifest",
        "Palette Role Map should cover at least five roles among canvas/background, surface, text, border, primary/accent, focus, and semantic colors.",
      );
    }
  }

  const contrastPlan = parseField(scope.manifestText, "Contrast Plan").toLowerCase();
  if (contrastPlan) {
    const contrastTargets = [/\b(text|body|copy)\b/, /\b(cta|button|action)\b/, /\b(focus|ring)\b/, /\b(disabled|muted)\b/, /\b(aa|contrast)\b/];
    const covered = contrastTargets.filter((target) => target.test(contrastPlan)).length;
    if (covered < 3) {
      push(
        failures,
        "incomplete-contrast-plan",
        scope.manifestRel ?? "design run manifest",
        "Contrast Plan should mention at least three targets among body text, CTA/button, focus ring, disabled/muted state, and AA/contrast.",
      );
    }
  }

  const risks = parseField(scope.manifestText, "Color Risks").toLowerCase();
  if (risks) {
    const riskTargets = [
      /\b(beige|cream|tan|brown|orange|warm|muddy)\b/,
      /\b(extra|multiple|competing|random).*\b(accent|color|colour)\b|\b(accent|color|colour).*\b(extra|multiple|competing|random)\b/,
      /\b(low|weak|poor).*\b(contrast|readability)\b|\b(contrast|readability).*\b(low|weak|poor)\b/,
      /\b(semantic|success|warning|error|danger)\b/,
    ];
    const covered = riskTargets.filter((target) => target.test(risks)).length;
    if (covered < 2) {
      push(
        failures,
        "incomplete-color-risk-plan",
        scope.manifestRel ?? "design run manifest",
        "Color Risks should mention at least two concrete risks such as muddy warm palettes, extra/competing accents, low contrast, or semantic color confusion.",
      );
    }
  }
}

function checkCreateUiPlanning(scope, failures) {
  if (!scope.active || !scope.manifestText || scope.label === "argv") return;
  const requiredFields = [
    ["Component Inventory", "Record the chosen UI primitives before building, such as Button, Field, Input, Toast, Inline Alert, Modal, Select, Tabs, Badge, or Chip."],
    ["Typography Token Plan", "Record the intended type roles: display, heading, body, paragraph, ui, numeric, or code."],
    ["Spacing Token Plan", "Record which spacing scale is used for layout, section, and component rhythm."],
    ["Radius Token Plan", "Record the radius personality for cards, fields, buttons, chips, overlays, and any pill usage."],
    ["Shadow Token Plan", "Record the elevation/state/text-shadow language or explicitly say no shadow."],
    ["State Coverage", "Record the relevant default, hover, focus, disabled, loading, empty, error, responsive, and reduced-motion states."],
  ];

  for (const [field, detail] of requiredFields) {
    if (!hasMeaningfulField(scope.manifestText, field)) {
      push(failures, "missing-create-ui-planning", scope.manifestRel ?? "design run manifest", `${field}: ${detail}`);
    }
  }

  const inventory = parseField(scope.manifestText, "Component Inventory").toLowerCase();
  if (inventory) {
    const knownComponent = /\b(button|field|label|input|textarea|select|combobox|checkbox|radio|switch|slider|tabs?|tab menu|segmented|navbar|sidebar|breadcrumb|pagination|toast|inline alert|alert|modal|popover|dropdown|tooltip|badge|chip|status|progress|spinner|stepper|avatar|accordion|separator)\b/.test(
      inventory,
    );
    if (!knownComponent) {
      push(
        failures,
        "unclear-component-inventory",
        scope.manifestRel ?? "design run manifest",
        "Component Inventory should name concrete primitives and not only describe visual style.",
      );
    }
  }

  const typography = parseField(scope.manifestText, "Typography Token Plan").toLowerCase();
  if (typography) {
    const typeRoles = [/\bdisplay\b/, /\bheading\b/, /\bbody\b/, /\bparagraph\b/, /\bui\b/, /\bnumeric\b/, /\bcode\b/];
    const covered = typeRoles.filter((role) => role.test(typography)).length;
    if (covered < 2) {
      push(
        failures,
        "incomplete-typography-token-plan",
        scope.manifestRel ?? "design run manifest",
        "Typography Token Plan should mention at least two roles such as heading, paragraph, body, ui, numeric, or code.",
      );
    }
  }

  const spacing = parseField(scope.manifestText, "Spacing Token Plan").toLowerCase();
  if (spacing) {
    const spacingRoles = [/\blayout\b/, /\bsection\b/, /\bcomponent\b/];
    const covered = spacingRoles.filter((role) => role.test(spacing)).length;
    if (covered < 2) {
      push(
        failures,
        "incomplete-spacing-token-plan",
        scope.manifestRel ?? "design run manifest",
        "Spacing Token Plan should distinguish at least two scales among layout, section, and component.",
      );
    }
  }

  const radius = parseField(scope.manifestText, "Radius Token Plan").toLowerCase();
  if (radius) {
    const hasRadiusTarget = /\b(card|surface|field|input|button|chip|tab|modal|popover|menu|overlay|component|pill|full)\b/.test(radius);
    if (!hasRadiusTarget) {
      push(
        failures,
        "unclear-radius-token-plan",
        scope.manifestRel ?? "design run manifest",
        "Radius Token Plan should explain how radius applies to surfaces and controls.",
      );
    }
  }

  const shadow = parseField(scope.manifestText, "Shadow Token Plan").toLowerCase();
  if (shadow) {
    const hasShadowIntent = /\b(elevation|surface|card|popover|modal|dropdown|focus|state|component|text|none|no shadow)\b/.test(shadow);
    if (!hasShadowIntent) {
      push(
        failures,
        "unclear-shadow-token-plan",
        scope.manifestRel ?? "design run manifest",
        "Shadow Token Plan should describe elevation, component state, text legibility, or explicitly say no shadow.",
      );
    }
  }

  const stateCoverage = parseField(scope.manifestText, "State Coverage").toLowerCase();
  if (stateCoverage) {
    const stateRoles = [
      /\bdefault\b/,
      /\bhover\b/,
      /\bfocus|focus-visible\b/,
      /\bdisabled\b/,
      /\bloading|pending\b/,
      /\bempty\b/,
      /\berror|invalid\b/,
      /\bresponsive|mobile\b/,
      /\breduced[- ]motion\b/,
    ];
    const covered = stateRoles.filter((role) => role.test(stateCoverage)).length;
    if (covered < 4) {
      push(
        failures,
        "incomplete-state-coverage",
        scope.manifestRel ?? "design run manifest",
        "State Coverage should cover at least four relevant states such as default, hover, focus, disabled, loading, empty, error, responsive, or reduced motion.",
      );
    }
  }
}

function escapeTerminal(value) {
  return String(value)
    .replace(/\x1B\[[0-?]*[ -/]*[@-~]/g, "")
    .replace(/[\r\n\t]/g, " ");
}

function lengthAllowed(contract, raw) {
  if (!contract.exists) return false;
  return contract.lengths.has(compactLength(raw));
}

function checkCommonTextRules(files, failures) {
  for (const file of files) {
    const isUiFile = /\.(css|scss|tsx|jsx|html|svg)$/.test(file.rel);
    if (/[ \t]+$/m.test(file.text)) push(failures, "trailing-whitespace", file.rel, "Remove trailing whitespace.");
    if (/\t+/m.test(file.text) && /\.(md|ts|tsx|js|jsx|css|scss|yml|yaml|json)$/.test(file.rel)) {
      push(failures, "tab-indentation", file.rel, "Use spaces for indentation unless the project explicitly requires tabs.");
    }
    if (/font-size\s*[:=][^;\n]*(vw|vh|vmin|vmax)/i.test(file.text)) {
      push(failures, "viewport-font-size", file.rel, "Viewport-based font-size is not allowed.");
    }
    if (isUiFile && /#(?:000|000000)\b/i.test(file.text)) {
      push(failures, "pure-black", file.rel, "StyleSeed forbids pure black in UI surfaces.");
    }
    if (isUiFile && /text-\[\s*var\(/i.test(file.text)) {
      push(failures, "tailwind-var-font-size", file.rel, "Do not use Tailwind text-[var(...)] for font-size.");
    }
    if (isUiFile && /\btext-\[\s*(?:\d+(?:\.\d+)?(?:px|rem|em|vw|vh)|var\()/i.test(file.text)) {
      push(failures, "arbitrary-type-size", file.rel, "Prefer semantic type tokens over arbitrary Tailwind text-[...] sizes.");
    }
    if (isUiFile && /\b(?:leading|tracking|font)-\[[^\]]+\]/i.test(file.text)) {
      push(failures, "hand-tuned-type-metrics", file.rel, "Avoid hand-tuned leading/tracking/font arbitrary classes unless documented in the design run.");
    }
    if (isUiFile && /\b(?:rounded|p|px|py|pt|pr|pb|pl|m|mx|my|mt|mr|mb|ml|gap)-\[\s*\d+(?:\.\d+)?(?:px|rem|em)\s*\]/i.test(file.text)) {
      push(failures, "arbitrary-spacing-radius", file.rel, "Prefer layout/section/component spacing and radius tokens over arbitrary Tailwind values.");
    }
    if (isUiFile && /\bshadow-\[[^\]]+\]/i.test(file.text)) {
      push(failures, "raw-shadow-stack", file.rel, "Prefer semantic elevation/state shadow tokens over raw arbitrary shadow stacks.");
    }
    if (isUiFile && /\p{Extended_Pictographic}/u.test(file.text)) {
      push(failures, "emoji-ui-icon", file.rel, "StyleSeed forbids emoji as UI icons; use one line-icon set in currentColor.");
    }
    if (/!important/.test(file.text)) push(failures, "important-style", file.rel, "Avoid !important.");
    if (/z-index\s*[:=]\s*["']?(9{3,}|\d{4,})/i.test(file.text)) {
      push(failures, "large-z-index", file.rel, "Large arbitrary z-index values need review.");
    }
    if (file.rel.endsWith(".md") && hasOpeningCodeFenceWithoutLanguage(file.text)) {
      push(failures, "missing-code-fence-language", file.rel, "Markdown code fences should declare a language.");
    }
  }
}

function checkAccessibilityRules(files, failures) {
  for (const file of files) {
    if (!/\.(css|scss|tsx|jsx|html|svg)$/.test(file.rel)) continue;
    const text = file.text;

    if (/<img\b(?![^>]*\balt=)/i.test(text)) {
      push(failures, "image-without-alt", file.rel, "Images need alt text or an explicit empty alt for decorative images.");
    }
    if (/<(?:input|select|textarea)\b(?![^>]*(aria-label|aria-labelledby|id=|name=))/i.test(text)) {
      push(failures, "form-control-without-name", file.rel, "Form controls need a label or accessible name.");
    }
    if (/<button\b(?![^>]*(aria-label|aria-labelledby|title=))[^>]*>\s*(?:<svg\b|<img\b|<Icon\b)[\s\S]{0,240}?<\/button>/i.test(text)) {
      push(failures, "icon-button-without-name", file.rel, "Icon-only buttons need an accessible name.");
    }
    if (/<button\b[\s\S]{0,500}<a\b/i.test(text) || /<a\b[\s\S]{0,500}<button\b/i.test(text)) {
      push(failures, "nested-interactive-control", file.rel, "Do not nest links and buttons inside each other.");
    }
    if (/<(?:div|span)\b(?=[^>]*\bonClick=)(?![^>]*(role=|tabIndex=|tabindex=|onKeyDown=|onKeyUp=|onKeyPress=))/i.test(text)) {
      push(failures, "nonsemantic-click-target", file.rel, "Clickable div/span needs semantic element or keyboard and role support.");
    }
    if (/\btabIndex=\{?[1-9]\d*\}?|\btabindex=["']?[1-9]\d*/i.test(text)) {
      push(failures, "positive-tabindex", file.rel, "Positive tab index creates unpredictable keyboard order.");
    }
    const hasHover = /:hover\b|\bhover:/i.test(text);
    const hasFocus = /:focus\b|focus-visible|focus:|\bfocus-visible:/i.test(text);
    if (hasHover && !hasFocus) {
      push(failures, "hover-without-focus", file.rel, "Hover styles need matching focus/focus-visible treatment.");
    }
    const hasInteractive =
      /<button\b|<a\b|<input\b|<select\b|<textarea\b|onClick=|\brole=["'](?:button|link|tab|switch|checkbox)["']/i.test(text);
    if (hasInteractive && !hasFocus) {
      push(failures, "interactive-without-focus-state", file.rel, "Interactive UI needs a visible focus state.");
    }
    if (/prefers-reduced-motion/.test(text) === false && /\b(animate-|animation:|transition:|motion\.|framer-motion)\b/i.test(text)) {
      push(failures, "motion-without-reduced-motion", file.rel, "Motion-heavy UI should include a reduced-motion path.");
    }
    if (/\b(error|invalid|required)\b/i.test(text) && !/(aria-invalid|aria-describedby|role=["']alert["']|aria-live)/i.test(text)) {
      push(failures, "error-state-without-a11y", file.rel, "Error states need aria-invalid, aria-describedby, role=alert, or aria-live support.");
    }
  }
}

function checkLengthsAgainstContract(files, contract, failures) {
  const propertyGroups = [
    { rule: "font-size-contract", property: "font-size", message: "Font-size must be declared in DESIGN.md." },
    { rule: "line-height-contract", property: "line-height", message: "Line-height must be declared in DESIGN.md when fractional." },
    { rule: "radius-contract", property: "border-radius", message: "Border radius must be declared in DESIGN.md." },
    { rule: "spacing-contract", property: "(?:margin|padding|gap|top|right|bottom|left)", message: "Spacing values must be declared in DESIGN.md when fractional." },
  ];

  for (const file of files) {
    if (!/\.(css|scss|tsx|jsx|html|svg)$/.test(file.rel)) continue;
    for (const group of propertyGroups) {
      const re = new RegExp(`${group.property}\\s*[:=]\\s*["']?(-?\\d+(?:\\.\\d+)?)(px|rem|em|%)`, "gi");
      for (const match of file.text.matchAll(re)) {
        const raw = `${match[1]}${match[2]}`;
        const isFractional = match[1].includes(".");
        if (isFractional && !lengthAllowed(contract, raw)) {
          push(failures, group.rule, file.rel, `${group.message} Found ${raw}.`);
        }
      }
    }

    for (const match of file.text.matchAll(/letter-spacing\s*[:=]\s*["']?(-?\d+(?:\.\d+)?)(px|rem|em|%)?/gi)) {
      const raw = `${match[1]}${match[2] || "px"}`;
      if (Number(match[1]) < 0) {
        if (!contract.allowsNegativeLetterSpacing) {
          push(failures, "letter-spacing-contract", file.rel, "Negative letter-spacing is not allowed by DESIGN.md.");
        } else if (!lengthAllowed(contract, raw)) {
          push(failures, "letter-spacing-contract", file.rel, `Negative letter-spacing ${raw} is not declared in DESIGN.md.`);
        }
      }
    }
  }
}

function checkColorsAgainstContract(files, contract, failures) {
  if (!contract.exists || contract.colors.size < 2) return;
  for (const file of files) {
    if (!/\.(css|scss|tsx|jsx|html|svg)$/.test(file.rel)) continue;
    for (const match of file.text.matchAll(/#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/g)) {
      const color = normalizeHex(match[0]);
      if (!contract.colors.has(color)) {
        push(failures, "color-contract", file.rel, `${color} is not declared in DESIGN.md.`);
      }
    }
  }
}

function checkBrandSpecificRules(files, contract, failures) {
  if (!contract.exists) return;

  for (const file of files) {
    if (!/\.(css|scss|tsx|jsx|html|svg)$/.test(file.rel)) continue;
    const text = file.text;

    if (contract.forbidPureWhiteBackground && /background(?:-color)?\s*[:=]\s*["']?#(?:fff|ffffff)\b/i.test(text)) {
      push(failures, "pure-white-background", file.rel, "DESIGN.md forbids pure white as a page background.");
    }

    if (contract.maxFontWeight !== null) {
      for (const match of text.matchAll(/font-weight\s*[:=]\s*["']?(\d{3,4})/gi)) {
        const weight = Number(match[1]);
        if (weight > contract.maxFontWeight) {
          push(failures, "font-weight-contract", file.rel, `Font weight ${weight} exceeds DESIGN.md ceiling ${contract.maxFontWeight}.`);
        }
      }
    }

    if (contract.forbidPillCtas && /(button|cta)[^{\n]*(?:\{|\n)[^}]*border-radius\s*:\s*(9999px|50px|999em|999rem)/i.test(text)) {
      push(failures, "pill-cta-contract", file.rel, "DESIGN.md says CTAs must not be pill-shaped.");
    }

    if (contract.requirePillButtons && /(button|cta)[^{\n]*(?:\{|\n)[^}]*border-radius\s*:\s*(0|2px|4px|6px|8px|12px)\b/i.test(text)) {
      push(failures, "button-radius-contract", file.rel, "DESIGN.md says buttons/CTAs should be pill-shaped.");
    }

    if (contract.forbidAccentButtons && contract.accentColors.size > 0) {
      for (const color of contract.accentColors) {
        const escaped = color.replace("#", "#?");
        const re = new RegExp(`(button|cta)[^{\\n]*(?:\\{|\\n)[^}]*background(?:-color)?\\s*:\\s*${escaped}\\b`, "i");
        if (re.test(text)) {
          push(failures, "accent-button-contract", file.rel, `DESIGN.md says accent color ${color} must not be used as a button background.`);
        }
      }
    }

    if (contract.forbidDropShadows && /box-shadow\s*:\s*(?!none\b)[^;\n]+/i.test(text)) {
      push(failures, "shadow-contract", file.rel, "DESIGN.md forbids drop shadows for this design system.");
    }
  }
}

function runDesignChecks() {
  const contract = readDesignContract();
  const failures = [];
  const scope = resolveTargetScope(failures);
  const files = scope.targets
    .map((rel) => {
      const full = resolve(rel);
      if (!fs.existsSync(full)) {
        failures.push({ rule: "missing-target-file", file: rel, detail: "Target file listed in the design run does not exist." });
        return null;
      }
      return { full, rel, text: readText(full) };
    })
    .filter((item) => item !== null)
    .filter((item) => item.text !== null);
  failures.push(...readFailures, ...traversalFailures);

  if (!scope.active && scope.targets.length === 0) {
    return { filesScanned: 0, failures, contract, scope };
  }

  if (contract.tooLarge) {
    push(failures, "file-too-large", "DESIGN.md", `DESIGN.md exceeds ${maxTextFileBytes} bytes and was not scanned.`);
  }

  if (!contract.exists) {
    push(failures, "missing-design-contract", contract.file, "DESIGN.md is required before design output can be validated.");
  }

  checkCommonTextRules(files, failures);
  checkAccessibilityRules(files, failures);
  checkLayoutPlanning(scope, failures);
  checkPalettePlanning(scope, failures);
  checkCreateUiPlanning(scope, failures);
  checkLengthsAgainstContract(files, contract, failures);
  checkColorsAgainstContract(files, contract, failures);
  checkBrandSpecificRules(files, contract, failures);

  return { filesScanned: files.length, failures, contract, scope };
}

function printResult(result) {
  if (result.failures.length > 0) {
    console.error(`OpenDock harness: ${title}`);
    console.error(`Focus: ${focus}`);
    console.error(`Scope: ${result.scope.label}`);
    console.error(`DESIGN.md: ${result.contract.exists ? "loaded" : "missing"}`);
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
  console.log(`DESIGN.md: ${result.contract.exists ? "loaded" : "missing"}`);
  console.log(`Files scanned: ${result.filesScanned}`);
  if (!result.scope.active && result.scope.targets.length === 0) console.log("No active design run detected.");
  console.log("Ultrawork passed.");
}

printResult(runDesignChecks());
