# M1. 서버 기반 명세

## 1. 목적

M1은 이후 모든 기능이 사용하는 인증된 Rust API, PostgreSQL schema, 기기별 session, migration, 변경 cursor를 구현한다. 이 단계의 결과는 “개인 Google 계정으로 로그인한 Mac과 휴대폰이 동일한 서버 사용자와 동일한 변경 순서를 본다”는 것이다.

M1에서는 일정이나 AI 기능을 넣지 않는다. 인증, 데이터 무결성, 재시도, schema 계약을 먼저 고정한다.

## 2. 선행조건

- M0 완료 게이트가 모두 통과했다.
- 서버 architecture, staging hostname, TLS 방식이 확정됐다.
- 모바일 runtime ADR이 확정됐다.
- 플랫폼별 Google OAuth proof 전달 방식이 M0 실기기에서 검증됐다.
- Google Cloud staging project에 OAuth consent screen과 필요한 client가 생성됐다.
- 로그인 허용 Google email과 안정적인 Google `sub`를 확인할 소유 계정이 준비됐다.
- access token signing key, refresh token pepper, database password의 secret mount 위치가 정해졌다.
- `SHARED_CONTRACTS.md`의 HTTP, ID, 시간, 오류 계약이 구현 기준으로 리뷰됐다.

## 3. 범위

### 3.1 포함

- Axum API application 구조와 typed error boundary
- PostgreSQL connection pool과 SQLx migration
- Google authorization code 교환과 identity 검증
- 단일 사용자 Google account allowlist
- 사용자와 기기 등록
- 짧은 수명의 Jimin OS access token
- 기기별 refresh session과 refresh token rotation/reuse detection
- 현재 기기 logout, 전체 기기 logout, 원격 기기 폐기
- 인증/권한 middleware
- idempotency record 기반
- monotonic `sync_changes`와 bootstrap/change API
- audit log와 PII/secret redaction
- code-generated OpenAPI와 route contract test
- staging에서 Mac/개인 휴대폰 동시 로그인 검증

### 3.2 제외

- Google Calendar scope와 refresh token 저장
- Calendar event, 할 일, 프로젝트, 기억, 대화 table
- WSS 연결과 push event
- 모바일 SQLite cache 구현
- Agent/Codex 상태를 API readiness에 연결하는 작업
- 다중 사용자 초대, 관리자 UI, 조직 기능
- password, email link, passkey 등 다른 로그인 수단
- public internet용 abuse 방어와 public signup

M1 schema는 `user_id`를 갖지만 개인용 한 명만 허용한다. 다중 사용자를 미리 지원하기 위한 tenant, organization, role graph는 만들지 않는다.

## 4. 컴포넌트와 의존성

```text
HTTP route
  → authentication/validation middleware
  → application use case
  → domain rule
  → repository/provider port
  → SQLx 또는 Google adapter
```

### 4.1 모듈 책임

| 위치 | 책임 | 금지 |
|---|---|---|
| `apps/api` | route 조립, middleware, process lifecycle | SQL 직접 작성, Google response 노출 |
| `crates/domain` | user/device/session/sync 상태와 validation | Axum, SQLx 의존 |
| `crates/application` | login, refresh, logout, bootstrap use case | provider-specific DTO 반환 |
| `crates/storage` | SQLx repository와 transaction | HTTP error 생성 |
| `crates/auth` | Google identity adapter, token issue/verify | Calendar scope 저장 |
| `crates/sync-protocol` | change DTO, cursor, bootstrap contract | client-specific cache logic |
| `crates/observability` | tracing, request ID, redaction | 원문 credential 기록 |

domain/application error는 하나의 `ApiError` mapper에서 상태 코드와 사용자용 오류 문구로 변환한다. database/provider error를 그대로 응답하지 않는다.

### 4.2 주요 library 역할

- Tokio: async runtime과 graceful shutdown
- Axum: HTTP router와 middleware
- SQLx: compile-checked query와 migration
- Serde: JSON serialization
- OpenAPI generator: route type에서 schema 생성
- `tracing`: 구조화 log와 span
- Ed25519 JWT library: access token 서명/검증
- constant-time/HMAC library: refresh token verifier 생성
- Google OIDC/JWKS verifier: `id_token` 검증

구체 crate와 버전은 M0 lockfile을 따른다. 두 개의 JWT/OIDC library를 혼용하지 않는다.

## 5. Google identity 로그인 계약

### 5.1 플랫폼 proof

클라이언트는 system browser 또는 Google 공식 SDK로 Authorization Code flow를 끝낸 뒤, backend용 일회용 authorization code를 서버에 전달한다. M0 ADR은 플랫폼별로 다음 profile을 고정해야 한다.

```text
OAuthPlatformProfile
- client_kind: macos | ios | android
- google_client_id
- allowed_redirect_uris
- token_endpoint_auth_method
- pkce_required
- expected_id_token_audience
```

서버는 요청의 `clientKind`로 미리 등록된 profile을 선택한다. 요청이 임의의 Google client ID, client secret, token endpoint를 지정하게 하지 않는다.

- macOS system-browser flow는 Authorization Code + PKCE를 사용한다.
- iOS/Android는 M0에서 검증한 공식 SDK의 backend authorization code 경로를 사용한다.
- profile이 PKCE를 요구하면 `codeVerifier`가 없거나 RFC 7636 길이/문자 규칙에 맞지 않을 때 거부한다.
- raw Google password, cookie, access token을 로그인 proof로 받지 않는다.
- M1 로그인에서 받은 Google access/refresh token은 Calendar 용도로 저장하지 않는다.

### 5.2 서버 검증

서버는 Google token endpoint 응답의 `id_token`을 다음 순서로 검증한다.

1. TLS와 Google 고정 token endpoint를 사용한다.
2. 서명 key를 Google JWKS에서 받아 `kid`에 맞는 key로 검증한다.
3. `iss`, `aud`, `azp`, `exp`, `iat`를 profile과 현재 시각에 맞춰 검증한다.
4. `sub` 존재 여부와 형식을 검증한다.
5. `email_verified == true`인지 확인한다.
6. 정규화한 email이 server allowlist와 일치하는지 확인한다.
7. 기존 사용자가 있으면 email이 아니라 `sub`가 같은지 확인한다.
8. code와 provider token을 즉시 memory에서 폐기하고 log에 남기지 않는다.

JWKS는 `Cache-Control` 범위에서 cache하고 unknown `kid`일 때 한 번 새로 가져온다. Google 장애 시 오래된 key를 무기한 신뢰하지 않는다.

## 6. API 계약

모든 JSON field는 `camelCase`를 사용한다. health와 auth exchange/refresh를 제외한 `/v1` route는 Bearer access token이 필요하다.

### 6.1 `POST /v1/auth/google/exchange`

인증 전 route다. IP와 client fingerprint 기준 rate limit, 작은 body limit를 적용한다.

```json
{
  "clientKind": "ios",
  "authorizationCode": "one-time-code",
  "codeVerifier": "pkce-verifier-when-required",
  "redirectUri": "registered-uri",
  "device": {
    "installationId": "01900000-0000-7000-8000-000000000001",
    "platform": "ios",
    "name": "Jimin's iPhone",
    "appVersion": "0.1.0",
    "osVersion": "platform-version"
  }
}
```

Validation:

- `clientKind`: configured enum만 허용
- `authorizationCode`: 빈 값 금지, 최대 길이 제한
- `codeVerifier`: profile 규칙에 따라 필수, 43~128자
- `redirectUri`: profile allowlist와 exact match
- `installationId`: UUIDv7
- `device.name`: trim 후 1~80자
- version field: 제어문자 금지, 길이 제한
- unknown field: 거부

성공 `200`:

```json
{
  "accessToken": "jimin-os-access-token",
  "accessTokenExpiresAt": "2026-07-10T10:15:00Z",
  "refreshToken": "josr_session.secret",
  "user": {
    "id": "01900000-0000-7000-8000-000000000010",
    "email": "owner@example.com",
    "displayName": "Jimin",
    "timeZone": "Asia/Seoul",
    "version": 1
  },
  "device": {
    "id": "01900000-0000-7000-8000-000000000011",
    "platform": "ios",
    "name": "Jimin's iPhone",
    "status": "active",
    "version": 1
  },
  "syncCursor": "0"
}
```

authorization code가 한 번 소비된 뒤 Jimin OS 응답 전달만 실패한 경우 클라이언트는 Google 로그인 flow를 새로 시작한다. 같은 code를 자동 재전송하지 않는다.

### 6.2 `POST /v1/auth/refresh`

```json
{
  "refreshToken": "josr_session.secret",
  "installationId": "01900000-0000-7000-8000-000000000001"
}
```

성공 `200`은 새 access token과 새 refresh token을 모두 반환한다. 이전 refresh token은 같은 transaction에서 `rotated`가 된다. 동일 token으로 두 요청이 경쟁하면 하나만 성공하고 다른 요청은 reuse detection 경로로 이동한다.

클라이언트는 새 token을 secure storage에 저장한 뒤에만 이전 token을 버린다. 응답이 유실되면 session family가 안전하게 복구되지 않을 수 있으므로 refresh rotation에는 짧은 grace record가 아니라 명시적 reuse 정책을 적용한다. 현재 개인 앱 정책은 rotated token 재사용을 family 침해로 간주하고 해당 family를 폐기하는 것이다.

### 6.3 `POST /v1/auth/logout`

- Bearer 인증 필요
- 현재 session과 현재 refresh token family를 폐기
- 성공 `204`
- 이미 폐기된 session은 idempotent하게 `204`

### 6.4 `POST /v1/auth/logout-all`

- Bearer 인증 필요
- 사용자 확인 action으로 취급
- 해당 사용자의 모든 session을 transaction에서 폐기
- 성공 `204`
- 실행한 현재 access token도 즉시 사용할 수 없어야 한다.

### 6.5 `GET /v1/me`

현재 profile을 반환한다. Google provider token이나 `google_sub`는 반환하지 않는다.

```json
{
  "id": "019...",
  "email": "owner@example.com",
  "displayName": "Jimin",
  "timeZone": "Asia/Seoul",
  "version": 1
}
```

### 6.6 `GET /v1/devices`

현재 사용자의 active/revoked 기기 metadata를 cursor pagination으로 반환한다. session/token hash는 반환하지 않는다.

### 6.7 `DELETE /v1/devices/{deviceId}`

- 다른 사용자의 device는 존재 여부를 숨기고 `404`
- 대상 device와 연결된 session family를 모두 폐기
- 현재 device를 삭제해도 허용하되 응답 후 현재 token은 무효
- 성공 `204`
- audit event `auth.device.revoked`

### 6.8 `GET /v1/sync/bootstrap`

새 client가 local cache를 만들 때 사용한다. `REPEATABLE READ` transaction 하나에서 snapshot과 그 snapshot에 대응하는 cursor를 읽는다.

```json
{
  "cursor": "42",
  "profile": {
    "id": "019...",
    "email": "owner@example.com",
    "displayName": "Jimin",
    "timeZone": "Asia/Seoul",
    "version": 1
  },
  "devices": []
}
```

M2 이후 calendar와 event collection이 이 response에 추가된다. additive field이므로 M1 client는 알 수 없는 field를 무시해야 한다.

### 6.9 `GET /v1/sync/changes?after={sequence}&limit={n}`

- `after`: client가 완전히 반영한 마지막 sequence, 초기값 `0`
- `limit`: server 기본값 적용, 최대 500
- sequence 오름차순

```json
{
  "items": [
    {
      "sequence": "43",
      "entityType": "user",
      "entityId": "019...",
      "operation": "upsert",
      "entityVersion": 2,
      "changedAt": "2026-07-10T10:00:00Z"
    }
  ],
  "nextCursor": "43"
}
```

`upsert`를 받은 client는 해당 resource read API 또는 bootstrap으로 authoritative state를 가져온다. `delete`는 tombstone으로 적용한다. cursor가 retention 범위보다 오래됐으면 `410`과 `sync.cursor_expired`를 반환하고 bootstrap을 요구한다.

PostgreSQL sequence는 `bigint`지만 JavaScript 정밀도 손실을 피하기 위해 API에서는 10진수 string으로 직렬화한다. client는 숫자 연산을 하지 않고 받은 값을 cursor로 그대로 저장·전송한다.

## 7. Access token과 guard

### 7.1 Access token

access token은 Ed25519로 서명한 JWT를 사용한다. signing key는 mounted secret이고 public verify key만 일반 config에 둘 수 있다.

필수 claim:

```text
iss  = configured private server issuer
aud  = jimin-os
sub  = internal user UUID
sid  = session UUID
did  = device UUID
jti  = token UUIDv7
iat, nbf, exp
```

header에 `kid`를 포함해 key rotation을 지원한다. 알고리즘 혼동을 막기 위해 verifier는 EdDSA만 허용한다. TTL은 config로 관리하고 token에 Calendar/OpenAI credential을 넣지 않는다.

### 7.2 Route guard

guard는 다음을 모두 확인한다.

1. Authorization header가 정확히 하나인지 확인
2. signature, issuer, audience, time claim 검증
3. `sub/sid/did` UUID parse
4. DB의 session과 device가 `active`인지 확인
5. session의 user/device가 claim과 일치하는지 확인
6. route handler에 `AuthenticatedPrincipal`만 전달

private network는 guard를 생략할 이유가 아니다. user scope가 있는 repository query는 항상 `user_id` 조건을 포함한다.

## 8. 데이터 모델

공통 mutable field인 `id`, `created_at`, `updated_at`, `version`은 생략하지 않는다.

### 8.1 `users`

```text
id uuid primary key
google_sub text unique not null
email text not null
normalized_email text not null
display_name text null
time_zone text not null default 'Asia/Seoul'
status text not null check (active | disabled)
last_login_at timestamptz not null
created_at, updated_at, version
```

email 변경은 허용하지만 같은 `sub`일 때만 기존 user를 갱신한다. allowlist는 별도 secret/config이고 DB email만으로 authorization하지 않는다.

### 8.2 `devices`

```text
id uuid primary key
user_id uuid not null references users
installation_id uuid not null
platform text not null check (macos | ios | android)
name text not null
app_version text not null
os_version text null
status text not null check (active | revoked)
last_seen_at timestamptz not null
revoked_at timestamptz null
created_at, updated_at, version
unique (user_id, installation_id)
```

`installation_id`는 device identity proof가 아니다. session 발급 대상의 stable key로만 사용한다.

### 8.3 `sessions`

```text
id uuid primary key
user_id uuid not null references users
device_id uuid not null references devices
family_id uuid not null
status text not null check (active | revoked | compromised | expired)
expires_at timestamptz not null
last_used_at timestamptz not null
revoked_at timestamptz null
revocation_reason text null
created_at, updated_at, version
```

### 8.4 `session_refresh_tokens`

```text
id uuid primary key
session_id uuid not null references sessions
token_verifier bytea unique not null
status text not null check (active | rotated | revoked | reused)
expires_at timestamptz not null
rotated_to_id uuid null references session_refresh_tokens
used_at timestamptz null
created_at, updated_at, version
```

refresh token은 `josr_<session-id>.<256-bit-random-secret>` 형식이다. DB에는 secret 원문 대신 server pepper를 사용한 HMAC-SHA-256 verifier만 저장한다. prefix의 session ID만 lookup에 사용한다.

### 8.5 `sync_changes`

`SHARED_CONTRACTS.md` schema에 다음 index를 둔다.

```text
primary key (sequence)
index (user_id, sequence)
index (user_id, entity_type, entity_id, sequence desc)
```

resource write와 change insert는 같은 DB transaction 안에서 실행한다.

### 8.6 `idempotency_records`

공통 계약의 field에 `state (pending | completed | failed)`와 `locked_until`을 포함한다. 동일 user/key/operation은 unique다. M1에서는 framework와 repository까지만 구현하고 Calendar mutation에서 실제 사용한다.

### 8.7 `audit_logs`

```text
id uuid primary key
occurred_at timestamptz not null
actor_user_id uuid null
actor_device_id uuid null
action text not null
target_type text null
target_id uuid null
outcome text not null
request_id uuid null
metadata jsonb not null default '{}'
```

`metadata`에 email, authorization code, token, provider response body를 넣지 않는다. 허용 예시는 `client_kind`, 내부 error class, provider status code다.

## 9. 상태 전이

### 9.1 Login

```text
code_received
  → google_exchanging
  → identity_validating
  → allowlist_checking
  → user_device_upsert
  → session_issued

google_exchanging → rejected
identity_validating → rejected
allowlist_checking → rejected
```

user/device/session 생성과 최초 `sync_changes` insert는 transaction 하나에서 처리한다. Google provider 호출은 transaction 밖에서 끝내 DB lock 중 외부 network를 기다리지 않는다.

### 9.2 Device

```text
active → revoked
revoked → active  # 새 Google 로그인으로 명시적 재등록할 때만
```

revoked device의 기존 session을 되살리지 않는다. 재등록은 새 session family를 만든다.

### 9.3 Session과 refresh token

```text
session: active → revoked
session: active → expired
session: active → compromised

refresh token: active → rotated
refresh token: active → revoked
refresh token: rotated → reused
```

rotation transaction:

1. session row와 token row를 `FOR UPDATE`한다.
2. token verifier와 status를 constant-time으로 확인한다.
3. active token을 `rotated`로 바꾼다.
4. 새 token row를 insert하고 `rotated_to_id`를 연결한다.
5. 새 access/refresh token metadata를 확정한다.
6. commit 후 원문 refresh token을 한 번만 응답한다.

rotated/revoked token이 다시 제출되면 같은 `family_id`의 session을 `compromised`로 바꾸고 모두 폐기한다. 사용자에게는 다시 로그인하라는 오류만 보이고 내부 탐지 상세는 audit에 남긴다.

## 10. Migration과 transaction 규칙

- migration은 forward-only SQLx migration을 사용한다.
- 모든 M1 DDL은 빈 DB와 M0 baseline DB 양쪽에 적용 가능해야 한다.
- migration 실행 전 checksum과 현재 schema version을 확인한다.
- transaction 가능한 DDL은 한 migration transaction에서 실행한다.
- production rollback은 down migration보다 이전 image + 검증된 backup restore를 기준으로 한다.
- staging dry-run은 임시 DB에 migration을 적용하고 schema snapshot을 비교한다.
- migration 실패 또는 application이 기대하는 version과 불일치하면 ready `503`이다.
- migration process는 advisory lock으로 한 instance만 실행한다.

`email`, token verifier, session foreign key index를 빠뜨리지 않는다. SQLx query가 user scope 없이 resource를 조회하는지 code review에서 확인한다.

## 11. 구현 순서

1. domain ID/time/error type과 config loader를 만든다.
2. SQLx pool, migration runner, transaction boundary를 구현한다.
3. M1 table migration과 repository integration test를 작성한다.
4. request ID, tracing, redaction, body limit middleware를 구현한다.
5. typed `ApiError`와 OpenAPI error schema를 구현한다.
6. Google OAuth platform profile config와 code exchange adapter를 구현한다.
7. Google ID token/JWKS 검증과 allowlist rule을 구현한다.
8. user/device upsert use case를 구현한다.
9. access token issuer/verifier와 authenticated principal middleware를 구현한다.
10. refresh token family, rotation, reuse detection을 구현한다.
11. logout, logout-all, device revoke를 구현한다.
12. `sync_changes` writer, bootstrap, changes API를 구현한다.
13. idempotency와 audit repository 기반을 구현한다.
14. route에서 OpenAPI를 생성하고 checked-in schema/client generation을 연결한다.
15. Mac과 휴대폰에서 실제 Google 로그인 및 session rotation을 검증한다.
16. container restart, key/config 누락, DB outage, Google outage를 검증한다.

## 12. 오류와 사용자 문구

| Code | HTTP | Retryable | 사용자 message | 내부 처리 |
|---|---:|---:|---|---|
| `auth.google_login_failed` | 401 | false | Google 로그인을 다시 진행해 주세요. | provider code/claim 상세 마스킹 |
| `auth.account_not_allowed` | 403 | false | 이 계정으로는 로그인할 수 없어요. | allowlist 실패 audit |
| `auth.session_expired` | 401 | false | 다시 로그인해 주세요. | session/token 상태 은닉 |
| `auth.refresh_reused` | 401 | false | 보안을 위해 다시 로그인해 주세요. | family compromised 처리 |
| `auth.device_revoked` | 401 | false | 이 기기에서 다시 로그인해 주세요. | device/session 확인 |
| `request.invalid` | 400 | false | 입력한 내용을 다시 확인해 주세요. | field error details 제한 |
| `sync.cursor_expired` | 410 | true | 최신 데이터를 다시 불러올게요. | bootstrap 요구 |
| `service.temporarily_unavailable` | 503 | true | 잠시 후 다시 시도해 주세요. | DB/Google class만 log |

같은 Google login 실패라도 account allowlist 여부를 공격자가 구분해 탐색하지 못하도록 rate limit과 일반화된 provider error를 사용한다. 개인 계정 owner가 필요한 상세는 audit에서 내부 code로 확인한다.

## 13. 보안 요구사항

- Google client secret, signing private key, refresh pepper는 mounted secret으로 읽는다.
- secret config 누락 시 process는 traffic을 받지 않고 ready 실패한다.
- authorization code, code verifier, Google token, Jimin OS token은 log field로 전달하지 않는다.
- `Debug` derive로 request DTO 전체를 log하지 않는다.
- Google token endpoint는 고정 URL과 TLS를 사용하고 redirect를 임의로 따라가지 않는다.
- JWKS response에 크기, content type, timeout 제한을 둔다.
- access token verifier는 허용 algorithm을 고정한다.
- refresh token verifier는 constant-time 비교한다.
- auth route에 IP 기반과 installation ID 기반 rate limit을 적용한다.
- 일반 JSON body size를 제한하고 auth body에는 더 작은 제한을 적용한다.
- CORS는 staging/prod app origin allowlist만 허용하고 wildcard credential을 금지한다.
- database error에 query, bind value, connection string을 노출하지 않는다.
- audit log는 append-only application permission을 적용한다.

## 14. 자동 테스트

### 14.1 Domain unit test

- email normalization과 allowlist 비교
- OAuth platform profile/redirect exact match
- code verifier validation
- access token claim validation과 clock boundary
- device state transition
- session rotation과 family compromise
- sync cursor serialization
- API error mapping과 사용자 문구

### 14.2 Google adapter test

- 정상 authorization code/token response fixture
- invalid signature, issuer, audience, `azp`, expiry
- `email_verified=false`
- unknown `kid`에서 JWKS refresh 한 번
- JWKS timeout/oversized response
- token endpoint 400/429/500 분류
- provider response body와 token이 log에 없는지 확인

실제 token과 개인 email을 fixture에 넣지 않는다.

### 14.3 PostgreSQL integration test

- empty DB migration과 schema snapshot
- user `sub` uniqueness와 device installation uniqueness
- 동시에 같은 device로 로그인할 때 한 device row만 생성
- refresh token rotation 경쟁에서 한 요청만 성공
- rotated token 재사용 시 family 폐기
- logout/device revoke 후 access token guard 거부
- resource write와 `sync_changes`의 atomicity
- bootstrap snapshot/cursor race 방지
- cursor pagination에서 누락·중복 없음
- migration advisory lock

### 14.4 HTTP/OpenAPI contract test

- 모든 protected route의 무인증 `401`
- 다른 user/device ID 접근 은닉
- body size, content type, unknown field, invalid UUID
- route와 generated OpenAPI path/method 일치
- error envelope와 request ID 존재
- rate limit `429`
- refresh/logout idempotency
- 오래된 sync cursor `410`

### 14.5 Build/gate

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
empty-database migration test
schema snapshot test
OpenAPI generation diff check
Docker image build
Compose smoke test
secret scan
```

## 15. 수동 검증

### 15.1 Mac

1. system browser로 허용 Google 계정에 로그인한다.
2. `/v1/me`와 device 목록을 확인한다.
3. access token 만료 상황에서 refresh rotation을 확인한다.
4. logout 후 기존 access/refresh token이 모두 거부되는지 확인한다.
5. 허용되지 않은 Google 계정의 로그인이 거부되는지 확인한다.

### 15.2 개인 휴대폰

1. 실제 앱에서 Google 로그인과 app 복귀를 완료한다.
2. token이 Keychain/Keystore에만 저장되는지 확인한다.
3. Mac과 같은 internal user ID와 profile을 받는지 확인한다.
4. 두 기기의 bootstrap cursor와 change 순서가 일치하는지 확인한다.
5. Mac에서 휴대폰 device를 폐기하고 휴대폰 요청이 즉시 거부되는지 확인한다.

### 15.3 서버 장애

1. API restart 후 session과 refresh rotation이 유지되는지 확인한다.
2. DB down에서 live/ready가 분리되는지 확인한다.
3. Google token endpoint 장애에서 기존 인증 session의 protected read가 계속 가능한지 확인한다.
4. signing key 또는 allowlist config 누락 시 ready가 실패하는지 확인한다.
5. audit/log에서 token, code, email 원문 노출 여부를 점검한다.

## 16. 산출물

- M1 DB migration과 schema snapshot
- auth/session/device/sync Rust module
- Google identity adapter와 sanitized fixtures
- `/v1/auth/*`, `/v1/me`, `/v1/devices`, `/v1/sync/*` route
- generated OpenAPI와 API client type
- secret/config reference 문서
- Google OAuth staging 설정 runbook
- session revoke/credential rotation runbook
- Mac/휴대폰 로그인 검증 기록
- 자동 테스트와 Backend Ultrawork 결과

## 17. 완료 게이트

- [ ] 허용 Google 계정만 Mac과 휴대폰에서 로그인할 수 있다.
- [ ] 두 기기가 email이 아니라 같은 Google `sub` 기반 internal user를 사용한다.
- [ ] authorization code와 provider/Jimin OS token이 DB 일반 column, log, error에 남지 않는다.
- [ ] access token의 signature/issuer/audience/time/session/device guard가 통과한다.
- [ ] refresh token rotation과 reuse detection 경쟁 테스트가 통과한다.
- [ ] logout, logout-all, 원격 device revoke가 즉시 적용된다.
- [ ] bootstrap과 changes cursor에 누락·중복이 없다.
- [ ] 모든 mutation 기반이 idempotency contract를 사용할 수 있다.
- [ ] migration을 빈 DB와 staging 복사본에 적용할 수 있고 실패 시 ready가 열리지 않는다.
- [ ] 실제 route와 generated OpenAPI가 일치한다.
- [ ] formatter, lint, unit/integration/contract test, image build가 통과한다.
- [ ] Mac과 개인 휴대폰의 실기기 인증 증거가 남아 있다.
- [ ] Google 장애가 기존 서버 data read와 API liveness를 중단시키지 않는다.
- [ ] Backend Ultrawork 관련 checklist에 실패가 없다.

이 게이트를 통과하기 전에는 Calendar refresh token을 저장하거나 일정 data migration을 추가하지 않는다.
