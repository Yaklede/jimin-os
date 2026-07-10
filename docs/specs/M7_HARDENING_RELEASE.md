# M7. 운영 안정화와 v0.1 출시 명세

이 문서는 [공통 구현 계약](SHARED_CONTRACTS.md)과 M0~M6 단계 명세를 운영 환경에서 검증하는 최종 계약이다.

## 1. 목적

M7의 목적은 M0~M6에서 구현한 기능을 반복 배포, 장애 감지, 데이터 복구, 안전한 rollback이 가능한 개인 운영 서비스로 완성하는 것이다.

이 단계는 기능 추가 단계가 아니다. 동일한 image digest를 staging에서 검증한 뒤 production으로 승격하고, 실패하면 데이터 손실 없이 이전 상태로 돌아갈 수 있음을 증명하는 단계다.

핵심 결과는 다음과 같다.

- pull request마다 재현 가능한 품질 검증
- `linux/amd64`, `linux/arm64` container image 생성
- staging·production의 완전한 데이터·secret·hostname 분리
- migration preflight, backup, 배포, health check의 일관된 실행
- PostgreSQL·attachment의 암호화 backup과 실제 restore 검증
- API, Calendar, Agent, Memory, Mac Worker의 상태 관측
- 인증·secret·container·network 보안 점검
- image와 database 호환성을 고려한 rollback
- Mac과 개인 휴대폰 실기기 release gate

## 2. 선행 조건

- M0~M6의 완료 기준이 각 단계의 자동·수동 테스트로 검증되어 있다.
- staging과 production용 private hostname, TLS, Google OAuth client가 분리되어 있다.
- 로컬 서버에 Docker Engine, Compose v2, backup 전용 저장 경로가 준비되어 있다.
- 서버 architecture가 `amd64` 또는 `arm64`로 확인되어 있다.
- GitHub Actions와 GHCR을 사용하고 repository에는 secret이 없는 상태다.
- production 데이터 encryption key의 별도 복구본을 사용자가 보관한다.

## 3. 범위

### 3.1 포함

- GitHub Actions CI, image build, SBOM, release manifest
- GHCR immutable image 배포
- Compose base·staging·production 구성
- rootless 또는 최소 권한 container 실행
- migration 검증·적용·호환성 규칙
- backup·retention·restore drill
- 구조화 로그, metric, health, 운영 상태 화면
- dependency·secret·container·API 보안 검사
- staging 승격과 production rollback 절차
- 운영 runbook과 v0.1 release checklist

### 3.2 제외

- Kubernetes
- public SaaS와 multi-region high availability
- 자동 failover PostgreSQL cluster
- public internet ingress
- 24시간 외부 관제 서비스 의존
- 무중단 schema 대수술 자동화
- 자동 production 배포와 무승인 migration
- Mac 로컬 파일 backup
- ChatGPT credential 원문 backup

## 4. 출시 원칙

1. source commit, image digest, schema version, compose config를 한 release manifest로 묶는다.
2. production은 tag가 아니라 digest로 image를 고정한다.
3. staging에서 검증한 digest만 production으로 승격한다.
4. production migration 전에는 검증 가능한 backup을 생성한다.
5. migration은 기본적으로 forward-only expand/contract 방식으로 작성한다.
6. Agent 장애는 일정·할 일·기억 API rollback 사유와 분리해서 판단한다.
7. backup이 있다는 사실이 아니라 빈 환경에 복구한 결과로 복구 가능성을 판정한다.
8. secret은 Git, image, log, backup bundle에 평문으로 포함하지 않는다.
9. release gate 실패는 수정하거나 사용자가 명시적인 예외와 이유를 기록하기 전까지 blocker다.

## 5. 산출물 구조

```text
.github/workflows/
├─ ci.yml
├─ images.yml
├─ security.yml
└─ release.yml

deploy/
├─ compose.yaml
├─ compose.staging.yaml
├─ compose.production.yaml
├─ env/
│  ├─ staging.example.env
│  └─ production.example.env
├─ gateway/
├─ scripts/
│  ├─ preflight.sh
│  ├─ deploy.sh
│  ├─ promote.sh
│  ├─ rollback.sh
│  ├─ backup.sh
│  ├─ restore-verify.sh
│  └─ collect-diagnostics.sh
└─ releases/
   └─ release-manifest.schema.json

docs/runbooks/
├─ DEPLOY.md
├─ ROLLBACK.md
├─ BACKUP_RESTORE.md
├─ CODEX_REAUTH.md
├─ GOOGLE_REAUTH.md
├─ DEVICE_WORKER_REVOKE.md
├─ SECRET_ROTATION.md
└─ INCIDENT_RESPONSE.md
```

Script는 대화형 질문에 의존하지 않는 `--check` 모드와 실제 변경을 수행하는 명시적 mode를 분리한다. destructive flag나 volume 삭제를 기본값으로 두지 않는다.

## 6. CI 명세

### 6.1 Pull request pipeline

`ci.yml`은 GitHub-hosted runner에서만 실행하고 다음 job을 분리한다.

| Job | 검사 |
|---|---|
| `rust-quality` | `cargo fmt --check`, clippy `-D warnings`, unit test |
| `rust-integration` | PostgreSQL service container, migration, integration test |
| `web-quality` | lockfile 고정 install, lint, typecheck, unit test |
| `contracts` | OpenAPI·WSS·worker protocol·Codex schema drift 검사 |
| `memory-eval` | M5 retrieval·candidate 출시 기준 |
| `compose-validate` | base와 두 overlay의 `docker compose config` 검증 |
| `image-smoke` | API·Agent image build, container smoke test; push하지 않음 |
| `desktop-smoke` | macOS Tauri compile·test |
| `mobile-smoke` | 확정된 모바일 framework의 iOS·Android compile·test |
| `migration-compat` | 이전 release DB fixture에 새 migration 적용 후 구·신 binary 호환 검사 |

규칙:

- dependency lockfile 변경은 명시적으로 diff에 포함한다.
- CI는 실제 Google, OpenAI, production secret을 사용하지 않는다.
- 외부 연동은 mock server와 sanitized contract fixture를 사용한다.
- flaky test 자동 무한 retry를 금지한다. 재시도한 test는 별도 결과로 표시하고 release blocker로 취급한다.
- generated OpenAPI·JSON Schema가 source와 다르면 실패한다.
- PR에서 self-hosted production runner를 절대 사용하지 않는다.

### 6.2 Security pipeline

`security.yml`은 PR과 기본 branch에서 다음을 실행한다.

- repository secret scan
- `cargo audit`
- Rust·npm license allowlist 검사
- npm dependency audit
- container image vulnerability scan
- Dockerfile·Compose configuration scan
- SBOM 생성 가능 여부 검사
- GitHub Actions를 commit SHA로 pin했는지 검사

Critical 또는 High 취약점은 다음 조건 중 하나가 아니면 blocker다.

- 실제 image와 실행 경로에 해당 package가 포함되지 않음을 증명함
- upstream fix가 없고 private-network·runtime mitigation을 문서화함
- 사용자가 만료일과 담당자를 포함한 예외를 승인함

### 6.3 Main image pipeline

`images.yml`은 기본 branch의 CI가 모두 통과한 commit에서만 실행한다.

1. API, Agent image를 multi-stage Dockerfile로 build한다.
2. BuildKit으로 `linux/amd64`, `linux/arm64`를 생성한다.
3. runtime image에 compiler, package manager, test fixture를 넣지 않는다.
4. OCI label에 commit SHA, source URL, build time, version을 기록한다.
5. SPDX 또는 CycloneDX SBOM과 provenance attestation을 생성한다.
6. GHCR에 `sha-<full-commit>` tag로 push한다.
7. registry가 반환한 platform별 digest와 manifest digest를 기록한다.
8. `latest` tag는 생성하지 않는다.

Image 이름은 다음 형식을 사용한다.

```text
ghcr.io/${GITHUB_REPOSITORY_OWNER}/jimin-os-api@sha256:<digest>
ghcr.io/${GITHUB_REPOSITORY_OWNER}/jimin-os-agent@sha256:<digest>
```

### 6.4 Release manifest

CI는 서명 가능한 JSON release manifest를 artifact로 만든다.

```json
{
  "version": "0.1.0",
  "commit": "full-git-sha",
  "images": {
    "api": "ghcr.io/.../jimin-os-api@sha256:...",
    "agent": "ghcr.io/.../jimin-os-agent@sha256:..."
  },
  "schemaVersion": 17,
  "minimumCompatibleSchemaVersion": 15,
  "composeConfigVersion": 1,
  "codexVersionRange": ">=0.142.3,<0.143.0",
  "clientApiRange": ">=1,<2",
  "createdAt": "RFC3339"
}
```

Manifest는 JSON Schema로 검증하고 checksum을 release 기록에 남긴다. staging과 production은 이 파일의 digest를 기준으로 실행한다.

## 7. CD와 승격 모델

### 7.1 보안 경계

- 공개 repository의 PR code를 로컬 서버에서 실행하지 않는다.
- 로컬 서버에 범용 GitHub self-hosted runner와 Docker socket을 함께 제공하지 않는다.
- 배포는 서버의 preinstalled script가 GHCR에서 승인된 digest를 pull하는 방식으로 한다.
- 배포 account는 Jimin OS Compose project와 지정 secret·backup directory만 접근한다.
- production 변경에는 사용자의 명시적 실행이 필요하다.

### 7.2 Staging 배포

`deploy.sh staging <release-manifest>`는 다음 순서로 실행한다.

1. manifest schema, checksum, commit, image digest를 검증한다.
2. 서버 architecture에 맞는 image manifest가 존재하는지 확인한다.
3. disk, memory, port, DNS, TLS, secret file 권한을 preflight한다.
4. 현재 release manifest를 보존한다.
5. production과 다른 project name·volume·network인지 검사한다.
6. image를 digest로 pull한다.
7. migration plan을 출력한다.
8. staging DB backup을 만든다.
9. staging migration을 적용한다.
10. Compose service를 갱신한다.
11. health, API contract, Calendar mock/실계정, Agent smoke를 실행한다.
12. 성공 release manifest와 검사 결과를 staging release history에 기록한다.

실패하면 새 container를 중지하고 이전 manifest로 staging을 복원한다. migration 호환성에 따라 DB restore 필요 여부를 명시한다.

### 7.3 Production 승격

`promote.sh <staging-release-id>`는 새 image를 다시 build하지 않는다.

승격 전 확인:

- staging release가 현재 manifest digest와 정확히 일치
- 자동 test와 staging smoke가 모두 통과
- Mac·개인 휴대폰 실기기 checklist 통과
- migration preflight와 restore verification 통과
- backup 저장소 여유 공간 충족
- 알려진 blocker 없음

승격은 production backup → migration → Compose update → health/smoke → release history 기록 순서로 실행한다.

### 7.4 Client 배포

- Mac과 모바일 client는 서버 API compatibility range를 빌드에 포함한다.
- 호환되지 않는 client는 쓰기 작업을 막고 업데이트 안내를 표시한다.
- 서버가 한 단계 이전 client API와 호환되는 기간을 유지한다.
- staging client는 production과 bundle ID, application ID, OAuth redirect, secure storage namespace를 분리한다.
- mobile store 배포 전까지 개인 실기기 build도 version·commit을 상태 화면에 표시한다.
- macOS·iOS·Android signing과 provisioning 방식은 M0에서 확정한 ADR을 따른다. CI compile smoke에는 production signing key를 제공하지 않는다.
- release artifact는 source commit과 server release manifest checksum을 함께 기록하고, 서명되지 않은 build를 production client로 배포하지 않는다.

## 8. Docker Compose 명세

### 8.1 구성 파일

`compose.yaml`은 공통 service와 보안 기본값을 정의한다.

- `gateway`
- `api`
- `agent`
- `postgres`
- `backup`

`compose.staging.yaml`과 `compose.production.yaml`은 hostname, resource, logging, secret 이름만 override한다. service 정의를 전체 복사하지 않는다.

Project name:

```text
jimin-os-staging
jimin-os-production
```

두 환경은 다음을 공유하지 않는다.

- PostgreSQL volume과 database user
- Codex home volume
- attachments와 backup directory
- session signing key
- Google OAuth client와 token encryption key
- hostname과 TLS certificate
- Compose network

### 8.2 Service 제약

#### `gateway`

- 사설 네트워크 interface에서만 443을 listen한다.
- TLS 1.2 이상을 사용하고 HTTP는 HTTPS로 전환한다.
- API body size, WebSocket idle timeout, basic rate limit을 설정한다.
- `/metrics`와 내부 admin endpoint를 외부 route로 노출하지 않는다.
- security header를 설정하되 native app OAuth callback을 깨지 않도록 contract test한다.

#### `api`

- non-root fixed UID/GID
- read-only root filesystem
- `/tmp`는 size 제한 tmpfs
- `cap_drop: [ALL]`, `no-new-privileges`
- DB와 Google API outbound만 필요
- secret은 `/run/secrets/*` read-only file로 받음
- `/health/live`, `/health/ready`, 내부 `/health/components`
- shutdown signal 후 새 쓰기를 받지 않고 in-flight transaction을 제한 시간 내 종료

#### `agent`

- API와 별도 container·process
- non-root, read-only root filesystem, `cap_drop: [ALL]`
- Docker socket, host root, production secret directory mount 금지
- 전용 `codex_home`, 제한된 `agent_workspace`, size 제한 tmpfs만 mount
- OpenAI outbound와 내부 API 통신만 허용
- Agent 실패가 API readiness를 실패시키지 않음

#### `postgres`

- 내부 network에서만 접근
- host port publish 금지
- environment별 전용 volume
- health check는 단순 process가 아니라 query 가능 여부 확인
- connection limit과 statement timeout 설정
- backup user는 필요한 최소 권한만 가짐

#### `backup`

- production database read와 backup target write만 허용
- Docker socket mount 금지
- encryption recipient만 포함하고 복호화 private key는 상시 container에 넣지 않음
- backup 성공·실패·마지막 검증 시각을 API에 보고

### 8.3 Image와 설정

- 모든 app image는 digest로 지정한다.
- `.env`에는 비민감 환경 선택값만 둔다.
- secret file은 repository 밖 environment별 directory에 두고 mode `0400` 또는 실행 UID가 읽는 최소 권한을 사용한다.
- Compose rendered config를 저장할 때 secret value가 포함되지 않는지 검사한다.
- container restart policy는 장애 loop가 원인을 숨기지 않도록 backoff·restart metric과 함께 사용한다.
- resource limit과 log rotation을 service별로 설정한다.

## 9. Migration 명세

### 9.1 도구와 파일

Rust migration runner는 SQLx migration metadata와 checksum을 사용한다. Migration source는 immutable이며 이미 적용한 파일을 수정하면 CI와 startup preflight가 실패한다.

```text
migrations/
├─ 0001_initial.sql
├─ 0002_calendar_sync.sql
└─ ...
```

각 migration PR에는 다음을 함께 기록한다.

- 목적과 영향을 받는 table
- lock·rewrite·disk 증가 위험
- 구 binary와 신 binary의 호환성
- data backfill 방법
- 검증 query
- application rollback 시 동작
- DB restore가 필요한 조건

### 9.2 Expand/contract 규칙

- column·table 추가는 먼저 nullable 또는 호환 default로 배포한다.
- 새 binary가 dual-read 또는 dual-write한 뒤 backfill한다.
- 기존 column 삭제·rename·타입 축소는 같은 release에서 하지 않는다.
- contract migration은 이전 binary가 더 이상 해당 schema를 사용하지 않음을 확인한 이후 별도 release로 수행한다.
- large index는 staging에서 lock과 disk를 측정하고 가능하면 `CONCURRENTLY`를 사용한다.
- transaction 밖에서만 가능한 DDL은 migration metadata에 명시하고 별도 복구 단계를 둔다.
- migration에서 외부 API를 호출하지 않는다.

### 9.3 Dry-run과 preflight

`preflight.sh`는 production을 변경하지 않고 다음을 수행한다.

1. 현재 schema version과 checksum 조회
2. pending migration 목록과 예상 compatibility 출력
3. 최신 production backup을 임시 PostgreSQL instance에 restore
4. 임시 DB에 새 migration 적용
5. migration verification query 실행
6. 새 API·Agent integration smoke 실행
7. 구 API binary가 최소 읽기·종료 가능한지 호환 검사
8. 결과와 temporary DB checksum 기록
9. temporary environment 제거

production에 적용하기 전 이 결과가 성공해야 한다. 단순 SQL 출력만 dry-run으로 간주하지 않는다.

### 9.4 Runtime 적용

- migration은 app replica가 임의로 동시에 실행하지 않는다.
- 배포 script가 PostgreSQL advisory lock을 얻은 전용 migration command를 한 번 실행한다.
- lock을 얻지 못하면 배포를 실패시킨다.
- 적용 전 backup ID와 release manifest를 migration audit row에 기록한다.
- 적용 후 schema version과 verification query를 확인한 뒤 app traffic을 전환한다.
- migration 실패 시 transaction 가능한 변경은 rollback한다.
- 비transactional DDL 실패는 runbook에 정의된 검증 후 restore 또는 forward fix를 선택한다.

## 10. Backup 명세

### 10.1 Backup 대상

| 대상 | 방식 | 비고 |
|---|---|---|
| PostgreSQL | `pg_dump --format=custom --no-owner --no-acl` | 사용자, 일정, 기억, 대화, 감사 로그 포함 |
| attachments | manifest + archive | content hash와 크기 포함 |
| release metadata | JSON | image digest, schema, Compose config version |
| encrypted app config | 별도 암호화 archive | secret value가 아닌 복구에 필요한 설정 |

기본 backup에서 제외:

- `codex_home`의 ChatGPT credential
- Mac Worker private key와 Mac 파일
- temporary Agent workspace
- build cache와 container layer
- 로그 원문

Codex는 복구 후 device-code login을 다시 수행한다. Google refresh token은 DB에서 application encryption 상태로 backup되므로 token encryption key의 별도 안전한 복구본이 필요하다.

### 10.2 Backup 생성

1. backup ID와 시작 시각을 생성한다.
2. DB dump와 attachments manifest를 임시 directory에 만든다.
3. dump가 열리고 PostgreSQL archive metadata를 읽을 수 있는지 검사한다.
4. 각 파일 SHA-256과 크기를 manifest에 기록한다.
5. `age` public recipient로 bundle을 암호화한다.
6. 암호화된 bundle을 production volume과 물리적으로 다른 backup mount에 atomic move한다.
7. 최종 bundle hash와 release manifest digest를 기록한다.
8. 성공한 뒤에만 retention을 적용한다.
9. API 운영 상태에 마지막 성공 ID·시각·검증 여부를 보고한다.

기본 schedule은 Asia/Seoul 03:30 매일이며 배포 직전에는 별도 pre-deploy backup을 만든다.

기본 retention:

- 최근 일별 7개
- 주별 4개
- 월별 6개
- v0.1 release 직전 backup은 수동 삭제 전까지 보존

retention 삭제는 암호화 bundle만 대상으로 하며 현재 release의 유일한 검증 backup은 제거하지 않는다.

### 10.3 Encryption key

- backup container에는 `age` public recipient만 둔다.
- private identity는 production 서버 상시 filesystem에 두지 않는다.
- 사용자가 별도 encrypted removable storage 또는 password manager에 private identity를 보관한다.
- 복구 훈련 시에만 명시적으로 제공하고 끝난 뒤 임시 파일을 제거한다.
- token encryption key와 session signing key는 서로 다른 key다.

## 11. Restore 명세

### 11.1 자동 restore verification

`restore-verify.sh <backup-id>`는 production을 덮어쓰지 않고 격리된 Compose project에서 실행한다.

1. bundle hash와 manifest schema 검증
2. 사용자가 제공한 key로 임시 directory에 decrypt
3. 빈 PostgreSQL volume 생성
4. `pg_restore --clean`이 아닌 빈 DB restore 수행
5. attachments hash 검증
6. schema version과 row-level invariant query 실행
7. API를 read-only verification mode로 시작
8. 로그인 fixture가 아닌 내부 smoke credential로 health·핵심 repository 조회
9. Calendar 외부 쓰기와 Agent tool 실행은 차단
10. 결과 report에 backup ID, release digest, row count, 오류 기록
11. 격리 environment와 평문 임시 파일 제거

검증 invariant:

- FK·unique constraint 위반 없음
- 사용자 한 명 allowlist row 존재
- calendar sync state와 event 참조 일치
- active memory가 source를 하나 이상 가짐
- conversation/message 순서 일치
- pending approval의 대상 operation 존재
- audit log sequence와 sync sequence가 유효

### 11.2 Production 복구

복구는 다음 순서로 수행한다.

1. incident와 선택한 backup ID 기록
2. production 쓰기 차단·client maintenance 상태 전환
3. 손상된 현재 DB도 별도 forensic snapshot으로 보존
4. 새 PostgreSQL volume에 backup restore
5. manifest와 일치하는 image digest로 isolated smoke
6. 필요한 forward migration 적용
7. invariant와 실제 Mac·폰 read smoke 확인
8. gateway가 새 DB를 사용하는 service로 전환
9. 쓰기 재개 후 Calendar 증분 sync와 Agent 상태 확인
10. 이전 손상 volume은 확인 전까지 삭제하지 않음

복구 중 Google Calendar에 pending write가 있었다면 idempotency key와 Google event version으로 재조정하며 무조건 다시 생성하지 않는다.

## 12. Observability 명세

### 12.1 Structured logging

Rust service는 `tracing` 기반 JSON log를 stdout에 출력한다.

필수 field:

- `timestamp`, `level`, `service`, `version`, `environment`
- `request_id`, `trace_id`
- hash 또는 opaque form의 `user_id`, `device_id`
- `agent_job_id`, `approval_id`, `worker_operation_id` when applicable
- `error_code`, `duration_ms`, `http_status`

로그 금지:

- access·refresh token, OAuth code, pairing secret
- 일정 제목·설명, 메시지·기억 원문
- 파일 본문과 patch 전체
- command stdout/stderr 전체와 environment value
- Authorization, Cookie header

Redaction은 logger 호출자의 선택이 아니라 공통 serializer layer에서 적용한다.

### 12.2 Metric

내부 network의 `/metrics`에 다음을 제공한다.

API:

- request count, status, latency histogram
- active WSS connection
- DB pool usage·timeout
- sync outbox backlog와 가장 오래된 event age

Calendar:

- 마지막 성공 sync age
- sync 성공·실패·410 resync count
- pending write·retry count

Agent:

- App Server process up
- process restart count
- job queue depth·oldest age
- turn 성공·실패·취소
- approval wait count

Memory:

- retrieval latency·result count
- no-result rate
- candidate create·accept·reject
- evaluation score는 build artifact에 기록하고 production label로 노출하지 않음

Worker:

- online node count
- reconnect count
- operation 상태별 count
- uncertain operation count

Operations:

- 마지막 backup 성공 age
- 마지막 restore verification 성공 age
- disk free bytes·inode
- certificate expiry remaining
- running release/schema version

사용자 ID, project ID, query text처럼 cardinality가 높거나 민감한 값은 metric label로 쓰지 않는다.

### 12.3 Health endpoint

| Endpoint | 의미 |
|---|---|
| `/health/live` | process event loop가 살아 있음; 외부 dependency 검사 안 함 |
| `/health/ready` | API가 인증·일정·기억 기본 요청을 받을 DB 상태인지 검사 |
| `/health/components` | 인증된 운영자에게 Calendar, Agent, backup, Worker 세부 상태 제공 |

Agent가 내려가도 API `/health/ready`는 성공할 수 있다. 대신 component status와 AI endpoint가 명확한 unavailable 응답을 반환한다.

### 12.4 Alert 조건

운영 상태 화면과 선택적 개인 알림에 다음을 표시한다.

- production API ready 실패
- Calendar 마지막 성공 sync가 허용 age 초과
- Agent process 반복 재시작 또는 ChatGPT 재로그인 필요
- backup 실패 또는 마지막 검증 backup age 초과
- disk free 15% 미만 또는 inode 부족
- TLS certificate 만료 임박
- `uncertain` Worker operation 존재
- migration checksum mismatch

동일 원인 알림은 deduplicate하고 복구 시 resolved event를 남긴다.

## 13. 보안 강화 명세

### 13.1 Application

- Google account allowlist를 모든 login exchange에서 재검증한다.
- access token은 짧게, refresh token은 기기별 rotation·reuse detection을 적용한다.
- 모든 쓰기 API는 인증 guard, ownership, body schema, length, rate limit, idempotency를 검증한다.
- 승인 API는 expected state와 action hash를 compare-and-set한다.
- native app bearer auth에 cookie 기반 CSRF surface를 추가하지 않는다.
- OAuth callback state·PKCE·redirect URI를 엄격히 검증한다.
- OpenAPI의 security requirement와 실제 middleware를 contract test한다.

### 13.2 Network

- gateway는 public interface에 bind하지 않는다.
- TLS certificate 검증을 client에서 비활성화할 수 없게 한다.
- DB, Agent internal API, metrics는 Compose internal network만 사용한다.
- PostgreSQL host port를 publish하지 않는다.
- Mac Worker는 outbound WSS만 사용한다.
- server firewall은 필요한 사설 ingress와 Google/OpenAI/GHCR outbound만 허용하는 것을 목표로 한다.

### 13.3 Container

- non-root, read-only rootfs, `no-new-privileges`, `cap_drop: ALL`
- privileged, host PID/network, Docker socket mount 금지
- base image digest pin과 최소 runtime package
- application write directory를 명시적 volume·tmpfs로 제한
- health command에 credential을 넣지 않음
- Agent와 API secret·volume 분리

### 13.4 Secret

- secret inventory에 소유자, 용도, 위치, 회전 절차를 기록한다.
- repository에는 `.env.example`만 두고 실제 값은 두지 않는다.
- Google token encryption key, session signing key, DB password, TLS key를 분리한다.
- 로그·diagnostic bundle에서 secret pattern을 다시 scan한다.
- rotation은 이전 key와 새 key의 제한된 overlap을 지원하고 완료 후 이전 key를 폐기한다.
- ChatGPT auth store 문제는 복사로 해결하지 않고 runbook에 따라 재로그인한다.

### 13.5 데이터와 개인정보

- 민감 원문을 log·metric에 남기지 않는다.
- backup은 암호화된 상태에서만 보존·이동한다.
- diagnostics는 기본적으로 config key, count, status만 포함한다.
- 운영자용 data query도 read-only script와 목적별 최소 column을 사용한다.
- 기기·Worker revoke가 기존 refresh/session·pending operation에 즉시 반영되는지 테스트한다.

## 14. Rollback 명세

### 14.1 Release history

환경별 `release_history`에 다음을 기록한다.

- release ID와 manifest checksum
- image digest
- source commit
- schema version before/after
- backup ID
- 시작·종료 시각과 결과
- 실행한 operator
- smoke·restore verification report ID
- rollback target

### 14.2 Application-only rollback

조건:

- schema가 이전 binary의 compatibility range 안에 있음
- migration이 additive하거나 이전 binary가 새 field를 무시함
- 새로운 client write가 이전 binary에서 손상되지 않음

절차:

1. 새 쓰기를 잠시 차단한다.
2. 이전 release manifest의 image digest를 로드한다.
3. 이전 binary의 schema compatibility check를 실행한다.
4. Compose를 이전 digest로 갱신한다.
5. health·핵심 read/write smoke를 확인한다.
6. 쓰기를 재개하고 rollback release history를 남긴다.

### 14.3 Database restore rollback

다음 경우 application-only rollback을 하지 않는다.

- destructive migration이 적용됨
- 이전 binary가 schema를 읽지 못함
- migration 일부 실패로 data invariant가 깨짐
- 새 release write를 이전 binary가 안전하게 해석할 수 없음

이 경우 M7.11의 새 volume restore 절차를 사용한다. 기존 DB volume을 즉시 덮어쓰거나 삭제하지 않는다.

### 14.4 자동 rollback 제한

- container health failure만으로 DB restore를 자동 실행하지 않는다.
- image rollback은 migration compatibility가 manifest로 증명될 때만 자동 제안할 수 있다.
- Calendar write, command, patch 같은 외부 side effect는 rollback으로 되돌렸다고 가정하지 않는다.
- 외부 side effect는 audit·idempotency record로 재조정한다.

## 15. Runbook 명세

각 runbook은 전제 조건, 확인 command, 예상 출력, 변경 단계, 성공 확인, 중단 조건, rollback을 포함한다. secret 실제 값과 개인 hostname은 넣지 않는다.

필수 runbook:

- 새 서버에 staging 설치
- staging에서 production 승격
- app-only rollback
- DB backup 수동 실행과 상태 확인
- 격리 restore verification
- production DB 복구
- Codex device-code 재로그인
- Google Calendar 재연동과 full resync
- 분실한 기기·Mac Worker revoke
- session signing·token encryption key rotation
- disk 부족과 corrupt backup 대응
- Agent 반복 crash와 일정 API 분리 운영
- diagnostic bundle 생성과 민감정보 검사

## 16. Staging 검증 시나리오

### 16.1 서버

1. 빈 staging volume에 Compose 배포
2. migration 적용과 재실행 idempotency
3. container 재시작 후 데이터·Codex thread read model 유지
4. Agent container 종료 중 일정·할 일·기억 API 정상
5. Google API 장애·410 resync·token refresh 실패 표시
6. OpenAI 장애와 ChatGPT 재로그인 필요 상태 표시
7. disk·DB connection·WSS 장애 주입 후 복구
8. backup 생성과 격리 restore verification
9. 이전 digest application rollback
10. schema 비호환 상황에서 DB restore rollback

### 16.2 Mac 앱

1. staging hostname과 certificate 확인
2. Google 로그인·로그아웃·session revoke
3. 일정 CRUD와 Calendar 반영
4. AI streaming·cancel·재연결
5. 기억 생성·후보 승인·무효화
6. Mac Worker pairing·patch·command 승인
7. 서버 배포 중 offline cache와 재동기화
8. server가 client API range를 벗어났을 때 안전한 안내

### 16.3 개인 휴대폰

1. 사설 네트워크에서 staging 접속
2. system browser Google 로그인
3. Mac이 꺼진 상태에서 오늘 일정·AI 사용
4. 네트워크 단절 중 최근 일정 조회와 pending 변경 생성
5. 재연결 후 중복 없이 Calendar 반영
6. background/foreground 중 AI stream 복구
7. 기억 후보와 patch·command 승인 중복 처리 방지
8. 앱 재설치와 이전 device session revoke
9. production과 staging secure storage·OAuth callback 분리

## 17. v0.1 Release gate

다음 항목은 모두 증거 링크 또는 report ID를 가져야 한다.

### 17.1 자동 품질

- Rust formatter, clippy, unit·integration test 통과
- frontend lint, typecheck, test 통과
- OpenAPI·event·worker·Codex schema drift 없음
- database migration·compatibility test 통과
- M5 memory eval threshold 통과
- desktop, iOS, Android build smoke 통과
- API·Agent multi-architecture image smoke 통과
- Critical/High 보안 blocker 없음
- Compose rendered config 검사 통과

### 17.2 데이터와 운영

- production 직전 backup ID 존재
- 해당 backup의 격리 restore verification 성공
- token encryption key와 backup decrypt key의 별도 복구 가능성 확인
- migration preflight report 성공
- staging과 production volume·secret·hostname 분리 확인
- 이전 release digest와 rollback manifest 보존
- disk 여유와 certificate 유효성 확인

### 17.3 기능 회귀

- Mac이 꺼져 있어도 휴대폰 일정 조회
- 휴대폰 일정 변경이 Google과 Mac에 반영
- offline cache와 재연결 idempotency
- ChatGPT 구독 기반 Agent streaming
- Agent 장애 중 핵심 API 정상
- 현재 유효한 기억과 출처 조회
- 승인 전 patch·command 미실행
- Mac Worker offline·revoke 정확성

### 17.4 문서와 보안

- 배포·rollback·backup·restore runbook 실제 수행
- hardcoded secret과 민감정보 log 없음
- device·Worker revoke 절차 검증
- production OAuth redirect와 allowlist 확인
- Compose에 Docker socket, privileged, public DB port 없음
- 알려진 예외는 이유, 위험, 완화, 재검토 조건을 기록

하나라도 blocker이면 `v0.1.0` tag를 만들지 않는다.

## 18. Release와 검증 기록

출시가 확정되면 다음을 보존한다.

- Git tag `v0.1.0`
- release manifest와 checksum
- image digest·SBOM·provenance
- migration preflight·적용 report
- backup·restore verification report
- staging Mac·휴대폰 checklist
- security scan 결과와 승인된 예외
- production smoke와 현재 release status
- 이전 release rollback target

Release note에는 사용자 관점 변경, known limitation, 필요한 재로그인·migration 유무를 적고 내부 token·hostname·구체 경로는 노출하지 않는다.

## 19. 구현 작업 분해

1. CI quality·integration·contract workflow 구현
2. security scan·license·secret·image scan 구현
3. multi-architecture Dockerfile과 GHCR digest publish 구현
4. release manifest schema·generator·검증 구현
5. Compose base·staging·production overlay와 secret 경계 구현
6. server preflight·deploy·promote script 구현
7. migration metadata·compatibility·dry-run restore 환경 구현
8. encrypted backup·retention·status reporting 구현
9. isolated restore verification과 invariant query 구현
10. structured log·redaction·metric·component health 구현
11. release history와 application/DB rollback 구현
12. 필수 운영 runbook 작성·실행 검증
13. staging 장애 주입과 Mac·휴대폰 회귀 수행
14. v0.1 release gate report 생성

각 작업은 formatter, lint, test, build와 관련 schema·운영 문서 갱신을 포함한다.

## 20. 완료 기준

- PR에서 formatter, lint, test, schema, migration, security gate가 재현 가능하게 실행된다.
- API·Agent image가 `amd64`와 `arm64` digest로 생성되고 `latest` 없이 배포된다.
- staging과 production의 DB, volume, secret, OAuth, hostname이 분리된다.
- 새 release가 migration preflight와 backup 없이는 production에 적용되지 않는다.
- PostgreSQL과 attachments backup을 빈 격리 환경에 복구하고 invariant를 통과한다.
- ChatGPT credential과 Mac private key가 backup·log·image에 포함되지 않는다.
- API, Calendar, Agent, Memory, Worker, backup, disk 상태를 원문 노출 없이 확인할 수 있다.
- 이전 digest로 application rollback이 가능하고 schema 비호환 시 새 DB volume restore가 가능하다.
- Mac과 개인 휴대폰에서 전체 release gate를 통과한다.
- 필수 runbook을 문서대로 실제 수행할 수 있다.
- blocker가 없고 증거가 연결된 경우에만 `v0.1.0` release를 만든다.
