# M0 로컬 배포 검증 기록 — 2026-07-10

## 범위와 판정

- 검증 범위: 개발 Mac의 M0 container build, Compose 보안 계약, internal-CA TLS, API/DB health, 미인증 Agent, Codex pin, 배포 자동화
- 로컬 자동 검증 결과: **PASS**
- 전체 M0 완료 게이트: **FAIL — 미실행 필수 항목 존재**
- 미실행 필수 항목: 실제 Linux server 배포, 배포 Agent의 ChatGPT device auth와 실제 turn, Mac/개인 휴대폰 앱, 실기기 TLS, registry digest rollback

이 기록은 전체 M0 완료 선언이 아니다. 증거가 없는 항목을 PASS로 표시하지 않았다.

## 식별 정보

- 검증 기준 commit: `ee9848b6978113f6667958dae685897214d59414` 이후 작업 중인 working tree
- 검증 source bundle SHA-256: `01d9314be2e341d0889b7e1ec17d05446360ecbc1b677e3ff9c52f4a6a157f44` — nonignored tracked/untracked file의 path+content를 정렬해 계산하며 이 검증 기록 자체는 제외
- Compose project: 격리된 local project `jimin-os-local`
- 환경: local
- Codex: `@openai/codex@0.144.1`
- Codex npm integrity 확인: PASS — build가 `deploy/versions.env`의 exact integrity와 registry metadata 일치를 강제함
- API local image ID: `sha256:8659b2019430104493b9c8018a08c83455559cb65fc505e8aa9ad02d61d08320`
- Agent local image ID: `sha256:f2e9a8d8ee7a0f10d0801a54fe9caee11933423a0f9c85edbe5a8868b89f625f`
- Gateway local image ID: `sha256:abe97135f98f5698bef5b6e7739d621666198eb69614d843450d3993133c6d5f`
- PostgreSQL upstream manifest digest: `sha256:f3bd19c606e442c3d7bdfa8002e03fe260a1023351e0ea4598032022b68dd6e3`
- Registry release digest: 미실행

Local image ID는 registry manifest digest가 아니며 staging release 근거로 사용하지 않는다.

## 환경 inventory

- Mac: macOS 26.0.1 (`25A362`), arm64, 48 GiB memory
- Docker: 24.0.7
- Docker Compose: 2.23.3-desktop.2
- Rust: 1.95.0
- Node: 22.14.0
- pnpm: 11.5.0
- 실제 Linux server: 미확인
- 개인 휴대폰: 미확인
- local service user: gateway `65532:65532`, API `10001:10001`, Agent `1000:1000`, PostgreSQL `999:999`

## 자동 검증

| Command 또는 시나리오 | 결과 | 근거 요약 |
|---|---|---|
| `bash -n scripts/*.sh scripts/lib/*.sh` | PASS | 모든 배포·복구 script 문법 통과 |
| `./scripts/validate-compose.sh local ...` | PASS | 네 service, 보안 설정, secret 비노출, host port 경계 확인 |
| `./scripts/validate-compose.sh staging ...` | PASS | digest fixture로 staging merge와 `--no-build` 전제 확인 |
| `./scripts/test-deploy-state.sh` | PASS | rollback 상태 전이와 config/release/checked-in pin의 권위 우선순위 확인 |
| `./scripts/deploy-local.sh ...` | PASS | caller의 오염된 image/Codex env를 무시하고 build → health wait → TLS/API/Codex/non-root smoke → release state 기록 완료 |
| API multi-stage image build | PASS | pinned Rust/Debian base와 non-root runtime |
| Agent multi-stage image build | PASS | pinned Rust/Node/Codex, npm integrity, non-root runtime |
| Gateway image build | PASS | pinned Caddy, binary file capability 제거, non-root runtime |
| `cargo fmt --check` | PASS | workspace formatter 통과 |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | workspace lint 통과 |
| `cargo test --workspace` | PASS | 46개 test function(45 unit + 1 환경 연동 integration)과 doc tests 통과; 실제 PostgreSQL 연동은 별도 smoke로 확인 |
| Backend Ultrawork harness | PASS | 337 files scan, 실패 없음 |
| `./scripts/scan-secrets.sh` | PASS | tracked·untracked 전체 후보에서 key/token/credential file과 URL 패턴 검사, 값은 출력하지 않음 |
| tracked diff와 전체 nonignored text whitespace 검사 | PASS | 아직 untracked인 구현 파일까지 포함해 trailing whitespace와 conflict marker 없음 |
| local image history secret pattern 검사 | PASS | DSN, auth file, private-key marker, validation fixture 없음 |

## Compose와 runtime 보안

- [x] gateway만 host port를 publish한다.
- [x] gateway, API, Agent, PostgreSQL이 non-root로 실행된다.
- [x] 네 service root filesystem이 read-only다.
- [x] capability를 모두 제거하고 `no-new-privileges`를 적용한다.
- [x] Docker socket과 host root mount가 없다.
- [x] `codex_home`은 Agent만 mount한다.
- [x] PostgreSQL network는 internal이고 gateway가 직접 접근하지 않는다.
- [x] actual secret 대신 runtime secret file mount만 사용한다.
- [x] image와 base image에 floating `latest`가 없다.
- [x] Agent verification prompt는 image의 read-only `/opt/jimin-agent/fixtures/generic-prompt.txt`에 있으며 host fixture와 SHA-256이 일치한다.

## Health와 장애 격리

| Scenario | 기대 결과 | 결과 |
|---|---|---|
| 정상 local stack | TLS live 200, ready 200 | PASS |
| fresh PostgreSQL volume 동시 시작 | API live 선기동, background migration 후 ready 200 | PASS — DB 초기화 순서와 무관하게 재시작 없이 수렴 |
| fresh Agent volume | `authRequired`, API live/ready 정상 | PASS |
| Agent probe 중 `CODEX_HOME` 단일 소유자 | serve 중지, one-shot probe, 원래 실행 상태 복구 | PASS — 미인증 계정은 안전하게 거절됨 |
| DB down | live 200, ready 503 | PASS — 실행 중 단절과 API/gateway cold restart 모두 확인; DB 복구 후 ready 200 복귀 |
| migration mismatch | live 200, ready 503 | PASS — local PostgreSQL checksum 변조와 API/gateway cold restart로 재현, 원복 후 ready 200 복귀 |
| Agent down | API live/ready 유지 | PASS — Agent와 API 장애 경계 분리 확인 |
| Codex incompatible fixture | Agent `incompatible`, API 정상 | PASS — 지원 범위 밖 `0.142.3`을 거절하고 조회 가능한 terminal health state를 유지 |
| Codex child 3회 연속 종료 | health marker 제거, 1초/2초 제한 복구 후 예산 소진 종료 | PASS — container restart count `0`, 내부 crash budget이 단일 소유자임을 확인; 5분 안정 실행 후 연속 실패 횟수 reset은 unit test로 확인 |

## TLS

- TLS mode: internal CA
- trusted CA와 `localhost` hostname 검증: PASS — export된 public root를 `curl --cacert`로 사용
- TLS 검증 우회 옵션: 사용하지 않음
- Mac Keychain 설치와 앱 요청: 미실행
- 개인 휴대폰 CA 설치와 앱 요청: 미실행
- untrusted CA rejection: PASS
- wrong hostname rejection: PASS
- staging private hostname/certificate: 미실행

## Codex App Server

- [x] `codex --version`이 pinned `0.144.1`과 같다.
- [x] npm registry integrity가 pinned integrity와 같다.
- [x] fresh volume에서 Agent가 `authRequired` 상태로 계속 실행된다.
- [x] account probe는 content-free summary만 만들며 검증 script가 raw JSON을 log에 출력하지 않는다.
- [x] account 검증은 `authenticated=true`, `accountType=chatgpt`, `runtimeState=ready`를 모두 강제한다.
- [x] turn 검증은 고정 fixture와 `gpt-5.4`만 사용하고 completed, delta, authoritative message item, size/hash metadata를 강제한다.
- [x] probe가 실패해도 `trap`이 원래 실행 중이던 Agent를 복구한다.
- [x] 복구 trap은 Agent가 bounded wait 안에 실제 health를 통과해야 성공한다.
- [x] Agent serve는 인증된 `accountType=chatgpt`만 `ready`로 인정하고 다른 account type은 `unsupportedAccount`로 차단한다.
- [x] idle App Server notification 20,000건 fixture를 content-free로 drain해 stdout backpressure 없이 health를 유지한다.
- [x] 개발 Mac의 기존 ChatGPT 로그인으로 compatibility와 `account/read` probe가 통과한다.
- [x] 실제 turn이 현재 계정의 `turn_usage_limit_exceeded`를 원문 비노출 안전 오류로 반환한다.
- [ ] 실제 ChatGPT device-code 또는 공식 fallback login
- [ ] 사용량 복구 후 배포 Agent에서 인증된 실제 non-personal turn과 streaming 완료
- [ ] Agent restart 후 인증 지속성

M0 adapter는 단일 probe request를 순차 처리한다. 여러 in-flight request의 pending map, unknown notification 운영 metadata, `protocol_error`·`crashed`의 별도 조회 API는 M4 Agent Runtime에서 구현한다. M0에서는 bounded JSONL, idle drain, content-free failure log와 crash budget까지만 완료 범위로 본다.

## Staging·실기기·rollback

- registry multi-architecture build/push: 미실행
- Linux server digest deployment: 미실행
- previous digest application rollback: 미실행
- PostgreSQL/Codex volume ID rollback 전후 비교: 미실행
- Mac release-equivalent 앱: 미실행
- 개인 휴대폰 install, TLS, secure storage, deep link, lifecycle, SQLite: 미실행

## 후속 검증 순서

1. Linux server inventory와 private hostname/TLS 경로를 확정한다.
2. clean commit에서 multi-architecture image를 push하고 manifest digest를 기록한다.
3. staging 배포 후 device auth와 `verify-codex-probes.sh`를 실행한다.
4. DB/Agent/Codex 장애 격리와 image rollback을 실행한다.
5. Mac과 개인 휴대폰에서 TLS 및 lifecycle 검증을 완료한다.

민감정보 검토: PASS — token, OAuth code, prompt/response 원문, 개인 파일, private key, `auth.json`을 기록하지 않았다.
