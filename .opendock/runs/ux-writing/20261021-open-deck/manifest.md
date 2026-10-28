# UX Writing Run Manifest

Status: reviewed

## Target Files

- `apps/desktop/src/open-deck-copy.ts`
- `apps/desktop/src/components/ScheduledWorkPanel.tsx` (accessible name reuses its adjacent label)

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: ko-KR
- Product concept: one-touch companion for the existing Jimin OS workspace
- Tone: existing Korean 해요체; internal error codes stay internal

## Copy Review

- Korean: fallback titles use 새 업무 요청 and 예약 업무.
- English: protocol identifiers are developer-only.
- Terms: 일정 and 할 일 retain distinct meanings.
- Error messages: safe bounded error codes are translated by Open Deck into actionable Korean messages.
- Buttons and CTAs: existing OS dialog labels are reused.
- Empty/loading/success states: disconnected state clears the companion read model.
- Naming: Jimin OS is the product; OS is the tablet navigation label.

## Rewrites

No existing product copy was changed. Mac navigation reuses current screens and forms. Native tablet write confirmations use the existing 일정/할 일 vocabulary and explain optional Google Chat completion replies.

## Exceptions

The broader repository harness has five unchanged heuristic findings documented in open-deck.md. No new public technical strings or user-facing permissions flow was added to Jimin OS.
