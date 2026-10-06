# UX Writing Run Manifest

Status: complete

## Target Files

- `apps/desktop/src/components/TaskWorkKind.tsx`
- `apps/desktop/src/components/TaskCompletionDialog.tsx`
- `apps/desktop/src/components/DeadlinePicker.tsx`
- `apps/desktop/src/copy/deadlinePicker.ts`
- `apps/desktop/src/components/InflowPromotionDialog.tsx`
- `apps/desktop/src/components/PlanningCreateDialog.tsx`
- `apps/desktop/src/components/PlanningItemEditor.tsx`
- `apps/desktop/src/components/ProjectInflowPanel.tsx`
- `apps/desktop/src/components/GmailInflowReview.tsx`
- `apps/desktop/src/components/ProjectsWorkspace.tsx`
- `apps/desktop/src/components/AssistantInteractiveCanvas.tsx`
- `apps/desktop/src/components/HomeWorkspace.tsx`
- `apps/desktop/src/components/PlanningWorkspace.tsx`
- `docs/qa/2026-10-06-task-work-kind-copy.md`

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: ko; existing service names retained
- Product concept: review incoming requests, assign work, record and share completion results
- Tone: clear, professional, 해요체

## Copy Review

- Korean: 확인 업무 / 개발 업무 / 일반 업무, 날짜 / 시간 / 분 distinguish actions and fields.
- English: Google Chat is the existing public provider name; no new English prose.
- Terms: 할 일 for a result to complete; 기한 for its deadline; 한국 시간 for timezone preview.
- Error messages: preserve input on failure; conflict explains how to reopen the latest task.
- Buttons and CTAs: 결과를 남기고 완료하기 / 결과 없이 완료하기 / 달력 열기.
- Empty/loading/success states: saving disables repeated submission; cancellation never reports completion.
- Naming: 업무 유형 identifies classification without claiming AI automatically chose it.

## Rewrites

| Before | After | Reason |
| --- | --- | --- |
| Native date/time input with restricted minutes | 날짜, 시, 분 and explicit 달력 열기 | Make each choice discoverable and touch-accessible. |
| Completion without a result entry | 확인 결과 남기기 | Explain what is saved and when the original Chat receives a reply. |

## Exceptions

None for new copy. Unrelated pre-existing copy is outside this task.

## Harness evidence

Raw source scan flags language syntax as public terminology. New rendered copy was compared with the source and checked through the copy evidence file plus `deadlinePicker.ts`: pass. Raw-source findings and the scoped review limitation are recorded in `docs/qa/2026-10-06-task-work-kind.md`; no claim is made that the raw component scan passed.
