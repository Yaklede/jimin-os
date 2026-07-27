# UX Writing Run Manifest

Status: complete

## Target Files

- `TERMS.md`
- `apps/desktop/src/copy.ts`
- `apps/desktop/src/copy/projects.ts`
- `crates/storage/src/intelligence.rs`
- `crates/storage/src/push.rs`

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: ko
- Product concept: 새 업무와 실제 지연을 구분해 설명하는 개인 AI 비서
- Tone: 쉽고 직접적인 해요체, 상태를 과장하지 않는 중립적 표현

## Copy Review

- Korean: 새로 추가한 일을 `밀린 일`로 단정하지 않고 `열린 일 변화`로 표현
- English: 이번 범위에 새 공개 영문 문구 없음
- Terms: `밀린 일`을 제거하고 실제 지연은 `기한 지난 일`, `정체된 일`로 구분
- Error messages: 다시 열기와 상세 조회 실패 문구에 새로고침·재선택 행동을 짧게 안내
- Buttons and CTAs: 이번 범위에 버튼 문구 변경 없음
- Empty/loading/success states: 주간 변화가 없을 때 `열린 일 수는 그대로예요`로 안내
- Naming: 내부 `backlog` 필드는 공개 UI에서 `열린 일 변화`로 표현

## Rewrites

| Before | After | Reason |
| --- | --- | --- |
| 밀린 일 변화 | 열린 일 변화 | 새 업무 생성과 실제 지연을 구분하기 위해 |
| 밀린 일이 1개 늘었어요 | 열린 일이 1개 늘었어요 | 현재 열린 업무 수의 변화를 중립적으로 설명하기 위해 |
| 계속 들어오는 일을 처리량과 밀린 일로 확인해요 | 계속 들어오는 일을 유입량과 처리량으로 확인해요 | 운영형 프로젝트의 흐름을 정확히 설명하기 위해 |
| 할 일을 다시 열지 못했어요. 최신 상태를 확인한 뒤 다시 눌러 주세요. | 다시 열지 못했어요. 새로고침 후 시도해 주세요. | 사용자가 할 복구 행동을 짧고 구체적으로 안내하기 위해 |
| 상세 내용을 불러오지 못했어요. 잠시 후 다시 선택해 주세요. | 불러오지 못했어요. 잠시 후 다시 선택해 주세요. | 중복 명사를 줄이고 재시도 행동을 유지하기 위해 |

## Exceptions

없음.
