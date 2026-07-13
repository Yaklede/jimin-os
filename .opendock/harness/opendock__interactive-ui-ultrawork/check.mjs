#!/usr/bin/env node
import { activeStatuses, runRoot, title } from "./constants.mjs";
import { readRunManifests, readTargets, validateRunManifest } from "./run-validation.mjs";
import { validateSource } from "./source-validation.mjs";
import { escapeTerminal, push } from "./utils.mjs";

function printFailures(failures, activeRun, targetCount) {
  console.error(`OpenDock harness: ${title}`);
  console.error("Status: failed");
  console.error(`Active run: ${activeRun ?? "none"}`);
  console.error(`Targets scanned: ${targetCount}`);
  console.error(`Failures: ${failures.length}`);
  for (const failure of failures.slice(0, 120)) {
    console.error(`- [${escapeTerminal(failure.rule)}] ${escapeTerminal(failure.file)}: ${escapeTerminal(failure.detail)}`);
  }
  if (failures.length > 120) console.error(`... ${failures.length - 120} more failures omitted`);
}

function run() {
  const failures = [];
  const runs = readRunManifests(failures);
  const activeRuns = runs.filter((run) => activeStatuses.has(run.status));

  if (activeRuns.length === 0 && failures.length === 0) {
    console.log(`OpenDock harness: ${title}`);
    console.log("Status: ready");
    console.log("Active run: none");
    console.log("Targets scanned: 0");
    console.log("Ready: no active interaction run to validate.");
    return;
  }

  if (activeRuns.length > 1) {
    push(
      failures,
      "multiple-active-runs",
      runRoot,
      `Expected one active run, found ${activeRuns.length}: ${activeRuns.map((run) => run.manifestRel).join(", ")}`,
    );
  }

  const activeRun = activeRuns[0];
  if (!activeRun) {
    printFailures(failures, null, 0);
    process.exit(1);
  }

  const tier = validateRunManifest(activeRun, failures);
  const targets = readTargets(activeRun, failures);
  validateSource(activeRun, tier, targets, failures);

  if (failures.length > 0) {
    printFailures(failures, activeRun.manifestRel, targets.length);
    process.exit(1);
  }

  console.log(`OpenDock harness: ${title}`);
  console.log("Status: passed");
  console.log(`Active run: ${activeRun.manifestRel}`);
  console.log(`Implementation tier: ${tier}`);
  console.log(`Targets scanned: ${targets.length}`);
  console.log("Interaction quality gate passed.");
}

run();
