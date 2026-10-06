# UX Writing Run Manifest

Status: complete

## Target Files

- `crates/storage/src/scheduled_work/execution.rs`

## Writing Contract

- WRITING.md reviewed: yes
- TERMS.md reviewed: yes
- Locale: ko-KR
- Product concept: 공동 담당자에게 반복 업무를 안내하는 개인 비서
- Tone: 기존 해요체와 할 일·담당자 용어 유지

## Copy Review

- Korean: 담당자 목록은 쉼표와 공백으로 읽기 쉽게 구분한다.
- English: 새 영어 사용자 문구는 추가하지 않는다.
- Terms: 기존 담당자·할 일·멘션 용어를 유지한다.
- Error messages: 실제 미등록 이름만 표시하고 연결에서 등록하도록 안내한다.
- Buttons and CTAs: 변경 없음. 기존 예약 시작 절차를 유지한다.
- Empty/loading/success states: 빈 담당자는 담당자 미정으로 표시하고 잘못된 멘션을 만들지 않는다.
- Naming: 기능 이름 변경 없음.

## Rewrites

| Before | After | Reason |
| --- | --- | --- |
| @송인준, 김경주 | @김경주, @송인준 | 정리된 이름 각각을 실제 멘션 대상으로 표시 |
| 등록된 두 이름 전체를 미등록이라고 안내 | 실제로 명부에 없는 이름만 안내 | 사용자가 이름을 다시 등록하는 불필요한 작업 방지 |

## Exceptions

없음. Rust를 지원하지 않는 정규식 도구의 한계와 실제 검사 근거는 QA 문서에 구분했다. 자동 문구 검사 통과로 주장하지 않는다.
