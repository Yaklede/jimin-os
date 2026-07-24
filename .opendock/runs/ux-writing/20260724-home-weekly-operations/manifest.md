# UX Writing Run Manifest

Status: complete

## Target Files

- `apps/desktop/src/components/HomeWorkspace.tsx`
- `apps/desktop/src/copy.ts`

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: ko
- Product concept: 사용자의 업무 흐름을 판단하고 실행까지 연결하는 개인 AI 비서
- Tone: 쉽고 직접적인 해요체, 현재 상태와 다음 확인 대상을 함께 안내

## Copy Review

- Korean: 운영형 프로젝트를 완료율 대신 새 일, 완료한 일, 밀린 일, 정체된 일로 설명
- English: 이번 범위에 새 공개 영문 문구 없음
- Terms: 기존 `밀린 일`, `정체된 일` 계약을 그대로 사용
- Error messages: 이번 범위에 새 오류 문구 없음
- Buttons and CTAs: `결과 접기`로 현재 요청 결과를 닫는 행동을 명확하게 표시
- Empty/loading/success states: 문제가 없는 주간 흐름도 별도 문장으로 안내
- Naming: 내부 API와 집계 구현 용어를 노출하지 않음

## Rewrites

| Before | After | Reason |
| --- | --- | --- |
| 운영형 프로젝트도 완료율과 100% 완료 기준으로 판단 | 이번 주 새 일, 완료한 일, 밀린 일, 정체된 일로 판단 | 계속 운영되는 프로젝트에 잘못된 완료 기대를 만들지 않기 위해 |
| 처리 결과를 닫는 명시적 행동 없음 | `결과 접기` 제공 | 홈을 다시 현재 브리핑 중심으로 돌리기 위해 |

## Exceptions

없음.
