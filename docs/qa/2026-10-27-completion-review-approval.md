# 완료 보고 → 소유자 최종 확인 — 2026-10-27

Branch: `codex/scheduled-chat-completion`, base `567f6dc`.

## 적용 범위

- 기존 회사 OAuth 예약 알림의 담당자 완료 답글은 즉시 완료가 아니라 완료 확인 대기다.
- Mac의 기존 native 알림 피드와 Android FCM에 완료 확인 요청을 추가했다. Android 알림 클릭은 홈의 해당 보고로 이동한다.
- 홈에 미확인 완료 보고만 간략히 표시한다. 보고별 메모·현재 내용·담당자·마감을 펼쳐 읽고 확인하거나 재작업을 요청한다.
- 일반 완료, 일감 수정, AI 직접 상태 변경은 대기 보고를 우회하지 못한다.
- 최종 확인과 CS·Redmine·완료 웹훅 예약은 원자적이다. 외부 전송 성공과 최종 확인 저장 성공은 구분한다.
- 완료 웹훅은 제목과 ‘완료되었어요.’만 전송한다. 이전 상세 payload도 provider 직전 단계에서 축약한다. 신규 배정 상세는 그대로 보존한다.
- 운영 배포·운영 데이터 변경·실기기 설치·외부 Chat/Redmine 전송은 하지 않았다.

## 검증

| 항목 | 결과 |
| --- | --- |
| API 단위·인증 guard·OpenAPI | 125 통과 |
| Storage 단위 | 92 통과 |
| Google 단위 | 42 통과 |
| Native 알림 plugin / Mac desktop 단위 | 2 / 10 통과 |
| 실제 격리 PostgreSQL 계약 / 기존 예약 회귀 | 2 통과, 63→65 업그레이드·재실행 포함 |
| Frontend | 82 파일, 503 통과 |
| TypeScript / Vite | 통과, 기존 500KB chunk 경고 유지 |
| Rust fmt / Clippy -D warnings / API build | 통과 |
| Android 알림 plugin Kotlin compile | 통과 |

DB 계약은 실제 운영 주소나 인증정보를 사용하지 않는 `scripts/test-scheduled-chat.sh`로 검증했다.

- 보고 시 task completed=0, CS 완료 큐=0, Redmine 큐=0, 완료 웹훅=0
- 두 담당자의 동시 보고에도 pending 1건, Android 알림 1건
- owner-scoped Mac feed 포함, push UUIDv7 및 claim/ack 정상
- 다른 소유자 조회·승인 차단
- 일반 완료와 AI 직접 완료 우회 차단
- 수정된 일감의 오래된 승인 차단, 새 버전 확인 후 중복 승인에도 외부 큐 1회
- 완료 메모 보존, 재작업 사유 필수, 반려 후 재보고 가능
- 반려 재시도에서 다른 사유를 조용히 덮어쓰지 않음, 원본 회사 연결을 삭제해도 pending 보고 보존
- Google Chat/Discord의 상세·담당자 멘션을 포함한 오래된 완료 이벤트도 제목만 렌더링

## 화면 확인

실제 `CompletionReviewPanel`과 CSS에 5개의 격리 모의 보고를 표시했다. Chrome headless 1440×1000 dark / 390×1000 light, reduced motion으로 확인했다.

- 기본 3개 → 전체 5개 펼침, 현재 내용·마감 보기
- 재작업 사유가 없을 때 버튼 비활성, 사유 포함 전송
- 완료 확인의 review/task 버전 전송
- 409 충돌 시 재조회, 입력 중인 사유 보존, 자동 승인 없음
- 가로 넘침 없음, 44px 이상 작업 버튼, JavaScript page error 없음
- 검토 중 내용이 바뀌면 버튼을 막고 ‘새 내용 확인’을 요구한다.
- 성공 안내는 6초 뒤 숨기며 빈 성공 카드가 홈에 남지 않는다.
- 캡처: `/tmp/jimin-completion-review-1440.png`, `/tmp/jimin-completion-review-390.png`

검토 중 갱신 차단과 성공 카드 정리는 두 viewport에서 별도 재검증했다. 개발 mock 소스 2개와 이 작업의 Vite 서버는 검증 후 정리했다.

## 품질 게이트·발견 사항

- Backend Ultrawork는 `apps/api` 범위에서 통과했다. Rust를 파싱하지 않으므로 Cargo와 실제 DB 계약으로 별도 검증했다.
- UX Writing 게이트는 신규 copy 2개에서 통과했다. TSX 전체 검사는 `token`, `undefined`, `setError` 코드와 정상 placeholder 속성까지 문구로 오인한다. 해당 진단은 공개 문구가 아니며 수동으로 렌더링 문구·버튼·상태·복구 안내를 확인했다. Rust 문구도 수동 확인했다. harness 자체는 수정하지 않았다.
- Android 첫 컴파일은 SDK 경로가 없어 실패했고, 기존 SDK를 `ANDROID_HOME`으로 지정한 후 통과했다. 외부 Tauri의 기존 deprecation 경고만 남는다.
- 브라우저 첫 mock glob이 하위 decision 경로를 포함하지 않아 409 시뮬레이션이 실패했다. 경로 정규식을 수정한 뒤 두 viewport의 실제 버튼 요청까지 다시 통과했다.
- AI UpdateTask는 기존 validator가 completed 입력을 거절하며, SetTaskStatus는 신규 대기 guard로 차단한다. 두 경로를 DB 계약에서 구분해 검증했다.

## 미실행 및 배포 유의

실제 Mac 알림 배너·Android FCM 수신·Google 답글·Redmine 완료는 운영 계정으로 시험하지 않았다. 실제 운영 완료를 유발하므로 별도 승인/배포 뒤 전용 테스트 일감으로 확인해야 한다.

서버/schema 65 → 화면 → Android 순서로 적용한다. Mac native feed는 기존 payload 호환이며 앱이 백그라운드 실행 중이고 알림이 허용되어야 한다. Mac 앱을 완전히 종료했을 때도 알림을 전달하는 별도 시스템 서비스는 추가하지 않았다. Android 새 알림 이동은 클라이언트 갱신이 필요하다. 기존 물리 기기·운영 앱은 건드리지 않았다.

DB downgrade/drop은 하지 않는다. 문제 발생 시 예약을 정지하고 pending 보고와 outbox를 보존하는 호환 복구 버전을 사용한다. 배포 전 승인된 백업을 확보한다.
