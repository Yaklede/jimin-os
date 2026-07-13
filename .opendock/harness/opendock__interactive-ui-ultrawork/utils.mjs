import fs from "node:fs";
import path from "node:path";

import { blockedTargetSegments, root, targetExtensions } from "./constants.mjs";

export function normalize(value) {
  return String(value).split(path.sep).join("/");
}

export function escapeTerminal(value) {
  return String(value).replace(/[\x00-\x1f\x7f-\x9f]/g, (char) => {
    const code = char.charCodeAt(0).toString(16).padStart(2, "0");
    return `\\x${code}`;
  });
}

export function push(failures, rule, file, detail) {
  if (failures.some((failure) => failure.rule === rule && failure.file === file && failure.detail === detail)) return;
  failures.push({ rule, file, detail });
}

export function parseField(text, name) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = text.match(new RegExp(`^${escaped}\\s*:\\s*(.*)$`, "im"));
  return match ? match[1].trim() : "";
}

export function sectionText(text, heading) {
  const lines = text.split(/\r?\n/);
  const escaped = heading.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const start = lines.findIndex((line) => new RegExp(`^##\\s+${escaped}\\s*$`, "i").test(line.trim()));
  if (start < 0) return "";
  const out = [];
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^##\s+/.test(lines[index].trim())) break;
    out.push(lines[index]);
  }
  return out.join("\n");
}

export function hasDetailedEvidence(value) {
  const cleaned = String(value)
    .replace(/[`*_]/g, "")
    .replace(/\s+/g, " ")
    .trim();
  if (cleaned.length < 16) return false;
  return !/^(todo|tbd|pending|n\/?a|none|미정|없음|검증 예정|문제 없음)[.!]?$/i.test(cleaned);
}

export function hasCoreValue(value) {
  const cleaned = String(value).replace(/[`*_]/g, "").trim();
  return cleaned.length >= 3 && !/^(todo|tbd|pending|describe.*|미정|작성.*)$/i.test(cleaned);
}

export function safeTargetPath(candidate) {
  const value = String(candidate).trim().replace(/^['"`]+|['"`]+$/g, "");
  if (!value || value.includes("://") || path.isAbsolute(value)) return null;
  const normalized = path.posix.normalize(value.replaceAll("\\", "/"));
  if (normalized === "." || normalized === ".." || normalized.startsWith("../")) return null;
  const segments = normalized.split("/");
  if (
    segments.some((segment) => {
      const lower = segment.toLowerCase();
      return blockedTargetSegments.has(lower) || lower.startsWith(".env");
    })
  ) {
    return null;
  }
  if (!targetExtensions.has(path.posix.extname(normalized).toLowerCase())) return null;
  const absolute = path.resolve(root, normalized);
  if (absolute !== root && !absolute.startsWith(`${root}${path.sep}`)) return null;
  return normalized;
}

export function hasSymlinkSegment(rel) {
  let current = root;
  for (const segment of rel.split("/")) {
    current = path.join(current, segment);
    if (!fs.existsSync(current)) return false;
    if (fs.lstatSync(current).isSymbolicLink()) return true;
  }
  return false;
}

export function extractTargetCandidates(text) {
  const section = sectionText(text, "Target Files");
  const targets = [];
  for (const line of section.split(/\r?\n/)) {
    if (!/^\s*[-*]\s+/.test(line)) continue;
    const inline = [...line.matchAll(/`([^`]+)`/g)].map((match) => match[1]);
    const candidates = inline.length > 0 ? inline : [line.replace(/^\s*[-*]\s+/, "").trim()];
    targets.push(...candidates);
  }
  return [...new Set(targets)];
}
