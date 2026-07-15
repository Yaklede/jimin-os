# P0 Completion Verification — 2026-07-15

Status: P0 code and local Desktop/Mobile QA complete; external operational verification remains
Owner: Codex
Scope: 업무·프로젝트·일정·웹훅 명령의 저장 신뢰성, Desktop/Mobile 수동 조작, Google Calendar 연결 준비, Android 로컬 알림, 로컬 운영 회귀

## 완료 판정

P0의 저장·API·Desktop/Mobile 공통 UI 구현과 자동·로컬 실앱 검증을 완료했다. 실제 Google 계정, 개인 Linux 서버, Apple 배포 인증서가 필요한 검증은 코드 완료와 분리해 아래 `외부 조건`에 남겼다.

Android 에뮬레이터에서는 알림 권한·설정, 실제 알림 표시와 자동 정리, 알림 탭 후 대상 일정 이동·강조, 위조 딥링크 거절, 재부팅 후 복원과 오래된 알람 정리를 검증했다. macOS 앱은 최신 로컬 서버를 보도록 `/Applications`에 다시 설치하고 ad-hoc deep/strict 서명 검증과 실행을 확인했다.

## 완료 원칙

각 섹션은 아래 순서를 모두 통과해야 완료로 바꾼다.

1. 관찰 가능한 완료 조건을 체크리스트로 고정한다.
2. 구현하고 formatter, lint, unit/integration test, build를 실행한다.
3. Desktop과 Mobile viewport에서 loading, empty, error, success, disabled, focus, reduced-motion 상태를 확인한다.
4. 발견한 결함을 이 문서의 결함 로그에 추가한다.
5. 결함을 수정하고 같은 검증을 다시 실행한다.
6. 외부 자격 증명이나 실제 장비가 필요한 항목은 코드 완료와 실환경 검증을 분리한다.

## A. 음성·텍스트 요청 일관성

- [x] 조회 명령은 데이터를 변경하지 않는다.
- [x] 같은 음성·텍스트 요청의 정확한 재전송은 `clientMutationId`로 한 번만 저장한다.
- [x] 요청 문장, 처리 상태, 결과, 다음 행동을 한 흐름에서 확인한다.
- [x] 음성, 직접 입력, 승인형 AI가 같은 저장·Calendar outbox 계약을 사용한다.
- [x] 생성·수정·완료·복구·삭제·다건 요청 결과가 실제 저장 결과와 일치한다.
- [x] 저장하지 못한 작업을 완료했다고 표현하지 않는다.
- [x] 날짜 표현은 Asia/Seoul 기준으로 해석하고 저장 시 절대 시각으로 변환한다.

## B. 업무관리 수동 CRUD

- [x] 일정과 할 일을 Desktop/Mobile에서 직접 생성할 수 있다.
- [x] 일정을 수정·삭제하고, 할 일을 수정·완료·복구·삭제할 수 있다.
- [x] 프로젝트를 생성·수정·삭제할 수 있다.
- [x] 완료한 할 일은 별도 목록에서 확인하고 다시 진행할 수 있다.
- [x] 프로젝트 삭제 시 연결된 할 일 처리와 웹훅 전달 이력을 보존한다.
- [x] 처리 후 대상 화면으로 이동하거나 결과에서 관련 항목을 확인할 수 있다.
- [x] 개인/회사 워크스페이스 범위를 저장·조회 계약에서 분리한다.

## C. 프로젝트 웹훅

- [x] 설정 생성·수정·활성화/비활성화·삭제가 가능하다.
- [x] 인증 비밀값을 응답과 로그에 노출하지 않고 교체·제거할 수 있다.
- [x] 테스트 전송, 프로젝트/할 일 자동 전송, 재시도, 전달 이력을 확인한다.
- [x] 실패한 전달을 lease owner와 attempt로 fencing한 뒤 안전하게 다시 보낼 수 있다.
- [x] 같은 전달은 재시도 중에도 동일한 delivery ID와 `Idempotency-Key`를 유지한다.
- [x] 삭제된 프로젝트의 전달 이력은 immutable audit로 남고 재시도할 수 있다.

## D. Google Calendar

- [x] 연결 상태, 마지막 동기화, 오류와 복구 행동을 확인한다. Mutation 실패는 정제된 `last_error_code`로 표시하고 성공 시 오류를 해제한다.
- [x] 연결 해제 시 저장된 provider token, schedule link, mutation payload, idempotency record를 폐기하되 수동 일정은 보존한다.
- [x] 앱·AI·승인형 AI·음성에서 만든 일정을 동일한 durable outbox로 선택한 Google Calendar에 반영한다. (코드·PostgreSQL 통합 검증)
- [x] 생성·수정·삭제 재시도에서 deterministic provider event ID, lease, version 순서로 중복 일정을 막는다. (코드·PostgreSQL 통합 검증)
- [x] Google 계정이 연결되지 않은 상태에서도 동일 `clientMutationId`의 정확한 재전송은 한 일정으로 수렴한다.
- [ ] 실제 Google 계정 OAuth와 provider 양방향 CRUD를 검증한다. (외부 설정 필요)

## E. Android 알림과 운영 준비

- [x] 일정·마감 15분 전 로컬 알림 모델과 중복 방지 키가 있다.
- [x] Android 알림 권한과 채널을 사용자 맥락에서 요청하고 영구 거절 시 설정 이동 경로를 제공한다.
- [x] 실제 알림을 누르면 관련 일정·할 일 화면으로 이동하고 대상을 강조한다.
- [x] 잠금 화면 알림은 private visibility로 상세 내용 노출을 제한한다.
- [x] 시작·resume 시 권한과 예약 상태를 다시 동기화하고 예약 실패를 UI에 표시한다.
- [x] 앱이 임의로 만든 nonce가 없는 외부 딥링크는 거절한다.
- [x] 재부팅 뒤 활성 알림을 복원하고 삭제·만료된 항목의 오래된 알람을 정리한다.
- [x] 로컬 Docker 재시작 뒤 데이터와 Agent 상태를 복구한다.
- [x] macOS 앱과 Android 에뮬레이터가 같은 로컬 서버 데이터로 동작한다.
- [ ] 실제 개인 Linux 서버 배포·VPN·백업/복원을 검증한다. (실환경 필요)

## 품질 게이트

- [x] Backend Ultrawork — 611 files scanned
- [x] Design Ultrawork — 10 targets
- [x] Interactive UI Ultrawork — 10 targets
- [x] UX Writing Ultrawork — 2 targets
- [x] Rust format, clippy, workspace test, release build
- [x] PostgreSQL integration — 26/26
- [x] Storage unit test — 17/17
- [x] Frontend format, lint, test, build, security scan — 19 files / 89 tests
- [x] Docker API health, restart, persistence smoke test
- [x] Desktop browser responsive and live CRUD QA
- [x] macOS debug/release package build, launch, render, API 200 smoke test
- [x] Android Kotlin compile, build, install, permission/settings/reboot smoke test
- [x] Android 실제 알림 tap-to-target smoke test — 391/360/320px 이동·강조 PASS
- [ ] Apple production signing/notarization — 외부 인증서·설정 필요

## 수동 UX·CRUD 검증

### Desktop/Mobile responsive

- Desktop: 1440×900, 1024×768, 800×900
- Mobile: 390×844, 360×800, 320×568
- 모든 viewport에서 수평 overflow와 화면 밖 조작 요소가 없음을 확인했다.
- compact desktop sidebar와 mobile bottom navigation 전환, focus 표시, reduced-motion, loading/empty/error/success 상태를 확인했다.
- 320px 설정 화면에서 처리 모델·effort 제어와 CTA가 잘리지 않고 최소 조작 높이를 유지함을 확인했다.

### 프로젝트·할 일·웹훅

1. 프로젝트 생성 → 제목·설명·기한 수정 → 상세 반영을 확인했다.
2. 프로젝트 할 일 생성 → 제목·메모·우선순위·기한 수정 → 완료 → 완료 목록 → 복구 → 삭제를 확인했다.
3. 프로젝트 웹훅 생성 → 일시 중지 → 재개 → URL·이벤트 수정 → 인증 교체 → 인증 제거를 확인했다.
4. 실패하도록 만든 테스트 전송이 `다시 시도 예정`으로 표시되고 immutable delivery history에 남는 것을 확인했다.
5. 웹훅 연결 해제와 프로젝트 삭제 뒤에도 전달 감사 이력이 유지되는 것을 확인했다.

### 일정·할 일

1. 일정 생성 → 시간·제목 수정 → 삭제 후 활성 목록에서 제거됨을 확인했다.
2. 독립 할 일 생성 → 설명·우선순위·기한 수정 → 완료 목록 이동 → 복구 → 삭제를 확인했다.
3. 완료·복구 뒤 프로젝트와 일정 화면의 열린 일 수가 즉시 동기화됨을 확인했다.
4. QA용 활성 프로젝트, 웹훅, 일정, 할 일을 종료 시 모두 정리했다. soft-delete 감사 행은 설계대로 보존한다.

## macOS 패키지 검증

| Artifact | 결과 | 크기 | SHA-256 |
| --- | --- | --- | --- |
| `/Applications/Jimin OS.app/Contents/MacOS/jimin-desktop` | release build, ad-hoc deep/strict sign, launch, API 200 PASS | 6.4 MB app | `5685a3ea818bcb804d540cdfd93cd519285c68ce404ed227cfffbd435420ff6b` |
| `target/release/bundle/dmg/Jimin OS_0.1.0_aarch64.dmg` | release bundle PASS | 2.4 MB | `4d7f4e90473c40931e17c2be4c2325701f0c0928dfabf6d6d4a7a10ace54403c` |

로컬 테스트 설치 스크립트가 번들 전체를 ad-hoc deep sign한 뒤 `codesign --verify --deep --strict`를 통과했고 `/Applications/Jimin OS.app` 실행과 schemaVersion 20 API 응답을 확인했다. Apple Developer identity 기반 배포 서명, notarization, Gatekeeper 배포 검증은 자격 증명이 필요한 외부 조건으로 유지한다.

## 결함 로그

| ID | 섹션 | 상태 | 발견·재현 | 수정 | 재검증 |
| --- | --- | --- | --- | --- | --- |
| P0-001 | A | fixed | 음성 결과 effect가 callback identity 변경 시 다시 실행될 수 있었다. | callback ref로 실행 경계를 고정했다. | frontend 89 tests와 음성 요청 회귀 PASS |
| P0-002 | B | fixed | 수동 일정·할 일·프로젝트의 생성·수정·삭제 경로가 화면별로 달랐다. | 공통 Planning/Project API 계약과 편집 dialog를 연결했다. | Desktop/Mobile live CRUD matrix PASS |
| P0-003 | A | fixed | 음성 전용 명령과 Agent action의 저장·결과 표현 범위가 달랐다. | 음성·직접 입력·승인형 AI를 동일 planning action/result와 outbox 경로로 연결했다. | API/Storage test 및 browser result flow PASS |
| P0-004 | D | blocked-external | 실제 Google 계정 OAuth와 provider CRUD 증거가 없다. | OAuth/outbox/연결 해제 코드는 완료했다. | 실제 client ID/secret과 redirect URI 확보 후 별도 QA |
| P0-005 | E | fixed | Android 알림 모델·권한·채널·딥링크가 없었다. | private local notification plugin, 권한·설정·reconcile·pending navigation을 구현했다. | Kotlin compile, emulator 권한/설정, 알림 표시·정리·tap routing PASS |
| P0-006 | E | blocked-external | 실제 개인 Linux 서버 배포·VPN·백업/복원 증거가 없다. | Docker 로컬 실행과 restart persistence는 완료했다. | 실서버 접근 권한 확보 후 별도 운영 QA |
| P0-007 | UX | fixed | Calendar 연결 해제 오류 문구에 다음 행동이 없고 harness가 source keyword를 오탐했다. | 오류 문구에 발생 내용과 복구 행동을 넣고 코드 키워드 오탐을 manifest evidence로 분리했다. | UX Writing Ultrawork 2 targets PASS |
| P0-008 | C | fixed | 웹훅에 설정 수정·중지·실패 수동 재전송 계약이 없었다. | expectedVersion 수정, pause/resume, owner-scoped retry, 인증 교체·제거를 구현했다. | PostgreSQL 26/26와 browser webhook lifecycle PASS |
| P0-009 | D | fixed | Calendar 연결 해제 저장 함수가 Backend lint의 100줄 제한을 초과했다. | transaction helper로 분리했다. | clippy `-D warnings`와 Backend Ultrawork 611 files PASS |
| P0-010 | D | fixed | provider 응답 유실 시 앱 일정이 중복될 수 있고 음성·AI 일정은 Google mutation 경로를 사용하지 않았다. | migration 0020, deterministic Google ID, leased outbox worker, 공통 mutation journal을 구현했다. | PostgreSQL duplicate/retry/AI/voice cases PASS |
| P0-011 | A | fixed | 같은 음성 할 일 생성 요청을 재시도하면 중복 Task가 생길 수 있었다. | UUIDv7 `clientMutationId`를 저장 idempotency key로 사용하고 payload/owner 재사용을 거절했다. | PostgreSQL exact replay·mismatch regression PASS |
| P0-012 | D | fixed | Calendar mutation 중 연결 해제, 불완전 credential, 비활성 calendar, provider 400/404가 orphan이나 영구 대기를 만들 수 있었다. | account row lock, sending conflict, destination 재검증, terminal guard, local-first purge와 provider 오류 분리를 구현했다. | PostgreSQL race/error cases와 Rust 전체 gate PASS |
| P0-013 | C | fixed | 프로젝트·할 일 저장 성공과 자동 웹훅 queue가 서로 다른 transaction이면 한쪽만 남을 수 있었다. | 도메인 mutation과 webhook delivery enqueue를 한 transaction으로 묶었다. | PostgreSQL atomic commit/rollback cases PASS |
| P0-014 | C/D | fixed | 만료된 worker가 lease 회수 후 늦게 성공·실패를 기록할 수 있었다. | lease owner와 attempt fencing을 상태 전이에 적용했다. | 2-worker lease/crash recovery regression PASS |
| P0-015 | C | fixed | 프로젝트 삭제 시 과거 webhook delivery 조회·재시도 근거가 사라질 수 있었다. | delivery snapshot을 immutable audit로 보존하고 삭제된 프로젝트에도 안전한 retry를 허용했다. | PostgreSQL deleted-project history/retry 및 browser disconnect/delete PASS |
| P0-016 | A/B | fixed | AI `DeleteProject`가 프로젝트만 지우고 분리된 할 일의 동기화 결과를 누락할 수 있었다. | detached task 결과와 webhook sync를 같은 action result에 포함했다. | Storage agent action regression PASS |
| P0-017 | API | fixed | 일부 delete/retry endpoint의 OpenAPI request body가 실제 route와 달랐다. | request DTO와 OpenAPI `request_body` 계약을 일치시켰다. | API schema/route test와 Backend gate PASS |
| P0-018 | D | fixed | Google 미연결 일정의 정확한 네트워크 재전송도 중복 일정이 될 수 있었다. | manual/voice schedule에 `clientMutationId` exact replay를 적용했다. | PostgreSQL unconnected manual/voice replay PASS |
| P0-019 | C | fixed | 웹훅 `occurredAt`이 명시적인 RFC 3339 계약이 아니었다. | 모든 자동 이벤트 payload에 parse 가능한 RFC 3339 timestamp를 기록했다. | Storage unit/PostgreSQL timestamp parse assertion PASS |
| P0-020 | E/UI | fixed | 시작·resume 권한 갱신, 영구 거절, 예약 실패, cold start pending navigation, 작은 화면과 reduced motion 경계가 빠져 있었다. | lifecycle reconcile, 설정 이동, 오류 상태, pending navigation, focus/320·390/reduced-motion 처리를 추가했다. | Android emulator와 320/360/390 browser QA PASS |
| P0-021 | E/Security | fixed | 외부 앱이 notification 딥링크 형태의 intent를 위조할 수 있었다. | 앱 내부에서 발급·소비하는 nonce 검증을 추가했다. | nonce 없는 forged deep link가 무시됨을 emulator에서 확인 |
| P0-022 | E | fixed | 기기 재부팅 후 알림이 사라지거나 삭제된 일정의 알람이 남을 수 있었다. | boot receiver에서 활성 payload만 복원하고 stale payload/alarm을 정리했다. | BOOT_COMPLETED 후 활성 reminder 복원, stale 제거와 AlarmManager cancel PASS |
| P0-023 | Test | fixed | emulator·PostgreSQL QA fixture가 다음 실행의 목록과 알람에 남을 수 있었다. | fixture별 cleanup과 종료 검증을 추가했다. | QA 종료 후 활성 fixture 0, stale alarm 0 확인 |
| P0-024 | B/UI | fixed | WebView date input에 값을 채워도 `onChange`가 발생하지 않아 프로젝트·할 일 기한이 `null`로 저장됐다. | 프로젝트 생성·수정과 할 일 수정 date input을 `onInput`으로 변경했다. | 할 일 `7월 18일까지`, 프로젝트 `2026. 7. 31.` 저장·재조회 PASS |
| P0-025 | E/UI | fixed | 실제 Android 알림을 누르면 대상 일정이 아니라 홈에 머물렀고 자동 스크롤 시 상단이 status bar 아래로 들어갈 수 있었다. | 의미적으로 같은 planning range 갱신을 차단하고 pending navigation을 보존했으며 safe-area scroll padding과 nearest 정렬을 적용했다. | 실제 알림 → tap → 일정 이동·강조를 391/360/320px에서 PASS, forged nonce 회귀 PASS |
| P0-026 | Packaging | fixed-local / blocked-external-production | 원본 release bundle의 linker ad-hoc 서명만으로는 strict 검증이 실패했다. | 설치 스크립트에서 최종 앱 번들을 deep ad-hoc sign하고 strict 검증하도록 했다. | `/Applications` 로컬 설치·strict verify·launch PASS; Developer ID/notarization은 외부 조건 |
| P0-027 | D | fixed | Calendar outbox worker가 반복 중단되면 최대 시도 8회를 넘겨 attempt 9 이상을 다시 claim할 수 있었다. | 만료된 claimed/sending 행을 transaction 안에서 먼저 terminalize하고 claim 후보에 최대 시도 조건을 추가했다. | attempt 8 실패·lease 해제·idempotency/account 상태·재claim 불가 PostgreSQL 회귀 PASS |
| P0-028 | E | fixed | 알림 trigger 뒤 target 전 재부팅하면 복원 과정에서 아직 유효한 알림 payload가 삭제됐다. | target이 미래면 trigger 경과 여부와 무관하게 즉시 다시 예약하고 target이 지난 항목만 정리했다. | past-trigger/future-target reboot 즉시 알림과 payload cleanup emulator E2E PASS |
| P0-029 | UI | fixed | 긴 화면에서 스크롤한 뒤 탭을 바꾸면 이전 window scroll이 남아 새 화면 제목과 조작부가 가려졌다. | route 변경 시 취소 가능한 layout-frame scroll reset과 reduced-motion 분기를 추가했다. | 일정·홈 전환 시 scrollY 0, heading safe-area 노출, reduced-motion PASS |
| P0-030 | B/UX | fixed | 프로젝트 일감 수정 실패도 생성 전용 문구인 `추가하지 못했어요`로 표시됐다. | 생성·수정 모두 사실에 맞는 `프로젝트의 일을 저장하지 못했어요`와 재시도 행동으로 통합했다. | UX Writing gate와 프로젝트 편집 실패 상태 PASS |
| P0-031 | Harness | fixed | Interactive UI target 목록에 UI 하네스가 지원하지 않는 Kotlin/Rust 플러그인 파일이 포함됐다. | native plugin은 Backend/플러그인 검증 증거로 분리하고 UI target은 실제 UI source 10개로 한정했다. | Interactive UI Ultrawork 10 targets PASS |

## 검증 실행 기록

| Cycle | Section | Command / Environment | Result |
| --- | --- | --- | --- |
| 0 | 전체 | P0 API·Storage·UI·plugin 정적 범위 점검 | PASS — 구현/검증 target 확정 |
| 1 | Rust | `cargo fmt --all -- --check` | PASS |
| 1 | Rust | `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| 1 | Rust | `cargo test --workspace` | PASS — workspace 전체 |
| 1 | Rust | `cargo build --workspace --release` | PASS |
| 1 | Storage | storage unit tests | PASS — 17/17 |
| 2 | PostgreSQL | fresh `postgres:16-alpine`, `JIMIN_TEST_DATABASE_URL=postgres://…@127.0.0.1:55432/jimin_test cargo test -p jimin-storage --test postgres -- --test-threads=1` | PASS — 26/26 |
| 2 | Frontend | format, lint, test, build, security scan | PASS — 19 files / 89 tests |
| 2 | Backend gate | `node .opendock/harness/opendock__backend-ultrawork/check.mjs` | PASS — 611 files scanned |
| 2 | Design gate | Design Ultrawork current run targets | PASS — 10 targets |
| 2 | Interaction gate | Interactive UI Ultrawork current run targets | PASS — 10 targets |
| 2 | Writing gate | UX Writing Ultrawork current run targets | PASS — 2 targets |
| 3 | Docker | pinned image rebuild, API/agent/gateway/PostgreSQL health, restart, persistence count | PASS — TLS/phone-test API 200, schemaVersion 20, 프로젝트 2·일감 15·일정 22 유지 |
| 3 | Browser | 1440/1024/800 desktop, 390/360/320 mobile responsive + live CRUD | PASS — overflow 없음, project/task/schedule/webhook lifecycle 완료 |
| 3 | macOS | release Tauri `.app`/DMG build, `/Applications` 재설치, ad-hoc deep/strict sign, launch, API health | PASS — Developer ID/notarization만 외부 조건 |
| 3 | Android | APK build/reinstall, permission/settings, 실제 tap-to-target, forged deep link, reboot recovery, alarm cleanup | PASS — versionCode 1000, versionName 0.1.0, 391/360/320px |

## 외부 조건

아래 항목은 P0 코드 결함이 아니라 실제 계정·서버·배포 자격이 있어야 완료할 수 있는 운영 검증이다.

- Google OAuth client ID/secret, 등록된 redirect URI, 실제 Google Calendar 계정: provider 양방향 CRUD 검증
- 실제 개인 Linux 서버와 VPN 접근, 백업 저장소: 배포·재시작·백업/복원 검증
- Apple Developer signing identity와 notarization credential: macOS 배포 서명·Gatekeeper 검증

외부 조건이 없어도 코드, mock/integration test, 로컬 Docker, browser responsive, macOS 로컬 설치 앱, Android emulator 검증은 모두 완료했다. 위 세 운영 항목은 실제 자격 증명과 서버 접근이 준비되면 같은 체크리스트로 별도 검증한다.
