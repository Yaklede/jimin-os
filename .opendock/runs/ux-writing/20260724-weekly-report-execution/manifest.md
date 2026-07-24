# UX Writing Run Manifest

Status: complete

## Target Files

- `apps/desktop/src/copy.ts`
- `apps/desktop/src/copy/projects.ts`
- `crates/storage/src/push.rs`

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: Korean
- Product concept: 사용자의 업무 흐름을 먼저 파악하고 다음 행동까지 연결하는 개인 AI 비서
- Tone: 간결하고 구체적인 존댓말, 사용자가 바로 할 수 있는 행동 중심

## Copy Review

- Korean: 쉬운 말과 자연스러운 능동형을 사용했습니다.
- English: 이번 작업에서 새로 노출한 영문 문구가 없습니다.
- Terms: 내부 구현 용어 대신 `주간 운영 리포트`, `지난 주간 리포트`, `이번 주 먼저 처리할 일`을 사용했습니다.
- Error messages: 이번 작업에서 새 오류 문구를 추가하지 않았습니다.
- Buttons and CTAs: `열린 일 확인하기`, 할 일별 완료·상세·수정 행동으로 다음 동작을 명확히 했습니다.
- Empty/loading/success states: 지난 리포트가 없을 때 다음 주부터 변화가 쌓인다는 안내를 제공합니다.
- Naming: 저장된 리포트와 현재 리포트를 혼동하지 않도록 현재·지난 주간 리포트를 구분했습니다.

## Rewrites

| Before | After | Reason |
| --- | --- | --- |
| 주간 수치만 표시 | 이번 주 먼저 처리할 일 | 요약을 실제 행동으로 연결하기 위해 |
| 이력 없음 | 아직 비교할 지난 리포트가 없어요. 다음 주부터 변화가 쌓여요. | 현재 상태와 다음 변화를 함께 설명하기 위해 |

## Exceptions

없음.
