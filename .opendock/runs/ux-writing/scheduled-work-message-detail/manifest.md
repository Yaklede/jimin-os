# UX Writing Run Manifest

Status: complete

## Target Files

- `apps/desktop/src/components/ScheduledWorkPanel.tsx`
- `crates/storage/src/scheduled_work/execution.rs`

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: ko
- Product concept: 예약해 둔 일을 담당자별로 쉽게 확인하는 개인 비서
- Tone: 짧은 해요체, 할 일과 일정 구분 유지

## Copy Review

- Korean: 제목만 / 제목 + 내용, 알림에 표시할 내용
- English: Google Chat 고유명사 유지
- Terms: 할 일, 마감일, 담당자, 일정
- Error messages: 변경 없음; 기존 다음 행동 안내 유지
- Buttons and CTAs: 기존 미리보기, 저장, 시작하기 유지
- Empty/loading/success states: 기존 상태 유지; 표시 내용 변경 시 미리보기 무효화
- Naming: 예약마다 설정하며 일반 일감 배정 메시지는 변경하지 않음

## Rewrites

| Before | After | Reason |
| --- | --- | --- |
| 모든 알림에 프로젝트·상세·완료 기준·링크 포함 | 기본은 담당자 아래 제목 (마감일·시간) 목록 | 확인해야 할 일을 빠르게 훑어보기 |
| 표시 내용 설정 없음 | 제목만 / 제목 + 내용 | 필요한 상세 수준을 예약별로 선택 |
| 2026년 9월 16일 18:00 | 9월 16일 18:00 마감 | 같은 해는 간결하게, 다른 해는 연도 유지 |

## Exceptions

- 없음. 기한 없는 일은 `기한 없음`, 미지정 담당자는 `담당자 미정`으로 명시. 정규식 도구 오탐과 Rust 미지원 한계는 QA 문서에 구분함.
