# 운영·개발 대화방 분리 및 Open Deck 통합

Status: verified locally; production rollout pending

## 실행 계약

- 운영방은 업무 수집, 개발자방은 배정·완료 보고 역할로 연결한다. 기존 연결·예약은 명시적으로 바꾸기 전까지 유지한다.
- 대화방 역할은 수집용 / 배정·완료 보고용 / 둘 다이며, 보고 전용 방의 일반 대화는 업무 후보로 만들지 않는다.
- 프로젝트별 배정 목적지와 멘션 명단 연결을 명시적으로 선택한다. 활성화 전에는 기존 웹훅 전송을 유지하며 활성화 후 Google Chat 중복 배정·완료 웹훅은 보내지 않는다. Discord와 원본 CS 답글은 유지한다.
- 일반 배정도 기존 회사 OAuth로 보내 실제 메시지·스레드·task/version 영수증을 저장한다. 해당 스레드의 현재 담당자만 완료 보고할 수 있다.
- 한 작업만 담긴 배정 스레드는 첫 줄 `완료` 또는 `완료 T-번호`를 허용한다. 여러 작업을 담는 예약 스레드는 `완료 T-번호`가 필요하다. 일상 문장에서 완료를 추측하지 않는다.
- 담당자 보고 → 확인 대기·Mac/Android 알림 → 소유자 승인 → OS/원본 CS/개발자방/Redmine 완료 순서를 유지한다. 완료 전송은 제목만 간략히 보낸다.
- UUID/소유자/프로젝트/대화방/보낸 사람/현재 담당자/일감 버전/실제 전송 영수증을 대조하고, 재시도·중복·동시 요청을 검증한다.
- Open Deck 4개 커밋을 최신 코드에 병합한다. snapshot·일정 생성·일감 완료·화면 이동·재연결·보안 경계를 회귀 검증한다.

## UI 범위와 상태

- 기존 프로젝트 연결/업무 수집과 예약 편집 구조를 유지한다. 대화방 역할 선택과 작은 배정 연결 폼만 추가한다.
- 기존 neutral/primary 토큰, Wanted Sans, 44px 이상 조작 영역, 16px 모바일 입력, visible focus를 재사용한다. 새 motion/dependency/palette는 추가하지 않는다.
- 명시적 저장, 요청 중 중복 차단, 실패와 재시도 안내, stale version 충돌을 처리한다. 프로젝트 변경/언마운트 시 오래된 응답을 버린다.
- 검증: 데스크탑/모바일·light/dark·키보드·reduced motion·overflow·빈/로딩/실패·저장 후 재조회. 운영 화면과 실기기는 개발 테스트에 사용하지 않는다.

## 검증·배포 게이트

- [x] Open Deck 병합 후 TypeScript / 전용 16 tests
- [x] source 역할·routing API 인증/소유권·OpenAPI·입력/버전 검증
- [x] 실제 격리 PostgreSQL 65→66 및 원본 데이터·기존 예약·기존 웹훅 회귀
- [x] 일반 배정의 OAuth 요청 형식·영수증·완료 보고·승인/반려·중복/권한/충돌 (합성 DB/단위 테스트; 실제 전송 미실행)
- [x] frontend 전체·Rust fmt/clippy/tests/build·Android compile/release-mode dev
- [x] desktop/mobile 개발 화면 실제 조작
- [ ] Open Deck 최신 Mac 연동 및 기존 protocol/capability 회귀
- [x] Backend/UX Writing gates (Rust 보완 및 비공개 코드 오탐은 아래 기록)
- [ ] exact SHA 공식 이미지, 최신 백업·격리 복원·66 전진 복구 준비 후 운영 전환
- [ ] 운영 Mac/Android 업데이트, native 기동·설치 hash·Open Deck 읽기 확인

실제 업무의 승인·완료, 실제 Google Chat/FCM/Redmine 시험 전송은 별도 전용 테스트 승인 없이 실행하지 않는다. 운영 설정을 자동 활성화하거나 현재 열린 예약 초안을 폐기하지 않는다.

## 발견 사항

- Open Deck `codex/open-deck-bridge`의 4개 커밋이 정식 배포에서 누락됐다. 최신 코드와 병합하여 앞으로 일반 배포에도 포함되도록 한다.
- 의존성 symlink가 있는 checkout에서 pnpm 자동 재설치가 no-TTY로 중단됐다. 기존 의존성은 변경하지 않았으며 `pnpm_config_verify_deps_before_run=false`로 검사해 통과했다.
- 기존 `postgres.rs`의 webhook projection 기대값이 완료 알림에도 상세 필드를 요구했다. 운영 기준 SHA `474f914`도 이미 제목만 보내므로, 완료 이벤트의 엄격한 allowlist를 상세 필드 없이 수정했다. 다른 이벤트의 상세/비공개 정보 제한은 유지했다.
- 전체 OpenAPI 경로 스냅샷에 이번 두 경로를 추가했다. 인증·버전·purpose enum 계약은 별도 API 테스트로 검증했다.
- 새 패널의 다시 불러오기 버튼이 기본 스타일에서 44px보다 작아 명시적인 44px 터치 영역을 적용했다.
- Android 일반 dev identifier config는 생성된 Java 경로와 달라 빌드가 거부됐다. 기존 Android 전용 dev config와 `jiminDevPackage=true`를 사용하고 APK ID가 `io.jimin.os.dev`임을 확인했다. 생성 폴더 삭제/재생성 및 실기기 개발 앱 설치는 하지 않았다.
- Android API34 WebView의 기존 appearance-control 뒤로가기 handler가 닫힌 패널에도 `matches(':popover-open')`를 호출했다. 이 WebView는 해당 selector를 지원하지 않아 DOMException이 발생하고 이후 route handler가 호출되지 않았다. native 평가 결과의 정확한 예외를 확인했으며, popover API 지원 확인 및 selector guard만 추가했다(전역 예외 무시로 녹음/모달 보호를 우회하지 않음). 비지원/부분지원/정상 지원 회귀 2 tests를 추가했다. 서버 변경 없는 클라이언트 전용 보완이다.

## 검증 결과

- frontend: TypeScript, Prettier(변경 파일), 최종 86개 파일 531 tests 통과.
- storage: `scripts/test-postgres-integration.sh` 57개 격리 DB 시나리오 통과. `scripts/test-scheduled-chat.sh` 3개 DB 계약 통과(65→66 재실행, 기존 예약 보존, 역할·권한·버전·승인/반려·중복·CS/Redmine 원자성 포함).
- Rust: API 128 / storage 93 / desktop 15 lib tests, fmt, API/storage/desktop all-targets clippy, API/native build. Open Deck native 전용 5 tests와 frontend pending-review 409 회귀도 포함.
- `cargo test --workspace --quiet` 전체 통과. 환경 변수 없이 early-return하는 DB 테스트를 성공 근거로 세지 않고, 위 격리 DB 57+3 실행 결과를 별도로 사용했다. 네트워크 live smoke 등 5 ignored tests는 자동 실행하지 않았다.
- browser: 로컬 합성 preview에서 1440px light/dark, 390px dark, 360px light. 저장/재조회/503/409/중복 submit/해제 확인·Escape·focus 복귀/수집 역할 변경/44px·16px 입력/가로 overflow·pageerror 없음 확인. `/tmp/jimin-chat-routing-qa.mjs`, `/tmp/jimin-routing-{width}-{theme}.png`.
- Android: 별도 `Jimin_Routing_Oct28_API34`, `emulator-5582`, release-mode dev APK 빌드/ID 확인/설치/실제 탭·native select 조작. 개인 휴대폰/태블릿은 테스트하지 않음.
- Backend harness 통과(이 harness는 Rust AST를 읽지 않으므로 Cargo 및 실 DB 계약으로 보완).
- UX Writing: 변경 공개 문구를 WRITING/TERMS 기준으로 직접 점검했다. copy/scheduledChat.ts harness 통과. chatRouting.ts의 두 `*Error:` 키만 있는 줄과 TSX의 null/undefined/error 변수/기존 placeholder 속성은 정규식의 비공개 코드 오탐이다. 각 실제 오류 문구에는 실패 사실과 다시 확인/저장할 행동이 있다. 문구 예외를 승인하거나 실패를 숨긴 것은 아님.

아직 운영 외부 전송/실업무 승인 테스트 및 최종 클라이언트 교체는 하지 않았다. 실제 개발자방 선택은 사용자 확인 후 활성화하며, 기존 수집/예약/웹훅 목적지를 임의 변경하지 않는다. 운영 전환 결과는 아래에 별도 기록한다.
