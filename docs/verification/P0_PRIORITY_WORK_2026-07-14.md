# P0 최우선 작업 구현·QA 체크리스트

- 기준일: 2026-07-14 (Asia/Seoul)
- 범위: Google Calendar, 배포 설치본, 일정 관리, 프로젝트 Webhook
- 원칙: 각 체크 항목은 구현 증거와 QA 결과가 모두 있을 때만 완료 처리한다.

## 1. Google Calendar 실제 연결과 동기화

### 구현 체크리스트

- [x] OAuth 설정 누락·부분 설정·정상 설정을 서버가 구분한다.
- [x] Google 계정 연결 시작과 callback이 계정 연결 상태를 저장한다.
- [x] 최초 동기화가 캘린더 목록과 선택된 캘린더 일정을 저장한다.
- [x] 증분 동기화 토큰을 암호화해 저장하고 다음 동기화에서 사용한다.
- [x] 만료된 증분 토큰(HTTP 410)은 전체 동기화로 안전하게 복구한다.
- [x] 앱에서 생성·수정·삭제한 Google 일정이 Google Calendar mutation으로 이어진다.
- [x] Google에서 변경한 일정이 Jimin OS에 반영된다.
- [x] 주기 동기화가 중복 실행 없이 수행되고 실패를 재시도한다.
- [x] 마지막 성공 시각과 허용된 실패 코드를 앱에서 확인할 수 있다.
- [x] 자격 증명·토큰·일정 원문이 로그에 노출되지 않는다.

### QA 기록

- [x] 설정 검증 단위 테스트
- [x] OAuth/API 인증·오류 응답 테스트
- [x] 최초/증분/410 복구 동기화 테스트
- [x] 생성·수정·삭제 mutation 테스트
- [x] 로컬 Docker API 회귀 테스트
- [ ] 실제 Google 계정 연결 테스트 — 서버에 `JIMIN_GOOGLE_CALENDAR_CLIENT_ID`와 OAuth client secret이 없어 외부 조건 대기

## 2. 최신 macOS·Android 설치본

### 구현 체크리스트

- [x] 최신 프런트엔드와 Rust 코어로 macOS 앱을 패키징한다.
- [x] `/Applications/Jimin OS.app`을 최신 설치본으로 교체한다.
- [x] 최신 Android APK를 빌드한다.
- [x] 연결된 Android 에뮬레이터에 최신 APK를 재설치한다.
- [x] macOS `0.1.0`, Android `versionCode=1000`으로 설치본을 확인한다.

### QA 기록

- [x] macOS 실행·로딩·서버 연결·핵심 탐색 회귀
- [x] Android 실행·세이프 에어리어·서버 연결·핵심 탐색 회귀
- [x] 라이트/다크 모드와 412/700/800/1024/1280px 반응형 레이아웃 회귀
- [x] 네트워크 오류와 재연결 상태 회귀

## 3. 일정 관리 완성

### 구현 체크리스트

- [x] 일정 생성·수정·삭제가 모두 수동 UI에서 가능하다.
- [x] 일/주/월 단위로 기간을 이동할 수 있다.
- [x] 최근 3개월보다 오래된 일정을 날짜 기준으로 조회할 수 있다.
- [x] 일정 목록의 로딩·빈 상태·오류·재시도 상태가 구분된다.
- [x] Google 일정에는 연결 상태와 마지막 동기화 시각이 표시된다.
- [x] 연결된 Google 일정의 수정·삭제가 provider mutation으로 이어진다.
- [x] 홈/검색 결과에서 선택한 일정으로 이동하고 강조한다.

### QA 기록

- [x] 일정 CRUD API 테스트
- [x] 기간 경계·시간대·과거/미래 조회 테스트
- [x] 데스크톱 일정 탐색·수정·삭제 브라우저 QA
- [x] 모바일 일정 탐색·수정·삭제 QA
- [x] Google 미연결 UI와 연결 상태 분기 회귀
- [ ] 실제 Google 연결 상태의 provider mutation 재 QA — 1번과 동일한 OAuth 설정 대기

## 4. 프로젝트·일감 Webhook과 관리 보강

### 구현 체크리스트

- [x] 프로젝트별 Webhook URL·이벤트·인증 정보를 설정할 수 있다.
- [x] 설정 화면에서 테스트 전송과 결과 확인이 가능하다.
- [x] 프로젝트 및 일감 생성·수정·완료·복구·삭제 이벤트를 전송한다.
- [x] 전달 실패를 지수 백오프로 최대 5회 재시도한다.
- [x] 전달 이력에서 상태·시각·응답 코드·재시도 횟수를 확인한다.
- [x] Webhook 비밀값을 암호화하고 응답·로그·이력에 노출하지 않는다.
- [x] 이벤트마다 idempotency key를 발급한다.
- [x] 프로젝트에서 다음 행동과 위험을 입력·수정·확인할 수 있다.
- [x] 개인/회사 workspace 범위가 API와 UI에서 섞이지 않는다.

### QA 기록

- [x] Webhook 설정 검증·권한 테스트
- [x] 테스트 전송·자동 전송·재시도·이력 테스트
- [x] 프로젝트/일감 회귀 테스트
- [x] 개인/회사 workspace 격리 테스트
- [x] 데스크톱/모바일 UI 회귀

## 결함·수정·재검증 기록

| 작업 | 발견한 문제 | 수정 | 재검증 증거 | 상태 |
| --- | --- | --- | --- | --- |
| 1 | 증분 동기화 토큰과 Google mutation 경로가 없었고 OAuth 설정 상태가 한 가지로 보였음 | 암호화 sync token, 410 전체 재동기화, CRUD mutation, 5분 worker, 설정 상태 분기를 구현 | Google/API/storage 단위 테스트와 Docker readiness 통과. 실제 계정은 OAuth client 설정 대기 | 조건부 완료 |
| 2 | 최초 macOS 번들의 ad-hoc 서명이 resource seal 검증에 실패 | 설치본을 deep ad-hoc 재서명 | `codesign --verify --deep --strict`, 앱 실행 PID, Android 설치 PID 확인 | 완료 |
| 3 | 수동 삭제와 무제한 기간 이동이 없고 모델 확장 뒤 에이전트 테스트 fixture 4곳이 누락 | soft delete, day/week/month range, 편집 가능 상태, fixture를 source별로 수정 | API CRUD/409 충돌/삭제, Android 편집·삭제, Rust 161 tests 통과 | 완료 |
| 4 | migration metadata 누락, 전달 이력 cascade 손실, agent action 이벤트 누락, 성공 notice 소실, 시험 전송 상태 지연 | schema 19 metadata, immutable delivery snapshot, agent transaction queue, projectId 기반 state, bounded polling 구현 | 실제 HTTP receiver로 4종 204·고유 idempotency·Authorization 확인, 실패 5회 재시도, Android 생성/시험/해제 재 QA | 완료 |
| 공통 | 프런트 4개 파일이 Prettier 규칙과 불일치 | 대상 파일만 Prettier 적용 | format/typecheck/57 tests/Vite build 재실행 통과 | 완료 |
| 공통 | OpenDock 1차 검사에서 체크박스 이름, 대비·radius 계획, timer 정리, 실패 상태의 다음 행동 문구가 부족 | 명시적 input name/value, 구체적 token 계획, timeout 정리, `전송 실패 · 연결 확인` 문구 적용 | Design, Interactive UI, UX Writing, Backend Ultrawork 재실행 통과 | 완료 |

## 최종 검증 증거

- Rust: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build --workspace --release` 통과
- Frontend: Prettier, TypeScript, Vitest 57/57, Vite production build 통과
- Database/API: Docker readiness `ready`, configuration/database/migrations `ok`, schema version `19`
- macOS: `/Applications/Jimin OS.app` 서명 검증 통과, 버전 `0.1.0`, 실행 확인
- Android: API 36 에뮬레이터 설치·실행, `versionCode=1000`, day/week/month·일정 CRUD·웹훅 UI 검증
- Responsive/accessibility: 412x915 및 700/800/1024/1280px에서 가로 넘침 없음, 44px 터치 대상과 focus 상태 확인
