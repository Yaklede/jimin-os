# UX Writing Run Manifest

Status: handoff

## Target Files

- `apps/desktop/src/copy/scheduledWork.ts`
- `apps/desktop/src/components/ScheduledWorkPanel.tsx`
- `apps/api/src/scheduled_work.rs`
- `crates/storage/src/scheduled_work/execution.rs`
- `apps/agent/src/worker_loop.rs`

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: ko-KR
- Product concept: 반복 업무를 최신 데이터로 처리하는 개인 비서
- Tone: 해요체, 수행 결과와 다음 행동을 분리

## Copy Review

- Korean: 예약 업무, 할 일, 기한을 기존 용어와 통일
- English: Google Chat 고유명 유지
- Terms: 사용자에게 worker, cron, outbox 등 내부 용어를 노출하지 않음
- Error messages: 조회 실패와 조건에 맞는 일이 없는 경우를 구분
- Buttons and CTAs: 예약 만들기 / 전송 내용 미리보기 / 확인하고 예약 시작
- Empty/loading/success states: 초안 저장은 실행·전송 완료로 표현하지 않음
- Naming: 예약 업무, 후속 확인, 이번만 건너뛰기

## Rewrites

| Before | After | Reason |
| --- | --- | --- |
| 발송 완료 | 전송 중 / 처리 완료 / 확인 필요 | 실제 외부 전달 결과를 구분 |
| 밀린 일 | 기한이 지난 일 | 신규 일감을 지연 상태로 오해하지 않도록 구분 |

## Exceptions

없음. 기존 파일의 관련 없는 문구는 이번 수정 범위에 포함하지 않음.

## Evidence

문구·에러 복구·버튼·빈 결과·초안/실행 상태 checklist 완료. TSX 정규식의 비문구 오탐은 `docs/qa/2026-09-14-scheduled-work.md`에서 구분하고 실제 렌더링으로 확인함.
