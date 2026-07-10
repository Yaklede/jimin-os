# M2. Google Calendar 명세

## 1. 목적

M2는 Google Calendar를 Jimin OS의 일정 원본 provider로 연결하고, 서버 PostgreSQL에 항상 조회 가능한 read model을 만든다. Google 또는 network가 잠시 실패해도 마지막으로 동기화한 일정은 조회할 수 있어야 하며, Jimin OS에서 만든 변경은 유실이나 중복 없이 Google에 반영돼야 한다.

M2가 끝나면 다음 흐름이 서버에서 완결돼야 한다.

```text
Google Calendar 변경
→ full/incremental sync
→ PostgreSQL read model
→ Jimin OS 일정 API

Jimin OS 일정 mutation
→ local transaction/outbox
→ Google Calendar conditional write
→ provider version 반영
→ sync change 발행
```

## 2. 선행조건

- M1 완료 게이트가 모두 통과했다.
- staging Google Cloud project에서 Calendar API가 활성화됐다.
- OAuth consent screen, 개인정보 처리 설명, app 이름, test user가 구성됐다.
- private staging hostname의 고정 Google OAuth callback URL이 등록됐다.
- M1 internal user의 Google `sub`와 Calendar 연결 account의 `sub`를 비교할 수 있다.
- Google token encryption key와 key version이 mounted secret으로 준비됐다.
- staging 전용 Google Calendar 또는 실제 개인 계정 내 명확한 test calendar가 준비됐다.
- provider 호출 fixture와 실제 staging 호출을 구분하는 config가 있다.

## 3. 범위

### 3.1 포함

- 로그인과 분리된 Google Calendar 권한 연결/재연결/해제
- server-owned OAuth state, PKCE, callback, offline refresh token
- 최소 Calendar scope 요청
- Calendar list full/incremental sync
- primary 및 선택된 calendar event full/incremental sync
- per-calendar `syncToken` 저장과 `410 Gone` full resync
- PostgreSQL 일정 read model
- 단일 일정 조회, 생성, 수정, 삭제
- all-day와 timed event
- recurring event/exception의 읽기와 기간 조회용 occurrence expansion
- Jimin OS에서 recurring event 생성·수정·삭제는 read-only 처리
- deterministic provider event ID와 idempotency record
- outbox mutation worker와 retry/error classification
- Google ETag 기반 conditional update/delete
- 충돌 보존과 명시적 해결 API
- manual sync와 sync 상태 조회
- Google 장애, token 만료, rate limit, partial page 실패 복구
- calendar/event 변경의 `sync_changes` 발행

### 3.2 제외

- Google Calendar push notification/webhook
- 참석자 초대와 응답 변경
- Google Meet 생성과 conference 수정
- reminder override 편집
- recurring series/instance 생성·수정·삭제
- focus time, out-of-office, working location 생성
- calendar 자체 생성, 공유, ACL 수정
- Gmail에서 생성된 일정 편집
- 여러 Google 계정 동시 연결
- Google Tasks 연동
- client SQLite와 offline mutation queue
- 일정 추천, AI 자동 일정 생성

외부에서 만든 unsupported event type과 recurring event는 읽어서 표시하되 `isEditable=false`로 반환한다.

## 4. Google 권한과 OAuth 연결

### 4.1 요청 scope

M2는 다음 scope만 추가 요청한다.

```text
openid
email
https://www.googleapis.com/auth/calendar.events
https://www.googleapis.com/auth/calendar.calendarlist.readonly
```

`calendar.events`는 일정 읽기/쓰기에, `calendar.calendarlist.readonly`는 calendar 목록과 접근 role 확인에 사용한다. 전체 `calendar` scope는 요청하지 않는다.

참고:

- [Choose Google Calendar API scopes](https://developers.google.com/workspace/calendar/api/auth)
- [CalendarList.list](https://developers.google.com/workspace/calendar/api/v3/reference/calendarList/list)

### 4.2 연결 시작

`POST /v1/calendar/connections/google/authorizations`

- Jimin OS Bearer 인증 필요
- 현재 session/device에 연결 transaction을 묶는다.
- client가 임의 callback URL이나 Google client ID를 전달하지 않는다.

요청:

```json
{
  "clientKind": "ios"
}
```

서버 동작:

1. UUIDv7 transaction ID, high-entropy state, PKCE verifier/challenge를 만든다.
2. state는 HMAC verifier만 저장하고 PKCE verifier는 application-level encryption한다.
3. user/session/device와 allowed app return profile을 transaction에 저장한다.
4. `access_type=offline`, `include_granted_scopes=true`를 사용한다.
5. 기존 refresh token이 없거나 `reauthRequired` 상태일 때만 명시적 consent를 요구한다.
6. 고정 server callback URL을 사용해 Google authorization URL을 만든다.

성공 `201`:

```json
{
  "authorizationId": "019...",
  "authorizationUrl": "https://accounts.google.com/o/oauth2/v2/auth?..."
}
```

authorization URL, state, code를 일반 log에 남기지 않는다.

### 4.3 Google callback

`GET /oauth/google/calendar/callback?state=...&code=...`

이 route는 browser callback이라 Jimin OS Bearer header를 요구하지 않는다. 대신 single-use state와 저장된 session binding을 검증한다.

1. state verifier로 transaction을 찾고 row lock을 건다.
2. 만료, 이미 사용됨, 취소됨을 거부한다.
3. 저장된 PKCE verifier와 server OAuth client로 code를 교환한다.
4. 반환된 ID token의 서명/issuer/audience/time claim을 M1 규칙으로 검증한다.
5. Google `sub`가 현재 Jimin OS user의 `google_sub`와 같은지 확인한다.
6. grant에 필수 Calendar scope가 모두 포함됐는지 확인한다.
7. refresh token을 암호화해 저장한다. reconnect 응답에 새 refresh token이 없으면 기존 유효 token을 보존한다.
8. transaction을 `completed`, account를 `active`로 바꾸고 initial sync job을 enqueue한다.
9. platform profile에 등록된 app link로 복귀하거나, 앱으로 돌아가라는 민감정보 없는 완료 페이지를 표시한다.

다른 Google 계정이면 새 token을 best-effort revoke하고 연결을 만들지 않는다.

### 4.4 연결 상태와 해제

`GET /v1/calendar/connections/google`

```json
{
  "status": "active",
  "email": "owner@example.com",
  "grantedScopes": ["calendar.events", "calendar.calendarlist.readonly"],
  "lastSuccessfulSyncAt": "2026-07-10T10:00:00Z",
  "reauthRequired": false,
  "version": 3
}
```

실제 refresh/access token과 provider `sub`는 반환하지 않는다.

`DELETE /v1/calendar/connections/google?expectedVersion={version}`

1. account를 `revoking`으로 compare-and-set한다.
2. scheduler와 mutation claim을 중단한다.
3. Google revoke endpoint 호출을 best-effort로 수행한다.
4. encrypted token, sync token, event staging data를 삭제한다.
5. calendar/event read model을 purge하고 client용 delete `sync_changes`를 발행한다.
6. account를 `revoked`로 바꾼다.

Provider revoke가 실패해도 local credential과 cached data purge는 끝낸다. 성공은 `204`다.

## 5. API 계약

### 5.1 Calendar 목록

`GET /v1/calendar/calendars`

```json
{
  "items": [
    {
      "id": "019...",
      "name": "내 캘린더",
      "timeZone": "Asia/Seoul",
      "color": "provider-color-id",
      "accessRole": "owner",
      "isPrimary": true,
      "syncEnabled": true,
      "syncStatus": "idle",
      "version": 2
    }
  ],
  "nextCursor": null
}
```

Google calendar ID는 email일 수 있으므로 API에 노출하지 않고 internal UUID를 사용한다.

`PATCH /v1/calendar/calendars/{calendarId}`

```json
{
  "syncEnabled": true,
  "expectedVersion": 2
}
```

- primary calendar는 `syncEnabled=false`로 바꿀 수 없다.
- `reader` 이상만 sync할 수 있다.
- false→true는 full sync를 enqueue한다.
- true→false는 해당 calendar의 event read model을 purge하고 tombstone을 발행한다.
- 성공 `200`, version conflict `409`

### 5.2 기간별 일정 조회

`GET /v1/calendar/events?from={RFC3339}&to={RFC3339}&timeZone={IANA}&calendarId={uuid}&cursor={opaque}`

- `from` inclusive, `to` exclusive
- `from < to`
- 요청 가능한 window 상한을 config로 두고 초과하면 `422`
- `timeZone`은 표시와 recurrence expansion 기준이며 IANA zone만 허용
- `calendarId`는 반복할 수 있고 생략하면 sync-enabled 전체
- pending delete와 provider tombstone은 기본 목록에서 제외

응답 item:

```json
{
  "id": "019...",
  "occurrenceId": "opaque-occurrence-id",
  "calendarId": "019...",
  "title": "일정 제목",
  "description": "표시 가능한 설명",
  "location": "장소",
  "start": {
    "kind": "dateTime",
    "dateTime": "2026-07-10T09:00:00+09:00",
    "timeZone": "Asia/Seoul"
  },
  "end": {
    "kind": "dateTime",
    "dateTime": "2026-07-10T10:00:00+09:00",
    "timeZone": "Asia/Seoul"
  },
  "status": "confirmed",
  "eventType": "default",
  "isRecurring": false,
  "isEditable": true,
  "syncState": "synced",
  "version": 4
}
```

all-day 일정은 `kind=date`와 `date=YYYY-MM-DD`를 사용한다. Google의 all-day `end.date`는 exclusive라는 의미를 그대로 유지한다. 자정 UTC로 변환하지 않는다.

Google description은 untrusted HTML일 수 있다. 서버는 허용되지 않은 markup/script를 제거한 표시용 plain text를 반환하고, client는 이를 HTML로 직접 렌더링하지 않는다.

### 5.3 단일 일정 조회

`GET /v1/calendar/events/{eventId}`

- internal canonical event를 반환한다.
- recurring occurrence는 기간 조회의 `occurrenceId`로만 식별하며 M2 mutation 대상이 아니다.
- 다른 user event와 purge된 event는 `404`

### 5.4 일정 생성

`POST /v1/calendar/events`

Header:

```text
Idempotency-Key: <UUIDv7>
```

timed event 요청:

```json
{
  "calendarId": "019...",
  "title": "회의",
  "description": "준비할 내용",
  "location": "온라인",
  "start": {
    "kind": "dateTime",
    "dateTime": "2026-07-10T09:00:00+09:00",
    "timeZone": "Asia/Seoul"
  },
  "end": {
    "kind": "dateTime",
    "dateTime": "2026-07-10T10:00:00+09:00",
    "timeZone": "Asia/Seoul"
  }
}
```

all-day event 요청:

```json
{
  "calendarId": "019...",
  "title": "휴가",
  "start": { "kind": "date", "date": "2026-07-10" },
  "end": { "kind": "date", "date": "2026-07-11" }
}
```

Validation:

- calendar가 `writer` 또는 `owner`
- title trim 후 1~300자
- description 최대 8192자, location 최대 1024자
- start/end kind 동일
- timed event는 RFC 3339 offset과 유효한 IANA timezone 필요
- all-day는 date만 허용하고 end는 start보다 뒤의 exclusive date
- recurrence, attendees, conference field는 unknown/unsupported로 거부

서버는 local event와 mutation/outbox/idempotency/change를 transaction에서 만든다. Google 응답을 기다리지 않고 local resource 생성 성공을 `201`로 반환한다.

```json
{
  "id": "019...",
  "calendarId": "019...",
  "title": "회의",
  "syncState": "pendingCreate",
  "version": 1,
  "mutationId": "019..."
}
```

provider event ID는 `jos` + internal UUIDv7의 lowercase hex로 미리 만든다. Google이 허용하는 base32hex 문자 범위에 들어가며 같은 create retry가 새 event를 만들지 않게 한다. insert payload의 private extended property에 internal event/mutation ID를 넣는다.

### 5.5 일정 수정

`PATCH /v1/calendar/events/{eventId}`

```json
{
  "title": "변경된 회의",
  "start": {
    "kind": "dateTime",
    "dateTime": "2026-07-10T10:00:00+09:00",
    "timeZone": "Asia/Seoul"
  },
  "end": {
    "kind": "dateTime",
    "dateTime": "2026-07-10T11:00:00+09:00",
    "timeZone": "Asia/Seoul"
  },
  "expectedVersion": 4
}
```

- `Idempotency-Key` 필수
- JSON Merge Patch가 아니라 명시적 typed patch DTO를 사용
- `expectedVersion` compare-and-set
- recurring/unsupported/read-only event는 `422` 또는 `403`
- local desired state를 저장하고 `syncState=pendingUpdate`로 반환
- Google update에는 저장된 provider ETag를 `If-Match`로 보냄
- 성공 `200`

### 5.6 일정 삭제

`DELETE /v1/calendar/events/{eventId}?expectedVersion={version}`

- `Idempotency-Key` 필수
- local row를 즉시 `pendingDelete`로 바꾸고 일반 기간 조회에서 숨김
- provider delete에는 `If-Match` 사용
- provider 완료 전이므로 성공 `202`

```json
{
  "mutationId": "019...",
  "status": "queued"
}
```

Google `404`는 이미 삭제된 것으로 보고 성공 처리한다. `412`는 conflict로 전환하고 최신 event를 다시 가져온다.

### 5.7 Mutation 조회와 충돌 해결

`GET /v1/calendar/mutations/{mutationId}`

```json
{
  "id": "019...",
  "eventId": "019...",
  "operation": "update",
  "status": "conflict",
  "retryable": false,
  "createdAt": "2026-07-10T10:00:00Z"
}
```

desired event body와 provider 원문은 일반 목록에 노출하지 않는다. 해당 event read API는 최신 Google state와 conflict 존재 여부를 반환한다.

`POST /v1/calendar/mutations/{mutationId}/resolutions`

Header `Idempotency-Key` 필수.

```json
{
  "resolution": "keepGoogle",
  "expectedEventVersion": 6
}
```

또는:

```json
{
  "resolution": "retryMine",
  "expectedEventVersion": 6
}
```

- `keepGoogle`: 보존한 desired patch를 폐기하고 conflict를 resolved 처리
- `retryMine`: 최신 provider ETag를 기준으로 새 mutation을 생성
- terminal mutation을 다시 resolve하면 저장된 idempotent 결과 반환
- 자동 last-write-wins는 사용하지 않는다.

### 5.8 수동 동기화와 상태

`POST /v1/calendar/synchronizations`

- Bearer와 `Idempotency-Key` 필수
- 이미 queued/running sync가 있으면 동일 run ID 반환
- 성공 `202`

`GET /v1/calendar/synchronizations/status`

```json
{
  "connectionStatus": "active",
  "lastSuccessfulSyncAt": "2026-07-10T10:00:00Z",
  "calendars": [
    {
      "calendarId": "019...",
      "status": "idle",
      "lastSuccessfulSyncAt": "2026-07-10T10:00:00Z",
      "lastError": null
    }
  ]
}
```

Provider error body, sync token, retry timestamp의 내부 정밀값은 노출하지 않는다.

## 6. 데이터 모델

공통 mutable field `id`, `created_at`, `updated_at`, `version`을 포함한다.

### 6.1 `calendar_accounts`

```text
id uuid primary key
user_id uuid unique not null references users
provider text not null check (google)
provider_subject text not null
email text not null
status text not null check (connecting | active | reauth_required | revoking | revoked | error)
granted_scopes text[] not null
refresh_token_ciphertext bytea null
refresh_token_nonce bytea null
encryption_key_version integer null
calendar_list_sync_token_ciphertext bytea null
calendar_list_sync_token_nonce bytea null
last_successful_sync_at timestamptz null
last_error_code text null
created_at, updated_at, version
```

`provider_subject`는 M1 `users.google_sub`와 같아야 한다. refresh/sync token은 같은 key를 사용하더라도 서로 다른 nonce와 AAD를 사용한다.

### 6.2 `calendar_oauth_authorizations`

```text
id uuid primary key
user_id uuid not null
session_id uuid not null
device_id uuid not null
state_verifier bytea unique not null
pkce_verifier_ciphertext bytea not null
pkce_nonce bytea not null
encryption_key_version integer not null
client_kind text not null
status text not null check (pending | exchanging | completed | failed | expired | cancelled)
expires_at timestamptz not null
failure_code text null
created_at, updated_at, version
```

완료/만료 transaction은 secret field를 즉시 null 또는 암호학적 삭제가 가능한 값으로 제거한다.

### 6.3 `calendars`

```text
id uuid primary key
account_id uuid not null references calendar_accounts
provider_calendar_id text not null
name text not null
description text null
time_zone text not null
color_id text null
access_role text not null
is_primary boolean not null
provider_selected boolean not null
sync_enabled boolean not null
provider_etag text null
provider_deleted_at timestamptz null
created_at, updated_at, version
unique (account_id, provider_calendar_id)
```

기본 `sync_enabled` 정책은 `is_primary OR provider_selected`다. 사용자의 이후 선택은 provider selected 변경보다 우선한다. primary는 항상 true다.

### 6.4 `calendar_sync_states`

```text
id uuid primary key
calendar_id uuid unique not null references calendars
status text not null
sync_token_ciphertext bytea null
sync_token_nonce bytea null
query_fingerprint text not null
last_started_at timestamptz null
last_successful_sync_at timestamptz null
consecutive_failures integer not null default 0
next_attempt_at timestamptz null
lease_owner text null
lease_expires_at timestamptz null
last_error_code text null
created_at, updated_at, version
```

`query_fingerprint`는 initial과 incremental sync의 provider query option이 달라지는 실수를 막는다.

### 6.5 `calendar_events`

```text
id uuid primary key
user_id uuid not null references users
calendar_id uuid not null references calendars
provider_event_id text not null
provider_etag text null
provider_updated_at timestamptz null
ical_uid text null
provider_status text not null
event_type text not null
title text not null
description_text text null
location text null
time_kind text not null check (date | date_time)
start_at timestamptz null
end_at timestamptz null
start_date date null
end_date date null
source_time_zone text null
recurrence jsonb null
recurring_provider_event_id text null
original_start jsonb null
visibility text null
transparency text null
html_link text null
is_editable boolean not null
sync_state text not null
provider_deleted_at timestamptz null
created_at, updated_at, version
unique (calendar_id, provider_event_id)
```

DB check constraint로 date event는 `start_date/end_date`만, timed event는 `start_at/end_at/source_time_zone`만 갖게 한다. recurrence JSON은 provider string array schema를 검증한 뒤 저장한다.

### 6.6 `calendar_sync_runs`와 staging

```text
calendar_sync_runs
- id
- account_id
- calendar_id nullable          # null이면 calendar list run
- kind: full | incremental
- status: queued | claimed | fetching | applying | completed | retry_wait | failed | cancelled
- base_sync_token_fingerprint
- next_sync_token_ciphertext
- next_sync_token_nonce
- encryption_key_version
- page_count
- item_count
- lease_owner, lease_expires_at
- last_error_code
- created_at, updated_at, version

calendar_sync_staging_events
- run_id
- provider_event_id
- normalized_payload jsonb
- provider_status
- primary key (run_id, provider_event_id)
```

staging payload도 개인 일정이므로 일반 log나 debug dump에 넣지 않는다. completed/failed run의 staging row는 정리한다.

### 6.7 `calendar_mutations`

```text
id uuid primary key
user_id uuid not null
event_id uuid not null
operation text not null check (create | update | delete)
status text not null
idempotency_record_id uuid not null
desired_payload jsonb not null
expected_event_version bigint not null
expected_provider_etag text null
provider_event_id text not null
attempt_count integer not null default 0
next_attempt_at timestamptz null
lease_owner text null
lease_expires_at timestamptz null
last_error_code text null
resolved_at timestamptz null
created_at, updated_at, version
```

`desired_payload`는 일정 본문이므로 암호화 DB 전체/backup 보호 범위에 포함하고 log에 출력하지 않는다.

## 7. 동기화 알고리즘

### 7.1 Calendar list

1. account가 active인지 확인하고 job lease를 얻는다.
2. sync token이 없으면 `showDeleted=true`, `showHidden=true`로 full `calendarList.list`를 호출한다.
3. sync token이 있으면 같은 option과 token으로 incremental 호출한다.
4. 모든 page를 가져온 후 internal calendar metadata를 transaction에서 upsert/tombstone한다.
5. 마지막 page의 `nextSyncToken`만 암호화해 저장한다.
6. 새로 sync-enabled가 된 calendar에 event full sync를 enqueue한다.
7. 삭제/비활성 calendar의 event를 purge하고 client tombstone을 발행한다.

incremental list에서도 deleted/hidden entry를 받아야 하므로 initial query option을 그에 맞춰 고정한다.

### 7.2 Event full sync

event sync는 canonical resource를 유지하기 위해 `singleEvents=false`, `showDeleted=true`를 고정한다. `timeMin`, `timeMax`, `orderBy`, `q`, `updatedMin`을 사용하지 않는다. 기간 제한 없이 recurring master와 exception을 가져오고, API query 시 서버가 bounded range에서 occurrence를 확장한다.

1. `full` run row와 staging area를 만든다.
2. page를 순회하며 provider DTO를 validate/normalize해 staging에 upsert한다.
3. 중간 page 실패 시 active read model과 기존 sync token을 변경하지 않는다.
4. 마지막 page에서 `nextSyncToken`을 받는다.
5. DB transaction에서 staging을 active read model에 merge한다.
6. staging에 없는 기존 provider event를 tombstone한다.
7. 새 sync token, 성공 시각, sync change를 같은 transaction에서 반영한다.
8. transaction 후 staging을 정리한다.

Google 공식 동기화 계약상 `nextSyncToken`은 마지막 page에만 나오므로 중간 token을 저장하지 않는다.

참고: [Synchronize Calendar resources efficiently](https://developers.google.com/workspace/calendar/api/guides/sync)

### 7.3 Event incremental sync

1. 저장한 token과 정확히 같은 query fingerprint로 `events.list(syncToken=...)`를 호출한다.
2. `nextPageToken`이 있으면 원래 sync token과 같은 option으로 계속 page를 가져온다.
3. 변경과 삭제를 staging에 저장한다.
4. 마지막 `nextSyncToken`을 받은 뒤 transaction에서 upsert/tombstone과 token 교체를 수행한다.
5. deleted normal event는 tombstone한다.
6. cancelled recurring exception은 parent series lifetime 동안 필요한 식별자를 유지한다.

Google은 incremental 결과에 삭제 entry를 포함한다. 삭제 entry에 `id` 외 field가 없을 수 있으므로 mapper는 제목/시간이 없다고 실패하면 안 된다.

### 7.4 `410 Gone`

Google이 sync token을 무효화해 `410`을 반환하면:

```text
incremental_syncing
→ reset_required
→ full_syncing
→ idle
```

- invalid token을 폐기한다.
- 기존 active read model은 full sync 성공 전까지 유지한다.
- 새 full run을 staging에서 완료한 뒤 원자적으로 교체한다.
- 사용자에게는 “일정을 다시 동기화하고 있어요.” 상태를 표시할 수 있는 code를 반환한다.
- 410을 일반 transient retry로 반복하지 않는다.

### 7.5 Scheduler와 lease

- PostgreSQL `FOR UPDATE SKIP LOCKED`로 queued sync/mutation을 claim한다.
- 한 calendar에는 active sync run이 하나만 존재하도록 partial unique index를 둔다.
- lease 만료 후 다른 worker가 claim할 수 있다.
- provider 호출 중 DB transaction을 열어두지 않는다.
- polling interval, retry ceiling, lease 길이는 config다.
- manual sync와 scheduler sync가 만나면 기존 active run ID를 재사용한다.
- mutation worker와 provider pull sync가 같은 event를 갱신할 때 provider ETag와 local version으로 순서를 결정한다.

## 8. 일정 mutation 상태 전이

### 8.1 공통

```text
queued → claimed → calling_provider → succeeded
                         ├→ retry_wait → queued
                         ├→ conflict
                         ├→ reauth_required
                         └→ failed_permanent

conflict → resolved_keep_google
conflict → queued_as_new_mutation
```

terminal mutation은 다시 실행하지 않는다. 같은 idempotency key와 request hash는 저장된 HTTP 결과를 반환한다.

### 8.2 Create

1. API transaction에서 deterministic provider ID를 예약한다.
2. Google `events.insert`에 그 ID와 private internal IDs를 보낸다.
3. 성공하면 provider ETag/version을 local event에 반영한다.
4. timeout 후 재시도는 같은 provider ID로 `events.get`을 먼저 확인한다.
5. `409`면 같은 ID의 event를 가져와 private internal ID가 일치할 때 성공으로 reconcile한다.
6. 다른 event면 permanent collision으로 처리하고 새 ID를 자동 발급하지 않는다.

Google은 insert의 conditional modification을 지원하지 않지만, client-specified ID가 이미 존재하면 insert가 다시 성공하지 않는 특성을 idempotency에 사용한다.

### 8.3 Update/Delete

Google request에 저장된 provider ETag를 `If-Match`로 보낸다.

- match: mutation 성공, 새 ETag 또는 tombstone 반영
- `412 Precondition Failed`: 최신 provider event를 fetch하고 local read model을 갱신한 뒤 desired change를 conflict에 보존
- update `404`: provider 삭제로 reconcile하고 conflict 또는 삭제 결과로 전환
- delete `404`: 성공으로 처리

ETag가 달라졌을 때 자동으로 사용자의 변경을 덮어쓰지 않는다.

참고: [Google Calendar conditional modifications](https://developers.google.com/workspace/calendar/api/guides/version-resources)

## 9. Recurrence와 시간 처리

### 9.1 저장

- recurring master의 RRULE/RDATE/EXDATE를 원문 의미를 보존해 저장한다.
- exception은 `recurringEventId`와 `originalStartTime`을 보존한다.
- cancelled exception도 parent series가 있는 동안 유지한다.
- IANA timezone을 보존하고 UTC instant만으로 recurrence를 재생성하지 않는다.

### 9.2 조회 expansion

- 기간 조회에서만 recurrence를 요청 범위로 제한해 expansion한다.
- exception과 cancellation을 master occurrence에 overlay한다.
- `occurrenceId`는 event internal ID, original local start, timezone을 조합한 opaque stable ID다.
- DST gap/fold, 월말, 윤년, all-day exclusive end를 fixture로 검증한다.
- expansion 개수에 상한을 두고 초과 시 pagination 또는 명시적 오류로 처리한다.

### 9.3 편집

M2에서는 `isRecurring=true` event와 occurrence를 편집하지 않는다. API는 `calendar.recurring_edit_not_supported`를 반환한다. external exception을 단일 event처럼 잘못 수정하지 않는다.

## 10. Provider 오류 분류

| Google 결과 | 분류 | 처리 |
|---|---|---|
| 400 invalid request | permanent | mutation 실패, 입력/mapper test 추가 |
| 401 access token 만료 | refreshable | token refresh 후 한 번 재호출 |
| `invalid_grant` | reauth | account `reauth_required`, job 중단 |
| 403 insufficient scope | reauth/permanent | scope 확인 후 reconnect 안내 |
| 403 read-only calendar | permanent | access role 갱신, mutation 거부 |
| 404 event 없음 | operation별 reconcile | delete는 성공, update는 최신 상태 확인 |
| 409 insert conflict | reconcile | deterministic ID로 GET 후 소유 확인 |
| 410 sync token invalid | reset | active data 유지한 full sync |
| 412 ETag mismatch | conflict | 최신 provider state와 desired patch 모두 보존 |
| 429 | transient | `Retry-After` 우선, jitter backoff |
| 500/502/503/504 | transient | bounded retry, 마지막 read model 유지 |
| timeout/connection reset | unknown outcome | GET/reconcile 후 재시도 |

재시도 횟수보다 operation의 실제 적용 여부 확인이 먼저다. permanent failure는 자동 반복하지 않는다.

## 11. 오류와 사용자 문구

| Code | HTTP | Retryable | 사용자 message |
|---|---:|---:|---|
| `calendar.not_connected` | 409 | false | Google Calendar를 먼저 연결해 주세요. |
| `calendar.authorization_failed` | 400 | false | Calendar 연결을 다시 진행해 주세요. |
| `calendar.account_mismatch` | 403 | false | 로그인한 Google 계정을 확인해 주세요. |
| `calendar.reauth_required` | 409 | false | Google Calendar를 다시 연결해 주세요. |
| `calendar.read_only` | 403 | false | 이 캘린더의 일정은 변경할 수 없어요. |
| `calendar.invalid_time_range` | 422 | false | 일정 시간을 다시 확인해 주세요. |
| `calendar.recurring_edit_not_supported` | 422 | false | 반복 일정 편집은 아직 지원하지 않아요. Google Calendar에서 변경해 주세요. |
| `calendar.version_conflict` | 409 | false | 다른 곳에서 일정이 변경됐어요. 최신 내용을 확인해 주세요. |
| `calendar.sync_token_invalid` | 503 | true | 일정을 다시 동기화하고 있어요. |
| `calendar.provider_unavailable` | 503 | true | 마지막으로 저장된 일정을 보여드려요. 잠시 후 다시 시도해 주세요. |
| `calendar.rate_limited` | 503 | true | 잠시 후 일정을 다시 동기화할게요. |

API error `details`에는 field name이나 conflict ID처럼 복구에 필요한 값만 넣는다. Google response body, event 본문, token, ETag 원문은 넣지 않는다.

## 12. 보안과 개인정보

- Calendar refresh token과 sync token은 AEAD로 application-level encryption한다.
- ciphertext마다 random nonce와 key version을 저장한다.
- AAD에 environment, user ID, account ID, token type을 넣어 token swap을 막는다.
- access token은 memory cache만 사용하고 DB에 평문 저장하지 않는다.
- refresh는 account별 singleflight로 묶어 token stampede를 막는다.
- encryption key는 API/scheduler service에만 mount하고 client/Agent에 전달하지 않는다.
- event 제목, 설명, 위치, 참석자, Google calendar ID를 일반 log와 metric label에 넣지 않는다.
- Google HTML description과 external link는 untrusted input으로 처리한다.
- OAuth state는 high entropy, single-use, session/device-bound이며 callback에서 constant-time 검증한다.
- callback은 고정 redirect와 CSP를 사용하고 code/state를 HTML이나 analytics에 남기지 않는다.
- connection 해제 시 cached event와 credential을 purge한다.
- backup은 token ciphertext와 일정 본문을 포함하므로 production data와 같은 접근 통제를 적용한다.
- Google OAuth client secret과 token encryption key rotation runbook을 작성한다.

## 13. 구현 순서

1. Calendar domain DTO, time type, state machine, provider port를 정의한다.
2. M2 migration과 DB constraint/index를 작성한다.
3. token AEAD/key-version adapter와 rotation test를 구현한다.
4. server-owned OAuth authorization/callback/account state를 구현한다.
5. Google token refresh singleflight와 provider HTTP client를 구현한다.
6. Calendar list full/incremental sync와 staging apply를 구현한다.
7. event mapper와 full sync를 구현한다.
8. event incremental sync, deletion, `410` reset을 구현한다.
9. recurrence expansion과 time/all-day contract를 구현한다.
10. Calendar/event read API와 sync status API를 구현한다.
11. idempotent create transaction과 deterministic provider ID를 구현한다.
12. update/delete ETag conditional mutation을 구현한다.
13. conflict 보존/해결과 mutation status API를 구현한다.
14. scheduler claim/lease/retry와 manual sync deduplication을 구현한다.
15. `sync_changes`, audit, OpenAPI schema를 연결한다.
16. mock provider integration test를 모두 통과한다.
17. staging Google account와 test calendar에서 실제 OAuth/sync/CRUD를 검증한다.
18. token revoke, 410, 412, 429, server restart recovery를 검증한다.

## 14. 자동 테스트

### 14.1 Domain/time unit test

- timed/all-day DTO validation
- RFC 3339 offset과 IANA timezone
- all-day exclusive end
- DST gap/fold와 Asia/Seoul day boundary
- provider access role→`isEditable`
- unsupported/recurring mutation 차단
- sync/account/mutation state transition
- provider error retry classification
- deterministic event ID 문자/길이 property test

### 14.2 OAuth/token test

- state single-use/session binding/expiry
- callback CSRF와 account `sub` mismatch
- missing scope와 missing refresh token 처리
- token AEAD encrypt/decrypt/AAD mismatch/key version
- concurrent access token refresh singleflight
- revoke 후 credential 접근 불가
- callback/log redaction

### 14.3 Google adapter fixture test

- calendar list full/incremental pagination
- event full/incremental pagination
- deleted event에 ID만 있는 payload
- recurring master/exception/cancelled exception
- last page에만 `nextSyncToken`
- query fingerprint 불일치 차단
- 410 reset
- 401 refresh, invalid_grant, 403, 429, 5xx, timeout
- insert timeout 후 GET reconcile
- insert 409 same/different internal ID
- update/delete 412 ETag conflict

Google adapter test는 local mock HTTP server를 사용하고 실제 Google credential을 CI에 넣지 않는다.

### 14.4 PostgreSQL integration test

- empty DB와 M1 DB migration
- encrypted token column에 plaintext 미존재
- account/user uniqueness
- calendar/event provider ID uniqueness
- 한 calendar active sync run unique constraint
- lease expiry와 `SKIP LOCKED` claim
- full sync staging 실패 시 active data 유지
- successful apply와 sync token/change atomicity
- incremental delete tombstone
- create API/idempotency/outbox atomicity
- 같은 idempotency key의 동일/다른 request
- expectedVersion conflict
- concurrent mutation과 provider sync
- disconnect purge와 delete change 발행

### 14.5 HTTP/OpenAPI contract test

- 모든 Calendar route auth guard
- 다른 user/internal ID 접근 `404`
- create/update/delete validation과 body limit
- mutation response status `201/200/202`
- error envelope/사용자 message/request ID
- OpenAPI route/schema 일치
- bootstrap에 calendar/event 추가 후 M1 client 호환성
- changes pagination/tombstone

### 14.6 회귀 명령

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
M1 → M2 migration test
OpenAPI generation diff check
Docker image build
Compose smoke with mock Google
secret/log redaction test
```

## 15. 수동 검증

### 15.1 OAuth와 초기 sync

1. Mac에서 허용 Google 계정으로 Calendar 연결을 시작한다.
2. consent screen scope가 명세와 일치하는지 확인한다.
3. 다른 Google 계정을 선택하면 연결이 거부되는지 확인한다.
4. primary/selected calendar와 일정 수가 Google Calendar와 일치하는지 확인한다.
5. 연결 후 server restart에서도 refresh와 sync가 유지되는지 확인한다.

### 15.2 Google → Jimin OS

1. Google Calendar 웹에서 timed event를 만든다.
2. Google Calendar 휴대폰 앱에서 all-day event를 만든다.
3. recurring series의 한 occurrence를 변경하고 하나를 취소한다.
4. manual/incremental sync 후 API read model과 기간 조회를 확인한다.
5. title/time/location 수정과 삭제가 반영되는지 확인한다.

### 15.3 Jimin OS → Google

1. Mac API client에서 timed/all-day event를 만든다.
2. 같은 idempotency key 요청을 반복해 Google event가 하나인지 확인한다.
3. event를 수정하고 삭제한다.
4. Google Calendar 웹과 개인 휴대폰 앱에서 반영을 확인한다.
5. read-only/recurring event mutation이 안전하게 거부되는지 확인한다.

### 15.4 충돌과 장애

1. Jimin OS에서 읽은 뒤 Google Calendar에서 먼저 수정해 412 conflict를 만든다.
2. `keepGoogle`, `retryMine` 두 resolution을 각각 검증한다.
3. 저장된 sync token을 staging fixture에서 무효화해 active data를 유지한 full resync를 확인한다.
4. Google network를 차단하고 마지막 read model 조회가 가능한지 확인한다.
5. mutation provider 응답 직전에 worker를 종료하고 재시작 후 중복 event가 없는지 확인한다.
6. refresh token을 revoke하고 `reauthRequired`와 reconnect를 확인한다.
7. Calendar connection 해제 후 token/cached event가 purge되는지 확인한다.

## 16. 산출물

- M2 DB migration과 schema snapshot
- Google Calendar OAuth/token adapter
- Calendar list/event sync scheduler와 provider fixtures
- recurrence/time mapper와 tests
- Calendar/event/mutation/sync API
- updated generated OpenAPI와 client type
- Calendar 연결·재연결·해제 runbook
- sync reset/충돌/장애 대응 runbook
- staging test calendar 검증 기록
- token/log redaction 점검 결과
- Backend Ultrawork 결과

## 17. 완료 게이트

- [ ] Calendar permission은 Jimin OS 로그인과 분리되어 있고 최소 scope만 요청한다.
- [ ] callback state, PKCE, session binding, Google `sub` 일치 검증이 통과한다.
- [ ] refresh/sync token이 암호화돼 있고 log, error, 일반 API에 노출되지 않는다.
- [ ] Calendar list와 event full sync가 pagination 끝까지 완료된다.
- [ ] incremental sync가 삭제, recurring exception, 마지막 `nextSyncToken`을 정확히 처리한다.
- [ ] `410 Gone`에서 기존 일정 조회를 유지한 채 full resync한다.
- [ ] timed/all-day/recurring read model과 DST/timezone test가 통과한다.
- [ ] create/update/delete가 local transaction과 outbox를 먼저 확정한다.
- [ ] 동일 create 요청을 반복해도 Google event가 하나만 존재한다.
- [ ] update/delete가 ETag `If-Match`를 사용하고 412에서 사용자 변경을 잃지 않는다.
- [ ] Google 429/5xx/timeout과 worker restart에서 bounded retry/reconcile이 동작한다.
- [ ] Google 장애 중에도 마지막 일정 API와 M1 인증 API가 동작한다.
- [ ] Calendar connection 해제 후 credential, sync token, cached event가 purge된다.
- [ ] Mac에서 만든 일정이 Google Calendar 웹과 개인 휴대폰 앱에 보인다.
- [ ] Google Calendar 휴대폰 앱에서 만든 일정이 서버 read model에 보인다.
- [ ] migration, formatter, lint, unit/integration/contract test, image build가 통과한다.
- [ ] 실제 route와 OpenAPI가 일치하고 Backend Ultrawork checklist에 실패가 없다.

이 게이트를 통과한 뒤 M3 client는 Calendar API와 `sync_changes`를 기준으로 local SQLite cache를 구현한다. client가 Google API를 직접 호출하거나 refresh token을 저장하지 않는다.
