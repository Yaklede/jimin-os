# Android native layout correction — 2026-10-06

## Scope and isolation

- Dedicated checkout: `/Users/jimin/.codex/worktrees/android-ui-layout/jimin-os`.
- Branch: `codex/android-ui-layout-fix`, based on deployed implementation `1ff7d0ae85797dff73c793963b0233e4bd370704`.
- Changed only phone layout CSS and its regression tests. Main thread's uncommitted optional-completion-reply changes remain untouched.
- No API, database, server deployment, macOS installation, account reset or live task mutation.
- Changes are saved separately; they have not been merged into the main implementation checkout. Future releases must include this branch to retain the fix.

## Findings and fixes

- Translucent fixed bottom navigation allowed body text to show behind its labels. Navigation is now opaque, with one shared height/clearance calculation and a minimum bottom inset.
- Native phone header retained extra top clearance and desktop greeting line heights/motion. Phone-only rules now use compact spacing and a stable greeting.
- More menu was flush with the right screen edge. Added the standard 16px phone gutter.
- Strengthened visually-hidden content clipping without removing it from accessibility.
- The partial outline at the physical screen's left edge also appears in Android Settings and is consistent with Samsung's Edge panel handle, not an app layout element. No system setting was changed.

## Verification

- Typecheck, formatting and diff whitespace checks passed.
- All 70 frontend test files / 432 tests passed.
- Browser checks at 320, 411 and 430px for web/Android/iOS platform selectors: no horizontal overflow, opaque navigation, correct bottom clearance and compact greeting.
- Simulated 24px safe bottom inset: navigation 88px, content clearance 104px. More menu right gutter 16px.
- Desktop 1440px: sidebar retained, mobile navigation hidden, page heading unchanged.
- Physical Samsung device: fixture-only separate DEV package tested all/date grouping, inline detail, editor with Korean keyboard and visible save actions, projects and More menu. No production task was completed or edited.
- Isolated build initially lacked generated TauriActivity/ProGuard files because of cloned build caches. Restored generated files from the matching existing build; successful native launch verified before production installation. This required no tracked native source change.
- Final production package `io.jimin.os` updated with `adb install -r`, unchanged signer/versionCode, arm64-only and non-debuggable. Production asset check passed: private server URL, no design-preview chunk or loopback test URL.
- Existing authentication/data preserved. Home displayed today 4 / overdue 6 / all 26 after update. Current-process AndroidRuntime error output empty. More menu gutter visually verified on the production package.
- Pulled installed APK byte-compared equal to the release artifact. Temporary DEV package and device QA XML removed; production home left foreground.

## Artifacts

- Production APK: `outputs/android-ui-20261006/jimin-os-production-arm64.apk`.
- Size: 12,201,372 bytes; SHA-256: `f43152ff0f1d07618fb3474e66c738e8c0d3d62c2d2ec403a440113c630eba6b`.
- Rollback APK: `outputs/android-ui-20261006/previous-production.apk`.
- Screenshots: `/tmp/jimin-native-ui-production-home.png`, `/tmp/jimin-native-ui-production-more.png`, `/tmp/jimin-native-ui-dev-editor.png`.
- Native checks are bounded UI checks, not an assertion that every mobile feature, provider, recording or notification flow was exhaustively tested.

## Follow-up: missing microphone and incomplete queue selection

- User correctly reported that the bottom microphone was still missing. The deployed baseline had removed the launcher from `OsShell`; the first CSS-only correction did not restore it. The earlier completion claim did not cover this regression.
- Restored the existing microphone/voice-sheet handler alongside Home, Projects, Schedule, Meetings and More. Six bounded columns keep every existing destination directly accessible. The microphone is an in-flow 44px control, not an overlapping floating button.
- A permanently reserved scrollbar gutter left an unfilled strip on short inflow lists. Use an automatic gutter and extend the selection accent over the full row height. The real single-request card in the user's screenshot now fills the queue width on-device.
- Changes additionally target `OsShell.tsx`, its tests, and inflow selection CSS/tests. No new UI wording, API or provider behavior was introduced.
- All 70 frontend test files / 434 tests passed; typecheck, formatting and whitespace checks passed.
- Browser checks: web/Android/iOS selectors at 320/411/430px, light/dark and reduced motion. No horizontal overflow; all six touch targets at least 44px. Voice dialog opens/closes and More stays within the viewport. Desktop navigation remains unchanged/hidden.
- Physical production device: microphone visible, all destinations aligned, More popup within screen, and the same real inflow request's background and accent fill the row. Live speech recognition was not exercised to avoid unintentionally submitting a production command. Voice dialog activation was exercised in the isolated browser fixture.
- Installed production package with update-compatible signer, non-debuggable arm64 release, production server assets and no preview data. Authentication retained; no task or source-chat write performed. Main branch remains unmerged.
- Follow-up APK: `outputs/android-ui-20261006/jimin-os-production-mic-restored-arm64.apk`; SHA-256 `2425542a28c280de1958e489dd742ee129ad8682e2f89aeecd0801531c428489`.
- Screenshots: `/tmp/jimin-native-ui-mic-restored-home.png`, `/tmp/jimin-native-ui-mic-restored-more.png`, `/tmp/jimin-native-ui-inflow-full-fill.png`.
