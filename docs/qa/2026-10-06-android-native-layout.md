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
