# Android UI integration release — 2026-10-06

## Scope

- Integrate Android layout commits `4633d36` and `5daedf9` onto deployed main `75ba28f`.
- Resulting implementation: `d1ae7f7`; original optional completion reply implementation remains present.
- No API, database migration, provider scope, server image or transcription change.
- Restore the mobile microphone control, opaque navigation and safe-area clearance; align the More menu and compact phone greeting.
- Inflow selected rows fill their available width, without an always-reserved scrollbar strip or vertically inset accent.
- This does not redesign the outer PC card layout or change its calendar/list behavior.

## Verification

- TypeScript check passed.
- Frontend: 70 test files, 439 tests passed, including completion reply and existing functional tests.
- Frontend production build, scoped Prettier check and `git diff --check` passed.
- Isolated browser fixtures on port 1426; no production task or Chat message was modified.
- At 320px and 411px: no horizontal overflow; all six navigation columns remain available; microphone 44×44px, other primary destinations at least 48×56px.
- Navigation has opaque surface background. More menu stays inside viewport with gutter.
- Microphone opens the voice sheet and closes correctly. Live recognition was not tested; preview reports no speech captured.
- Inflow selection and registration form open on mobile; date/hour/minute controls remain present.
- At 1440px: mobile navigation hidden; inflow active row width equals queue client width (358px), accent top 0px, scrollbar gutter automatic.
- Existing completion form retains the optional reply field and one `완료하기` button.
- Production web asset verification passed: private server origin, no preview assets or local test URL.
- Production server readiness returned ready, schema 58; server deployment is unnecessary for these client-only changes.

## Deployment

- Client installation and release artifact identities will be appended after verification.
- Physical Android device was not connected at the start of this release.
