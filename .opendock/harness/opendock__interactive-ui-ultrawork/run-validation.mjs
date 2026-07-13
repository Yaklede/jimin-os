import fs from "node:fs";
import path from "node:path";

import { maxManifestBytes, maxTargetBytes, maxTargets, root, runRoot } from "./constants.mjs";
import {
  extractTargetCandidates,
  hasCoreValue,
  hasDetailedEvidence,
  hasSymlinkSegment,
  parseField,
  push,
  safeTargetPath,
  sectionText,
} from "./utils.mjs";

export function readRunManifests(failures) {
  const absoluteRunRoot = path.join(root, runRoot);
  if (!fs.existsSync(absoluteRunRoot)) return [];

  if (hasSymlinkSegment(runRoot)) {
    push(failures, "unsafe-run-root", runRoot, "Run root path must not contain symlinks.");
    return [];
  }
  const rootStats = fs.lstatSync(absoluteRunRoot);
  if (!rootStats.isDirectory()) {
    push(failures, "unsafe-run-root", runRoot, "Run root must be a real directory, not a symlink or file.");
    return [];
  }

  const runs = [];
  for (const entry of fs.readdirSync(absoluteRunRoot, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const manifestRel = `${runRoot}/${entry.name}/manifest.md`;
    const manifestFile = path.join(root, manifestRel);
    if (!fs.existsSync(manifestFile)) continue;
    const stats = fs.lstatSync(manifestFile);
    if (stats.isSymbolicLink() || !stats.isFile()) {
      push(failures, "unsafe-run-manifest", manifestRel, "Run manifest must be a regular file, not a symlink.");
      continue;
    }
    if (stats.size > maxManifestBytes) {
      push(failures, "run-manifest-too-large", manifestRel, `Run manifest exceeds ${maxManifestBytes} bytes.`);
      continue;
    }
    const text = fs.readFileSync(manifestFile, "utf8");
    runs.push({
      manifestRel,
      text,
      status: parseField(text, "Status").toLowerCase(),
    });
  }
  return runs;
}

function implementationTier(value) {
  const normalized = String(value).trim().toLowerCase();
  if (/^css\b/.test(normalized)) return "css";
  if (/^waapi\b/.test(normalized)) return "waapi";
  if (/^motion\b/.test(normalized)) return "motion";
  if (/special.*(?:timeline|svg)|timeline\s*\/\s*svg|svg choreography/.test(normalized)) return "special";
  return null;
}

function parseTopLevelField(text, name) {
  const lines = text.split(/\r?\n/);
  const firstSection = lines.findIndex((line) => /^##\s+/.test(line.trim()));
  return parseField(lines.slice(0, firstSection < 0 ? lines.length : firstSection).join("\n"), name);
}

function stateRows(text) {
  const rows = new Map();
  for (const line of sectionText(text, "Interaction State Matrix").split(/\r?\n/)) {
    if (!/^\s*\|/.test(line)) continue;
    const cells = line
      .split("|")
      .slice(1, -1)
      .map((cell) => cell.trim());
    if (cells.length < 2 || /^-+$/.test(cells[0]) || /^state$/i.test(cells[0])) continue;
    rows.set(cells[0].toLowerCase().replace(/\s+/g, " "), cells.slice(1).join(" | "));
  }
  return rows;
}

export function validateRunManifest(run, failures) {
  const requiredSections = [
    "Target Files",
    "Interaction State Matrix",
    "Input Parity Evidence",
    "Motion Evidence",
    "Async State Evidence",
    "Cleanup Evidence",
    "Responsive And Overflow Evidence",
    "Validation Evidence",
    "Exceptions",
  ];
  for (const heading of requiredSections) {
    if (!sectionText(run.text, heading)) {
      push(failures, "missing-run-section", run.manifestRel, `Missing section: ${heading}.`);
    }
  }

  const coreFields = ["Interaction Type", "Framework", "Primary Trigger", "Primary Feedback"];
  for (const field of coreFields) {
    if (!hasCoreValue(parseField(run.text, field))) {
      push(failures, "missing-run-field", run.manifestRel, `${field} needs a concrete value.`);
    }
  }

  const contractFields = ["Primary Completion", "Recovery Path", "Focus Contract"];
  for (const field of contractFields) {
    const value = parseTopLevelField(run.text, field);
    if (!hasCoreValue(value) || !hasDetailedEvidence(value)) {
      push(failures, "missing-run-field", run.manifestRel, `${field} needs a concrete top-level interaction contract.`);
    }
  }

  const tierValue = parseField(run.text, "Implementation Tier");
  const tier = implementationTier(tierValue);
  if (!tier) {
    push(
      failures,
      "invalid-implementation-tier",
      run.manifestRel,
      "Implementation Tier must be CSS, WAAPI, Motion, or special timeline/SVG.",
    );
  }

  for (const field of ["Library Decision", "Library Installation"]) {
    if (!hasDetailedEvidence(parseField(run.text, field))) {
      push(failures, "missing-run-field", run.manifestRel, `${field} needs a concrete decision and reason.`);
    }
  }

  const installPolicy = parseField(run.text, "Library Installation");
  if (!/(none|existing|preinstalled|human-approved|user-approved|no dependency|없음|기존|승인)/i.test(installPolicy)) {
    push(
      failures,
      "automatic-library-install",
      run.manifestRel,
      "Library Installation must state no install, an existing dependency, or a human-approved manual action.",
    );
  }
  if (
    /\b(?:npm|pnpm|bun)\s+(?:install|add|update|upgrade)\b|\b(?:pip|pip3)\s+install\b|\buv\s+(?:add|tool\s+install)\b|\b(?:brew|winget)\s+install\b/i.test(
      run.text,
    )
  ) {
    push(failures, "automatic-library-install", run.manifestRel, "Run manifest must not contain a library installation command.");
  }

  if (tier === "motion") {
    if (!/react/i.test(parseField(run.text, "Framework"))) {
      push(failures, "motion-choice-evidence", run.manifestRel, "Motion tier is reserved for React complex state in this dock.");
    }
    if (!hasDetailedEvidence(parseField(run.text, "Motion Complexity Evidence"))) {
      push(failures, "motion-choice-evidence", run.manifestRel, "Motion tier needs concrete complex-state evidence.");
    }
  }
  if (tier === "special" && !hasDetailedEvidence(parseField(run.text, "Special Choice Evidence"))) {
    push(failures, "special-choice-evidence", run.manifestRel, "Special timeline/SVG tier needs a concrete alternative analysis.");
  }

  const rows = stateRows(run.text);
  const requiredStates = [
    ["idle"],
    ["hover"],
    ["focus"],
    ["pressed/active", "pressed", "active"],
    ["loading"],
    ["error"],
    ["disabled"],
    ["reduced motion", "reduced-motion"],
  ];
  for (const alternatives of requiredStates) {
    const evidence = alternatives.map((state) => rows.get(state)).find(Boolean) ?? "";
    if (!hasDetailedEvidence(evidence)) {
      push(
        failures,
        "missing-state-evidence",
        run.manifestRel,
        `${alternatives[0]} needs observed behavior or a detailed non-applicable reason.`,
      );
    }
  }

  const evidenceFields = [
    "Keyboard Evidence",
    "Touch Evidence",
    "Focus Evidence",
    "Reduced Motion Evidence",
    "Loading Evidence",
    "Error Evidence",
    "Disabled Evidence",
    "Cleanup Evidence",
    "Horizontal Overflow Evidence",
    "Validation Commands",
    "Validation Result",
  ];
  for (const field of evidenceFields) {
    if (!hasDetailedEvidence(parseField(run.text, field))) {
      push(failures, "missing-run-field", run.manifestRel, `${field} needs concrete validation evidence.`);
    }
  }
  if (!/(pass|passed|success|통과|성공)/i.test(parseField(run.text, "Validation Result"))) {
    push(failures, "missing-validation-result", run.manifestRel, "Validation Result must record a passing result before handoff.");
  }

  return tier;
}

export function readTargets(run, failures) {
  const candidates = extractTargetCandidates(run.text);
  if (candidates.length === 0) {
    push(failures, "missing-target-files", run.manifestRel, "Active run must list target files under Target Files.");
    return [];
  }
  if (candidates.length > maxTargets) {
    push(failures, "too-many-target-files", run.manifestRel, `Active run lists more than ${maxTargets} target files.`);
  }

  const targets = [];
  for (const candidate of candidates.slice(0, maxTargets)) {
    const rel = safeTargetPath(candidate);
    if (!rel) {
      push(failures, "unsafe-target-path", run.manifestRel, `Unsafe or unsupported target path: ${candidate}`);
      continue;
    }
    const full = path.join(root, rel);
    if (!fs.existsSync(full)) {
      push(failures, "missing-target", rel, "Target listed by the active run does not exist.");
      continue;
    }
    if (hasSymlinkSegment(rel)) {
      push(failures, "target-symlink", rel, "Target path must not contain symlinks.");
      continue;
    }
    const stats = fs.lstatSync(full);
    if (!stats.isFile()) {
      push(failures, "unsafe-target-path", rel, "Target must be a regular file, not a directory.");
      continue;
    }
    if (stats.size > maxTargetBytes) {
      push(failures, "target-too-large", rel, `Target exceeds ${maxTargetBytes} bytes.`);
      continue;
    }
    const buffer = fs.readFileSync(full);
    if (buffer.includes(0)) {
      push(failures, "binary-target", rel, "Target must be a text UI source file.");
      continue;
    }
    targets.push({ rel, text: buffer.toString("utf8") });
  }
  return targets;
}
