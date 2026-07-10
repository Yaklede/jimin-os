<!-- markdownlint-disable MD013 -->

# M3 Mac·모바일 클라이언트 상세 명세

이 문서는 macOS 앱과 개인 휴대폰 앱을 실제 일상 사용이 가능한 클라이언트로 만드는 구현 계약이다. 기간이나 인력 산출은 다루지 않는다. 공통 API·ID·시간·오류·동기화 규칙은 [공통 구현 계약](SHARED_CONTRACTS.md)을 따른다.

## 1. 단계 결과

M3가 끝나면 사용자는 다음을 할 수 있어야 한다.

1. macOS 앱과 개인 휴대폰에서 같은 Google 계정으로 로그인한다.
2. Mac이 꺼져 있어도 휴대폰에서 오늘·주간 일정을 보고 생성·수정·삭제한다.
3. 서버가 일시적으로 보이지 않아도 최근 일정과 할 일을 읽는다.
4. 오프라인에서 만든 변경을 잃지 않고 재연결 후 한 번만 서버에 반영한다.
5. 다른 기기에서 생긴 변경을 증분 동기화하고 충돌을 사용자에게 설명한다.
6. 서버, Google Calendar, 이후 M4 Agent의 장애를 서로 구분해 필요한 기능만 제한한다.
7. Tauri 2 모바일을 실제 휴대폰에서 검증하고 유지 또는 Expo/React Native 전환을 ADR로 확정한다.

핵심 성공 시나리오는 `Mac OFF → 휴대폰 앱 실행 → cache를 즉시 표시 → 로컬 서버와 동기화 → 일정 수정 → Google Calendar와 Mac 앱에 동일 결과 반영`이다.

## 2. 시작 조건

- M0에서 macOS Tauri shell과 모바일 후보 shell의 debug/release build가 만들어졌다.
- M1의 Google identity, device session, `/v1/sync/changes`, status API가 고정됐다.
- M2의 일정 CRUD, Google Calendar read model, `expectedVersion`, idempotency 계약이 통과했다.
- staging hostname과 TLS 인증서를 Mac과 개인 휴대폰이 신뢰한다.
- 실제 휴대폰 플랫폼, OS version, device model을 검증 기록에 남겼다.
- `minSupportedClientVersion`, server build SHA, schema version을 status API에서 읽을 수 있다.
- OAuth redirect URI와 deep link가 staging/production별로 분리됐다.

### UI 구현 전 별도 필수 조건

현재 저장소에 `DESIGN.md`와 `STYLESEED.md`가 없다면 화면 코드보다 먼저 사용자와 함께 두 계약을 확정한다. 이후 UI 작업마다 OpenDock design/UX writing run manifest를 만들고 해당 run의 target file만 검증한다. 이 명세는 정보 구조와 상태를 정하지만 accent, radius, shadow, motion, typography raw value를 임의로 확정하지 않는다.

## 3. 범위

### 포함

- macOS Tauri 2 앱
- Tauri 2 모바일 우선 구현과 실제 휴대폰 판정
- React + TypeScript 공통 feature/view-model 계층
- Google 로그인, device session 발급·갱신·폐기
- Google Calendar 추가 권한 연결·재연결·해제 화면
- 오늘, 일정, 할 일, 설정 화면
- SQLite cache와 명시적 local migration
- sync cursor, bootstrap, tombstone 처리
- offline mutation queue, optimistic UI, conflict UI
- HTTPS API와 WSS 재연결·replay
- 앱 foreground/background, cold start, process kill 복구
- 생체 인증 앱 잠금
- staging 연결 상태와 진단 정보
- M4 대화 화면이 사용할 client event/storage port

### 제외

- 모바일에서 Codex 또는 임의 subprocess 실행
- Mac Worker 파일·터미널 실행
- Calendar 외 provider 연동
- push notification 기반 background sync
- 다중 사용자·조직·공유 캘린더 권한 UI
- CRDT
- 공개 relay
- 범용 local file browser
- 앱 내 secret 또는 raw SQL console

## 4. 고정 설계 원칙

1. PostgreSQL과 Google Calendar read model이 원본이며 SQLite는 교체 가능한 cache다.
2. WSS는 변경 힌트와 실시간 UX를 위한 통로다. 영구 상태는 HTTP/read model로 복구한다.
3. server state, optimistic mutation, 화면 전용 state를 서로 다른 store로 둔다.
4. access/refresh token을 WebView의 `localStorage`, IndexedDB, 일반 JSON store, SQLite에 저장하지 않는다.
5. 모바일 shell이 Expo로 바뀌어도 feature/view-model과 API 계약은 유지한다.
6. 오프라인 변경은 성공 응답 전까지 삭제하지 않고 모든 쓰기에 `Idempotency-Key`를 보낸다.
7. 한 entity의 mutation은 생성 순서대로 처리한다. 서로 다른 entity만 제한적으로 병렬 처리한다.
8. cache가 있으면 네트워크 응답을 기다리지 않고 먼저 보여주고, freshness를 함께 표시한다.
9. Agent 장애는 일정·할 일 화면의 loading이나 readiness를 막지 않는다.
10. 모바일은 데스크톱의 축소판이 아니라 읽기·입력·승인에 맞춘 별도 shell이다.

## 5. 코드 경계

```text
apps/desktop
├─ src/                    # macOS navigation, window, feature composition
└─ src-tauri/              # secure store, SQLite, lifecycle, native commands

apps/mobile
├─ src/                    # mobile navigation, safe-area, feature composition
└─ src-tauri/              # iOS/Android entry, native plugin adapters

packages/api-client
├─ generated/              # OpenAPI generated types/client
└─ transport/              # auth refresh, request id, idempotency

packages/client-state
├─ ports/                  # cache, secure session, connectivity, lifecycle
├─ sync/                   # bootstrap, pull, outbox, conflict orchestration
├─ auth/
└─ features/               # today/calendar/tasks view-model

packages/design-tokens     # DESIGN.md/STYLESEED.md에서 생성한 semantic token
packages/chat-renderer     # M4에서 활성화
```

공통 React component는 feature 단위로 공유하되 다음 shell은 분리한다.

- macOS: sidebar, content pane, detail/inspector pane, keyboard/focus path
- mobile: contextual top bar, single-column content, top-level bottom navigation, safe area

### 필수 client port

```ts
interface SecureSessionStore {
  read(): Promise<DeviceSession | null>;
  write(session: DeviceSession): Promise<void>;
  clear(): Promise<void>;
}

interface LocalCache {
  migrate(): Promise<void>;
  readToday(date: string): Promise<TodaySnapshot>;
  applyChangePage(page: SyncChangePage): Promise<void>;
  enqueue(mutation: OfflineMutation): Promise<void>;
  listDispatchable(now: string): Promise<OfflineMutation[]>;
}

interface AppLifecycle {
  current(): "active" | "inactive" | "background";
  subscribe(listener: (state: string) => void): Unsubscribe;
}

interface Connectivity {
  current(): Promise<"online" | "offline" | "unknown">;
  subscribe(listener: (state: string) => void): Unsubscribe;
}
```

Tauri 구현은 typed command로 Rust에 위임한다. React 코드에서 임의 SQL을 실행하거나 secret file path를 알 수 없게 한다. Expo 전환 시 같은 port를 `expo-sqlite`, OS secure store, AppState, NetInfo 계열 adapter로 구현한다.

## 6. 플랫폼 capability 표

| capability | macOS Tauri | Tauri mobile | Expo 전환 시 |
| --- | --- | --- | --- |
| API/WSS | Rust 또는 공통 transport | Rust 또는 공통 transport | JS/native transport |
| SQLite | Rust repository | Rust repository | `expo-sqlite` adapter |
| session secret | Keychain adapter | Keychain/Keystore adapter | secure-store adapter |
| OAuth | system browser + deep link | system browser + universal/app link | AuthSession adapter |
| biometric lock | LocalAuthentication 계열 | iOS/Android biometric plugin | LocalAuthentication adapter |
| app lifecycle | Tauri window/app event | native mobile lifecycle plugin | AppState adapter |
| background work | foreground 중심 | OS가 허용한 짧은 flush만 | OS가 허용한 짧은 flush만 |

모바일에서 shell, sidecar, arbitrary filesystem, 장기 background loop에 의존하는 구현은 금지한다.

## 7. 정보 구조와 화면 계약

### 공통 top-level destination

- 오늘
- 일정
- 할 일
- AI: M4 server capability가 준비된 경우에만 노출
- 설정

M5의 기억과 M6의 Mac Worker는 각 단계에서 추가한다. 아직 사용할 수 없는 기능을 비활성 메뉴로 미리 노출하지 않는다.

### 오늘

- 첫 시선: 현재 날짜와 다음 일정
- primary action: `일정 추가하기`
- section 순서: 다음 일정 → 오늘 일정 → 오늘 할 일 → 동기화가 필요한 변경
- cache 표시 중이면 마지막 저장 시각을 과장 없이 표시한다.
- 빈 상태에서도 날짜 이동과 일정 추가는 가능해야 한다.

### 일정

- 기본 view: 모바일은 agenda/day, macOS는 agenda/week를 우선한다.
- month grid는 날짜 탐색용이며 일정 제목을 과밀하게 넣지 않는다.
- event detail은 title, 시간/all-day, timezone, location, calendar, sync state를 보여준다.
- create/edit form은 `Field → Label → Input → description/error` 구조를 사용한다.
- `isEditable=false`인 반복/특수 일정은 읽기만 제공하고 Google Calendar에서 변경하는 action을 보여준다.
- all-day의 종료일은 M2 계약대로 exclusive date이며 client가 하루를 임의로 더하거나 자정 UTC로 바꾸지 않는다.
- 삭제는 Google Calendar에서도 삭제된다는 결과를 먼저 설명한다.

### 할 일

- 오늘, 예정, 완료 filter
- create, title 수정, due date, 완료/되돌리기
- offline pending과 conflict를 색만으로 표시하지 않고 label과 accessible name을 제공한다.

### 설정

- 로그인 계정
- Google Calendar 연결 계정, 권한 상태, 재연결·해제
- 연결된 server environment와 app version
- 마지막 동기화 시각
- 보류/실패한 변경 개수
- 생체 잠금 on/off
- 현재 기기 로그아웃
- 진단 정보 복사: token, 일정 제목, 메시지 원문은 제외

### Google Calendar 연결 흐름

1. 연결되지 않았으면 일정 화면에 이유와 `Google Calendar 연결하기`를 보여준다.
2. client는 `POST /v1/calendar/connections/google/authorizations`에 현재 `clientKind`만 보낸다.
3. 반환된 URL을 system browser에서 연다. URL, state, code를 log나 WebView에 복사하지 않는다.
4. server callback이 platform app link로 돌아오면 authorization ID를 신뢰해 성공으로 단정하지 않고 연결 상태 API를 다시 읽는다.
5. account가 active가 된 뒤 initial sync 상태를 표시하고 완료 시 bootstrap/change pull을 실행한다.
6. `reauthRequired`면 기존 cache를 유지하면서 `Google Calendar 다시 연결하기`를 제공한다.
7. 연결 해제는 server read model과 local cache가 지워진다는 결과를 설명하고 `expectedVersion`으로 요청한다.

앱 신원 확인용 Google 로그인과 Calendar 추가 scope 연결은 별도 상태로 관리한다. Jimin OS에 로그인됐지만 Calendar가 연결되지 않은 상태를 정상적인 복구 가능 상태로 지원한다.

### 레이아웃·상태 공통 규칙

- 화면마다 primary action은 하나만 둔다.
- macOS는 반복 작업의 keyboard/focus 이동을 지원한다.
- mobile touch target은 최소 44px, desktop pointer control은 design contract 범위에서 정한다.
- mobile horizontal scroll은 carousel로 명시된 경우 외에는 허용하지 않는다.
- loading, empty, error, offline, pending, conflict 상태를 data surface별로 제공한다.
- visible focus ring, screen reader label, `aria-live`/native announcement, reduced-motion을 적용한다.
- 정상 상태를 장식용 색으로 채우지 않고 semantic error/warning/success는 의미가 있을 때만 쓴다.

## 8. 로컬 SQLite schema

SQLite는 WAL, foreign key, busy timeout을 명시적으로 설정한다. migration은 app start에서 UI hydrate 전에 수행하고 실패하면 DB를 임의 삭제하지 않는다.

```sql
local_schema_migrations(
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
)

sync_state(
  scope TEXT PRIMARY KEY,
  last_sequence TEXT NOT NULL DEFAULT '0',
  last_event_id TEXT NOT NULL DEFAULT '0',
  last_success_at TEXT,
  bootstrap_generation TEXT,
  server_schema_version TEXT,
  updated_at TEXT NOT NULL
)

calendar_connection_cache(
  singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
  status TEXT NOT NULL,
  email TEXT,
  reauth_required INTEGER NOT NULL DEFAULT 0,
  last_successful_sync_at TEXT,
  entity_version INTEGER,
  cached_at TEXT NOT NULL
)

calendars_cache(
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  timezone TEXT,
  access_role TEXT NOT NULL,
  is_primary INTEGER NOT NULL,
  sync_enabled INTEGER NOT NULL,
  sync_status TEXT NOT NULL,
  entity_version INTEGER NOT NULL,
  deleted_at TEXT,
  cached_at TEXT NOT NULL
)

calendar_events_cache(
  cache_key TEXT PRIMARY KEY,
  id TEXT NOT NULL,
  occurrence_id TEXT,
  calendar_id TEXT NOT NULL,
  title TEXT NOT NULL,
  description_text TEXT,
  location TEXT,
  starts_at TEXT,
  ends_at TEXT,
  start_date TEXT,
  end_date TEXT,
  is_all_day INTEGER NOT NULL,
  timezone TEXT,
  provider_status TEXT NOT NULL,
  event_type TEXT NOT NULL,
  is_recurring INTEGER NOT NULL,
  is_editable INTEGER NOT NULL,
  sync_state TEXT NOT NULL,
  server_mutation_id TEXT,
  entity_version INTEGER NOT NULL,
  deleted_at TEXT,
  cached_at TEXT NOT NULL
)

tasks_cache(
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  status TEXT NOT NULL,
  due_at TEXT,
  entity_version INTEGER NOT NULL,
  deleted_at TEXT,
  cached_at TEXT NOT NULL
)

outbox_mutations(
  client_mutation_id TEXT PRIMARY KEY,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  operation TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  base_version INTEGER,
  idempotency_key TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL,
  server_mutation_id TEXT,
  server_mutation_status TEXT,
  attempt_count INTEGER NOT NULL DEFAULT 0,
  next_attempt_at TEXT,
  last_error_code TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
)

sync_conflicts(
  id TEXT PRIMARY KEY,
  client_mutation_id TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  server_mutation_id TEXT,
  local_payload_json TEXT NOT NULL,
  server_payload_json TEXT NOT NULL,
  server_version INTEGER NOT NULL,
  resolution TEXT,
  created_at TEXT NOT NULL,
  resolved_at TEXT
)

seen_realtime_events(
  event_id TEXT PRIMARY KEY,
  seen_at TEXT NOT NULL
)
```

server sequence는 API의 decimal string을 그대로 저장한다. WSS `eventId`는 공통 OpenAPI type을 native adapter에서 canonical decimal text로 바꿔 저장한다. 숫자로 전달된 값이 JavaScript safe-integer 범위를 넘으면 조용히 반올림하지 않고 protocol incompatibility로 처리한다.

`calendar_events_cache.cache_key`는 일반 일정이면 event ID, 반복 occurrence면 event ID와 opaque occurrence ID의 충돌 없는 조합이다. 같은 event의 모든 occurrence를 delete/tombstone할 수 있도록 `id` index를 별도로 둔다. client는 provider ETag나 Google event ID를 저장하거나 전송하지 않는다.

### cache 보존 범위

- 지난 30일 일정
- 향후 90일 일정
- 오늘과 이번 주 할 일
- M4가 추가하는 최근 대화와 진행 중 job
- M5가 추가하는 pinned memory

범위를 벗어난 행은 성공한 sync transaction 뒤에 정리한다. pending mutation, conflict, 현재 화면이 참조하는 entity는 정리하지 않는다.

### 로컬 DB 보호

- DB에는 Jimin OS access/refresh token, Google token, ChatGPT credential을 저장하지 않는다.
- OS app sandbox와 platform file protection을 사용한다.
- 앱 로그아웃 시 session secret을 먼저 폐기하고 cache/outbox 삭제 여부를 사용자 선택에 따라 처리한다.
- device 폐기 또는 계정 변경 시 이전 사용자의 cache를 열지 않는다.
- 개인 일정 원문이 포함되므로 backup/export 대상에서 제외하는 설정을 플랫폼별로 확인한다.

## 9. client state model

```ts
type AuthState =
  | "unknown"
  | "signedOut"
  | "refreshing"
  | "signedIn"
  | "reauthRequired";

type ConnectionState =
  | "unknown"
  | "online"
  | "offline"
  | "reconnecting"
  | "incompatible";

type DataFreshness =
  | { kind: "empty" }
  | { kind: "cached"; savedAt: string }
  | { kind: "syncing"; savedAt?: string }
  | { kind: "fresh"; syncedAt: string }
  | { kind: "partial"; syncedAt?: string; failedScope: string };

type MutationState =
  | "queued"
  | "sending"
  | "applied"
  | "retryWait"
  | "conflict"
  | "rejected";
```

screen은 위 state를 합성한다. 예를 들어 cache가 있는 `offline`은 full-page error가 아니라 stale banner이고, cache가 없는 `offline`만 recovery action이 있는 empty/error surface다.

## 10. 시작·동기화 알고리즘

### cold start

```text
1. local migration 실행
2. cache를 읽어 화면에 hydrate
3. secure store에서 device session 조회
4. access token 갱신 또는 로그인 화면 전환
5. server status와 client compatibility 확인
6. WSS 연결 및 lastEventId 전달
7. lastSequence 이후 change page를 끝까지 pull
8. upsert change를 entity type별 resource read/batch read로 hydrate
9. hydrated entity + tombstone + cursor 갱신을 한 local transaction으로 처리
10. outbox를 entity별 FIFO로 dispatch
11. outbox 적용 뒤 change page를 다시 pull
12. lastSuccessAt 갱신, stale 표시 제거
```

2번까지는 네트워크를 기다리지 않는다. `sync_changes`의 upsert는 변경 metadata이므로 authoritative resource를 다시 읽기 전에는 cursor를 전진시키지 않는다. Calendar/event change는 M2 기간 조회와 단일 조회를 사용한다. 반복 일정이거나 새 일정인지 알 수 없는 change는 cache 보존 기간을 bounded window로 나눠 다시 조회하고 occurrence를 교체한다. resource read가 `404`면 소유권을 다시 확인한 뒤 tombstone과 동일하게 처리한다. 7~11번 중 실패해도 이미 보여준 cache와 outbox를 삭제하지 않는다.

### foreground resume

- background에 머문 동안의 WSS 연결을 신뢰하지 않는다.
- access token을 점검하고 `lastSequence`부터 HTTP pull한다.
- 동일 `eventId`와 이미 반영한 `entityVersion`은 무시한다.
- 화면에 진행 중 form이 있으면 server change로 입력값을 덮지 않고 conflict 후보로 유지한다.

### WSS

- WSS `sync.changed`는 즉시 local row를 직접 수정하지 않고 pull을 깨우는 신호다.
- reconnect 시 `lastEventId`를 전달하고 replay 뒤 HTTP pull로 최종 수렴한다.
- event queue가 넘치거나 replay gap이 있으면 full HTTP reconciliation을 실행한다.
- authentication, protocol, server shutdown close code를 구분해 재로그인·재연결·업데이트 안내로 매핑한다.
- exponential backoff와 jitter를 사용하고 foreground/manual retry는 backoff를 한 번 초기화할 수 있다.

### cursor 만료와 full bootstrap

- server가 `sync.cursor_expired`를 반환하면 기존 cache를 즉시 비우지 않는다.
- bootstrap 결과를 새 generation으로 stage한다.
- 모든 page와 checksum/마지막 sequence를 받은 뒤 한 transaction에서 generation을 교체한다.
- pending mutation과 conflict는 보존하고 새 server version에 대해 다시 평가한다.
- bootstrap 실패 시 이전 cache와 stale 표시를 유지한다.

## 11. offline mutation queue

### enqueue

1. client가 UUIDv7 `clientMutationId`와 `idempotencyKey`를 만든다.
2. 사용자가 본 server version을 `baseVersion`으로 저장한다.
3. outbox insert와 optimistic cache update를 한 local transaction으로 처리한다.
4. UI에 `저장 대기 중` 상태를 표시한다.

### dispatch 규칙

- 같은 `entityType + entityId`는 created_at 순서로 하나씩 보낸다.
- create 뒤 patch가 있으면 create 성공으로 server ID/version을 확정한 후 patch를 보낸다.
- client-generated UUID를 내부 ID로 그대로 사용할 수 있는 resource는 create 재매핑을 피한다.
- HTTP timeout 후에는 같은 idempotency key로 재전송한다.
- 성공 body와 sync change가 도착하는 순서는 가정하지 않는다. entity version으로 병합한다.

### 오류 분류

| 결과 | queue 처리 | 사용자 처리 |
| --- | --- | --- |
| network/timeout/5xx/429 | `retryWait`, backoff | cache 유지, 자동 재시도 안내 |
| 401 | dispatch 일시 중지, token refresh | 실패 시 로그인 안내 |
| 403 | `rejected` | 권한 안내, 자동 재시도 안 함 |
| 409 version conflict | `conflict`, snapshot 저장 | 최신 값과 내 변경을 비교 |
| 409 idempotency mismatch | `rejected`, 진단 기록 | 변경을 다시 작성하도록 안내 |
| 422 validation | `rejected` | 해당 field와 수정 행동 표시 |
| success | `applied`, server mutation ID 보존 | specific success 또는 provider 반영 중 표시 |

일반 resource의 `applied` row는 해당 server version이 pull로 확인된 뒤 삭제한다. Calendar 쓰기는 M2 응답의 `mutationId`를 저장하고 provider mutation이 `completed` 또는 명시적으로 해결될 때까지 원래 local payload를 보존한다.

즉시 발생한 Jimin OS `expectedVersion` 충돌은 다음 둘로 해결한다.

- `최신 내용 사용하기`: server snapshot을 적용하고 mutation을 종료한다.
- `내 변경 다시 적용하기`: server version을 새 `baseVersion`으로 사용해 새 mutation을 만든다.

Google `412` 등으로 M2의 Calendar mutation이 나중에 `conflict`가 되면 event read와 mutation status를 다시 읽는다. 이 경우 같은 PATCH를 직접 재전송하지 않고 다음 M2 resolution API를 호출한다.

- `최신 내용 사용하기` → `keepGoogle`
- `내 변경 다시 적용하기` → `retryMine`

두 유형 모두 `sync_conflicts.kind`로 구분하고 기존 conflict와 원래 mutation을 감사 가능한 local history로 남긴 뒤 정리 정책에 따라 삭제한다.

### 로그아웃

outbox가 비어 있지 않으면 바로 폐기하지 않는다.

- `변경사항 동기화하기`
- `변경사항 버리고 로그아웃하기`
- `계속 사용하기`

두 번째 선택은 결과를 설명하는 blocking decision으로 처리하고, 선택 후 session secret과 cache를 함께 지운다.

## 12. 오류·상태 UX 문구 계약

내부 error code, endpoint, payload, token, Codex라는 구현 용어를 일반 오류 화면에 노출하지 않는다.

| 상황 | 한국어 | English | action |
| --- | --- | --- | --- |
| cache loading | 일정을 불러오고 있어요 | Loading your schedule | 없음 |
| offline + cache | 오프라인이에요. 마지막으로 저장한 내용을 보여드려요. | You're offline. Showing the last saved version. | `다시 연결` / `Try again` |
| offline + no cache | 내용을 불러올 수 없어요. 연결을 확인하고 다시 시도해 주세요. | We couldn't load your data. Check your connection and try again. | `다시 시도` / `Try again` |
| queued mutation | 변경사항은 연결되면 저장돼요. | Your changes will be saved when you're back online. | 없음 |
| rejected mutation | 변경사항을 저장하지 못했어요. 내용을 확인하고 다시 시도해 주세요. | We couldn't save your changes. Check the information and try again. | `내용 확인하기` / `Review changes` |
| conflict | 다른 기기에서 이 내용이 바뀌었어요. 최신 내용을 확인해 주세요. | This was changed on another device. Review the latest version. | `최신 내용 보기` / `View latest` |
| Calendar not connected | Google Calendar를 연결하면 일정을 볼 수 있어요. | Connect Google Calendar to view your schedule. | `Google Calendar 연결하기` / `Connect Google Calendar` |
| Calendar reauth | Google Calendar를 다시 연결해 주세요. | Reconnect Google Calendar. | `다시 연결하기` / `Reconnect` |
| provider pending | Google Calendar에 반영하고 있어요. | Saving to Google Calendar. | 없음 |
| provider conflict | Google Calendar에서 이 일정이 바뀌었어요. 어떤 내용을 유지할지 선택해 주세요. | This event changed in Google Calendar. Choose which version to keep. | `변경 내용 확인하기` / `Review changes` |
| session expired | 다시 로그인해 주세요. | Sign in again. | `로그인하기` / `Sign in` |
| access denied | 이 작업을 할 수 있는 권한이 없어요. | You do not have access to this action. | `닫기` / `Close` |
| client too old | 앱을 업데이트해야 계속 수정할 수 있어요. 지금은 내용을 볼 수 있어요. | Update the app to keep editing. You can still view your data. | `업데이트 안내 보기` / `View update info` |
| AI unavailable | AI에 연결할 수 없어요. 일정과 할 일은 계속 사용할 수 있어요. | AI is unavailable. Your schedule and tasks still work. | `다시 시도` / `Try again` |

빈 상태는 다음 행동을 제공한다.

- `오늘 일정이 없어요.` + `일정 추가하기`
- `등록한 할 일이 없어요.` + `할 일 추가하기`

삭제 문구는 `이 일정을 삭제할까요? Google Calendar에서도 삭제돼요.`와 `일정 삭제하기` / `유지하기`를 사용한다. 반복 일정처럼 편집할 수 없는 항목에는 `이 일정은 여기에서 변경할 수 없어요. Google Calendar에서 변경해 주세요.`와 `Google Calendar 열기`를 제공한다. 단순 성공 toast는 `일정을 저장했어요`, `할 일을 완료했어요`처럼 결과를 구체적으로 말한다.

## 13. 인증·기기 보안

- OAuth는 system browser + Authorization Code + PKCE로만 수행한다.
- refresh session은 OS secure store에 device별로 보관한다.
- 한 기기에서 refresh 요청은 process 전체 single-flight로 직렬화한다.
- rotation 응답은 새 refresh secret을 secure store에 원자적으로 기록한 뒤 memory의 이전 값을 폐기한다. 저장 여부가 불명확하면 같은 이전 secret을 자동 재사용하지 않고 다시 로그인한다.
- WebView에 전달하는 access token은 memory에만 두고 app background 시 보존 정책을 최소화한다.
- refresh rotation/reuse 오류가 오면 local session을 폐기하고 다시 로그인한다.
- biometric은 서버 인증을 대체하지 않고 local app unlock만 담당한다.
- 화면 캡처 차단은 민감 화면별 product decision으로 두며, 기본적으로 app switcher snapshot에서 민감 내용을 가린다.
- 로그와 진단 export에서 일정 제목·본문, token, OAuth code, message 원문을 마스킹한다.
- Tauri command capability/allowlist는 필요한 command만 열고 shell plugin은 모바일 앱에 등록하지 않는다.
- staging과 production session, deep link, DB file을 섞지 않는다.

## 14. Tauri 모바일 판정 게이트

M0의 `ADR-0002-mobile-runtime.md` 결정을 완성된 client flow로 재검증한다. Tauri 유지 여부는 선호가 아니라 아래 증거로 판정한다. 개인 휴대폰 플랫폼에서 release-signed build를 기준으로 한다. 다른 모바일 플랫폼은 최소 build/simulator smoke를 통과해야 하며, 실기기 없이 해당 플랫폼 지원 완료를 선언하지 않는다.

### P0 필수 검증

| 검증 | 통과 조건 |
| --- | --- |
| install/launch | signed build 설치, cold start, process kill 후 재실행 가능 |
| OAuth/deep link | 앱 로그인과 Calendar 연결이 system browser 후 정확한 environment로 복귀, 취소/중복 callback 안전 |
| secure session | reboot와 app restart 후 유지, logout/device revoke 후 읽을 수 없음 |
| SQLite | migration, transaction rollback, tombstone, 10,000행 조회에서 데이터 손상 없음 |
| HTTPS/WSS | 사설 네트워크 TLS, reconnect, `lastEventId` replay, auth close 처리 |
| lifecycle | foreground/background와 network on/off 반복 후 cursor/outbox 유실 없음 |
| offline queue | create/update/delete 재전송에도 server write가 한 번만 발생 |
| biometric | 등록 안 됨, 취소, 실패, 성공, OS 설정 변경 경로가 모두 복구 가능 |
| release pipeline | 개발자 서명, staging config 주입, production secret 미포함 확인 |
| native crash | 주요 시나리오에서 Rust panic, WebView crash, native plugin crash 없음 |

### 유지 조건

- 모든 P0가 stable Tauri API, 공식 지원 plugin 또는 범위가 좁은 자체 plugin으로 통과한다.
- auth, SQLite, WSS, lifecycle 중 어느 하나도 플랫폼별로 별도 application logic을 복제하지 않는다.
- native plugin은 capability adapter 경계 안에 머물고 feature/view-model을 침범하지 않는다.
- release build와 debug build의 동작 차이가 테스트로 설명된다.

### Expo/React Native 전환 조건

다음 중 하나라도 발생하면 모바일 shell만 Expo/React Native로 전환한다.

- P0 기능이 실제 휴대폰 release build에서 재현 가능하게 동작하지 않는다.
- 해결을 위해 Tauri fork 또는 검증되지 않은 장기 patch를 유지해야 한다.
- secure store, OAuth, SQLite, WSS lifecycle을 하나의 좁은 plugin이 아니라 플랫폼별 application layer로 다시 만들어야 한다.
- background/foreground 복귀 시 outbox 또는 cursor 유실을 안정적으로 막을 수 없다.
- signing/build pipeline이 repeatable하지 않다.
- native crash의 원인을 app code에서 격리·회귀 테스트할 수 없다.

전환 시 macOS Tauri는 유지하고 `SecureSessionStore`, `LocalCache`, `AppLifecycle`, `Connectivity`, OpenAPI client 계약을 그대로 구현한다. M0 판정과 결과가 같으면 실기기 회귀 증거를 ADR에 추가하고, 결과가 달라지면 실패 재현, 선택 이유, migration 범위, 되돌릴 조건과 함께 ADR을 개정한다.

## 15. 자동 테스트

### TypeScript/unit

- auth state transition
- cache-first query와 stale/fresh 표시
- sync page merge와 tombstone
- entity version dedupe
- outbox FIFO와 서로 다른 entity 병렬 처리
- retry 분류와 backoff
- conflict resolution이 원래 local payload를 보존하는지
- WSS duplicate/gap/replay 처리
- locale별 error code mapping

### Rust/local integration

- 빈 DB부터 모든 local migration 적용
- 각 migration 이전 snapshot에서 upgrade
- transaction 중 process error 시 cursor와 row가 함께 rollback
- secure store read/write/clear contract fixture
- typed Tauri command의 invalid input, size limit, authorization
- cache purge가 pending/conflict 보호 규칙을 지키는지

### component/accessibility

- today/calendar/task의 loading, empty, error, offline, pending, conflict
- keyboard-only macOS flow
- screen reader label과 live update
- text size 확대, 긴 한국어/영어 label, 좁은 viewport
- reduced-motion
- touch target과 horizontal overflow

### contract/E2E

- generated OpenAPI client가 staging schema와 일치
- access token 만료 중 단일 refresh만 실행
- `sync.cursor_expired` full bootstrap
- idempotent offline create/update/delete
- Calendar `mutationId` 추적과 `keepGoogle`/`retryMine` conflict resolution
- all-day exclusive end와 recurring occurrence cache key
- WSS가 끊어진 동안 생긴 변경 HTTP 복구
- old client read-only mode
- logout/device revoke 후 API와 cache 접근 차단

## 16. Mac·실기기 수동 시나리오

각 시나리오는 app/server build SHA, device/OS, 시작 cursor, 결과 screenshot 또는 log ID를 남긴다. 개인 데이터가 보이는 원본 screenshot은 저장소에 커밋하지 않는다.

1. Mac 앱과 휴대폰에서 로그인하고 Calendar를 연결한 뒤 같은 오늘 일정을 확인한다.
2. Mac을 종료하고 휴대폰에서 일정을 생성해 Google Calendar 반영을 확인한다.
3. 휴대폰을 airplane mode로 바꾸고 최근 일정을 연다.
4. 오프라인에서 일정 생성·수정·삭제를 각각 수행하고 앱을 강제 종료한다.
5. 재실행·재연결 후 각 변경이 한 번만 반영됐는지 확인한다.
6. Mac에서 같은 일정을 먼저 수정해 mobile conflict UI를 확인한다.
7. Google Calendar에서 pending update 대상 일정을 바꿔 provider conflict와 두 resolution을 확인한다.
8. WSS를 끊고 서버에서 변경한 뒤 reconnect replay와 HTTP reconciliation을 확인한다.
9. 서버를 재시작하는 동안 cache가 유지되고 복귀 후 cursor가 이어지는지 확인한다.
10. access token 만료, refresh revoke, 모든 device revoke를 각각 검증한다.
11. 앱 background/foreground 중 M4 stream이 있으면 stream 복구 port가 상태를 보존하는지 확인한다.
12. 생체 인증 취소·실패·성공과 OS에서 biometric 변경 후 재인증을 확인한다.
13. 로그아웃 시 pending mutation 세 가지 선택을 검증한다.

## 17. 구현 순서

1. platform port와 shared view-model contract를 먼저 만든다.
2. macOS/mobile shell에 environment, version, lifecycle 최소 화면을 연결한다.
3. SQLite schema와 migration runner를 만든다.
4. secure session store, 앱 로그인, Calendar authorization/deep link를 연결한다.
5. cache-first today/calendar/task query를 구현한다.
6. sync pull, transactional cursor, tombstone, full bootstrap을 구현한다.
7. offline outbox, optimistic update, retry, conflict resolver를 구현한다.
8. WSS reconnect/replay를 붙이고 HTTP reconciliation을 강제한다.
9. 각 data surface의 loading/empty/error/offline/pending/conflict UI를 완성한다.
10. biometric lock과 settings/device revoke를 구현한다.
11. P0 실제 휴대폰 검증을 수행하고 Tauri/Expo ADR을 확정한다.
12. design, UX writing, backend harness와 전체 회귀 시나리오를 통과한다.

## 18. 완료 게이트

- [ ] Mac을 끈 상태에서 휴대폰으로 오늘 일정을 볼 수 있다.
- [ ] 서버가 꺼진 상태에서도 cache 범위의 일정과 할 일을 읽을 수 있다.
- [ ] 오프라인 mutation이 app kill 후에도 남고 재연결 후 한 번만 반영된다.
- [ ] sync page와 cursor가 같은 local transaction에서 처리된다.
- [ ] cursor 만료 bootstrap 실패가 기존 cache를 지우지 않는다.
- [ ] conflict에서 local payload와 server snapshot을 모두 복구할 수 있다.
- [ ] Calendar provider conflict는 직접 PATCH 재전송 없이 M2 resolution API로 처리한다.
- [ ] 반복 occurrence와 all-day exclusive end가 cache/표시에서 보존된다.
- [ ] token이 WebView storage, SQLite, log에 남지 않는다.
- [ ] 오래된 client가 mutation을 막고 read-only로 동작한다.
- [ ] macOS keyboard/focus와 모바일 touch/screen reader 검증이 통과한다.
- [ ] 개인 휴대폰 P0 결과와 mobile shell ADR이 남아 있다.
- [ ] formatter, lint, typecheck, unit/integration/E2E, desktop/mobile release smoke가 통과한다.
- [ ] OpenDock design/UX writing/backend 해당 gate가 통과한다.

## 19. 구현 참고

- [Tauri 2 mobile prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Tauri mobile plugin development](https://v2.tauri.app/develop/plugins/develop-mobile/)
- [Tauri official plugin support table](https://v2.tauri.app/plugin/)
- [Tauri SQL plugin](https://v2.tauri.app/plugin/sql/)
- [Tauri distribution](https://v2.tauri.app/distribute/)
