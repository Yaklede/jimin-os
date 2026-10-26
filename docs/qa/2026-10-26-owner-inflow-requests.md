# Owner-authored Google Chat requests

Date: 2026-10-26 (Asia/Seoul)

## Scope and cause

The owner reported that a request such as #4390 appeared only after someone else replied. The current production source was `1d8010658598747fc1a98e64a33a205b88152f00`.

Two server read-model defects reproduced this behavior: owner-only conversations were marked reviewed by `all()` over an empty external-message iterator, and unlinked owner conversations classified other than `new_task` were removed from the candidate response entirely. Collection itself accepts owner messages.

## Changes

- Owner-only conversations use their actual review timestamps. Posting a request is not reviewing it.
- Unlinked owner requests remain available in the existing uncertainty queue if classification is not `new_task`.
- Existing mixed conversations still ignore a simple owner reply when computing whether an external request was reviewed.
- Linked owner follow-ups and app non-task messages keep their existing suppression behavior.
- Agent instructions explicitly analyze owner-authored action requests without requiring an external reply, while retaining the non-task and linked-follow-up rules.
- No database migration, request/response schema change, authentication change or production client asset change.
- Existing self-authored push/Mac notification suppression remains unchanged; this fix concerns classification and visibility in the app.

## Target files

- `apps/api/src/lib.rs`
- `apps/api/src/inflow_visibility_tests.rs`
- `apps/agent/src/inflow_analysis.rs`
- `apps/desktop/src/components/HomeInflowReview.test.ts`
- This QA record.

Pre-existing changes to `docs/PROJECT_PLAN.md` and `docs/qa/2026-10-20-itsm-task-sync.md` are outside this commit and are preserved.

## Verification

- Red phase: 7 new API scenarios executed before the fix; 4 failed on owner-only visibility/review state, 3 existing-suppression cases passed.
- Green phase: all 7 new scenarios passed after the fix.
- API unit suite: 123 passed.
- Storage unit suite: 90 passed.
- Agent unit suite: 94 passed; explicit live smoke tests are opt-in.
- Frontend: 80 files / 494 tests passed, including owner new-request and uncertainty rendering cases.
- TypeScript and scoped Prettier passed.
- Isolated PostgreSQL: all 57 scenarios passed against actual migrated databases. The test-only container was removed by the script cleanup.
- API/Agent warnings-as-errors Clippy and formatting passed, including final checks after all test-fixture additions.
- Frontend build passed; the existing >500 kB main-chunk warning remains (503.66 kB / gzip 139.05 kB).
- Backend harness was invoked from `apps/api` to avoid scanning unrelated directories: exit 0, 1 text file scanned. This scanner does not include Rust source extensions; its result is not treated as Rust source validation. Rust formatting, Clippy, tests, build and manual scoped review supply the relevant gate.
- Initial live AI smoke could not start with the globally installed CLI 0.157.0 because this repository pins 0.159.0. The global installation was not changed. A separate official npm-cached 0.159.0 runtime is used for the retry.
- Actual live AI retry passed with pinned CLI 0.159.0: one synthetic owner-authored message, no external reply, classified `new_task`, assignee extracted and action items populated. No real provider data or business actions were used. The other explicit ITSM live smoke remains opt-in/unexecuted for this change.
- API/Agent optimized release build passed.
- Desktop/mobile browser QA uses actual React components and styles with isolated synthetic response fixtures and a callback instead of any server writes. Initial fixture-loader failures (CJS import shape, UTF-8 encoding and test-page HMR exports) were corrected in the temporary QA harness, not application code. Promotion without a deadline requires the existing explicit no-deadline switch; after selecting it, both viewport flows passed.
- Browser evidence: desktop 1440×1000 and mobile 390×844, owner-only new request visible, assignment modal opens with the expected assignee, synthetic registration callback succeeds, no horizontal overflow or page JavaScript errors. Dialog screenshots wait for entrance animation completion. No physical phone or installed app was replaced for QA.
- Screenshots: `/Users/jimin/.codex/visualizations/2026/07/10/019f4aaf-8eb3-7920-a27b-0ae41399b7b0/owner-inflow-20261026/`.

## Backend gate review

- Formatter/lint/test/build commands exist in the workspace and changed services.
- No request body, endpoint, schema or authorization guard changed.
- New behavior uses already owner-scoped stored message data; it does not query another owner or expand provider permissions.
- No credential or private provider payload was added to code or logs; smoke fixtures are synthetic.
- No migration or historical DB update is required. Existing explicit review timestamps and decisions are preserved.
- Deployment must keep schema 63, current data/settings/OAuth credentials and the transcription image unchanged. Application recovery must use the current schema-63-compatible release, not the older schema-58 `previous.env` or historical snapshot.

## Deployment

User explicitly requested tests followed by production deployment. The authorized infrastructure thread `019f1288-7a14-7121-b49c-e66b246b2382` completed read-only preflight: current SHA/schema matched, 5 services healthy/restart 0, no active jobs or waiting locks, SSH/Twingate/health checks healthy. Exact tested source SHA and immutable image references will be supplied before cutover.

No real business task, Google Chat message, webhook, push or Redmine ticket was created or changed for QA. Production deployment and post-cutover read-only verification remain pending.
