# Inflow registration modal

## Scope and contract

- Incoming Chat requests remain read-only in the home/project detail area; registration opens a native modal using the shared InflowItemRow.
- Desktop: centered modal, independently scrolling fields, fixed registration/cancel footer. Mobile: full-screen surface, safe-area padding, no horizontal overflow.
- Reuse the existing promotion request, deadline conversion, assignee/priority, source reaction/reply and automatic Chat-notification policy. No backend or notification contract changes.
- Preserve modified draft fields during source revisions and persist the active draft using the existing session storage. Restoring a draft does not automatically reopen the modal.
- Cancel, close, backdrop, Escape and native mobile-back paths protect modified input. Pending submission blocks closing. Confirmation allows continued editing or explicit discard.
- Native dialog top layer isolates background interactions. Background scroll is locked and restored on cleanup. Keyboard focus wraps within the dialog and returns to the opener when still present.
- New-reply context is available inside the modal, not behind its inert backdrop. Existing latest-analysis readiness guards remain enforced.

## Verification

- Frontend: 407 tests / 65 files passed; TypeScript, scoped Prettier and client build passed.
- Unit regressions cover close/confirm/block policies, busy close control, dialog labels and saved-draft restoration without automatic reopening. Existing revision merge, deadline and inflow regressions passed.
- Isolated browser preview: 1280×720, 390×844 and 320×640. Registration footer remained in viewport; body content scrolls and page has no horizontal overflow. Temporary viewport override reset after testing.
- At 320×640, dialog covered 320×640 and footer occupied y=564–640. At 390×844, footer occupied y=768–844.
- Modified title survived cancellation confirmation/continue and full page reload. Explicit discard restored the suggested title when reopened.
- Clean Escape restored focus to the registration opener. Dirty Escape/backdrop/cancel opened confirmation. Tab and Shift+Tab wrap between the first/last modal controls.
- Missing deadline retained the form and displayed recovery guidance. Preview registration with 김경주 and today's 23:45 deadline succeeded; task count changed from 8 to 9 and modal closed. Root/body scroll locks were cleared afterwards.
- Scoped UX-writing static checker reports 68 code-only false positives (null/undefined, placeholder prop and error references); rendered public copy manually reviewed. A clean static-harness pass is not claimed.

## Evidence and boundaries

- `/tmp/jimin-inflow-modal-qa-20261006/desktop.jpg`
- `/tmp/jimin-inflow-modal-qa-20261006/mobile.jpg`
- Preview remains at `http://localhost:1424/` with synthetic data only.
- No production task or Google Chat message was created. No production installation, server deployment or migration performed in this task.
- Physical Android native-back/keyboard and macOS WebKit modal interaction remain to be verified in a separate development build; browser responsive checks and mobile-back routing implementation are not claimed as physical-device QA.

## Authorized production release — 2026-10-06

- User explicitly requested production deployment after the development implementation.
- PR #4 merged: https://github.com/Yaklede/jimin-os/pull/4. Reviewed source commit `aed0643c3231ad3eefa7ab717a5ce2aee5208dd6`; merge commit `4cbf2bdc2cecdc6d8128165e62d6530e5a5baca4`. Source trees match for apps/services/packages/scripts/deploy.
- Both client builds passed the production-origin and preview-disabled asset verifier for `https://os.jimin.ai.kr`.
- macOS: stable TeamIdentifier `9L5V75TP2R`, production bundle `io.jimin.os`, signature verified. Installed `/Applications/Jimin OS.app`, quit normally and relaunched through native UI. Production home data loaded successfully; no active recording or unsaved form was present before restart.
- macOS executable SHA-256: `2a77b77be16f6345047f7d71517e2def513574cd8e0d6e447a3f66db1acf3f60`.
- Previous macOS bundle retained at `/tmp/jimin-os-release-inflow-modal-20261006/previous-installed.app` for rollback.
- Android: connected physical SM_S948N updated with `install -r`, after ABI/signature/version compatibility checks. Non-debuggable arm64 release, 12,193,612 bytes; app process/resumed activity verified. No uninstall or data clearing; no localhost API reverse mapping remains.
- Android lastUpdateTime: `2026-10-06 15:16:01`; APK SHA-256: `e5a04d0be535fe79901e870b220de62e2638806a1f0c266ac7ed4275641daaeb`.
- APK retained at `/Users/jimin/Desktop/study/jimin-os/releases/inflow-modal-20261006/jimin-os-production-arm64.apk`.
- Server `/health/live` and `/health/ready` passed; API build `7414ed94aba418fee43ca25c572d20b4288e7e16`, schema 57. This release changes client UI only, so no backend rebuild/restart or migration was performed.
- Native macOS registration interaction QA was not continued while the user was actively operating the app. Android native-back/keyboard QA remains unexecuted; installation/launch checks do not substitute for those tests. No real task or Google Chat message was created for release testing.
