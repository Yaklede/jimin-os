# UX Writing Run Manifest

Status: ready-for-review

## Target Files

- `apps/desktop/src/copy.ts`

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: ko-KR
- Product concept: 개인·회사 메일에서 실행할 업무를 먼저 정리하는 개인 AI 비서
- Tone: 짧고 자연스러운 존댓말, 사용자의 다음 행동을 바로 안내

## Copy Review

- Korean: 확인 완료
- English: 이번 범위에 없음
- Terms: 사용자 화면에 내부 구현 용어를 노출하지 않음
- Error messages: 발생한 문제와 재시도 방법을 함께 제공
- Buttons and CTAs: `할 일로 정리`, `이번에는 제외`, `나중에 보기`처럼 결과가 분명한 동사 사용
- Empty/loading/success states: 로딩·빈 화면·전체 실패·부분 실패 문구 제공
- Naming: 개인/회사 워크스페이스와 Gmail 원문을 구분

## Rewrites

| Before | After | Reason |
| --- | --- | --- |
| 없음 | 새로 확인할 메일 | 들어온 메일 전체가 아니라 확인이 필요한 업무 후보임을 명시 |
| 없음 | 선택한 시간이 되면 홈에 다시 보여드려요. | `나중에 보기`가 언제 다시 나타나는지 설명 |
| 없음 | 보낸 사람 정보 없음 | Gmail 원본 필드가 비어 있을 때 빈 UI를 방지 |
| 일반 처리 실패 | 이 메일은 다른 곳에서 먼저 처리됐어요. 메일을 다시 확인해 주세요. | 409 충돌의 원인과 다음 행동을 구체적으로 안내 |
| 내부 분석 코드 | 원문은 그대로 보관했어요. 다시 분석해도 안 되면 Gmail 원문을 확인해 주세요. | 내부 코드를 노출하지 않고 복구 행동을 안내 |
| 각 워크스페이스의 최신 메일 100개부터 확인해요. 이전 메일은 더 보기로 이어서 불러올 수 있어요. | 확인할 메일을 100개씩 보여줘요. 더 보기가 나타나면 이어서 확인할 수 있어요. | 내부 조회 단위보다 사용자가 화면에서 할 수 있는 행동을 중심으로 안내 |
| 일부 메일을 불러오지 못했어요. | 회사 워크스페이스의 메일이나 프로젝트를 불러오지 못했어요. 전체를 다시 확인해 주세요. | 실패한 범위와 복구 행동을 구체적으로 안내 |

## Exceptions

- 없음
