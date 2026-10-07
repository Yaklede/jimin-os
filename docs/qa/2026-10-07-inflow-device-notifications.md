# New work notifications — 2026-10-07

## Scope and cause

- Branch: `codex/pc-inflow-full-fill`.
- The API already queued Google Chat/Gmail inflow deliveries, but Android's native validator only accepted task/schedule types and rejected those deliveries.
- macOS had no native inflow notification path. Its existing local reminder adapter is Android-only.
- Production apps, production databases, provider conversations, and the user's physical phone were not modified for this QA.

## Implemented

- Android accepts Google Chat/Gmail inflow, brief, and weekly-report pushes without broadening the task/schedule alarm API. Expired pushes are discarded, and Unicode text limits match the server.
- Chat notification taps open the relevant project/inflow; Gmail/digests open home. Existing task/schedule routing remains unchanged.
- The server notifies only current, ready actionable analysis and unread pending messages. Linked task status updates use `일감에 새 답글`; new requests use `새 업무 요청`.
- Excludes pre-connection history, messages older than 24 hours, reviewed/dismissed items, disabled sources/accounts, the connected Chat account's own comments, and noise.
- Message revision, rather than analysis metadata version, identifies Android deliveries. Reanalyzing the same message does not send another notification. Newly registered devices do not receive analyses predating their registration.
- macOS has a native 30-second authenticated read-only poller while the app is running/minimized. The first request establishes a server-clock baseline; reopening does not replay the old inbox. Keyset pagination and a two-minute overlap account for delayed commits, with 24-hour in-memory revision deduplication.
- Mac bursts above three items are summarized into one bounded notification per page. Failed dispatches retry. Credentials stay in memory; logout clears configuration. Requests have a timeout, redirects are disabled, and streamed responses are capped at 512 KiB.
- Mac settings include an on/off control, connectivity recovery copy, and truthful running/minimized and OS-permission guidance.
- New `/v1/push/inflow` route requires a live signed session, validates cursor input, is owner-scoped, and is reflected in OpenAPI. No database migration; schema remains 58.

## Verification

| Gate | Result |
| --- | --- |
| Frontend tests | 71 files / 447 tests passed |
| Frontend typecheck + production web build | Passed; preview and local-phone-test flags disabled |
| Production asset guard | Passed for `https://os.jimin.ai.kr` |
| Scoped Prettier + cargo fmt + git diff check | Passed |
| Clippy, affected API/storage/desktop/notification packages, all targets | Passed with `-D warnings` |
| Rust library tests | API 115 + storage 85 + desktop 9 + native notification plugin 2 = 211 passed |
| Real PostgreSQL: new Chat requests / existing-task replies | Passed, including history/read/noise/own-comment exclusion, owner isolation, reanalysis deduplication, and cancellation after review |
| Real PostgreSQL: Gmail lifecycle and native notification feed | Passed; account/workspace/revision/decision/promotion lifecycle and push idempotency retained |
| Real PostgreSQL: task/schedule notification regression | Passed; device ownership transfer and reminder queue idempotency retained |
| Android native build | DEV APK `io.jimin.os.dev` built and installed only on `emulator-5554` |
| Android UI | Home/navigation rendered; notification permission control changed to `알림 켜짐` after interaction without losing the screen |
| Scoped Backend Ultrawork | Passed on an isolated copy of the changed Kotlin/backend targets plus package scripts; this harness does not inspect Rust, so Rust uses the compiler, tests, clippy and manual review above |

PostgreSQL tests ran against disposable local databases in `jimin-inflow-notification-qa-20261007`, not production. The container was removed after the tests.

Screenshots: `/tmp/jimin-inflow-notification-emulator.png`, `/tmp/jimin-inflow-notification-settings.png`.

## Findings fixed during verification

- Notification plugin 2.5.1 conflicted with the existing pinned Tauri plugin version; pinned the compatible notification plugin 2.3.3 instead of upgrading unrelated Tauri packages.
- Refactored native dispatch for testable retry/burst/dedup behavior and clippy compliance.
- Corrected numeric Google Chat provider IDs in test fixtures and adjusted Gmail live-message fixture timing relative to account creation.
- Corrected expected OpenAPI path ordering; added an explicit unsigned-session rejection regression.
- Added a user recovery action to Mac connection-failure copy and corrected scoped test formatting.

UX Writing Ultrawork scans the actual changed TSX/copy targets. Three source-regex findings remain: `event.payload`, React `return null`, and the internal `status === "invalid"` branch. These are not rendered copy; all visible error copy contains a recovery action. No user-approved exceptions were requested or assumed. Rust copy is not covered by that regex scanner and was reviewed manually.

## Limits / not yet verified

- No production deployment or production client replacement in this task. Production enablement requires API deployment, a rebuilt Mac app, and a production Android APK update; a web-only deployment is insufficient.
- Real Firebase delivery, actual OS notification appearance, and a real notification tap have not been exercised. The emulator DEV build deliberately has no production Firebase configuration; permission UI is not evidence of FCM delivery.
- Mac force-quit notifications are not implemented; the native worker must be running. macOS must allow notifications for Jimin OS. The desktop plugin does not provide the Android-style notification action routing.
- Notifications follow existing provider sync and AI analysis, not the exact instant Google receives a message. Offline devices/servers and OS notification policies can delay or suppress delivery.
- Current sync cadence: Chat every minute, Gmail every five minutes, followed by analysis; push delivery worker checks every five seconds and the Mac feed every 30 seconds.
- Full repository regression, production connectivity/permissions, and external-provider mutations were not run. Tests were scoped to this change and existing reminder regressions.
