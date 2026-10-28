# UX Writing Run Manifest

Status: completed

## Target Files

- `apps/desktop/src/copy/chatRouting.ts`
- `apps/desktop/src/copy/scheduledChat.ts`
- `apps/desktop/src/components/ProjectChatRoutingPanel.tsx`
- `apps/desktop/src/components/ProjectInflowPanel.tsx`

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: ko-KR
- Product concept: 운영 요청 수집과 개발자 배정·완료 보고를 분리하는 개인 업무 비서
- Tone: 해요체, 현재 결과와 다음 행동을 명확하게 표시

## Copy Review

- Korean: reviewed against WRITING.md and TERMS.md
- English: developer-only protocol identifiers are not public copy
- Terms: 수집용 / 배정·완료 보고용 / 둘 다
- Error messages: 실패 사실과 다시 확인/저장할 행동을 포함
- Buttons and CTAs: 연결하기, 저장하기, 연결 해제
- Empty/loading/success states: 실제 연결 상태만 표시
- Naming: 내부 source/receipt/route 명칭은 공개 문구에 사용하지 않음

## Rewrites

| Before | After | Reason |
| --- | --- | --- |
| 확인할 대화에 배정방도 등록 | 수집용과 배정·완료 보고용 역할 선택 | 개발 대화를 업무 후보로 수집하지 않음 |

## Exceptions

None approved. The regex harness flags private TSX null/undefined/error identifiers and the two multiline `*Error:` keys rather than public copy. Actual strings include recovery actions; scheduledChat copy harness passes. See docs/qa/2026-10-28-chat-routing-open-deck.md for scoped evidence.
