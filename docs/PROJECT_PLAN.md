# Jimin OS 전체 개발 계획

> 기준일: 2026-07-10
>
> 상태: 구현 전 계획
>
> 목표: 로컬 서버에 배포하고 Mac과 개인 휴대폰에서 실제 검증할 수 있는 `v0.1`

> 2026-07-12 정정: [ADR-0004](adr/ADR-0004-device-pairing-app-identity.md)의
> QR 기기 등록은 단일 사용자·VPN 전용 배포라는 제품 전제와 맞지 않아
> [ADR-0005](adr/ADR-0005-vpn-trusted-personal-access.md)로 대체됐다. 앱은
> VPN으로 제한된 개인 서버에서 자동으로 session을 만들며, Google OAuth는
> M2 Google Calendar 연결에만 사용한다. ChatGPT 계정은 개인 서버의 Codex
> App Server가 별도 managed device-code 로그인으로 관리한다.

## 1. 제품 목표

Jimin OS는 개인의 일정, 할 일, 프로젝트, 결정과 대화를 한곳에 연결하고 과거 맥락을 현재 판단과 다음 행동으로 이어 주는 개인 AI 운영체제다.

첫 버전은 다음 질문에 답할 수 있어야 한다.

1. 오늘 일정과 우선순위는 무엇인가?
2. 과거에 이 결정을 왜 내렸고 현재도 유효한가?
3. 다음으로 해야 할 행동은 무엇인가?
4. 필요하면 어떤 작업을 실행할 수 있으며, 사용자의 승인이 필요한가?

일정과 개인 데이터는 Mac 상태와 무관하게 접근할 수 있어야 한다. Mac은 시스템의 본체가 아니라 로컬 파일과 터미널 작업을 제공하는 선택적 실행 노드다.

## 2. 전제와 기본 결정

로컬 서버의 정확한 OS와 CPU는 구현 착수 시 확인한다. 계획은 다음 조건을 만족하도록 구성한다.

- Linux 기반 로컬 서버
- Docker와 Docker Compose 사용 가능
- `linux/amd64`와 `linux/arm64` 모두 빌드 가능
- 서버는 상시 켜져 있고 OpenAI 및 Google API로 outbound 통신 가능
- Mac과 개인 휴대폰은 LAN 또는 사설 네트워크로 서버에 접근
- 외부 인터넷에는 API를 직접 공개하지 않음
- 사용자는 한 명이며 VPN ingress가 앱 접근의 전제

| 항목 | 결정 |
|---|---|
| 항상 켜진 본체 | 로컬 서버의 Rust backend |
| 원본 데이터 | 로컬 서버 PostgreSQL |
| AI 런타임 | 서버 내부 `codex app-server` subprocess |
| AI 인증 | 서버 관리자가 한 번 수행하는 ChatGPT device-code login |
| 앱 접근 | VPN 전용 서버의 자동 Jimin OS session |
| 일정 원본 | Google Calendar + 서버 동기화 read model |
| Mac 앱 | Tauri 2 + React + TypeScript |
| 모바일 앱 | Tauri 2 우선 검증, 실패 기준 충족 시 Expo/React Native로 전환 |
| 공통 코어 | Rust workspace + 공유 TypeScript 패키지 |
| 모바일/데스크톱 캐시 | SQLite |
| 원격 이벤트 | HTTPS + WSS |
| 배포 | Docker Compose |
| 외부 접속 | Twingate 등 사설 네트워크 우선 |

## 3. 전체 아키텍처

```mermaid
flowchart LR
    subgraph Clients["사용자 기기"]
        MAC["macOS Tauri 앱"]
        PHONE["iOS / Android 앱"]
        CACHE["SQLite 오프라인 캐시"]
    end

    subgraph HomeServer["상시 실행 로컬 서버"]
        GW["TLS Gateway"]
        API["Rust API / Sync Server"]
        PG["PostgreSQL"]
        AGENT["Rust Agent Runner"]
        CODEX["Codex App Server"]
        SCHED["Scheduler / Calendar Sync"]
    end

    subgraph OptionalMacNode["선택적 Mac Worker"]
        WORKER["Mac Worker"]
        LOCAL["로컬 파일 / 터미널"]
    end

    GOOGLE["Google OAuth / Calendar"]
    OPENAI["OpenAI"]

    MAC <-->|"HTTPS / WSS"| GW
    PHONE <-->|"HTTPS / WSS"| GW
    MAC --- CACHE
    PHONE --- CACHE
    GW --> API
    API <--> PG
    API <--> SCHED
    SCHED <--> GOOGLE
    API <--> AGENT
    AGENT <-->|"JSON-RPC over stdio"| CODEX
    CODEX --> OPENAI
    WORKER -->|"outbound authenticated WSS"| API
    WORKER <--> LOCAL
```

### 핵심 경계

- 모바일과 Mac 앱은 동일한 서버 API를 사용한다.
- 일정, 할 일, 기억, 대화 read model은 서버에 있으므로 Mac이 꺼져 있어도 사용할 수 있다.
- `codex app-server`는 Agent Runner가 `stdio`로만 연결하며 네트워크에 직접 노출하지 않는다.
- 서버에서 실행되는 Codex는 서버에 허용된 파일과 도구만 접근한다.
- Mac 파일이나 터미널이 필요하면 Mac Worker가 온라인일 때만 별도 승인 후 실행한다.
- 모바일과 Mac 앱은 ChatGPT 토큰을 받거나 저장하지 않는다.

## 4. 구성요소별 책임

### 4.1 Rust API / Sync Server

- QR 기기 연결 코드 교환과 device session 발급·갱신·폐기
- 사용자 세션 발급과 갱신
- 일정, 할 일, 프로젝트, 기억 CRUD
- Google Calendar 동기화 상태 관리
- 클라이언트 증분 동기화 cursor 제공
- 대화와 Agent 작업 생성
- 승인 요청 상태 머신
- WSS 이벤트 전달
- 기기 및 Mac Worker 등록·폐기
- 감사 로그와 health endpoint 제공

### 4.2 Rust Agent Runner

- Codex CLI 버전과 호환성 확인
- App Server subprocess 시작·감시·재시작
- JSON-RPC 요청/응답 correlation
- thread, turn, stream 이벤트 변환
- 명령·파일 변경 승인 요청을 Jimin OS 이벤트로 변환
- 작업 재시도와 실패 상태 기록
- App Server 이벤트를 대화 read model에 반영

Agent Runner와 API를 처음에는 하나의 Rust workspace에서 개발하되 배포 프로세스는 분리한다. Agent 장애가 일정 API 가용성에 영향을 주지 않게 하기 위함이다.

### 4.3 Scheduler / Calendar Sync

- Google Calendar 최초 전체 동기화
- `syncToken` 기반 증분 동기화
- 토큰 무효화 응답 시 전체 재동기화
- 일정 생성·수정·삭제를 Google Calendar에 반영
- 동기화 실패 재시도와 마지막 성공 시각 기록
- Asia/Seoul 기준 날짜 경계 처리

MVP는 private server가 외부 webhook을 받지 않아도 되도록 polling 방식으로 시작한다. 기본 주기는 5분이며 앱에서 수동 새로고침할 수 있다.

### 4.4 Mac 앱

- 오늘 화면, 일정, 할 일, 기억, 대화
- AI 응답 스트리밍
- 승인 요청 처리
- 서버 및 Mac Worker 상태 표시
- Mac Worker 시작·중지와 권한 설정
- 로컬 SQLite 캐시

### 4.5 모바일 앱

- 오늘 일정과 할 일 조회·수정
- 최근 기억 및 대화 조회
- AI 대화와 스트리밍
- 실행 승인 또는 거절
- 마지막 동기화 시각과 서버 상태 표시
- 오프라인 조회와 변경 대기열
- 생체 인증으로 앱 잠금 해제

### 4.6 Mac Worker

- 서버로만 outbound 연결
- 사용자가 허용한 workspace 목록 제공
- 파일 읽기·수정과 명령 실행 요청 수신
- 매 작업 또는 세션 단위 승인
- 명령 결과와 변경 파일을 서버에 반환
- Mac sleep/종료 시 offline 상태 전환

Agent Runner는 App Server에 로컬 MCP bridge를 등록하고, Mac Worker가 online일 때만 다음과 같은 제한 도구를 제공한다.

```text
mac.list_workspaces
mac.read_file
mac.search_files
mac.propose_patch
mac.run_command
```

MCP bridge는 실제 실행을 직접 하지 않는다. 승인 레코드를 생성하고 Mac Worker에 작업을 전달한 뒤 결과만 Codex에 반환한다.

Mac Worker는 `v0.1`에서 한 개 workspace와 제한된 명령 실행으로 수직 검증하고, 전체 개발 에이전트 기능은 이후 확장한다.

## 5. 인증 체계

Jimin OS 로그인과 AI 로그인은 서로 분리한다.

### 5.1 Google Calendar 연결

Google OAuth는 Jimin OS 앱 로그인에 사용하지 않고 Calendar 연결 권한에만 사용한다.

- 앱이 QR 기기 연결로 등록된 뒤 사용자가 필요할 때만 system browser에서 Google Calendar 권한을 연결한다.
- embedded WebView로 Google 로그인 페이지를 띄우지 않는다.
- iOS/Android/macOS별 Calendar OAuth client와 redirect URI를 구성한다.
- Authorization Code + PKCE를 사용한다.
- 서버가 Calendar refresh token을 암호화해 저장한다.
- Calendar scope는 필요한 최소 범위부터 요청한다.
- Calendar 연결 해제 시 해당 provider token을 폐기하며 Jimin OS 기기 session은 유지한다.

### 5.2 ChatGPT 로그인

ChatGPT 로그인은 서버의 AI 런타임 전용이다.

1. 서버 배포 후 관리자가 `codex login --device-auth`를 실행한다.
2. 개인 브라우저에서 일회용 코드를 승인한다.
3. Codex 인증 저장소를 전용 persistent volume에 보관한다.
4. Agent container만 해당 volume을 읽을 수 있다.
5. 앱 클라이언트에는 ChatGPT credential을 전달하지 않는다.

서버가 headless 환경이면 device-code login을 우선 사용한다. 인증 파일을 저장소, 이미지, 로그, 백업 원문에 포함하지 않는다.

### 5.3 Jimin OS 세션

- QR 기기 연결 코드가 한 번만 교환되면 서버가 기기별 access/refresh session을 발급한다.
- access token은 짧게 유지한다.
- refresh token은 기기별로 발급하고 폐기할 수 있게 한다.
- 서버에는 사용자가 생성한 QR 기기 연결 코드와 등록된 device만 저장한다.
- 모든 쓰기 endpoint는 인증 guard, 입력 검증, 크기 제한을 적용한다.
- 승인 결정은 compare-and-set으로 한 번만 처리한다.

## 6. 가용성 기준

| 상황 | 일정/할 일 | 기억/과거 대화 | 서버 AI | Mac 로컬 작업 |
|---|---:|---:|---:|---:|
| 서버 ON, Mac ON | 가능 | 가능 | 가능 | 가능 |
| 서버 ON, Mac OFF | 가능 | 가능 | 가능 | 불가능 |
| 서버 일시 OFF, 모바일 cache 존재 | 읽기 가능 | 최근 데이터 읽기 가능 | 불가능 | 불가능 |
| 인터넷 단절, 같은 LAN에서 서버 접근 가능 | 서버 데이터 가능 | 가능 | OpenAI 연결 불가 | 로컬 요청 대기 |
| Google API 일시 장애 | 마지막 동기화 데이터 가능 | 가능 | 가능 | 가능 |

핵심 출시 조건은 Mac이 꺼진 상태에서도 휴대폰에서 오늘 일정을 열 수 있는 것이다.

## 7. 데이터 소유권과 최소 스키마

### 원본 위치

| 데이터 | 원본 |
|---|---|
| 사용자 및 기기 세션 | PostgreSQL |
| Google Calendar 동기화 상태 | PostgreSQL |
| 일정 read model | PostgreSQL |
| 할 일·프로젝트 | PostgreSQL |
| 개인 기억·출처 | PostgreSQL |
| 대화 read model | PostgreSQL |
| Codex thread 원본 | Agent volume의 `CODEX_HOME` |
| ChatGPT credential | Agent 전용 secret volume |
| 모바일·데스크톱 캐시 | 각 기기의 SQLite |
| Mac 로컬 파일 | Mac 원본, 서버에는 필요한 결과만 저장 |

### 핵심 테이블

```text
users
devices
sessions

calendar_accounts
calendar_sync_states
calendar_events

projects
tasks

memories
memory_sources
memory_revisions

conversations
messages
agent_jobs
agent_events
approvals

worker_nodes
worker_capabilities
audit_logs
sync_changes
```

### 클라이언트 동기화

- 서버가 `sync_changes.sequence`를 단조 증가시킨다.
- 클라이언트는 마지막 `sequence` 이후 변경만 요청한다.
- 일정과 메시지는 서버 우선으로 처리한다.
- 모바일 오프라인 변경은 idempotency key와 함께 재전송한다.
- 충돌 시 서버 값을 유지하되 사용자 변경 이력을 보존한다.
- CRDT는 다중 사용자 요구가 생기기 전까지 도입하지 않는다.

## 8. Google Calendar 동기화

### 읽기

1. Google OAuth 완료
2. Calendar 목록과 기본 캘린더 조회
3. 최초 전체 event sync
4. 마지막 페이지의 `nextSyncToken` 저장
5. 이후 `syncToken`으로 증분 변경만 조회
6. 삭제 이벤트도 read model에 반영
7. Google이 `410 Gone`을 반환하면 token과 read model을 정리하고 전체 sync 재실행

### 쓰기

```text
모바일 또는 Mac에서 일정 변경
→ 서버 validation
→ pending 상태 저장
→ Google Calendar API 반영
→ 성공 응답과 Google event version 저장
→ sync_changes 발행
→ 연결된 기기 갱신
```

Google 반영 실패 시 사용자 변경을 잃지 않고 `sync_failed`로 보관하며 다시 시도할 수 있게 한다.

### 오프라인 캐시 범위

- 지난 30일 일정
- 향후 90일 일정
- 오늘과 이번 주 할 일
- 최근 대화와 고정 기억
- 마지막 동기화 시각

## 9. Codex App Server 통합

App Server는 서버 내부 Agent Runner의 child process다.

```text
Rust Agent Runner
  └─ codex app-server --listen stdio://
       ├─ initialize
       ├─ account/read
       ├─ thread/start / thread/resume
       ├─ turn/start
       ├─ item/* stream
       └─ approval request/response
```

### 호환성 규칙

1. M0 개발 기준은 실제 account와 provider 호환성을 검증한 `codex-cli 0.144.1`이다.
2. 해당 버전으로 JSON Schema를 생성하고 저장한다.
3. 운영에서는 정확한 한 버전보다 최소/최대 지원 범위를 둔다.
4. 서버 시작 시 `codex --version`과 필수 method를 검사한다.
5. 지원 범위를 벗어나면 일정 API는 계속 제공하고 AI 기능만 unavailable로 전환한다.
6. App Server 원본 타입은 `codex-client` 크레이트 밖으로 노출하지 않는다.
7. App Server의 experimental WebSocket transport는 사용하지 않는다.

### 서버 도구 경계

- Agent container는 root로 실행하지 않는다.
- Docker socket을 mount하지 않는다.
- 기본 root filesystem은 read-only로 둔다.
- 쓰기가 필요한 workspace와 temp만 별도 volume으로 연다.
- production secrets 디렉터리는 Agent tool sandbox에 노출하지 않는다.
- 네트워크 권한은 필요한 destination만 허용하는 방향으로 단계적으로 제한한다.
- 파일 변경과 위험 명령은 앱 승인 없이는 실행하지 않는다.

## 10. 배포 구조

### Docker Compose 서비스

```text
gateway
  - TLS 종료
  - private hostname
  - API/WSS reverse proxy

api
  - Rust Axum API
  - sync protocol
  - auth/session
  - calendar scheduler

agent
  - Rust Agent Runner
  - Codex CLI/App Server
  - isolated workspace

postgres
  - primary data store

backup
  - scheduled pg_dump
  - retention and restore verification
```

Redis는 MVP에 넣지 않는다. Agent job queue는 PostgreSQL row lock과 `SKIP LOCKED`로 시작한다.

### Persistent volume

- `postgres_data`
- `codex_home`
- `agent_workspace`
- `attachments`
- `backups`

### Secret

- Google OAuth client secret
- Google token encryption key
- Jimin OS session signing key
- database password
- gateway TLS 관련 secret
- Codex auth store

secret은 이미지와 Git에 포함하지 않는다. Docker secret 또는 root-only mounted file을 우선 사용한다.

### 네트워크

- `gateway`만 사용자 사설 네트워크에서 접근 가능
- `postgres`와 `agent`는 내부 Compose network만 사용
- Mac Worker는 서버로 outbound 연결하므로 Mac에 inbound port가 필요 없음
- 서버는 OpenAI, Google, image registry로 outbound 가능해야 함
- TLS certificate와 private DNS를 준비한다

## 11. 환경 분리

| 환경 | 위치 | 용도 |
|---|---|---|
| local | 개발 Mac | unit/integration, 빠른 UI 개발 |
| staging | 로컬 서버 | 실제 네트워크, Google OAuth, Codex, 개인 폰 검증 |
| production | 로컬 서버 | 개인 일상 사용 |

staging과 production은 Compose project, database, volume, hostname, OAuth redirect를 분리한다.

Google Cloud 프로젝트도 staging과 production을 분리한다.

- staging은 OAuth `Testing` 상태와 개인 계정 test user로 사용한다.
- Testing 상태에서 profile 외 scope를 사용하면 refresh token이 7일 후 만료될 수 있음을 테스트 조건에 포함한다.
- production은 개인용 앱으로 `In production` 상태를 사용한다.
- 개인용 100명 미만 앱은 검증 없이 사용할 수 있지만 로그인 과정에서 unverified 경고가 나타날 수 있다.
- production 전환 전 최소 Calendar scope, 개인정보 처리 설명, 앱 이름과 지원 정보를 정리한다.

예시:

```text
https://jimin-os-dev.<private-domain>
https://jimin-os.<private-domain>
```

같은 물리 서버를 사용하더라도 staging에서 검증한 image digest만 production으로 승격한다.

## 12. 저장소 구조

```text
jimin-os/
├─ apps/
│  ├─ api/                 # Rust HTTP/WSS server
│  ├─ agent/               # Rust Agent Runner + Codex process manager
│  ├─ desktop/             # macOS Tauri app
│  ├─ mobile/              # iOS/Android client
│  └─ mac-worker/          # optional local execution node
├─ crates/
│  ├─ domain/
│  ├─ application/
│  ├─ storage/
│  ├─ auth/
│  ├─ calendar-google/
│  ├─ codex-client/
│  ├─ agent-runtime/
│  ├─ sync-protocol/
│  ├─ worker-protocol/
│  └─ observability/
├─ packages/
│  ├─ design-tokens/
│  ├─ chat-renderer/
│  ├─ client-state/
│  └─ api-client/
├─ migrations/
├─ schemas/
│  ├─ api/
│  └─ codex/
├─ deploy/
│  ├─ compose.yaml
│  ├─ compose.staging.yaml
│  ├─ compose.production.yaml
│  ├─ gateway/
│  └─ scripts/
├─ docs/
│  └─ adr/
├─ Cargo.toml
└─ pnpm-workspace.yaml
```

## 13. API와 이벤트 범위

### 주요 API

- `POST /v1/auth/google/exchange`
- `POST /v1/auth/refresh`
- `POST /v1/auth/logout`
- `GET /v1/today`
- `GET /v1/calendar/events`
- `POST /v1/calendar/events`
- `PATCH /v1/calendar/events/{id}`
- `DELETE /v1/calendar/events/{id}`
- `GET /v1/tasks`
- `POST /v1/tasks`
- `GET /v1/memories`
- `POST /v1/memories`
- `GET /v1/conversations`
- `POST /v1/conversations/{id}/turns`
- `POST /v1/approvals/{id}/decision`
- `GET /v1/sync/changes?after={sequence}`
- `GET /v1/nodes`
- `DELETE /v1/devices/{id}`
- `GET /health/live`
- `GET /health/ready`

### WSS 이벤트

```text
sync.changed
calendar.sync.started
calendar.sync.completed
calendar.sync.failed
agent.turn.started
agent.message.delta
agent.tool.progress
agent.approval.required
agent.approval.resolved
agent.turn.completed
agent.turn.failed
node.status.changed
```

모든 이벤트는 `event_id`, `created_at`, 관련 entity ID를 포함한다. 재연결 시 마지막 `event_id` 이후 이벤트를 복구한다.

## 14. `v0.1` 범위

### 반드시 포함

- Docker Compose로 로컬 서버 배포
- Google 계정 한 개 allowlist 로그인
- Google Calendar 최초/증분 sync
- 오늘 및 월간 일정 조회
- 일정 생성·수정·삭제
- 할 일 기본 CRUD
- Mac과 개인 휴대폰에서 동일한 데이터 확인
- 모바일과 데스크톱 SQLite 캐시
- Mac이 꺼진 상태의 휴대폰 일정 조회
- 서버 장애 시 마지막 일정 오프라인 조회
- 서버의 ChatGPT device-code login
- Codex App Server 기반 대화와 스트리밍
- 기본 기억 저장·검색·출처
- 승인 요청과 감사 로그
- 서버 backup/restore 검증
- Mac Worker 수직 검증 1개

### 이후로 미룸

- Gmail 및 다른 Google 서비스
- Slack, Discord, Telegram 연동
- public relay와 공개 SaaS 운영
- 다중 사용자·조직 기능
- 복수 AI provider 자동 라우팅
- 무승인 자율 작업
- 대규모 지식 그래프
- CRDT
- Google Calendar webhook
- 정교한 푸시 알림
- 여러 Mac Worker 동시 실행

## 15. 단계별 구현 게이트

일정이나 기간을 먼저 고정하지 않는다. 각 단계의 상세 명세와 완료 게이트를 충족한 뒤 다음 단계로 넘어간다. 구현 중 범위가 달라지면 해당 명세와 ADR을 먼저 갱신한다.

| 단계 | 구현 목적 | 다음 단계 진입 조건 | 상세 명세 |
|---|---|---|---|
| M0. 배포·기술 스파이크 | 서버·App Server·모바일의 핵심 기술 위험 제거 | staging health, Mac/폰 연결, Codex stream, 모바일 프레임워크 결정 | [M0 명세](specs/M0_DEPLOYMENT_SPIKE.md) |
| M1. 서버 기반 | API, DB, 인증, session, sync 공통 기반 확정 | 개인 Google identity로 인증하고 두 기기에서 동일 데이터 확인 | [M1 명세](specs/M1_SERVER_FOUNDATION.md) |
| M2. Google Calendar | Calendar 원본과 서버 read model의 안전한 양방향 동기화 | Mac 없이 폰에서 일정 조회·변경 후 Google 반영 확인 | [M2 명세](specs/M2_GOOGLE_CALENDAR.md) |
| M3. Mac·모바일 클라이언트 | 공통 기능, 오프라인 cache, 재연결과 상태 UX 구현 | Mac/실기기 online·offline 시나리오와 중복 동기화 방지 통과 | [M3 명세](specs/M3_CLIENTS.md) |
| M4. 서버 AI | App Server adapter, job, stream, 승인과 장애 격리 | 서버 재시작 후 대화 재개, Agent 장애 중 일정 API 정상 | [M4 명세](specs/M4_SERVER_AGENT.md) |
| M5. 기억 시스템 | 출처와 유효성을 가진 장기 기억 검색 | 평가 질문에서 현재 유효한 기억과 근거를 재현 | [M5 명세](specs/M5_MEMORY.md) |
| M6. Mac Worker | 승인된 Mac 로컬 파일·명령 작업 연결 | 허용 workspace에서 제한 작업을 정확히 한 번 실행 | [M6 명세](specs/M6_MAC_WORKER.md) |
| M7. 안정화·릴리스 | 배포, backup/restore, 보안, 관찰성, rollback 완성 | 모든 출시 판정과 restore drill, 실제 기기 회귀 통과 | [M7 명세](specs/M7_HARDENING_RELEASE.md) |

상세 명세의 공통 계약과 사용 방법은 [단계별 명세 인덱스](specs/README.md)와 [공통 구현 계약](specs/SHARED_CONTRACTS.md)을 따른다.

## 16. 첫 구현 순서

첫 번째 작업 묶음은 UI보다 배포 가능한 수직 구조를 만드는 데 집중한다.

1. Cargo/pnpm workspace 생성
2. `api`, `agent`, `desktop`, `mobile` 최소 앱 생성
3. PostgreSQL migration runner 구현
4. `/health/live`, `/health/ready` 구현
5. Compose로 `api + postgres + gateway` 실행
6. Mac에서 local stack 연결 확인
7. 로컬 서버 staging 배포
8. 개인 폰 브라우저 또는 최소 앱에서 health 확인
9. Codex CLI 설치와 `CODEX_HOME` volume 구성
10. `codex login --device-auth` 운영 절차 작성
11. App Server JSON Schema 생성
12. headless `thread/start → turn/start → delta → completed` 통합 테스트
13. 최소 Mac/모바일 앱에서 서버 status 표시
14. Tauri 모바일 secure storage, WSS reconnect, 앱 resume 실기기 검증
15. Google OAuth 개발 client와 Calendar test calendar 구성

이 묶음이 끝나기 전에는 정교한 화면이나 자동 기억 추출을 구현하지 않는다.

## 17. 테스트 계획

### 자동 테스트

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm lint
pnpm typecheck
pnpm test
database migration test
OpenAPI contract test
Docker image build
Compose smoke test
desktop build smoke
iOS/Android build smoke
```

### Mac 검증

1. Google 로그인
2. 오늘 일정 조회 및 수정
3. AI 대화 스트리밍
4. 승인 요청 처리
5. 서버 재연결
6. cache에서 offline 일정 조회
7. Mac Worker 연결과 제한 명령 실행

### 개인 휴대폰 실기기 검증

1. 사설 네트워크에서 staging 접속
2. system browser Google 로그인
3. 오늘·주간 일정 조회
4. 일정 생성 후 Google Calendar와 Mac 앱에 반영
5. Mac을 끈 상태에서 일정과 AI 대화 사용
6. 네트워크를 끄고 최근 일정 조회
7. 오프라인 변경 후 재연결 동기화
8. AI stream 중 background/foreground 전환
9. 승인 요청 수신과 중복 처리 방지
10. 앱 재설치 후 기존 device session 폐기 확인

### 서버 검증

- container 재시작 후 데이터와 Codex session 유지
- PostgreSQL backup에서 빈 staging 환경 복구
- Google access token refresh
- invalid `syncToken` 전체 재동기화
- Agent container 장애 중 일정 API 정상
- 디스크 부족, Google 장애, OpenAI 장애의 사용자 상태 표시
- 이전 image digest로 rollback

## 18. 배포와 업데이트 절차

```text
main merge
→ CI test/build
→ linux/amd64 + linux/arm64 image 생성
→ commit SHA로 image tag
→ staging pull/up
→ migration dry-run + backup
→ Mac/폰 smoke test
→ 동일 image digest를 production으로 승격
→ health 확인
→ 실패 시 이전 digest로 rollback
```

Database migration은 transaction 가능한 변경을 우선한다. 배포 전 backup을 만들고 staging 복구 테스트가 없는 migration은 production에 적용하지 않는다.

## 19. 보안 기준

- API를 public internet에 직접 노출하지 않는다.
- 모든 클라이언트 통신은 TLS를 사용한다.
- Google OAuth는 system browser/공식 SDK와 PKCE를 사용한다.
- 단일 Google account allowlist를 적용한다.
- Calendar refresh token은 application-level encryption 후 저장한다.
- ChatGPT credential은 Agent 전용 volume에만 둔다.
- Docker socket, host root, secret directory를 Agent에 mount하지 않는다.
- Agent 및 API container를 non-root로 실행한다.
- 요청 body schema, 크기, rate limit을 검증한다.
- 모든 외부 입력과 tool 결과를 신뢰하지 않는다.
- 승인과 민감 작업을 audit log에 남긴다.
- 로그에서 token, 일정 본문, 메시지 원문을 기본 마스킹한다.
- 기기·Mac Worker credential을 개별 폐기할 수 있게 한다.

## 20. 주요 위험과 대응

| 위험 | 대응 |
|---|---|
| 로컬 서버 다운 | 클라이언트 SQLite cache, backup, health 알림, 복구 문서 |
| App Server protocol 변경 | Codex version 범위, version별 schema, adapter 격리 |
| Headless ChatGPT 로그인 실패 | device-code 우선, 공식 auth cache 복사 절차를 비상 경로로 문서화 |
| Tauri 모바일 제약 | M0 실기기 스파이크, 실패 시 모바일만 Expo/RN 전환 |
| Google OAuth 복잡성 | 플랫폼별 client 분리, PKCE, 최소 scope, test user로 시작 |
| Calendar 누락/중복 | syncToken, idempotency key, Google event version, 410 full resync |
| Agent가 서버 secret 접근 | 별도 container, volume 분리, non-root, read-only rootfs |
| Mac Worker 오용 | outbound-only, capability allowlist, 매 작업 승인, 즉시 폐기 |
| 범위 팽창 | `v0.1` 외 연동과 자동화 동결 |

## 21. `v0.1` 출시 판정

다음 시나리오가 모두 통과해야 한다.

1. 로컬 서버를 새 환경에 Compose 명령으로 배포할 수 있다.
2. 개인 Google 계정 외 사용자는 로그인할 수 없다.
3. Mac을 끈 상태에서 휴대폰으로 오늘 일정을 볼 수 있다.
4. 휴대폰에서 만든 일정이 Google Calendar와 Mac 앱에 반영된다.
5. 서버가 잠시 꺼져도 휴대폰에서 마지막 일정은 볼 수 있다.
6. 서버에서 ChatGPT 구독으로 AI 대화를 실행하고 응답을 스트리밍한다.
7. Agent 장애가 일정·할 일 API를 중단시키지 않는다.
8. 잘못된 기억을 출처와 함께 확인하고 무효화할 수 있다.
9. 승인 전에는 파일 변경이나 명령이 실행되지 않는다.
10. Mac Worker offline 상태가 정확히 표시된다.
11. PostgreSQL과 필요한 volume을 backup에서 복원할 수 있다.
12. staging에서 검증한 동일 image를 production으로 승격하고 rollback할 수 있다.

## 22. 공식 참고 자료

- [OpenAI Codex App Server](https://developers.openai.com/codex/app-server/)
- [OpenAI Codex 인증](https://developers.openai.com/codex/auth/)
- [Google OAuth 2.0 for iOS & Desktop](https://developers.google.com/identity/protocols/oauth2/native-app)
- [Google OAuth 앱 검증이 필요하지 않은 경우](https://support.google.com/cloud/answer/13464323)
- [Google OAuth 앱 audience와 Testing 제한](https://support.google.com/cloud/answer/15549945)
- [Google Calendar 증분 동기화](https://developers.google.com/workspace/calendar/api/guides/sync)
- [Tauri 2](https://v2.tauri.app/)
- [Tauri 모바일 플러그인 개발](https://v2.tauri.app/develop/plugins/develop-mobile/)

## 23. 운영 업무 개선 계획 — 2026-10-08

기존 기능과 승인 기록을 보존하며, 개발 환경에서 확인한 뒤 별도 승인으로 운영에 배포한다. 운영 앱과 개인 휴대폰은 개발 테스트 대상으로 바꾸지 않는다. 모바일 테스트는 에뮬레이터에서 진행한다.

### 기능 추가

| 구분 | 작업                                        | 완료 기준                                                           | 순서                                         |
| ---- | ------------------------------------------- | ------------------------------------------------------------------- | -------------------------------------------- |
| N1   | 예약 업무에 한국 공휴일 제외                | 공식 날짜 자료, 제외 이력, 자료 미확인 시 발송 보류, 직접 실행 확인 | 1차                                          |
| N2   | AI가 새 업무로 분류하지 않은 외부 요청 보존 | 자동 폐기하지 않고 ‘확인 필요’에서 원문·재분석·제외 제공            | 1차                                          |
| N3   | 처리 실패 모아 보기                         | 저장·배정·알림·원문 표시·답글을 구분하고 필요한 단계만 재시도       | 개발 완료·컨펌 대기, 외부 실전송은 배포 검증 |
| N4   | 업무 진행 상태와 대기 사유                  | 진행 전·진행 중·대기·완료와 담당자·대기 원인 표시                   | 개발 완료·컨펌 대기                          |
| N5   | 기존 일감의 새 답글로 변경 제안             | 신규 일감으로 중복 승격하지 않고 기한·내용 변경을 승인              | 개발 완료·컨펌 대기                          |
| N6   | 조건부 후속 관리                            | 응답 없음·마감 임박·완료 확인 등 조건, 중복 발송 방지·실행 이력     | 개발 완료·컨펌 대기                          |

### 기존 기능 변경

| 구분 | 작업                                            | 완료 기준                                                 | 순서                |
| ---- | ----------------------------------------------- | --------------------------------------------------------- | ------------------- |
| C1   | 예약 설정·미리보기·실행 이력에 공휴일 상태 표시 | 기존 예약은 제외 옵션 꺼짐, 휴일·미확인 사유 구분         | 1차                 |
| C2   | 티켓번호와 AI 분류로 외부 요청 숨기지 않기      | 불확실한 요청은 보존하되 자동 일감 등록·알림은 하지 않음  | 1차                 |
| C3   | 저장과 외부 전송 상태 분리                      | 일부 실패가 전체 성공으로 보이지 않고 각각 재시도 가능    | 개발 완료·컨펌 대기 |
| C4   | 홈 업무 확인·완료와 담당자별·일자별 보기        | PC·모바일에서 AI 요청 없이 확인·완료·상세 수정            | 개발 완료·컨펌 대기 |
| C5   | ‘결정할 일’ 독립 탭을 홈에 통합                 | 중복 유입 화면 제거, 승인·ITSM 확인·실패 재시도·기록 보존 | 1차                 |
| C6   | 보고서에서 실제 업무로 연결                     | 집계 기준 표시, 관련 일감·유입·대기 사유 바로 확인        | 개발 완료·컨펌 대기 |

### 1차 작업과 검증

1. 공휴일 달력은 [한국천문연구원 특일 정보](https://www.data.go.kr/data/15012690/openapi.do)를 서버에서 동기화한다. 월별 자료가 비어 있어도 조회 완료 여부를 기록해 ‘공휴일 없음’과 ‘자료 없음’을 구분한다.
2. 제외 옵션을 켠 예약은 한국 시간의 예약 날짜로 판단한다. 휴일에는 최초 안내와 후속 안내를 모두 제외하고 이력을 남긴다. 자료를 확인하지 못하면 발송하지 않는다. 직접 실행은 명시적인 확인 후에만 예외 처리한다. 기존 예약과 일감의 마감일은 바꾸지 않는다.
3. 외부 대화가 AI의 다른 분류로 숨겨지는 경로를 제거한다. ‘새 업무 / 기존 일감 / 확인 필요’를 한 영역의 탭으로 제공하고, 불확실한 분류는 자동 승격하지 않는다.
4. ‘결정할 일’ 메뉴는 제거하되 고유한 승인·재시도·ITSM 연결 확인과 기록은 홈의 접힌 영역에 통합한다. 이전 화면 주소는 홈으로 연결한다.
5. 서버 단위·PostgreSQL 격리 테스트, 화면 타입 검사·회귀 테스트·빌드, PC·모바일 반응형 및 에뮬레이터 확인을 수행한다. 실제 공휴일 자료 연동에는 서버용 인증키 설정이 필요하며 미설정 상태는 별도로 보고한다.

1차 당시 상태: 1차 기능 구현과 개발 환경 회귀 확인을 완료했다. 아래의 1차 결과는 당시 기록이며, 현재 후속 완료 상태는 마지막 절에서 별도로 기록한다.

### 1차 확인 결과

- 서버·저장소 단위 테스트 201개, 프런트엔드 71개 파일의 테스트 449개 통과. 전체 Rust workspace 테스트와 경고를 오류로 취급한 Clippy 검사도 통과했다. 환경이 필요한 테스트의 기본 생략과 별도 실행은 구분한다.
- PostgreSQL 격리 회귀 54개 통과. 별도의 새 데이터베이스에서 예약 업그레이드·전송·복구 계약을 명시적으로 실행해 통과했다. 기존 예약의 기본값, 두 전송 경로의 최초·후속 공휴일 제외, 자료 미확인·만료 시 전송 차단, 초안 저장, 직접 실행 확인·중복 방지, 일감 마감일 유지, UTC 날짜 및 자정 이후 지연 실행을 포함한다.
- API release 빌드, 프런트엔드 타입 검사·일반 production 빌드와 Android 개발용 APK 빌드 통과. Android는 `io.jimin.os.dev`임을 확인한 뒤 `emulator-5554`에만 설치했다. 운영 앱과 개인 기기는 변경하지 않았다.
- PC 1440×1000, 모바일 웹 390×844 및 Android WebView 411×914에서 탭·접힌 승인 영역·공휴일 미리보기·직접 실행 확인을 확인했다. 가로 넘침이 없고 모바일 탭은 높이 44px, 하단 메뉴 4개와 중앙 마이크는 유지된다. Android 뒤로가기는 예약 창을 닫고 앱 화면을 유지했다.
- 개발 예시 화면은 `http://127.0.0.1:1423/`에서 제공한다. 실제 전송은 하지 않고, 공휴일은 ‘예시 공휴일’임을 명시한다.
- 비밀정보 검사에서 기존 URL 거부 테스트의 가짜 비밀번호가 탐지됐다. 허용된 테스트 전용 표기로 교체하고 해당 네이티브 테스트 3개 및 비밀정보 검사를 다시 통과했다. 실제 인증정보가 새로 포함된 것은 아니다.
- Backend Ultrawork 정적 검사는 자동 통과하지 않았다: 문자열 검색이 타입의 `token: string`, 명시적인 개발용 예시 토큰과 자연어 요청 생성의 `update`를 각각 인증정보·SQL 조합으로 탐지한 기존 3건이다. 실제 인증정보·SQL 실행·로그 여부를 수동 검토했으며 이 항목을 자동 통과나 사용자 승인 예외로 기록하지 않는다.
- UX Writing 정적 검사도 6개 대상에서 111건을 출력했다. 코드 식별자 `null`·`undefined`·`token`, 여러 줄 문구의 키와 기존 CTA를 사용자 문구로 판단하는 경고가 포함된다. 이번 새 문구의 공개 용어, 원인·다음 행동, 직접 실행의 예외 확인은 수동 검토했다. 정적 검사 전체 통과나 승인된 예외는 아니다.
- 실제 공공데이터 인증키·실 API 응답, 운영 서버 적용·운영 알림 전송·실기기 설치는 수행하지 않았다. schema-58 실제 운영 백업의 복원은 배포 단계에서 검증한다. 신규 필드와 schema guard 때문에 규칙을 끄는 것만으로 구 API로 되돌릴 수 없으며, schema-59 호환 이미지 또는 검증한 백업 복원 절차를 사용한다.

### 후속 1단계: 원문 Chat 반영 복구

- 홈의 `Chat 반영 확인하기`는 처리 중이거나 실패한 원문 반영만 표시하고 기본으로 접는다. 성공한 대화 이력을 홈에 다시 노출하지 않는다.
- 할 일 저장, 원문 체크 표시, 담당자·마감 답글을 각각 표시한다. 업무 아님 사유 답글도 같은 영역에서 별도로 다시 보낼 수 있다.
- 수동 재시도가 성공한 체크·답글 시각을 지우던 문제를 수정했다. 이미 성공한 단계와 저장한 일감은 유지하고 남은 단계만 재요청한다. 처리 중에는 중복 요청 버튼을 제공하지 않는다.
- 프런트엔드의 오래된 실패 버전이 최신 성공 버전을 덮어 복구 항목으로 다시 나타나지 않도록 회귀 테스트를 추가했다.
- 당시 N3/C3 전체 완료는 아니었다. 담당자 배정 웹훅, 기기 알림 전달 실패의 통합 복구는 아래 후속 완료에 포함했고, 실제 Google Chat 전송 검증은 배포 단계에 남긴다. 원문 복구 변경 자체에는 새 DB migration이 없다.
- 개발 확인과 맥 알림 진단의 범위·제한은 `docs/qa/2026-10-08-inflow-recovery-and-mac-notifications.md`에 기록한다. 운영 서버·운영 앱·개인 휴대폰 배포는 하지 않았다. Android Now Bar는 사용자가 필요 없다고 답해 제외했다.

### 후속 개발 완료와 컨펌 요청

1. N3/C3: 원문 Chat 표시·답글에 더해 배정 웹훅과 기기 알림 실패를 홈의 접힌 영역에서 분리해 확인·재시도한다. 성공한 단계는 보존하고 처리 중·만료된 대상은 중복 전송하지 않는다.
2. N4/C4: 홈에서 오늘·기한 지남·전체, 담당자별·일자별, 진행 상태를 직접 선택한다. 일감의 상세·수정·선택적 답글 완료를 홈에서 처리하며, 프로젝트에서도 같은 진행 상태와 필수 대기 사유를 제공한다. 여러 담당자의 일감은 각각의 그룹에 보이지만 전체 건수는 중복 계산하지 않는다.
3. N5: 기존 일감의 현재 답글 분석 결과를 바탕으로 내용·기한 변경을 명시적으로 승인한다. 승인 시 최신 버전을 확인하고 기록을 남기며, 새 일감은 만들지 않고 이후 답글은 읽지 않은 상태로 보존한다.
4. N6: 마감 임박·대기 중 응답 없음·최근 30일 완료 확인 조건을 예약에 추가했다. 미리보기와 실행 때의 상태를 각각 확인하고 조건이 달라지면 건너뛴다. 예약 슬롯의 중복 발송 방지도 유지한다.
5. C6: 주간 리포트의 집계 기준과 실제 근거 목록을 제공하고 선택한 일감 상세·대기 사유·유입 대화로 연결한다.
6. Mac: 계정별 최근 확인 위치를 안전하게 저장하고 서버 확인·알림 전달 상태와 테스트 알림을 설정에서 확인한다. 테스트 알림 접수는 확인했으나 실제 OS 배너 표시는 별도 검증이다.
7. 프런트엔드 479개, API/저장소 202개, agent 84개, Mac 네이티브 10개와 PostgreSQL 격리 56개 테스트를 통과했다. 별도 예약 통합 테스트, migration 트랜잭션 롤백, scoped Clippy·타입 검사·release/web/Android/Mac 개발 빌드와 PC·320/390px 모바일·Android 에뮬레이터 실제 조작을 완료했다. 정적 문구 검사 경고와 미실행 외부 검증은 `docs/qa/2026-10-08-section23-completion.md`에 구분했다.

현재 상태: 계획된 남은 개발과 개발 환경 화면 검증을 마쳤으며 **사용자 컨펌 대기**다. 검수용 예시 화면은 `http://127.0.0.1:1423/`에 유지한다. 실제 공휴일 API 인증키/응답, Google Chat·FCM 실전송, Mac 배너 표시와 실제 운영 백업 복원은 배포·외부 검증 게이트다. 운영 서버·운영 데스크탑·개인 휴대폰은 변경하지 않았다. 사용자 컨펌 뒤 개발 산출물 정리와 별도 승인된 운영 배포를 진행한다.

사용자 피드백 반영: 주로 확인하는 요청 영역을 상단에 복원했다. 새 업무 요청·기존 일감 업데이트·확인 필요 탭과 대화 요청 영역을 먼저 보여주고, 직접 일감 확인·완료 기능은 그 아래에 유지한다. PC·모바일 순서 회귀를 추가해 프런트엔드 481개 테스트와 일반 웹 빌드를 통과했다. 개발 웹에서 확인할 수 있으며, 이번 배치 변경의 네이티브 재빌드·운영 배포는 하지 않았다.

추가 배치 피드백 반영: 예약 업무를 인사말 바로 아래의 맨 상단으로 복원했다. 순서는 예약 업무 → 업무 요청 탭 → 대화 요청 → 할 일이다. 알림 전송/Chat 반영 복구와 승인·처리 기록은 `확인·처리 기록` 하나에 묶고 기본으로 접어 메인에 별도 보조 카드를 노출하지 않는다. 필요한 재시도와 기록은 그대로 보존한다. 프런트엔드 484개 테스트와 타입 검사·일반 웹 빌드를 통과했고 PC·390/320px 모바일에서 예약 창 닫기, 기본 접힘, 복구 실패/성공과 건수 감소를 실제 조작으로 확인했다. 최신 근거와 정적 문구 경고는 `docs/qa/2026-10-08-section23-completion.md` 마지막 절에 기록했다. 개발 웹만 갱신했으며 사용자 컨펌 전 운영·실기기는 변경하지 않았다.

일감 배정 내용 불일치 수정: 입력한 수정 내용은 일감에 저장되지만 Chat에는 기존 AI 분석이 전송되던 원인을 확인했다. 배정 모달에서 승인한 공유 내용은 별도 공개 스냅샷으로 저장해 배정 알림·원문 답글이 같은 내용을 쓰도록 변경했다. 미리보기는 입력에 즉시 반응하며 담당자·마감·내용·관련 링크와 길이 제한을 확인할 수 있다. 일반 내부 메모 수정은 자동으로 외부에 공개하지 않는다. 프런트엔드 489개, API/저장소 205개, PostgreSQL 격리 56개 테스트 및 PC·390/320px 모바일 편집/등록 화면 확인을 완료했다. schema 61→62 적용·트랜잭션 롤백을 격리 DB에서 확인했으며 운영 배포에는 서버·DB·클라이언트 변경이 모두 필요하다. 개발 웹에서 컨펌 대기 중이고, 실제 Chat 전송·운영 배포·실기기 설치는 하지 않았다. 근거 및 정적 검사/빌드 경고는 QA 기록에 구분한다.

### ITSM 마감일·완료 상태 반영 — 2026-10-20

- 검증된 원문 티켓 하나를 일감에 연결하고, 등록·기한 변경·완료 시 반영 요청을 같은 저장 작업에 기록한다. 여러 티켓은 명시적으로 선택하며 함께 반영하기를 끌 수 있다. 기존 관리자 키 파일을 재사용하고, 원격 쓰기는 마감일·상태 두 필드에 제한한다.
- 한국 날짜로 마감일을 반영한다. 한 티켓에 여러 일감이 있으면 가장 늦은 기한을 사용하며 연결된 일감 전체가 완료되기 전에는 종료하지 않는다. 처음 기한 없이 등록한 경우는 기존 날짜를 유지하고, 이후 기한을 지우면 원격 날짜도 지운다.
- LLM 처리와 분리된 작업자가 재조회로 실제 반영을 확인한다. 일시 오류는 제한적으로 재시도하고 권한·상태 전환 실패는 홈의 기존 접힌 확인·처리 기록에서 ITSM만 다시 시도한다. OS 저장·완료와 성공한 Chat 반영은 유지하며 결과를 기기 간 동기화한다.
- 프런트엔드 492개, API/저장소 206개, agent 93개(실계정 모델 시험 1개 미실행), PostgreSQL 격리 57개 테스트와 PC·390/320px 모바일 조작을 통과했다. API/agent release·일반 웹 빌드, scoped Clippy·formatter, schema 62→63 트랜잭션 롤백을 확인했다. 정적 문구 경고와 빌드 크기 경고는 `docs/qa/2026-10-20-itsm-task-sync.md`에 분리했다.
- 개발 예시 화면은 `http://127.0.0.1:1423/`에 유지한다. 운영 서버·실티켓·운영 앱·개인 휴대폰은 변경하지 않았다. 운영 반영은 schema 63, API·agent·클라이언트를 함께 배포하고 실제 회사 완료 상태와 통제된 티켓으로 확인해야 한다.
