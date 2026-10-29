# Google Chat 완료 보고의 어순 호환

Status: verified locally; production rollout pending

## 범위와 원인

- 기존 첫 줄 parser는 `완료 T-번호`만 허용해 `T-번호 완료`를 일반 대화로 무시했다. 새 단위 테스트에서 실제 예시 번호를 사용해 `Ignore` 응답을 재현했다.
- 첫 줄에 `완료 T-번호` 또는 `T-번호 완료`가 있을 때 동일한 일감 코드와 다음 줄 완료 메모를 반환한다. 기존 소문자 코드 정규화, 앞뒤 공백, 2,000자 메모 제한을 유지한다.
- 정확한 명령만 허용한다. `미완료`, `완료 예정`, 일상 문장, 잘못된 번호, 여러 번호, 제어 문자는 완료로 추측하지 않는다.
- 예약 알림과 일반 배정은 동일 parser를 사용한다. 단일 일반 배정 스레드의 기존 `완료` 단독 보고도 그대로 유지한다.
- 담당자 신원·소유자·프로젝트·스레드·실제 전송 영수증·현재 버전 검증은 변경하지 않는다. 완료 보고는 확인 대기만 만들며, 소유자 승인 전 OS·CS·Redmine 완료를 금지한다.
- API 계약, 화면, 앱 패키지, 마이그레이션, 수집 커서와 기존 예약 설정은 변경하지 않는다. 배포 전 무시된 과거 답글을 자동 재생하거나 실제 업무를 대신 승인하지 않는다.

## 검증

- [x] 수정 전 새 단위 테스트 2개 실패로 원인 재현.
- [x] storage 단위 테스트 95개 통과(양방향 어순, 실제 예시 번호, 공백·소문자·CRLF·메모, 모호한 번호와 문장·메모 제한 포함).
- [x] `bash scripts/test-scheduled-chat.sh`: 격리 PostgreSQL 예약/일반 배정/예약 실행 계약 3개 통과. 역순 단독 보고의 확인 대기 생성, 양방향·단독 보고의 중복 방지, 메모 보존, 담당자 검증, 승인/반려, OS·CS·Redmine 원자성 확인.
- [x] `bash scripts/test-postgres-integration.sh`: 기존 PostgreSQL 통합 57개 시나리오를 각각 빈 격리 DB에서 실행해 통과.
- [x] `cargo fmt --all -- --check`, API/storage/agent all-targets clippy(-D warnings), API 단위 128 tests 및 API/agent build 통과.
- [x] 변경 파일과 Cargo 계약만 복제한 임시 범위에서 Backend Ultrawork 통과. 이 harness는 Rust AST를 검사하지 않으므로 Cargo와 실제 DB 검증으로 보완했다. 요청·인증·SQL·로그·API·schema 경계의 production 변경은 없고 parser의 명령 어순만 확장했다.
- [ ] 정확한 공식 이미지 SHA 운영 반영과 health/schema 확인.

## 검증 중 발견·반영

- 추가한 거부 시나리오 때문에 합성 feedback이 기존 한 번의 lease 한도(10개)를 넘었다. retry 테스트가 아직 최초 전송을 받지 않은 다음 batch까지 attempt=2로 가정하던 문제를 발견했다. fixture의 최초 batch를 모두 lease한 뒤 재시도 조건을 검사하도록 고쳤다. production lease/재시도 동작이나 assertion을 완화하지 않았다.
- 테스트 스크립트의 기존 실행 권한이 없으므로 파일 권한을 바꾸지 않고 `bash scripts/test-scheduled-chat.sh`로 실행한다.

## 안전·배포

- 운영 테스트 메시지 전송, 실제 승인·완료, 수집 커서 초기화, 과거 답글 재처리는 수행하지 않는다.
- 운영 배포는 기존 로컬 서버 인프라 담당 세션을 통해 수행한다. schema 66 그대로 유지하며 API/필요한 Agent만 교체한다. PostgreSQL/transcriber 이미지 다운로드·재생성, VPN/DNS 변경, Mac/Android 재설치는 하지 않는다.
