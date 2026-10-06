# Representative image and scoped UI regression fixes

## Scope

- Change the hamster representative image locally; restore the default.
- Prevent stretched appearance-popover rows and excessive blank space.
- Restore the home follow-up new-request action removed during design integration.
- Keep expanded home inflow assignment forms reachable instead of clipping them.
- Preview-only archive/create responses allow safe new-request interaction testing.

## Causes and changes

- The continuation still had its handler but no rendered new-request button. Restored the button even when results are collapsed. Busy states disable it without hiding it; callback failures preserve existing input and report a recovery action.
- Appearance sizing depended on popover/Grid free-space distribution. Explicit content sizing, max-content rows and 44px/68px controls prevent stretching.
- Inflow selection had a maximum height but its implicit auto Grid row grew to 1,143px. The outer card clipped it while the detail itself had no overflow. A zero-minimum flexible track and viewport-bounded maximum now produce a real detail scroll region. Mobile retains natural page scrolling.

## Verification

- Frontend: 401 tests passed in 64 files; typecheck and Prettier check passed.
- `git diff --check`: passed.
- Image upload, reload persistence, unsupported-file rejection preserving prior image, and default restore: verified in isolated localhost preview.
- Image resize, MIME/data URL validation, quota/storage failure, abort cleanup and subscription cleanup: unit tests passed.
- New request: visible with collapsed results at 390px; successful preview archive/create clears the continuation and focuses the new composer. Completed/failed/cancelled/declined and queued/running/waiting/retry states covered by regression tests.
- Appearance: 320×640, 430×932 and 1440×1400 layout measurements verified. Mode controls remain 44px and color controls 68px. Short viewports scroll within the panel. macOS WebKit Dev appearance panel was also visually checked.
- Assignment: desktop list and calendar detail scroll to the register action. At 1280×720, list detail became 544px high with 1,143px scroll content; calendar detail also scrolled and its register button was visible after keyboard navigation.
- Mobile 390×844: detail overflow remains visible with no horizontal page overflow. Example task registered with 김경주 and today's 23:45 deadline; preview task count changed from 8 to 9. No real task or external Chat message was created.
- Final macOS Dev bundle built successfully and verified with stable code signing. WebKit detail scrolling reached the register/cancel buttons in a short window; the restored new-request action also switched to an empty composer successfully.
- Scoped UX writing checker: 21 static-code false positives (internal error identifiers, `null`, `undefined`, object key), documented in the writing manifest; not reported as a clean pass. Rendered public copy manually checked.

## Evidence

- `/tmp/jimin-representative-image-qa/new-request-mobile.png`
- `/tmp/jimin-representative-image-qa/inflow-assignment-desktop.png`
- `/tmp/jimin-representative-image-qa/inflow-assignment-mobile.png`
- `/tmp/jimin-representative-image-qa/inflow-assignment-macos.png`

## Release boundary

These changes are development-only. No production server migration, production app replacement or physical Android installation was performed. Native verification uses the separate `io.jimin.os.dev` bundle. Representative-image preferences are device-local, not cross-device synced. Native Android file-picker behavior remains unverified on a physical device.
