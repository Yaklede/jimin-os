# Assistant task destination continuity — 2026-10-06

Scope: restore original 일감 보기 naming and reliably reveal the selected task in the project task tab. Preserve current approved styling and production data APIs.

## Findings

- The public Vercel app already contained the 지금 할 일 list. Its renamed 일감확인 action successfully navigated to the project and highlighted the matching task, verified in the browser before editing.
- The original source comparison followed prefers-color-scheme while its preview adapter set an unused theme attribute. This caused its appearance to differ across browser/system settings.

## Changes

- Restore 일감 보기 action wording; correct its task-destination error text.
- Explicit task destinations activate the tasks tab, then focus/scroll the highlighted row after it renders. Task data refresh alone does not force a manually chosen tab back to tasks.
- Comparison snapshot at /tmp/jimin-os-original-view adds a preview-only CSS shim for explicit light/dark queries using the original palettes. Original application source and redesigned theme preferences remain independent.

## Validation

- Keyboard Enter on the web action displayed 지금 할 일 and focused the matching first row.
-390px mobile detail opened directly below 계산 문구 수정; its44px destination action displayed all8 task rows and focused that selected second row.
- Mobile document width375px within390px viewport; desktop width1265px within1280px viewport.
- Explicit original dark canvas: rgb(23,23,32).
- TypeScript passed; Vitest55files/340tests passed; preview production build passed.
- Design, interactive-ui and UX-writing gates plus git diff whitespace check passed.
- Manual weekly-tab selection remained selected after Refresh; task data refresh alone does not force navigation.
- Screenshots: /tmp/jimin-task-destination-desktop.png and /tmp/jimin-task-destination-mobile.png.

This revision is saved locally and has not been deployed. Preview data is synthetic; no external message or real task mutation was performed.
