# Design PR #1 integration / production regression

## Source and safety

- Production baseline: e958112069564d2d7cf529b706478067c53ef5c3 (`codex/scheduled-work`).
- Design: 158ab9202bf61f4cb280b6416a9469dc3f02f4c5 (PR #1).
- Integrate both histories, never replace the latest functional branch with stale main.
- Production apps stay unchanged during isolated QA. Never send real Chat/FCM messages or change business tasks for a smoke test.
- Server deployment belongs to the designated infrastructure thread. Preserve transcription images and persistent data; no migration change is expected.

## Required checklist

- [x] Conflict resolution preserves latest production callbacks and state.
- [x] Backend/API contracts unchanged; Android-only visual bridge justified below.
- [x] New/existing inflow tabs, reviewed history, exclusion reason/reply and draft preservation.
- [x] Received-date month/week calendar, full scrollable queue, detail and original links.
- [x] Task detail/edit/complete/restore and assignee/deadline verified in isolated preview; parent/subtask logic retained.
- [x] Project scope/category, operational reporting, inflow/integrations/activity and scheduled work retained.
- [x] Conversations/new request persistence, composer and approval/failure recovery retained and covered by existing tests.
- [x] Gmail/ITSM/Chat mentions/source acknowledgments and notification settings: implementations unchanged; backend contracts pass.
- [x] Meeting audio/pad/close confirmation, native signals/FCM retained; native back/relaunch passes on emulator.
- [x] Frontend tests, typecheck and formatting pass; production macOS build/origin verification passes.
- [x] Backend tests/Clippy/format, isolated PostgreSQL and assistant/client/mobile contracts pass.
- [x] Dark/light and all five colors persist across reload; 320/390/430/1440/2000 widths have no page overflow.
- [x] Scoped keyboard/tab/detail focus checked; reduced-motion rules retained; loading/error/empty/populated rendering checked.
- [x] Release contains production origin, no preview fixtures or localhost overrides.
- [x] Reviewed merge, infrastructure readiness check and available production desktop installation; Android release prepared, physical installation unavailable without a connected device.

## Issue ledger

| Issue | Resolution | Verification |
| --- | --- | --- |
| PR targets stale main, 13 production commits absent | Integrate from latest production branch | Both histories retained in integration tree |
| Six merge conflicts across inflow/tasks/projects | Preserve functional side plus design additions | No conflict markers; typecheck/tests pass |
| Design Home inflow omitted task-open callback | Restore callback in relocated panel | Existing update opens its project/task destination |
| Design decision cards removed direct review actions | Keep shared inflow actions with new tokens | Existing review/retry/source reply callbacks retained |
| Calendar/production inflow both use fixed element IDs | Scope titles by new/existing tab | No duplicate DOM IDs in populated preview |
| Assistant result quick completion disappeared | Restore TaskSelectionControl and fix its grid columns | Selection → named completion → restore manually verified |
| Completed records reopened by default | Restore production collapsed default | Manual and regression test |
| Existing update calendar described unregistered candidates | Separate scope copy for existing tasks | Scoped copy and markup test |
| Preview mark-seen shape differed from real API | Use decision=mark_seen | Preview test and manual read-state refresh |
| Native bars used OS mode instead of chosen app mode | Strict light/dark-only visual bridge; no device-data access | Android compiles, native light/dark screenshot and back smoke |
| Secret scanner treated task-selection CSS as an API key | Require token boundary for sk- prefix | Full security scan passes; real-key boundary retained |
| Production build could inherit preview flag | Force VITE_DESIGN_PREVIEW=0 in private client builder | Production assets exclude preview chunks and use private origin |

## Evidence

- Frontend: prettier, TypeScript, Vitest — 379/379 tests in 62 files passed.
- Rust: cargo fmt --all --check; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings.
- PostgreSQL: scripts/test-postgres-integration.sh, 52 isolated scenarios passed; assistant:contracts reran successfully.
- Client build configuration and mobile QA safety guard tests passed; security scan passed.
- CUA isolated preview: registration title/deadline draft survives new/existing tab switch; registering adds one task; reason dismissal and mark-seen remove request from attention; full loaded queue stays scrollable; task completion/restoration/edit saves; links remain clickable; calendar month/week and appearance settings work.
- Emulator: io.jimin.os.dev only; cold start, accessibility tree, native back, relaunch passed. Light/dark toggles keep system-bar icons readable; native Back closes the appearance popover, then calendar-to-home Back works. MainActivity changes only system-bar appearance; existing back and zoom restrictions unchanged.
- Emulator proof: /tmp/jimin-design-integration-qa-20261006-theme/2026-10-06T05-15-34-842Z/02-restored.png; /tmp/jimin-design-integration-light.png; /tmp/jimin-design-integration-dark.png.
- Native/third-party limits: no physical device connected; no real Chat send, OAuth reconnect, FCM push, microphone recording or production business-task mutation performed. These are not claimed as live end-to-end verified. Their production implementations and backend tests are retained.
- Known design exception from the incoming PR: light blue #4594fb with white action text has about 3.05:1 contrast, below small-text AA. This is a documented historical user-approved choice in DESIGN.md, not a new blanket accessibility approval.

## Deployment

- PR #2 was merged using the reviewed integration head ee6898e10c075aad7452648bcf21035fe4095320. PR #1 is also confirmed merged. Production main is 78308a3f7c13d92cd8a14f1041c1f3707c4d552f; its tree 11a7ea2ec788295e5542988c31875b2ca68202d6 is identical to the tested integration tree.
- The designated infrastructure thread confirmed that Nginx/Caddy serve API only, not static web UI (/ and /index.html both 404). Backend/API/Agent/migrations/production Compose are unchanged, so no unnecessary rebuild/restart/migration was performed.
- Server verification: backend 7414ed94aba418fee43ca25c572d20b4288e7e16, schema 57/57/57, migration failures 0; five services healthy; internal and Twingate live/ready checks 200. Existing image digests, persistent data, secrets and rollback backups preserved. Server-side UI asset verification is not applicable because no UI assets are hosted there.
- Production macOS uses io.jimin.os and the same stable signing identity/team (9L5V75TP2R) as the previous app. Signed candidate was staged, signature-verified and atomically installed at /Applications/Jimin OS.app. Installed executable SHA-256: 4bddaf7df28695f8659b76e6df0f549b06be21b36d59ce4cf178b72365786be8. Rollback copy: /tmp/jimin-os-release-design-integration-20261006/previous-installed.app; additionally preserved the actual replaced bundle as replaced-installed.app in that directory.
- Android production release build passed, including production-origin and APK safety checks (io.jimin.os, non-debuggable, arm64-only, size limit, valid signature). APK: /Users/jimin/Desktop/study/jimin-os/releases/design-integration-20261006/jimin-os-production-arm64.apk; SHA-256 502efcb46681a36fbfb75f6adbb2337b6b5b8a2842b70d5e332953b4dedda26b. Only emulator-5554 is currently connected. No physical-device installation is claimed.
- The user first requested waiting for an active request; no quit occurred during that wait. When the request remained in processing, the user explicitly asked to restart. The app was then quit via native UI and relaunched after installation. The prior request result reloaded from the real server after restart; its answer requested missing task/ITSM identifiers. No resend or business-task mutation was performed by this release workflow.
- Installed production UI verified with the real server: today tasks 5, scheduled work 1 with next run 17:30, new inflow 3/existing update 1, calendar/list controls, real source links and original/analysis disclosures, theme/color popover. User subsequently interacted with the updated UI, so further automated production clicks were stopped to avoid interfering. Proof: /tmp/jimin-design-integration-production.png.
- Isolated browser viewport override was reset, the agent-created preview tab/server and emulator were stopped, the temporary native QA script removed. Emulator-only debug APK was moved out of the workspace build output to the release rollback directory; it was never installed on a physical device. Production release APK remains available separately.
- Server confirmation: /Users/jimin/Desktop/local-pc/reports/2026-10-06-jimin-os-design-integration-server-check.md.
