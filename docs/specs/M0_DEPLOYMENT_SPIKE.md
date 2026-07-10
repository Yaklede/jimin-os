# M0. 배포·기술 스파이크 명세

## 1. 목적

M0는 제품 기능을 만드는 단계가 아니다. 이후 구현을 막을 수 있는 배포, Codex App Server, 모바일 실기기 문제를 실제 환경에서 먼저 제거하고, 동일한 구조를 다시 만들 수 있는 저장소 기반을 남기는 단계다.

M0가 끝나면 다음 사실이 추측이 아니라 실행 결과로 확인돼야 한다.

1. Rust API와 PostgreSQL을 Docker Compose로 로컬 서버에 배포할 수 있다.
2. Mac과 개인 휴대폰이 TLS를 사용해 staging API에 접근할 수 있다.
3. 서버 컨테이너에서 ChatGPT 계정 인증을 유지하고 Codex App Server와 `stdio` JSONL로 대화할 수 있다.
4. 실제 휴대폰에서 선택한 클라이언트 기술로 설치, 보안 저장소, OAuth 복귀, 앱 생명주기, 재연결을 구현할 수 있다.
5. 서버 CPU와 클라이언트 플랫폼에 맞는 빌드·배포·rollback 절차가 명령 단위로 기록돼 있다.

## 2. 선행조건

### 2.1 로컬 서버

구현 전에 아래 값을 `docs/runbooks/environment-inventory.md`에 기록한다. 비밀정보는 기록하지 않는다.

- Linux 배포판과 커널
- CPU architecture: `amd64` 또는 `arm64`
- Docker Engine과 Compose plugin 버전
- 사용 가능한 CPU, memory, disk와 volume 경로
- 서버 timezone과 NTP 동기화 상태
- 서비스용 non-root UID/GID
- staging private hostname과 DNS 해석 경로
- Mac과 휴대폰에서 서버로 가는 LAN 또는 사설 네트워크 경로
- OpenAI, Google, container registry에 대한 outbound DNS/HTTPS 가능 여부
- 방화벽에서 외부에 노출할 gateway port

### 2.2 개발 기기

- macOS와 Xcode 버전
- Rust stable toolchain, Node.js, pnpm, Tauri CLI 버전
- 개인 휴대폰 OS, OS 버전, CPU architecture
- iOS라면 Apple signing team과 실기기 provisioning 가능 여부
- Android라면 SDK, NDK, JDK와 USB 또는 wireless debugging 가능 여부
- 휴대폰에서 사설 네트워크와 staging private hostname에 접근할 수 있는 상태

### 2.3 계정

- ChatGPT 구독 계정에서 Codex 사용 가능
- headless 장치용 device-code 로그인이 계정 또는 workspace 설정에서 허용됨
- M1/M2에서 사용할 Google Cloud staging project를 생성할 권한

## 3. 범위

### 3.1 포함

- Cargo workspace와 pnpm workspace 골격
- `api`, `agent`, `desktop`, `mobile`의 최소 실행 진입점
- PostgreSQL 연결과 빈 baseline migration
- `/health/live`, `/health/ready` 구현
- `gateway`, `api`, `agent`, `postgres` Compose 서비스
- staging TLS와 private network 접근
- 서버 architecture에 맞는 image build와 실행
- `codex` 실행 파일의 명시적 버전 고정
- Codex App Server JSON Schema 생성과 Rust adapter probe
- ChatGPT device-code 로그인과 인증 volume 지속성 검증
- `initialize → initialized → account/read → thread/start → turn/start → stream → turn/completed` 수직 검증
- macOS 최소 앱의 staging health 호출
- 개인 휴대폰 최소 앱의 staging health 호출
- Tauri 2 mobile 유지 또는 모바일 기술 전환을 결정하는 ADR
- 컨테이너 재시작, schema mismatch, 인증 없음, 네트워크 단절 검증
- 개발용 배포 및 이전 image rollback runbook

### 3.2 제외

- Google 계정 로그인과 Jimin OS session 발급
- Google Calendar 연결과 일정 데이터
- 사용자용 화면 구조와 디자인 완성
- 대화, 기억, 할 일의 영구 데이터 모델
- 모바일 오프라인 cache와 실제 mutation queue
- Codex 승인, 파일 변경, terminal 실행
- public internet 공개와 public relay
- production 배포
- 무인 예약 작업 또는 ChatGPT 구독을 사용한 background automation

스파이크 코드라도 완료 게이트에 사용되는 코드는 `main`에서 재현 가능해야 한다. 일회성 shell history나 개인 기기에만 남은 설정은 산출물로 인정하지 않는다.

## 4. 확정할 기술 계약

### 4.1 서버 workspace

```text
apps/
├─ api/                  # health와 readiness를 제공하는 Axum binary
├─ agent/                # Codex child process manager의 최소 binary
├─ desktop/              # Tauri 2 macOS probe
└─ mobile/               # Tauri 2 또는 ADR로 선택한 mobile probe
crates/
├─ codex-client/         # App Server wire type과 process adapter
├─ observability/        # tracing, request ID, redaction
└─ storage/              # SQLx pool과 migration runner
deploy/
├─ compose.yaml
├─ compose.staging.yaml
├─ versions.env
└─ gateway/
schemas/
└─ codex/<codex-version>/
```

버전은 lockfile과 `deploy/versions.env`에서 고정한다. image tag, package version, base image에 `latest`를 사용하지 않는다. 실제 테스트한 Codex 버전과 binary checksum 또는 image digest를 함께 기록한다.

### 4.2 Compose 서비스

| 서비스 | 책임 | 외부 노출 | 필수 제약 |
|---|---|---:|---|
| `gateway` | TLS 종료와 API reverse proxy | 예 | private hostname만 수신 |
| `api` | health, readiness, DB migration 확인 | 아니요 | non-root, read-only root filesystem |
| `agent` | Codex App Server child process와 probe | 아니요 | 별도 UID, 별도 volume, Docker socket 금지 |
| `postgres` | staging DB | 아니요 | 내부 network, healthcheck, persistent volume |

M0에서는 `api`와 `agent`를 서로 다른 process/container로 유지한다. `agent`가 죽거나 인증되지 않아도 `api`의 liveness는 정상이어야 한다.

### 4.3 Persistent volume

```text
postgres_data   # PostgreSQL data
codex_home      # Codex가 관리하는 인증·thread 상태
agent_workspace # 스파이크용 비민감 workspace
```

`codex_home`은 `agent`만 mount한다. gateway와 API에서 읽을 수 없어야 한다. volume backup 과정에서도 `auth.json` 원문을 일반 backup artifact에 포함하지 않는다.

### 4.4 TLS 결정

M0에서 아래 중 한 경로를 선택하고 ADR에 남긴다.

1. private DNS hostname에 대해 공인 CA 또는 DNS-01로 발급한 인증서를 사용한다.
2. gateway 내부 CA를 사용하고 Mac과 휴대폰에 root certificate를 명시적으로 신뢰시킨다.

plain HTTP는 `127.0.0.1` probe 이외에 허용하지 않는다. 선택한 경로는 iOS App Transport Security 또는 Android Network Security Config를 완화하지 않고 실기기에서 통과해야 한다. 인증서 검증을 끄는 개발 옵션은 완료 증거로 인정하지 않는다.

## 5. 최소 API 계약

### 5.1 `GET /health/live`

process event loop가 응답하는지만 확인한다. DB, Google, Codex 상태 때문에 실패시키지 않는다.

```json
{
  "status": "ok",
  "service": "api",
  "buildSha": "git-sha"
}
```

- 정상: `200`
- body에 hostname, 환경 변수, token, filesystem path를 포함하지 않는다.

### 5.2 `GET /health/ready`

필수 config, DB 연결, migration version을 확인한다.

```json
{
  "status": "ready",
  "checks": {
    "configuration": "ok",
    "database": "ok",
    "migrations": "ok"
  },
  "schemaVersion": 1
}
```

- 준비됨: `200`
- 준비되지 않음: `503`
- Agent와 OpenAI 상태는 readiness 조건에 넣지 않는다.
- 상세 접속 문자열이나 SQL error는 응답하지 않는다.

### 5.3 Agent probe

Codex probe는 외부 HTTP route로 공개하지 않는다. 다음 CLI만 제공한다.

```text
jimin-agent probe compatibility
jimin-agent probe account
jimin-agent probe turn --prompt-file <fixture>
```

prompt fixture는 개인 데이터가 없는 저장소 내 test fixture만 사용한다. CLI는 machine-readable JSON summary를 stdout으로 내보내고, Codex JSONL wire log와 진단 log는 서로 분리한다.

## 6. Codex App Server 계약

Codex App Server는 `agent`가 child process로 실행한다. 외부 network listener를 열지 않고 `codex app-server --listen stdio://` 또는 기본 `codex app-server`를 사용한다. 공식 문서에서 WebSocket transport는 experimental/unsupported로 표시되므로 사용하지 않는다.

참고:

- [Codex App Server](https://developers.openai.com/codex/app-server/)
- [Codex authentication](https://developers.openai.com/codex/auth/)

### 6.1 연결 초기화

1. child stdin/stdout/stderr를 별도 pipe로 연다.
2. stdout을 최대 line size가 제한된 UTF-8 JSONL decoder로 읽는다.
3. 연결당 정확히 한 번 `initialize`를 보낸다.
4. 응답 성공 후 `initialized` notification을 보낸다.
5. M0에서는 `experimentalApi`를 활성화하지 않는다.
6. `clientInfo.name`, `title`, `version`을 Jimin OS adapter 값으로 보낸다.

초기화 전 method 호출, 중복 초기화, malformed JSONL은 명시적인 probe 실패다.

### 6.2 수직 대화 흐름

```text
initialize
→ initialized
→ account/read { refreshToken: false }
→ thread/start
→ turn/start
→ turn/started
→ item/started
→ item/agentMessage/delta*
→ item/completed
→ turn/completed
```

- `item/completed`의 최종 item을 authoritative state로 취급한다.
- `turn/completed.turn.status`가 `completed`일 때만 probe 성공이다.
- `failed`와 `interrupted`를 성공으로 간주하지 않는다.
- request `id`와 pending oneshot channel을 map으로 관리한다.
- 알 수 없는 notification은 log metadata만 남기고 연결을 끊지 않는다.
- 알 수 없는 response ID, line size 초과, stdout EOF는 protocol error다.
- stderr는 구조화 log로 수집하되 credential과 대화 원문을 마스킹한다.

### 6.3 Schema와 호환성

테스트한 Codex binary로 다음 산출물을 생성한다.

```text
codex app-server generate-json-schema --out schemas/codex/<version>/json
codex app-server generate-ts --out schemas/codex/<version>/typescript
```

각 schema directory에는 다음 metadata를 둔다.

```json
{
  "codexVersion": "actual-tested-version",
  "binarySha256": "sha256",
  "generatedAt": "UTC timestamp",
  "stableApiOnly": true
}
```

`codex-client` 밖으로 generated App Server type을 노출하지 않는다. M0 probe는 시작 시 `codex --version`을 읽고 지원 목록 밖이면 `incompatible` 상태로 종료한다. API health는 계속 정상이어야 한다.

### 6.4 인증 운영

headless 서버에서는 관리자가 다음 절차로 로그인한다.

1. `agent`와 같은 `CODEX_HOME` volume을 mount한 admin one-shot container를 실행한다.
2. `codex login --device-auth`를 실행한다.
3. 개인 브라우저에서 일회용 code를 승인한다.
4. `account/read`로 `chatgpt` account가 보이는지 확인한다.
5. admin container를 종료하고 `agent`만 volume을 사용하게 한다.

device-code 인증이 계정에서 제공되지 않으면 공식 fallback인 browser login 후 auth cache 복사 경로를 별도 runbook으로 검증한다. 인증 파일은 password와 동일하게 취급한다. command output, issue, chat, Git에 붙이지 않는다.

### 6.5 Runtime 상태 전이

```text
stopped
  → starting
  → initializing
  → ready

initializing → auth_required
initializing → incompatible
initializing → protocol_error
ready → crashed → restart_wait → starting
ready → stopping → stopped
```

| 상태 | 의미 | 외부 동작 |
|---|---|---|
| `ready` | handshake, account, turn probe 가능 | Agent 기능 가능 |
| `auth_required` | OpenAI 인증 없음 또는 만료 | 로그인 runbook 안내 |
| `incompatible` | binary/schema 지원 범위 불일치 | 배포 중단, 일정 API 영향 없음 |
| `protocol_error` | JSONL 또는 handshake 계약 위반 | captured fixture로 회귀 테스트 추가 |
| `crashed` | child process 비정상 종료 | pending request 실패 처리 후 제한 재시작 |

무한 restart loop를 만들지 않는다. 연속 crash 횟수와 마지막 원인을 저장하고 operator가 확인할 수 있게 한다.

## 7. 모바일·Mac 기술 스파이크

### 7.1 공통 probe 화면

제품 화면을 만들지 않고 다음 상태만 표시한다.

- app build/version
- server URL의 민감하지 않은 label
- TLS 연결 성공/실패
- `/health/live`, `/health/ready` 결과
- 마지막 성공 시각
- foreground/background 이후 재연결 결과
- secure storage round-trip 결과

실제 UI 구현을 시작할 때는 별도 design run manifest가 필요하다. M0 진단 화면을 제품 디자인 기준으로 재사용하지 않는다.

### 7.2 실기기 필수 검증

1. release-equivalent signing으로 앱 설치
2. private hostname TLS 검증
3. fake session token을 Keychain/Keystore에 저장·조회·삭제
4. system browser 열기와 앱 deep link/app link 복귀
5. 앱 background 전환 중 연결 종료 처리
6. foreground 복귀 후 health 재요청
7. network OFF/ON 후 중복 요청 없이 재연결
8. 앱 강제 종료와 재실행 후 secure storage 유지
9. SQLite plugin 또는 선택한 local DB의 create/migrate/read/write
10. build artifact 재설치 후 data migration 가능성 확인

fake token만 사용하며 실제 Google 또는 Jimin OS token을 진단 log에 출력하지 않는다.

### 7.3 Tauri mobile 판정

다음 조건을 모두 만족하면 Tauri 2 mobile을 유지한다.

- 개인 휴대폰에 반복 설치 가능한 build 절차가 있다.
- system browser OAuth 복귀와 secure storage가 공식 plugin 또는 작은 자체 plugin으로 동작한다.
- HTTPS/WSS, background/foreground, SQLite가 재현 가능하게 동작한다.
- 필요한 native code가 platform bridge에 국한되고 공통 React code를 침범하지 않는다.
- plugin fork나 검증되지 않은 abandonware에 핵심 인증을 의존하지 않는다.

다음 중 하나면 모바일만 Expo/React Native로 전환한다.

- OAuth, secure storage, lifecycle 중 하나가 실기기에서 안정적으로 구현되지 않는다.
- 핵심 plugin을 장기 fork해야만 한다.
- release build 또는 store signing 경로가 재현되지 않는다.
- native crash 원인을 app code에서 통제할 수 없다.
- Tauri 유지가 React Native의 제한된 Rust 연동보다 명확히 많은 platform-specific code를 요구한다.

결정은 `docs/adr/ADR-0002-mobile-runtime.md`에 증거, 실패 재현, 선택, 대안을 기록한다. “익숙해 보임”이나 “나중에 해결”은 판정 근거가 아니다.

## 8. 구현 순서

1. 환경 inventory와 private network/TLS 경로를 기록한다.
2. Cargo/pnpm workspace와 pinned toolchain/lockfile을 만든다.
3. Axum API에 health route, tracing, graceful shutdown을 구현한다.
4. PostgreSQL baseline migration과 readiness check를 구현한다.
5. non-root multi-stage image를 만들고 local Compose smoke test를 통과한다.
6. gateway TLS를 구성하고 Mac browser/curl에서 staging health를 확인한다.
7. 개인 휴대폰 browser에서 같은 endpoint의 TLS를 확인한다.
8. Codex binary를 pin하고 schema를 생성한다.
9. `codex-client` JSONL codec, correlation, initialization을 구현한다.
10. device-code 로그인 runbook을 실행하고 volume 지속성을 확인한다.
11. thread/turn streaming probe와 child crash test를 통과한다.
12. Mac 최소 앱에서 health, secure storage, lifecycle을 검증한다.
13. 개인 휴대폰 최소 앱에서 동일 검증과 OAuth 복귀 probe를 수행한다.
14. 모바일 runtime ADR과 Codex compatibility ADR을 확정한다.
15. staging image digest, migration, health, rollback runbook을 재실행한다.

## 9. 오류와 보안 처리

| 실패 | 기대 처리 |
|---|---|
| DB 미연결 | ready `503`, live `200`, secret 없는 구조화 log |
| migration 불일치 | API traffic 수신 전 ready 차단 |
| 인증서 불신 | 앱이 연결을 거부하고 인증서 검증을 우회하지 않음 |
| Codex 인증 없음 | Agent `auth_required`, API 정상 |
| Codex version 불일치 | Agent `incompatible`, schema 재생성 전 실행 금지 |
| App Server stdout malformed | pending request 실패, fixture 보존, 제한 재시작 |
| App Server child crash | EOF 감지, pending request 일괄 실패, crash loop 제한 |
| OpenAI 단절 | Agent probe 실패만 기록, API/DB readiness 영향 없음 |
| phone background 전환 | 연결 정리, foreground에서 bounded backoff 재연결 |

추가 보안 조건:

- Compose에 Docker socket을 mount하지 않는다.
- process를 root로 실행하지 않는다.
- secret은 `_FILE` 형태의 mounted secret을 우선한다.
- `docker inspect`, build log, image history에 secret이 나타나면 실패다.
- API와 Agent는 서로 다른 DB role 또는 최소한 서로 다른 secret scope를 사용한다.
- health endpoint는 인증 없이 열 수 있지만 민감한 dependency detail은 반환하지 않는다.
- probe prompt와 response는 개인 데이터 없이 만들고 일반 log level에서는 본문을 남기지 않는다.

## 10. 자동 테스트

### 10.1 Rust와 DB

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
SQLx migration on empty PostgreSQL
readiness with DB up/down/migration mismatch
graceful shutdown and connection drain
```

### 10.2 Codex adapter

- JSONL fragmented read와 여러 message가 한 buffer에 온 경우
- max line size 초과
- malformed JSON
- unknown notification
- response correlation과 unknown response ID
- initialize 이전 요청 거부 fixture
- `item/agentMessage/delta` 순서 유지
- `item/completed` final state 우선
- child stdout EOF와 stderr flood
- supported/unsupported version 판정
- 실제 App Server를 사용하는 staging smoke test

### 10.3 배포

- server architecture image build
- 가능한 경우 `linux/amd64`, `linux/arm64` build smoke
- Compose config validation
- container non-root 확인
- secret이 image history에 없는지 검사
- local Compose start/stop/restart
- staging pull/up과 health
- 이전 image digest rollback

### 10.4 클라이언트

- TypeScript lint/typecheck/test
- macOS build smoke
- 개인 휴대폰 target build smoke
- API schema에서 생성된 client compile
- secure storage adapter unit test
- lifecycle/reconnect reducer unit test

## 11. 수동 검증

### 11.1 Mac

1. private hostname으로 staging health를 연다.
2. 인증서 chain과 hostname 검증을 확인한다.
3. Mac 앱에서 live/ready가 표시되는지 확인한다.
4. server container restart 중 실패 후 자동 복구를 확인한다.
5. Agent container를 내리고도 API health가 유지되는지 확인한다.

### 11.2 개인 휴대폰

1. 사설 네트워크에서 staging hostname에 접근한다.
2. release-equivalent 앱을 설치한다.
3. TLS, secure storage, deep link, SQLite probe를 실행한다.
4. Wi-Fi와 mobile network 사이를 전환한다.
5. background/foreground와 강제 종료/재실행을 확인한다.
6. 인증서 검증 오류가 우회되지 않는지 확인한다.

### 11.3 서버와 Codex

1. fresh `codex_home`에서 `auth_required`를 확인한다.
2. device-code 로그인 후 account가 `chatgpt`인지 확인한다.
3. 개인 데이터 없는 prompt로 delta와 completed를 받는다.
4. Agent container를 재시작한 뒤 인증과 thread 상태 지속성을 확인한다.
5. child process를 강제 종료하고 crash recovery를 확인한다.
6. 지원하지 않는 version fixture에서 `incompatible`을 확인한다.

## 12. 산출물

- Rust/pnpm workspace와 lockfile
- `api`, `agent`, `desktop`, `mobile` 최소 app
- Compose와 gateway staging 설정
- baseline DB migration
- Codex generated schema와 adapter fixture
- environment inventory
- staging deploy/rollback runbook
- Codex login/credential recovery runbook
- 모바일 실기기 검증 기록
- `ADR-0001-codex-app-server-compatibility.md`
- `ADR-0002-mobile-runtime.md`
- 자동 테스트 결과와 image digest

실기기 증거에는 OS/app version, build SHA, 실행한 scenario, PASS/FAIL, 민감정보가 제거된 screenshot 또는 log summary를 포함한다.

## 13. 완료 게이트

다음을 모두 만족해야 M1을 시작할 수 있다.

- [ ] 저장소의 명령만으로 local Compose를 새로 만들 수 있다.
- [ ] 로컬 서버 staging에 동일 image를 배포할 수 있다.
- [ ] Mac과 개인 휴대폰이 TLS 검증을 유지한 채 health를 조회한다.
- [ ] DB down과 migration mismatch에서 live/ready가 계약대로 분리된다.
- [ ] Agent container가 non-root이고 API/PostgreSQL/Codex secret 경계가 분리됐다.
- [ ] ChatGPT device-code 로그인 또는 공식 fallback이 실제 서버에서 완료됐다.
- [ ] App Server stable API handshake와 한 turn의 streaming이 통과했다.
- [ ] Agent restart 후 인증 volume이 유지되고 API health는 독립적으로 정상이다.
- [ ] Codex version/schema metadata와 compatibility test가 커밋됐다.
- [ ] 개인 휴대폰에서 install, TLS, secure storage, deep link, lifecycle, SQLite가 검증됐다.
- [ ] Tauri mobile 유지 또는 전환 결정이 ADR로 확정됐다.
- [ ] 이전 image digest rollback을 실행했다.
- [ ] formatter, lint, test, build, migration, Compose smoke가 모두 통과했다.
- [ ] Backend Ultrawork의 M0 관련 항목에 실패가 없다.

하나라도 실패하면 M0를 완료로 표시하지 않는다. 단순히 후속 단계 backlog로 넘길 수 있는 항목은 범위 밖 항목뿐이다.
