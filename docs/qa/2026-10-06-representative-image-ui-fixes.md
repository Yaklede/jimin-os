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
- Release inspection found native CSP needed `blob:` in the image directive for the local file decoding step. Only image blob loading was added; object/plugin content remains disallowed. A production-config regression test guards this requirement.

## Verification

- Frontend: 402 tests passed in 64 files after the native image-CSP regression was added; typecheck and Prettier check passed.
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

## Development verification boundary (before deployment authorization)

At this verification stage the changes were development-only. No production server migration, production app replacement or physical Android installation had been performed. Native verification used the separate `io.jimin.os.dev` bundle. Representative-image preferences are device-local, not cross-device synced. Native Android file-picker behavior remains unverified on a physical device.

## Production release (subsequent user authorization)

- User explicitly requested production deployment after development QA.
- PR #3 merged: `https://github.com/Yaklede/jimin-os/pull/3`. Main `b7346be5b5d8029b579d5032a3b405568acf333b` has the same tree as reviewed release head `d4a85a7`; no code changes were introduced by merge.
- Production macOS build passed with `VITE_DESIGN_PREVIEW=0`, `VITE_LOCAL_PHONE_TEST=0`, and `https://os.jimin.ai.kr`. Assets have no preview-home, preview-inflow, design-preview fixtures or loopback server override.
- Signed installed bundle `/Applications/Jimin OS.app`: identifier `io.jimin.os`, stable signing team `9L5V75TP2R`; strict signature verification passed. Executable SHA-256: `1c5e947c871c958802930072221403e3b5eff67cb2733068e0b99afa910d54e0`.
- Previous installed bundle preserved at `/tmp/jimin-os-release-ui-fixes-20261006/previous-installed.app`; actual replaced bundle also preserved as `replaced-installed.app`.
- The running app was not quit because a real assignment draft was open. Restart approval was requested; until approved or manually restarted, the running process still uses the previous executable. Installation alone is not claimed as live new-UI verification.
- Build-configuration guards, six mobile QA safety tests and secret scan passed.
- Designated infrastructure task confirmed backend code, migrations and production Compose unchanged: backend `7414ed94…`, schema 57, five services healthy, internal/Twingate live and ready 200. No API restart, transcriber image pull, migration or server deployment was needed. Report: `/Users/jimin/Desktop/local-pc/reports/2026-10-06-jimin-os-representative-image-server-check.md`.
- Production Android arm64 release build and APK safety verification passed: application ID `io.jimin.os`, non-debuggable, signed, one ABI, size 12,192,300 bytes (below the 12 MiB cap). APK: `/Users/jimin/Desktop/study/jimin-os/releases/ui-fixes-20261006/jimin-os-production-arm64.apk`; SHA-256 `1024fc13fffd1703cadc0765ba0eb2cbab972e8b602c7ba09a6563445500bc88`.
- Android production assets contain the private-server origin and no preview fixtures or loopback override. The temporary Firebase build configuration was removed by the build helper.
- Final `adb devices -l` inventory is empty. No physical Android installation or physical file-picker verification is claimed.
- Final macOS signature and installed executable checksum checks passed; the old process (PID 3602) remains running while restart approval is pending.
