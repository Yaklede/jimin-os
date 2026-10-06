# UX Writing Run Manifest

Status: reviewed

## Target Files

- `apps/desktop/src/copy/projects.ts`
- `apps/desktop/src/components/HomeInflowReview.tsx`
- `apps/desktop/src/components/ProjectInflowPanel.tsx`
- `apps/api/src/google_chat_oauth.rs`

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: Korean
- Product concept: 업무를 정리하고 처리하는 개인 비서
- Tone: 자연스러운 해요체, 행동이 분명한 버튼

## Copy Review

- Korean: 새 요청과 기존 일감의 추가 대화를 구분해요.
- English: 새 공개 영어 문구 없음.
- Terms: 확인함과 업무 아님을 구분해요. 내부 구현 용어는 화면에 쓰지 않아요.
- Error messages: 사유 저장 성공과 답글 전송 실패를 따로 설명하고 재전송을 안내해요.
- Buttons and CTAs: 확인했어요, 업무 아님으로 저장하기, 사유 답글 다시 보내기.
- Empty/loading/success states: 저장 중, 새 내용 있음, 확인함.
- Naming: 새 업무 요청, 기존 일감 업데이트, 확인한 대화.

## Rewrites

| Before | After | Reason |
| --- | --- | --- |
| 확인 완료 (대화를 제외함) | 확인했어요 (읽음만 저장함) | 확인과 제외의 다른 결과를 분명히 알려요. |
| 새로운 업무 요청을 정리했어요 | 새 업무 요청 / 기존 일감 업데이트 | 중복 등록과 신규 요청을 구분해요. |

## Exceptions

기존 문구를 전면 교정하지 않아요. 이번 작업에서 추가한 문구와 변경한 영역만 확인해요.
