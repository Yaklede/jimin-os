# Section 23 remaining development / confirmation gate

Worktree: `codex/holiday-home-workflow`, separate development app and emulator only.
Production deployment and physical-device installation require the user's confirmation.

## Scope

- N1/C1 public-holiday exclusion, N2/C2 uncertain requests retained, C5 home integration: existing changes retained.
- N3/C3 source Chat reaction/reply recovery and separate webhook/push retry.
- N4 workflow states and mandatory waiting reason, compatible with existing task lifecycle.
- C4 home direct assignee/date grouping, shared task editing/completion, no AI request needed.
- N5 explicit current-revision approval of additional request content/deadline changes.
- N6 fresh-state conditional reminders: approaching deadline, waiting without response, completion confirmation.
- C6 weekly metric evidence drill-down with identical leaf-task rules and waiting reasons.
- Mac notification diagnostics, test notification, account-scoped bounded restart checkpoint without credentials/content.
- Now Bar: excluded per user decision.

## Verification gate — development complete, confirmation pending

- [x] TypeScript and 479 tests in 76 frontend test files passed after the final modal fix. Prettier passed for all changed/new TypeScript, TSX and CSS targets; `git diff --check` passed.
- [x] Normal production web build passed with preview flags removed. No design-preview modules were found in the production assets. The main JS chunk is 502.24 kB / 138.36 kB gzip and triggers the existing 500 kB warning; this is a build warning, not a measured performance pass.
- [x] Rust formatting and scoped API/storage/agent/desktop Clippy with warnings denied passed. API/storage library tests: 202 passed. Agent binary: 84 passed, one live-environment test ignored. Mac native library: 10 passed. API release build passed.
- [x] Isolated PostgreSQL suite: 56 passed. The scheduled-work integration test was explicitly run and passed separately, including upgrade from schema 55 through 61 and new conditional-reminder cases. Default ignored integration tests are not counted as executed.
- [x] Migration rehearsal: an isolated schema-58 database applied 59/60/61 inside a transaction, then rolled back; schema 58 and absence of the new tables/column were verified. This does not replace an actual production backup/restore rehearsal.
- [x] PC 1440×1000 real interaction: home Today/Overdue/All, assignee/date grouping, status filtering, task editing/completion, current-reply change approval, per-step retry failure/success, report evidence-to-detail, scheduler X/dirty confirmation and conditional fields.
- [x] Mobile browser 390×844 and 320×760 real interaction: no horizontal overflow, touch controls at least 44px, direct completion with optional reply, correct counts, report detail, change-modal scrolling/fixed actions, dirty-close focus and content preservation. At 320px the two action buttons both measure 68px high and remain inside the viewport.
- [x] Android development APK built and installed only on `emulator-5554`, package `io.jimin.os.dev`. The native back key closes the scheduler and leaves MainActivity active; dirty editor close asks before discarding. Four navigation routes and the large center microphone remain. The latest APK was reinstalled after the final mobile fix. Physical device and production package were not touched.
- [x] Latest `Jimin OS Dev.app` built, relaunched, and its native settings/test action checked. The OS accepted the test notification and the app showed the qualified success message. Actual OS banner appearance and live-account polling are **not verified** by that acceptance.
- [x] Scoped backend checks (validation, explicit guards, SQL parameters, migration compatibility, retry isolation and API/schema contracts) and manual UX copy review completed. Static lexical warnings are recorded separately below, not misreported as an automatic pass.

## Issue ledger

- Fixed during development: conditional reminder test fixtures lacked new state fields; compile gate caught it.
- Fixed during development: conditional UI could re-enable schedules even though server rejects that combination.
- Fixed during development: change approval could exceed the public summary's 2,000-character DB constraint; approved content is now retained in bounded ordered chunks.
- Fixed during development: native notification checkpoint writes were non-atomic and did not persist pagination progress; temporary-file rename and per-page checkpoint added.
- Fixed during development: diagnostics counted malformed skipped messages as accepted; acceptance now reflects successful dispatch dedupe entries.
- Fixed during development: home's old shortcut ref no longer targeted the new direct task panel; section focus target restored.
- Fixed during development: comma-separated assignees appeared as one group; individual names are now grouped separately (shared tasks appear under each assignee).
- Fixed during visual testing: report evidence navigation changed tabs without selecting the requested task; it now opens the matching task detail, including waiting reason and linked conversation.
- Fixed during visual testing: project task detail/editor lacked workflow state and waiting reason; both now share the home workflow contract and preserve completed-task lifecycle.
- Fixed during visual testing: mobile change approval had a two-child header inside a three-column layout, causing a vertical title. Its fields were outside the scroll body, hiding the apply action. The modal now uses the shared header/fieldset/footer structure, equal-height actions and safe dirty-close focus.
- Fixed during visual testing: a single date/assignee group occupied only half of the desktop task area. A single group now spans the available width; multiple groups retain two columns.
- Cold-start browser verification produced no new console errors. A historical React effect-dependency warning at 05:10 UTC occurred during an intentional HMR source edit and remains in the tool's retained log; it was not reproduced after reload.

## Data and interaction contracts verified

- Waiting requires a reason; leaving waiting clears that reason. Completed lifecycle remains separate from the three open workflow states.
- Shared-assignee tasks appear in each relevant group, but Today/All totals count unique tasks. Today is due today; overdue is a separate range.
- Reply approval targets the current analyzed revision and a fresh open linked task. It adds only approved content/deadline changes, checks task/item versions, records an idempotent receipt and preserves later unread replies. A new task is not created.
- Webhook/push retry targets only the failed delivery. Successful source reactions/replies are retained. Pending automatic retry and expired/inactive push targets are not presented as manually retryable.
- Conditions use fresh task state: approaching deadline, waiting without external response, or unconfirmed completion within 30 days. Own-app replies do not count as an external response. State changes between preview and execution cause a skip; slot idempotency prevents duplicates.
- Report evidence uses the same leaf-task and period rules as the report metrics, paginates at 100 items and exposes the aggregation criteria.
- Native Mac restart checkpoints are scoped to account/base URL and contain bounded IDs/revisions/times, not credentials or message content. First connection does not replay historical notifications.

## Static checklist outcome

- UX Writing harness ran against nine current writing targets and emitted 284 lexical warnings (exit 1). The manual review distinguished code identifiers such as `null`/`undefined`/`token`, real input `placeholder` properties, multi-line copy references and accessibility group labels from actual public copy. Changed visible errors include cause/next action; approval/retry/close buttons describe their action. No human-approved exception is invented, and the harness is **not recorded as automatically passing**.
- The backend root scanner was not rerun as a whole-repository recursive audit. Current changes were reviewed and verified through scoped formatter/lint/test/build and explicit guard/schema/SQL/migration checks. The earlier three lexical findings are preserved in the first-phase ledger.
- Design/interaction checks were limited to current changed screens; no unrelated project-wide audit was claimed. Touch/keyboard parity, dirty-close handling, async failure/retry, unmount protection and reduced-motion styles were reviewed.

## Confirmation and deployment boundary

- Development preview: `http://127.0.0.1:1423/`. All displayed tasks/conversations and reminder previews are example data; no real Google Chat/webhook/FCM messages were sent by the screen tests. Example next-run timestamps are fixture values, not a live scheduler execution proof.
- The official public-holiday provider still needs the production server credential and live-response verification. Missing/stale data continues to fail closed for automatic delivery.
- Google Chat/FCM live delivery, Mac banner visibility under the user's Focus/screen-sharing/system preferences, and actual production backup/restore remain deployment/live-provider gates, not completed development checks.
- No production server deployment, production desktop update, physical-phone installation, commit or push performed. Now Bar remains excluded. Keep the development review environment until the user confirms; only then clean its build outputs and proceed with the authorized production rollout.

## Visual evidence

- `/tmp/jimin-section23-mobile-change-approval.jpg`: 320px change approval, visible footer actions.
- `/tmp/jimin-section23-home-confirmation.jpg`: desktop home date grouping, waiting reason, direct edit/completion and single-group full-width layout.
- `/tmp/jimin-section23-android-home.png`: Android development navigation and center microphone.
- `/tmp/jimin-section23-android-change-approval.png`: latest native Android approval form and visible footer; native back returned to the selected conversation without closing the app.

## User-directed layout correction — request area first

- Restored incoming requests and the assistant request area above the direct task queue. Scheduled reminders and failed-delivery recovery remain below those primary request areas. Existing request tabs, callbacks and mobile disclosure behavior are preserved.
- The direct task queue spans the desktop grid width; assignee/date filters and completion/edit actions remain available.
- Added rendered-layout regressions for desktop and mobile. Frontend: 481 tests in 77 files passed; TypeScript, scoped Prettier, normal web build and diff whitespace checks passed. The pre-existing 502.24 kB main-chunk warning remains.
- Browser interaction at 1440px and 390px verified request tabs, date grouping, and opening/closing the optional-reply completion dialog. At 320px, requests/assistant/tasks are in that order and no page-level horizontal overflow was found. These are browser checks, not a rebuilt native-client verification.
- Updated visual evidence: `/tmp/jimin-home-request-first-20261008.jpg`. Earlier home/native screenshots above describe the previous layout only. Native development bundles have not been rebuilt for this layout-only correction; production deployment and personal-device installation remain untouched, pending confirmation.

## User-reported section spacing correction

- Root cause: the home grid and its sibling history panel had no inter-section margin. Recovery cards also lacked a border, and the delivery summary had no shared focus/chevron structure.
- Added the shared section gap (desktop 20px, mobile 16px), aligned the history width/edges with home at 1920px and 320px, and applied the existing surface/border/radius tokens to both recovery cards. Request-first order remains unchanged.
- Real browser checks: all three collapsed panels open and close; delivery disclosure also responds to Enter. At 320px, header targets are at least 44px, cards/history align at x=16 with width 273px, and page width does not exceed the viewport. Desktop recovery/history separation measures 20px.
- Final frontend tests: 483 passed in 78 files. TypeScript, normal web build, scoped formatting and whitespace checks passed. Main JS remains above the existing 500 kB warning threshold (502.37 kB); no performance claim is made.
- Screenshots: `/tmp/jimin-home-section-spacing-desktop-20261008.jpg` and `/tmp/jimin-home-section-spacing-mobile-20261008.jpg`. Development web only; no native rebuild, production deployment or personal-device install.

## User-directed priority layout correction — scheduler first, secondary records collapsed

- Supersedes the previous layout evidence above: scheduled work is now immediately below the greeting, ahead of incoming requests. The order is scheduled work → incoming request tabs → assistant request area → direct task queue. The mobile scheduler no longer requires opening an extra disclosure before its button.
- Removed the separate delivery and Chat recovery cards from the main home grid. Both are preserved inside the single default-closed `확인·처리 기록` disclosure together with existing approvals/history. The header shortcut opens that same disclosure, not a duplicate panel.
- The summary separately counts attention-required items and items still processing. Successful source reflection is excluded; expired push failures still indicate that reconnection is required, while automatic retries remain distinguishable.
- Real PC browser interactions at 1440×1000: scheduler opens and X closes; history is closed on reload; the header opens it; grouped delivery/source retry fixtures show the first failure with a next action, then disappear after success. Attention counts decrease without resending completed source steps.
- Mobile 390×844 and 320×780: scheduler remains above requests; direct scheduler opening/X closing works; the collapsed summary uses two lines without horizontal overflow. At 320px the source retry action remains 44px high and within its parent. These are browser checks, not a rebuilt native-client test.
- Frontend regression suite: 484 tests in 78 files passed. Scoped formatting, TypeScript and normal web build passed. The main JS chunk is now 498.75 kB / 137.43 kB gzip; no chunk-size warning was emitted in this build, and no measured runtime-performance claim is made.
- UX Writing review was limited to `DecisionInboxWorkspace.tsx`. The static scanner emitted 31 lexical findings (exit 1): 12 code identifier findings and 19 error/state/copy-reference findings. Changed public copy and count labels were manually reviewed against WRITING/TERMS; static automatic passing or a human-approved exception is not claimed.
- Updated evidence: `/tmp/jimin-home-primary-layout-desktop-20261008.jpg`, `/tmp/jimin-home-secondary-collapsed-desktop-20261008.jpg`, `/tmp/jimin-home-secondary-collapsed-mobile-20261008.jpg`.
- Development web only. No real Google Chat/webhook/FCM messages, production deployment, physical-device installation, native rebuild, commit or push. User confirmation remains pending.

## Edited assignment content and outbound preview

- Confirmed cause: promotion saved the edited `notes` on the task, but separately copied the old AI summary/action/completion fields into public assignment details. Both the assignment webhook and source-thread reply preferred that old projection. An extra save was not required and could not resolve that split.
- Added explicit `assignmentContent` approval on the promotion request and migration 62's nullable reviewed snapshot. New registration uses the edited description for task storage, automatic assignment webhook, and source-thread reply in one fenced transaction. Legacy clients/rows retain their original behavior; intentionally empty reviewed text does not restore raw conversation or old AI text.
- Ordinary task/internal memo edits do not change the external snapshot. Explicit approval of a source reply can update an existing reviewed snapshot; the source revision and task version fences remain. Already sent messages and existing immutable outbox payloads are not silently rewritten.
- Added an optional, live `Chat 전송 내용 미리보기` in the registration modal. It includes project/title/assignee/deadline/priority, edited description and related links; provider mention markup depends on the configured directory. Long descriptions show the actual bounded body and a warning, while complete text remains on the task. The existing 1,600-character body limit and reserved reference block are retained.
- Cross-language shared fixtures compare the frontend preview and Rust formatter exactly, including edited text beyond 400 characters, Korean deadlines, CRLF, Unicode truncation, related links, intentional blank content, and legacy fallback. Integration checks compare the queued webhook body with the source-thread body and verify that private memo edits do not leak into either public projection. Existing webhook payload keys and ownership/revision constraints remain unchanged.
- Verified frontend: 489 tests in 79 files, TypeScript, scoped Prettier, normal production web build. API/storage unit tests: 205 passed. Scoped Clippy with warnings as errors, Rust formatting and the final API release build passed. PostgreSQL isolated regressions: 56 passed after the final content fix.
- Real browser: desktop preview updates after editing; 390×844 and 320×760 modals remain inside their viewport with 48px registration actions and no modal horizontal overflow. Mobile registration removes the candidate, increments the task count, and task detail contains the edited text. Date-bound and no-deadline previews were checked. These use synthetic development data and do not claim provider delivery.
- Migration rehearsal in an isolated, temporary schema-61 database applied 62 inside a transaction, observed version 62, rolled back to 61 with no new column, then applied 62 and checked nullability and the 10,000-character constraint. The owned temporary container was removed; unrelated Docker services were not touched. This is not a restored production-backup rehearsal.
- Fixed test findings: missing fixture field caught at compile time; zero-padded frontend month disagreed with Rust; an unnecessary new outbound payload key broke the documented projection allowlist and was removed. Tests were rerun after corrections. The final build still reports a >500kB main-JS warning (500.97kB / 138.24kB gzip); it is not hidden or presented as a performance pass.
- Scoped UX Writing scanner: 76 lexical findings (exit 1) across three targets, reviewed as type identifiers, actual input placeholder properties, status labels or error/copy references rather than the changed visible strings. Manual WRITING/TERMS review completed; no automatic harness pass or human-approved exception is claimed. Manifest: `.opendock/runs/ux-writing/2026-10-08-reviewed-assignment-content/manifest.md`.
- Backend checklist: request validation precedes persistence; the existing explicit authentication/owner guard, parameterized SQL, revision fences and atomic outbox remain; no raw description/credential logging was introduced. API request schema derives from the actual optional field, and migration compatibility/rollback boundaries are recorded in `migrations/README.md`. No recursive root audit was performed.
- Visual evidence: `/tmp/jimin-assignment-reviewed-preview-desktop-20261008.jpg`, `/tmp/jimin-assignment-reviewed-preview-mobile-20261008.jpg`, `/tmp/jimin-assignment-reviewed-preview-mobile320-20261008.jpg`.
- Development changes only; no production deployment, native rebuild/install, real Chat/webhook/FCM transmission, commit or push. Deployment requires server + schema 62 before the updated client, not a screen-only release. Live-account verification remains pending user confirmation.
