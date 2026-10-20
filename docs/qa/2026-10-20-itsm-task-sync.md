# ITSM task date and completion synchronization — development verification

Scope: `/Users/jimin/.codex/worktrees/pc-inflow-full-fill/jimin-os`, existing
`codex/holiday-home-workflow` checkout. Pre-existing changes were preserved.
No production deployment, real ticket write, Chat send, native installation,
commit or push is authorized or claimed by this implementation run.

## Implemented behavior

- Registration binds only a verified source ITSM ticket. One candidate defaults
  to selected; multiple candidates require a picker or explicit opt-out. The
  selected ticket/opt-out survive draft storage and source refreshes. Arbitrary
  links in editable task notes are never a write destination.
- Older clients with multiple reference tickets still register the task and
  preserve Chat delivery; without an explicit choice no ITSM ticket is guessed.
- Existing tasks link lazily on a later owner-authorized mutation only when
  prior verified analysis identifies exactly one ticket. Historical ambiguous
  or never-verified references are not automatically modified.
- Deadline changes use the latest deadline among noncancelled linked tasks,
  converted to Asia/Seoul's calendar date. Time remains in Jimin OS. Initial
  no-deadline registration preserves the remote date; explicitly clearing a
  previously synchronized date sends null once no linked task retains a date.
- Completion is sent only when every linked task is completed and no open
  child remains. Cancellation is not completion. Remote closed tickets are not
  reclassified or reopened. Only due_date/status_id can be transmitted; no
  title, description, assignee, private memo or comment field is written.
- The existing secret filename is retained. The user authorized the already
  held administrator key; code does not expose it to clients/logs/AI. Runtime
  validation reconstructs the destination from the configured HTTPS origin,
  refuses redirects/foreign origins, bounds response size and request time.
- A dedicated worker runs independently of ChatGPT authentication and LLM jobs.
  One lane per issue serializes work; local revision/worker/attempt fences
  preserve newer changes and reject expired acknowledgements. HTTP success is
  followed by a fresh issue read; ignored changes are explicit failures.
- OS storage/completion is not undone by provider failures. Queued, automatic
  retry and failed state are separated in the existing collapsed recovery area.
  Owner-guarded retry targets ITSM only. Result changes enter the existing
  cross-device change feed rather than requiring an app restart.

## Verification evidence

- Frontend: 492 tests in 80 files, TypeScript, scoped Prettier and normal web
  build passed. Main JS is 503.66kB / 139.05kB gzip; the existing >500kB warning
  remains and no runtime-performance improvement is claimed.
- API/storage: 116 + 90 unit tests passed. Agent: 93 passed, one explicit live
  model smoke ignored (not counted as executed). Nineteen scoped ITSM tests
  include loopback HTTP update/readback, closed-state mapping, workflow denial,
  403/422/503 classification, no-op retry, date clearing and foreign/stale guards.
- PostgreSQL: 57 isolated scenarios passed, including actual promotion linking,
  a shared ticket with two tasks, aggregate deadline, partial completion,
  durable failure/retry, cross-owner denial, same-worker expired lease fencing,
  newer revision retention, cross-device result receipt and explicit date clear.
- Schema 62→63 isolated rehearsal: transaction applied 63, rolled back to 62
  with both new tables absent, then applied 63 with no historical jobs. The
  owned test containers were removed; unrelated Docker services were untouched.
- Scoped Rust formatting/Clippy with warnings as errors and final API/agent
  release builds passed. No repository-wide static root scan
  was run: its current harness traverses unrelated files and cannot honor this
  task's scope. Authentication, body validation, SQL parameters, key handling,
  migration/rollback and actual request schema were manually checked in scope.
- Scoped UX scanner returned 89 lexical findings (exit 1), not an automatic
  pass. Changed public messages were reviewed manually; code identifiers and
  existing split-copy findings are distinguished in the writing manifest.

## Screen checks

- PC: two verified tickets block registration until selection; explicit opt-out
  enables registration without a ticket; re-enabling preserves the selection.
- Mobile 390×844: picker, deadline and registration footer remain accessible;
  selecting ticket 43 and registering decreases the incoming count and adds a
  task. This is synthetic development data, not a live ITSM write.
- Mobile 320×780: a single verified ticket is selected automatically. The page
  and form scroll widths are 320px, picker stays between x=41 and x=264, and the
  registration action is 48px high with bottom y=764 inside the viewport.
- The ITSM failure is distinct from Chat/push; its retry removes only the ITSM
  fixture, preserves other failures and reduces the attention count.
- Screenshots: `.playwright-mcp/itsm-sync-desktop-20261020.png` and
  `.playwright-mcp/itsm-sync-mobile-20261020.png`, plus
  `.playwright-mcp/itsm-sync-mobile320-20261020.png` under the user's main checkout.

## Findings resolved / remaining boundaries

- Compile/lint caught an oversized integration scenario; its single-lifecycle
  scope is documented explicitly rather than disabling lint workspace-wide.
- Lease acknowledgements were strengthened with attempt fencing so reusing a
  worker ID cannot acknowledge a newer expired/reclaimed lease.
- Canceled linked tasks now block automatic completion rather than acting as
  completed tasks. Missing permitted closed statuses report workflow denial.
- If every linked task is canceled/deleted, the worker performs no remote
  write; cancellation cannot inadvertently clear the upstream due date.
- The added cancellation scenario initially failed because the deletion path
  skipped ITSM queue invalidation. Both cancellation and soft deletion now
  invalidate the old leased revision; the regression checks the actual
  update/delete methods and a zero-active-task no-write result.
- Added durable change-feed receipts after provider results and manual retry;
  otherwise another signed-in device could retain the old pending/failure UI.
- Compatibility review preserved old clients' ambiguous-ticket registrations
  without an external write. Clippy caught identical empty match arms during
  that refinement; they were merged without changing the safety behavior.
- The current administrator key's live permissions and the company's actual
  completion state have not been exercised against production. No actual
  ticket was chosen for a controlled live test. Deployment, production backup
  restoration and a human-approved live test remain separate gates.
