import path from "node:path";

export const root = path.resolve(process.cwd());
export const runRoot = ".opendock/runs/interactive-ui";
export const title = "Interactive UI Ultrawork";
export const maxManifestBytes = 256 * 1024;
export const maxTargetBytes = 1024 * 1024;
export const maxTargets = 64;
export const activeStatuses = new Set(["active", "review", "ready", "ready-for-review", "handoff"]);
export const blockedTargetSegments = new Set([
  ".git",
  ".opendock",
  ".agents",
  ".claude",
  ".codex",
  ".cursor",
  ".ssh",
  "node_modules",
]);
export const targetExtensions = new Set([
  ".astro",
  ".css",
  ".htm",
  ".html",
  ".js",
  ".json",
  ".jsx",
  ".less",
  ".mjs",
  ".scss",
  ".svelte",
  ".ts",
  ".tsx",
  ".vue",
]);
