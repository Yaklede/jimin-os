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
