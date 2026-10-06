# Task work kind, completion results, and deadline picker

## Scope and safety

- Branch: `codex/task-work-kind`; isolated checkout `jimin-os-scheduled-work`.
- Production checkout, deployed server, desktop installation, and physical device were not changed.
- Development preview: `http://localhost:1423/`, synthetic data with mocked transport only.
- New migration: 0058. API, agent, database, and client must be released together; migration rollback is documented in `migrations/README.md`.

## Implemented

- Explicit general / verification / development choice in task creation, editing, Chat/Gmail promotion, and project quick-add.
- Work kind retained in storage, returned by API, included in assignment messages, and visible in lists/details with project filtering.
- Verification completion asks for an optional result; stores it with completion atomically and snapshots it in the original-source Google Chat delivery queue.
- Reopening clears the previous result and cancels pending old completion replies. Existing version fencing and provider idempotency remain in use.
- Legacy clients preserve existing work kind when updating without the new field; old tasks default to general.
- Shared deadline picker offers calendar, today/tomorrow, hour 00–23, and minute 00–59. Existing minute values survive editing, including project-task editing.

## Found and corrected during verification

1. Webhook payload whitelist rejected the new work kind. Contract test now permits the explicit work kind; completion results are not broadcast in generic webhooks.
2. Rust lint rejected an overlong update method. Optional kind validation was compacted without changing behavior.
3. Mobile completion dialog inherited full-screen stretched spacing. Scoped fit-content layout now keeps input and actions together.
4. Completion result validation initially rejected line breaks. Bounded nonempty multiline text is now accepted; blank, overlength, and unsafe-control inputs are rejected before mutation.
5. Assistant canvas previously marked a cancelled completion as finished. It now checks the returned task status before removing it.
6. Invalid persisted kind values are discarded instead of restoring an unsupported select value.
7. Final filtered-empty copy used a mistyped state name. TypeScript caught it; corrected to the existing `workKindFilter` before the final build.

## Automated checks

- Frontend TypeScript: pass.
- Frontend: 67 files, 414 tests pass.
- Frontend production build: pass.
- Relevant API / agent / storage unit tests: pass; explicit live AI smoke remains intentionally ignored.
- Relevant Rust clippy with warnings denied: pass.
- Workspace/all-target Rust check: pass.
- Isolated Postgres integration suite: 52 scenarios passed before final multiline-validation extension; final rerun result recorded below.
- Rust and changed frontend formatting, diff whitespace check, scoped backend and UX-writing gates: final results recorded below.

## Browser checks

- Mobile 390×844: selected tomorrow, 23:59, verification; created task and reopened editor. Date, 23 hour, 59 minute, and kind persisted.
- Completion dirty cancellation leaves task open; confirmed completion removes the correct task and increments completed count.
- Mobile completion result input and all three actions remain visible within compact scroll-safe dialog.
- Desktop project filter selects verification and hides development rows without changing underlying task counts.
- Home Chat promotion: selected verification, tomorrow 14:42, registered successfully, and actionable candidate count decreased by one.
- Calendar-open button opens the native calendar. Mobile promotion dialog has client width and scroll width both 390px, with sticky registration actions reachable.
- All interactions use only the synthetic development preview, not production or real Chat destinations.

## Not claimed / follow-up

- Actual Google Chat completion reply delivery requires a production/provider smoke after deployment; queue snapshot, formatter, and retry lifecycle are tested locally.
- No physical-device installation or native package release in this task.
- Conversational AI automatic work-kind classification is not added; this task adds explicit user selection.
- Design/interactive full-project harnesses were not requested; review stayed within changed UI scope.

## Final gate results

- Final isolated Postgres rerun: all 52 scenarios pass, including multiline result persistence and invalid result rejection before mutation.
- Relevant API / agent build, unit tests, and clippy: pass.
- Final frontend typecheck, 414 tests, and production build: pass after picker alignment and filtered-empty copy fixes.
- Rust formatter and diff whitespace checks: pass.
- Backend lexical harness in an isolated copy of changed backend targets: pass (3 recognized SQL/Markdown/JSON files). The old harness does not recognize Rust, so Rust request validation, authentication guards, bound parameters, logging, and transaction behavior were reviewed directly and tested separately.
- Raw UX harness on 13 changed component files reported 515 lexical findings, including TypeScript `null` / `undefined` and pre-existing code/copy. These are not 515 new public-copy defects. The new public copy was reviewed against the actual source, extracted into `2026-10-06-task-work-kind-copy.md`, and checked with the unchanged lexical harness alongside the deadline copy: pass. No project-wide harness or blanket terminology exception was added.
- Completion and picker screenshots: `/tmp/jimin-os-task-completion-mobile.jpg`, `/tmp/jimin-os-task-date-time-mobile.jpg`, `/tmp/jimin-os-task-assignment-desktop.jpg`.
