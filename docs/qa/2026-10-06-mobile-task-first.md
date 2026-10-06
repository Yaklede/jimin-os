# Mobile task-first development review — 2026-10-06

## Scope and isolation

- Branch: `codex/mobile-task-first`; baseline `b9f5021952f66fe1c1de257165314b32dabafa6d`.
- Development checkout only. Production server, `/Applications/Jimin OS.app`, physical devices and production records were not modified.
- Preview: `http://localhost:1425/`, explicitly enabled development fixtures (8 open tasks, new/existing conversations).
- No backend, migration, new UI dependency or palette changes.

## Implemented

- Five equally sized mobile navigation items: home, projects, schedule, meetings, more. Active icon/text emphasis without a filled navigation tile or protruding microphone.
- Mobile home starts with tasks: today, time-overdue, all; assignee/date grouping. Today means due today in Seoul, not all open work. Undated work remains in All.
- Separate expand/completion controls, inline task detail and links, existing verification-result completion flow retained.
- Authenticated all-open task API rather than a capped home snapshot. A stale home response cannot resurrect an ID excluded by a successful all-open response.
- Recurrence, incoming conversations, mail/report/decision content and previous assistant results are collapsed on phones. Desktop structures preserved.
- Incoming conversation list/detail switch on phones with an explicit back action; existing new/existing tabs and promotion modal retained.
- Five compact project tabs; optional project information collapsed, report remains accessible from its tab. Task titles and assignment badges have narrower-screen spacing.
- Full-height mobile planning forms with fixed heading/actions and independently scrolling fields; date and hour/minute controls align. Changed drafts require explicit discard and are not reset by a refresh of the same task.
- Keyboard-aware viewport/nav CSS and opacity-only mobile route motion with reduced-motion support; effects clean up listeners/timers.

## Evidence

- `npm run typecheck`: pass.
- `npm run build` without preview environment: pass; build output contained no design-preview chunk.
- `npm test`: 69 files, 427 tests passed (includes API, Seoul boundary, 177-item no-truncation, multi-assignee, stale snapshot and navigation coverage).
- `git diff --check`: pass.
- Browser 320×740, 390×844, 430×932 and 1440×900: no horizontal overflow at phone sizes; five navigation destinations and five project tabs reachable; desktop mobile queue absent and sidebar retained.
- Manual: all-task/date grouping, task expansion, title and 21:30 deadline save, development-task completion, verification-result cancel, discard confirmation, More/Escape, incoming conversation selection/back/promotion form.
- Screenshots: `/tmp/jimin-os-mobile-home-20261006.png`, `/tmp/jimin-os-mobile-project-20261006.png`, `/tmp/jimin-os-mobile-editor-20261006.png`.

## Issues and outstanding gates

- Android first debug build passed before final refinements. Latest Rust release compilation passed, but final DEV APK packaging did not complete; no stale APK installed.
- An intermediate native build ran out of disk space. Only this run's newly created Android debug build directory was removed, not pre-existing caches or user data. A later Gradle build failed reading workspace metadata. Restarting its daemon moved past that error; final retry waited on SSL/HTTP dependency reads (confirmed via JVM thread inspection), so the verified task-owned Gradle wrapper was stopped. No network/VPN/security setting changed.
- Native emulator UI automation could not bind the command-line emulator executable. Android keyboard, safe-area and physical back interactions remain unverified; responsive browser checks do not substitute for native QA.
- UX writing contracts and exposed new copy manually reviewed. Raw-source writing harness reports 309 findings dominated by code identifiers (`undefined`, `null`) and pre-existing strings; not claimed as a pass or a human-approved exception. Precision design/interaction ultrawork was not requested and not run.
- No production deployment or physical-device installation in this review.

## Review steps

1. Open preview at phone width; change Today → All, then Assignee → Date.
2. Expand a task, edit title/time, attempt closing to check draft protection, save.
3. Complete a development task; complete a verification task and check its result-entry dialog.
4. Expand incoming conversations, switch New/Existing, select/back, open task registration.
5. Open Projects → sample project, check all five tabs; use More for settings/decisions/memory.

## Follow-up: deadline card contrast

- User screenshot showed white titles on a full-width pale yellow pill. Cause: deadline badge selectors targeted every direct `span`, including the title/assignee wrapper, replacing its grid with inline-flex and painting the status background behind the title.
- Added a dedicated `home-deadline-brief__due-state` class and narrowed all 15 relevant CSS selectors. Title/assignee remain on the normal surface; only deadline status is a colored pill. No wording, task actions or backend changed.
- Added a regression assertion preventing the broad direct-span selector from returning. Typecheck, production frontend build and all 428 tests passed.
- Browser checked: copy wrapper is transparent/grid, title is `rgb(247,247,248)` on the dark surface; status pill alone has its warning/danger background. Screenshot: `/tmp/jimin-os-deadline-contrast-20261006.png`.
- Development only; no production deployment.

## Release preparation follow-up

- Final typecheck and all 428 tests passed again; client build configuration checks and six Mobile MCP QA guard tests passed.
- Latest DEV release APK packaged successfully (12,222,480 bytes) and was installed only on explicit `emulator-5554`, package `io.jimin.os.dev`. The previously stalled dependency reads completed; no security/network setting was changed.
- Native smoke passed cold start, accessibility, Back and relaunch. Additional native checks passed all/date grouping, task expansion, edit with Korean keyboard, keyboard dismissal retaining the editor, scroll to date/hour/minute and fixed footer, editor Back, More and Back. Screenshots: `/tmp/jimin-mobile-native-home-20261006.png`, `/tmp/jimin-mobile-native-editor-20261006.png`, `/tmp/jimin-mobile-native-editor-deadline-20261006.png`.
- Infrastructure chat verified existing production SHA `6c4a84eca0c2efbd36255484e2c6f72eaf5eb6b4`, schema 58, five healthy services and live/ready HTTP 200. Authenticated all-open task GET returned 27 unique tasks matching the database scope without pagination or omissions. Hosting is API-only; no server image rebuild, migration or separate static frontend deployment is needed.
- Production macOS build passed with private server URL and no preview fixtures. Android production build/install and installed-app smoke are recorded separately in the deployment report.
