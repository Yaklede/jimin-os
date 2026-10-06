# Home candidate review and registration

Date: 2026-10-06

This change is saved in the local workspace and preview. It has not been deployed to Vercel in this request.

## Behavior

- Home reviews AI-suggested Google Chat candidates by the date received. Schedule displays registered tasks by their deadlines.
- “할 일로 정리하기” opens the editable title, notes, assignee, priority and deadline form. “등록하기” submits it.
- “업무 아님” marks a candidate dismissed and excludes it from pending review; the original message and decision history remain.
- The preview supports registration and dismissal in session memory. Reloading restores its synthetic fixtures. It never sends Chat notifications or contacts the live API.
- The existing live API workflow is retained, including version/revision validation and post-decision refresh.

## Cause and changes

Light-theme border removal and matching field/detail backgrounds obscured inputs. A more specific generic flex selector also overrode the registration grid. The form now uses a distinct surface, visible neutral fields and a native keyboard-operable no-deadline switch. Title, notes and deadline span the form; assignee and priority share a row at sufficient widths and stack on narrow screens. Explicit accessible names identify controls.

The old preview rejected every decision POST, preventing demonstration of the flow. Its isolated adapter now validates decisions, adds exactly one task on registration, and retains dismissed/promoted history. Conflict or invalid input leaves candidate/task state unchanged.

## Verification

- Vitest: 55 files, 340 tests passed. Four preview decision tests cover soft dismissal/history, selected registration values and duplicate rejection, explicit no-deadline choice, stale revision and invalid assignee.
- TypeScript and production preview build passed.
- Browser: registration changed pending candidates 26 → 25 and tasks 8 → 9. The registered task appeared on October 7 with the selected deadline. Dismissal changed candidates 25 → 24 without adding a task.
- Opening focuses the title; cancellation returns focus to the opening action. Missing deadline blocks submission and focuses the date. Space toggles no-deadline and disables/re-enables date/time controls.
- Light and dark form fields, select arrows, native date/time icons and both switch states were checked.
- At 320px and 390px, form/date controls stack and stay within the viewport. At 1280px, title/notes/deadline span both tracks. This is browser responsive testing, not a physical-device test.

Screenshot: `/tmp/jimin-candidate-registration-light.png`.
