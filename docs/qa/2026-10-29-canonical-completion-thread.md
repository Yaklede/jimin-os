# 완료 알림의 배정 스레드 통일

## 범위와 계약

- 직접 완료와 완료 보고 승인은 같은 `task_chat_deliveries` 완료 outbox를 사용한다.
- 같은 사용자·프로젝트·수신 소스·일감의 실제 전송 성공한 배정 중 최신 버전의 스레드에 `제목 · 완료되었어요.` 한 건을 보낸다. 보고가 올라온 예약 메시지 스레드는 완료 목적지가 아니다.
- 첫 전송 시점에 성공한 배정 스레드가 없을 때만 채팅방에 새 메시지를 보낸다. 목적지는 전송 전에 영구 저장하고 재시도 때 바꾸지 않는다.
- 배정 스레드가 삭제되었거나 접근할 수 없으면 기존 재시도·실패 처리를 따르며 채팅방으로 우회하지 않는다.
- 동일 일감 버전의 완료 알림은 유일 제약으로 중복 생성하지 않는다. 전송 응답 유실·409 충돌 시 같은 provider message ID로 조회하며 추가 메시지를 만들지 않는다.
- 프로젝트 배정 경로가 없는 기존 예약 완료 보고도 승인된 원본 전송 기록을 확인한 뒤 같은 outbox를 사용한다. 실제 배정 기록이 없으면 채팅방으로 보낸다.
- 기존 CS 원문 완료 답글, Redmine 완료 동기화, Discord 알림, 재작업 요청 답글, 완료 확인 대기 알림은 유지한다.
- 새 공개 API·요청 body·클라이언트 계약·마이그레이션은 없다. 스키마 66을 유지한다.

## 배포 경계와 중복 방지

- 완료 outbox에 routing version 1을 기록한다. 전송 전 lease/attempt/owner와 현재 소스·경로·계정 권한을 재검증하고 목적지를 고정한다.
- 예전 worker가 이미 시도한 version 없는 완료 outbox는 채팅방 목적지를 유지한다. 응답이 유실된 기존 글과 다른 스레드로 보내 중복을 만들지 않는다. 첫 시도 전인 기존 outbox는 배정 스레드를 찾는다.
- version 1인 새 outbox가 목적지 저장 전에 실패한 경우에는 다음 시도에도 배정 스레드를 선택할 수 있다.
- 승인용 별도 `review_approved` 최종 답글은 새로 만들지 않는다. 이미 전송한 이력은 그대로 둔다.
- 운영 전환 전에 예전 `review_approved`의 queued/retry_wait/sending/failed가 없는지 확인한다. 과거 완료 알림 재처리, Chat 커서 초기화, 실제 일감 시험 완료는 하지 않는다.

## 검증

격리 PostgreSQL와 loopback HTTP fixture만 사용한다. 운영 계정·CS·Redmine·FCM에 시험 메시지를 보내지 않는다.

- PostgreSQL 전용 계약: 직접 완료, 보고 승인, 배정 기록 없음, 다른 일감 기록 제외, 최신 성공 배정, 배정 ACK 지연, 고정된 목적지 유지, lease/attempt 거부, receipt 스레드 불일치 거부, 중복 ACK 거부, 동시 중복 승인, CS·Redmine·Discord 각 한 건 유지.
- 기존 예약 보고 계약: 프로젝트 배정 경로 없는 승인도 권한 있는 완료 outbox와 채팅방 fallback으로 전송된다. 혼합·일괄 보고와 재작업 흐름도 유지한다.
- HTTP fixture: Google이 수락한 뒤 응답 유실 → 재시도 409 → 같은 ID GET; 요청 ID·본문·스레드 동일. 선택 스레드 404 때 새 글 없음. 다른 스레드 receipt 거부. 스페이스 교차·잘못된 ID 거부.
- Google API 계약: [공식 메시지 생성 문서](https://developers.google.com/workspace/chat/api/reference/rest/v1/spaces.messages/create)의 `requestId`, `messageId`, `REPLY_MESSAGE_OR_FAIL`을 따른다.

## 품질 게이트

- `cargo fmt --all --check`, 변경 서비스 4개 패키지 all-targets clippy `-D warnings`, API/Google/Storage 단위 테스트 128/46/101개와 API/Agent build: 통과.
- `cargo test --workspace`: 통과. 실제 모델 계정으로 전송하는 live smoke 2개는 실행하지 않았다. 기본 실행에서 ignore된 DB 전용 계약은 아래 전용 명령으로 별도 실행했다.
- `bash scripts/test-scheduled-chat.sh`: 격리 PostgreSQL의 예약·배정·완료 4개 계약 통과. 배포 경계의 예전 재시도/새 outbox 구분, revoked source와 expired lease 거부를 포함한다.
- `bash scripts/test-postgres-integration.sh`: 실제 격리 PostgreSQL 57개 시나리오 통과.
- 프런트엔드 타입 검사·86개 파일 531개 회귀 테스트: 통과. 기존 의존성 링크를 보존하기 위해 pnpm 자동 dependency 재설치 검사를 끄고 실행했다. 화면·native 변경은 없어 앱 재설치 대상이 아니다.
- Backend Ultrawork static harness는 이번 target와 package script 계약 복사본에 한정한다. 해당 스캐너는 Rust AST/SQL 권한 검증 도구가 아니므로 Rust clippy·컴파일·실제 DB 계약 및 수동 보안 검토로 보완한다.
- 새로운 인증 경로 없음. 기존 owner 인증과 task/review version guard, source/account/user/project 범위, 바인딩 SQL 유지. 새 로그·비밀·외부 임의 URL·클라이언트 지정 전송 목적지 없음.
- 이번 target만 복사한 Backend Ultrawork 정적 harness: 통과, 실패·승인 예외 없음. 스캐너가 Rust 확장자를 분석하지 않는 한계는 위의 실제 Rust/DB 검증으로 보완했다.
- 중간 검증에서 발견한 clippy 스타일 지적 2건과 비활성 소스가 `None` 대신 기존 계약의 `InvalidConfiguration`으로 거부된다는 테스트 기대값 1건을 수정하고 최종 명령을 다시 통과했다. 운영 로직을 느슨하게 변경하거나 테스트를 제외하지 않았다.

## 운영 배포

배포 실행·SHA·image digest·backup/restore 증거는 인프라 담당 세션의 최종 기록으로 확정한다. PostgreSQL·전사 이미지 재생성 없이 API/Agent만 업데이트한다. 운영 완료 이력은 재전송하지 않는다.
