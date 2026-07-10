# M0 검증 기록

## 식별 정보

- 검증일:
- 검증자:
- Git commit SHA:
- API image digest:
- Agent image digest:
- Gateway image digest:
- PostgreSQL image digest:
- Codex version:
- Codex package integrity 확인: PASS / FAIL
- Compose project:
- 환경: local / staging

실제 token, OAuth code, 일정·대화 원문, 개인 파일, private key, `auth.json`은 이 문서에 넣지 않는다.

## 환경 inventory

- Server distribution/kernel:
- Server architecture:
- Docker/Compose:
- CPU/memory/disk:
- Timezone/NTP:
- Service UID/GID:
- Private hostname label:
- Private network path:
- Phone platform/OS:
- Mac version:

## 자동 검증

| Command | Result | Evidence summary |
|---|---|---|
| `bash -n scripts/*.sh scripts/lib/*.sh` | PASS / FAIL | |
| `./scripts/validate-compose.sh local ...` | PASS / FAIL | |
| `./scripts/validate-compose.sh staging ...` | PASS / FAIL | |
| API image build | PASS / FAIL | |
| Agent image build | PASS / FAIL | |
| Gateway image build | PASS / FAIL | |
| `cargo fmt --check` | PASS / FAIL | |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS / FAIL | |
| `cargo test --workspace` | PASS / FAIL | |
| Backend Ultrawork harness | PASS / FAIL | |

## Compose 보안

- [ ] gateway만 host port를 publish한다.
- [ ] gateway, API, Agent, PostgreSQL이 non-root다.
- [ ] 모든 service root filesystem이 read-only다.
- [ ] Docker socket과 host root mount가 없다.
- [ ] `codex_home`은 Agent만 mount한다.
- [ ] PostgreSQL은 internal network에만 있다.
- [ ] 실제 secret이 config, image history, inspect, log에 없다.
- [ ] image tag와 base image에 `latest`가 없다.

## Health와 장애 격리

| Scenario | Expected | Result |
|---|---|---|
| 정상 | live 200, ready 200 | PASS / FAIL |
| DB down | live 200, ready 503 | PASS / FAIL |
| migration mismatch | live 200, ready 503 | PASS / FAIL |
| Agent down | API live/ready 유지 | PASS / FAIL |
| Agent 미인증 | `auth_required`, API 정상 | PASS / FAIL |
| Codex version 불일치 fixture | `incompatible`, API 정상 | PASS / FAIL |

## TLS

- TLS mode: internal / files
- Certificate issuer/fingerprint summary:
- Mac hostname verification: PASS / FAIL
- Phone hostname verification: PASS / FAIL
- Untrusted CA rejection: PASS / FAIL
- Wrong hostname rejection: PASS / FAIL
- 검증 우회 옵션 사용 안 함: PASS / FAIL

## Codex App Server

- [ ] `codex --version`이 pinned version과 같다.
- [ ] npm integrity가 `deploy/versions.env`와 같다.
- [ ] fresh volume에서 `auth_required`를 확인했다.
- [ ] device-code 또는 공식 fallback 로그인이 완료됐다.
- [ ] `initialize → initialized → account/read`가 통과했다.
- [ ] 비민감 turn에서 delta와 completed를 받았다.
- [ ] Agent restart 뒤 인증이 유지됐다.
- [ ] child crash와 protocol error가 API readiness에 영향을 주지 않았다.

## Mac·개인 휴대폰

| Scenario | Mac | Phone | Evidence summary |
|---|---|---|---|
| Release-equivalent install | PASS / FAIL | PASS / FAIL | |
| TLS health | PASS / FAIL | PASS / FAIL | |
| Secure storage fake token | PASS / FAIL | PASS / FAIL | |
| System browser/deep link | PASS / FAIL | PASS / FAIL | |
| Background/foreground | PASS / FAIL | PASS / FAIL | |
| Network OFF/ON reconnect | PASS / FAIL | PASS / FAIL | |
| Force quit/relaunch | PASS / FAIL | PASS / FAIL | |
| SQLite create/migrate/RW | PASS / FAIL | PASS / FAIL | |

## Rollback

- Rollback source release file:
- Target image digests:
- PostgreSQL volume ID before/after:
- Codex volume ID before/after:
- Post-rollback health: PASS / FAIL
- Database schema compatibility 확인: PASS / FAIL

## 결과

- Gate result: PASS / FAIL
- 실패 항목:
- 보안 예외와 승인자:
- 후속 작업:
- 민감정보 검토 완료: PASS / FAIL

실패 항목은 완료로 표시하지 않는다. 증거가 없는 항목은 PASS가 아니라 미검증으로 기록한다.
