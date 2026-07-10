# 공통 구현 계약

M0~M7 구현에서 공통으로 지켜야 하는 기술 계약이다. 단계 문서와 충돌하면 이 문서를 먼저 수정하고 ADR에 이유를 남긴다.

## 1. 기술 기준

### 서버

- Rust workspace
- async runtime: Tokio
- HTTP/WSS: Axum 계열
- database: PostgreSQL + SQLx migration
- serialization: Serde
- logging/tracing: `tracing`
- API schema: 코드에서 생성되는 OpenAPI
- error boundary: typed application error

구체 crate 버전은 M0에서 고정하고 lockfile을 커밋한다. `latest` 또는 범위 없는 dependency를 production build에 사용하지 않는다.

### 클라이언트

- React + TypeScript + Vite
- Tauri 2 desktop
- Tauri 2 mobile 우선, M0 실패 기준 충족 시 Expo/React Native
- server state와 local UI state를 분리
- SQLite cache와 명시적 migration
- Rust/OpenAPI에서 생성한 API type 사용

## 2. 명명과 식별자

- 외부 JSON field: `camelCase`
- Rust와 DB field: `snake_case`
- API path: 복수형 resource 명사와 kebab-case
- event type: `domain.entity.action` 형태의 lower-case dot notation
- 새 entity ID: UUIDv7
- Google, Codex 등 외부 ID는 별도 column에 저장하고 내부 ID로 사용하지 않는다.
- client가 생성하는 offline mutation ID도 UUIDv7을 사용한다.

## 3. 시간 계약

- 저장과 API 전송은 UTC RFC 3339를 사용한다.
- PostgreSQL은 `timestamptz`를 사용한다.
- 사용자 기본 timezone은 `Asia/Seoul`로 시작하되 profile 값으로 보관한다.
- 일정 원본 timezone과 all-day 여부를 별도 보존한다.
- 날짜만 있는 값은 `YYYY-MM-DD`로 전달하고 임의로 자정 UTC로 변환하지 않는다.
- client는 표시할 때만 사용자 timezone으로 변환한다.
- 서버 clock drift가 인증·승인에 영향을 주므로 NTP 상태를 운영 점검에 포함한다.

## 4. HTTP 계약

### 기본

- base path: `/v1`
- content type: `application/json`
- 인증: `Authorization: Bearer <jimin-os-access-token>`
- request correlation: `X-Request-Id`
- mutation 재시도: `Idempotency-Key`
- optimistic concurrency가 필요한 수정: `expectedVersion`
- 목록 pagination: opaque cursor

### 정상 응답

단일 resource는 불필요한 공통 envelope 없이 resource JSON을 반환한다. 목록은 다음 형식을 사용한다.

```json
{
  "items": [],
  "nextCursor": null
}
```

생성 응답은 `201`, 삭제 완료는 response body가 없으면 `204`를 사용한다.

### 오류 응답

```json
{
  "error": {
    "code": "calendar.sync_token_invalid",
    "message": "일정을 다시 동기화하고 있어요.",
    "requestId": "019...",
    "retryable": true,
    "details": {}
  }
}
```

`message`는 사용자에게 표시 가능한 문구다. 내부 stack, SQL, provider token, command 원문은 포함하지 않는다.

### 공통 상태 코드

| 상태 | 의미 |
|---:|---|
| 400 | 형식 또는 validation 오류 |
| 401 | 로그인 필요 또는 session 만료 |
| 403 | 로그인됐지만 권한 없음 |
| 404 | resource 없음 또는 접근 불가 resource 은닉 |
| 409 | version, idempotency, state 충돌 |
| 422 | 형식은 맞지만 도메인 규칙 위반 |
| 429 | rate limit |
| 503 | provider 또는 Agent 일시 unavailable |

## 5. 인증과 세션 계약

- Google identity의 안정 식별자는 email이 아니라 `sub`를 사용한다.
- email은 사용자 표시와 allowlist 보조 검증에 사용한다.
- access token은 짧은 수명으로 발급한다.
- refresh session은 device별 row와 token family를 가진다.
- refresh token 원문은 DB에 저장하지 않고 hash 또는 암호화된 형태로 저장한다.
- rotation 후 이전 refresh token 재사용을 탐지하면 해당 family를 폐기한다.
- logout은 현재 device session만 폐기하고, 별도 action으로 모든 device를 폐기할 수 있게 한다.
- ChatGPT credential과 Jimin OS session은 완전히 분리한다.

## 6. 데이터베이스 계약

모든 mutable table은 기본적으로 아래 field를 가진다.

```text
id uuid primary key
created_at timestamptz not null
updated_at timestamptz not null
version bigint not null default 1
```

- 외부 provider 데이터에는 provider ID와 provider version/etag를 보존한다.
- 사용자 데이터 삭제가 audit 요구와 충돌하면 soft-delete와 purge 시점을 구분한다.
- migration은 forward-only를 기본으로 하되 rollback은 이전 image + backup restore로 보장한다.
- production 적용 전 staging의 복사본에서 migration을 검증한다.
- migration 실패 시 application은 ready 상태가 되면 안 된다.
- schema 변경과 read/write code는 호환 가능한 순서로 배포한다.

## 7. Idempotency 계약

일정 쓰기, offline mutation, approval, Agent job, Mac command는 중복 실행을 허용하지 않는다.

```text
idempotency_records
- key
- user_id
- operation
- request_hash
- response_status
- response_body
- created_at
- expires_at
```

- 동일 key와 동일 request는 저장된 결과를 반환한다.
- 동일 key와 다른 request는 `409`를 반환한다.
- provider 호출 전 pending record를 만들고 결과를 원자적으로 갱신한다.
- timeout 이후 재시도는 provider ID 또는 operation ID로 실제 적용 여부를 먼저 확인한다.

## 8. 동기화 계약

### 변경 로그

```text
sync_changes
- sequence bigint generated always as identity
- user_id
- entity_type
- entity_id
- operation: upsert | delete
- entity_version
- changed_at
```

- client는 마지막 반영 `sequence`를 저장한다.
- `/v1/sync/changes?after=`는 sequence 오름차순으로 반환한다.
- 한 페이지를 완전히 local transaction에 적용한 뒤 cursor를 갱신한다.
- delete는 tombstone event로 전달한다.
- cursor가 보관 범위를 벗어나면 server가 full resync를 요구한다.

### 오프라인 mutation

```text
client_mutation_id
entity_type
operation
payload
base_version
created_at
```

- client는 성공 응답을 받기 전 mutation을 삭제하지 않는다.
- 네트워크 오류는 exponential backoff와 jitter로 재시도한다.
- validation/permission conflict는 자동 재시도하지 않는다.
- conflict는 사용자 데이터 손실 없이 UI에서 해결 가능해야 한다.

## 9. WSS 이벤트 계약

```json
{
  "eventId": 1042,
  "type": "agent.message.delta",
  "occurredAt": "2026-07-10T10:00:00Z",
  "entity": {
    "type": "turn",
    "id": "019..."
  },
  "payload": {}
}
```

- WSS 연결 전 HTTP authentication과 동일한 session을 검증한다.
- client heartbeat와 server timeout을 둔다.
- 재연결 시 마지막 `eventId`를 보내 누락 이벤트를 복구한다.
- delta는 중복 수신될 수 있으므로 `eventId`로 제거한다.
- 영구 상태는 WSS만 믿지 않고 HTTP/read model로 재조회할 수 있어야 한다.
- slow client 때문에 API나 Agent loop가 block되지 않도록 bounded queue를 사용한다.

## 10. 작업과 승인 상태 계약

### Agent job

```text
queued → claimed → running → waiting_approval → running
                                  └→ declined
running → completed
running → retry_wait → queued
running → failed
running → cancelled
```

### Approval

```text
pending → accepted
pending → declined
pending → expired
pending → cancelled
```

- terminal state는 다시 변경하지 않는다.
- approval decision은 DB compare-and-set으로 처리한다.
- 승인에는 요청 대상, 위험 요약, command/patch preview, 만료 시각을 포함한다.
- 승인된 payload와 실제 실행 payload의 hash가 같아야 한다.
- 수정된 요청은 새로운 approval을 생성한다.

## 11. 보안 계약

- 외부 API, Calendar event, AI output, tool output은 신뢰하지 않는다.
- secret은 Git, image layer, 일반 DB column, log에 저장하지 않는다.
- Google token은 application-level encryption을 적용한다.
- Codex auth store는 Agent 전용 volume에 둔다.
- API, Agent, gateway는 non-root로 실행한다.
- Agent에 Docker socket과 host root를 mount하지 않는다.
- client 입력과 upload에는 size, type, path validation을 적용한다.
- path는 canonicalize 후 allowlisted root 내부인지 다시 확인한다.
- command는 shell string보다 argv 구조를 우선한다.
- private network를 authentication 대체 수단으로 취급하지 않는다.
- production backup도 원본과 같은 수준으로 보호한다.

## 12. 로그와 관찰성 계약

### 포함

- request ID
- user/device ID의 비가역 또는 내부 식별자
- job/thread/turn/item ID
- operation 이름과 상태
- latency, retry count, provider status code
- deployment version과 schema version

### 제외 또는 마스킹

- access/refresh token
- OAuth authorization code
- Calendar event 제목·본문·참석자
- 대화 원문
- 파일 내용과 command output 원문
- 환경 변수 전체 dump

health를 다음처럼 분리한다.

- `/health/live`: process event loop가 응답하는가
- `/health/ready`: DB migration, DB 연결, 필수 config가 준비됐는가
- Agent와 Google 상태는 별도 dependency status로 제공하고 일정 API readiness를 직접 실패시키지 않는다.

## 13. 테스트 계약

### 단위 테스트

- domain state transition
- validation
- conflict resolution
- serialization
- retry classification

### 통합 테스트

- PostgreSQL migration과 query
- Google/Codex adapter fixture
- HTTP auth/guard/idempotency
- WSS reconnect/replay
- process crash/recovery

### 계약 테스트

- OpenAPI와 generated client
- Codex version별 JSON Schema
- DB schema snapshot
- event envelope와 backward compatibility

### E2E

- staging server
- macOS app
- 개인 휴대폰 실기기
- Mac OFF, provider 장애, server restart, offline/reconnect

테스트 fixture에는 실제 token, 개인 일정, 실제 대화, 개인 파일을 넣지 않는다.

## 14. 호환성과 변경 정책

- `/v1` 안의 additive field는 client가 무시할 수 있어야 한다.
- field 제거나 의미 변경은 새 API version 또는 migration window가 필요하다.
- client는 서버의 `minSupportedClientVersion`을 확인한다.
- 서버는 build SHA, schema version, Codex adapter version을 status에 제공한다.
- 구버전 client가 위험한 mutation을 보내면 읽기만 허용하고 업데이트 안내를 표시한다.
- Codex protocol type은 adapter 바깥으로 노출하지 않는다.

## 15. 공통 코드 리뷰 체크

- 이 변경은 어느 단계 명세의 어느 요구를 구현하는가?
- 실패했을 때 사용자 데이터가 보존되는가?
- 재시도하면 중복 실행되는가?
- 인증과 권한이 route 및 service 양쪽에서 보장되는가?
- 외부 입력이 로그나 prompt를 통해 secret에 접근할 수 있는가?
- Mac이 꺼지거나 Agent가 죽어도 일정 기능이 유지되는가?
- migration과 구버전 client가 호환되는가?
- 자동 테스트와 실제 기기 검증이 필요한가?
