# M0 local 배포 runbook

## 목적

개발 Mac의 Docker에 `gateway`, `api`, `agent`, `postgres`를 같은 Compose 계약으로 실행하고 TLS health smoke를 통과한다. 이 절차는 실제 secret을 repository에 만들지 않는다.

## 전제 조건

- Docker daemon과 Compose v2가 실행 중이다.
- Rust·Node build 산출물은 Docker multi-stage build 안에서 생성한다.
- `Cargo.lock`과 source가 현재 commit에 존재한다.
- `deploy/env/local.env.example`의 hostname과 port가 사용 가능하다.

## Secret 준비

기본 경로는 Git에서 제외되는 `deploy/secrets/local`이다. password는 password manager 또는 CSPRNG로 만들고 shell argument, issue, chat, log에 붙이지 않는다.

필수 파일:

```text
deploy/secrets/local/postgres_password
deploy/secrets/local/api_database_url
```

두 파일은 mode `0600` 또는 `0400`이어야 한다. `api_database_url`은 다음 형식이고 password는 URL encoding해야 한다.

```text
postgres://jimin_api:<encoded-password>@postgres:5432/jimin_os
```

`JIMIN_TLS_MODE=internal`에서는 TLS key 파일을 만들지 않는다. `files`로 바꾸면 `deploy/secrets/README.md`의 certificate 파일 두 개를 추가한다.

## 배포

```bash
./scripts/validate-compose.sh local deploy/env/local.env.example
./scripts/deploy-local.sh deploy/env/local.env.example
```

배포 script는 다음을 수행한다.

1. secret 파일의 존재, placeholder, 권한 확인
2. Compose 보안 계약 검증
3. pinned base image로 API, Agent, gateway build
4. 네 service 기동과 health 대기
5. internal CA public root export
6. TLS health, API probe, Codex version, non-root, Docker socket 부재 확인
7. 성공한 image reference를 user state directory에 기록

## Internal CA 신뢰

export된 public root는 다음 경로에 있다.

```text
${XDG_STATE_HOME:-$HOME/.local/state}/jimin-os/jimin-os-local/ca/root.crt
```

root certificate fingerprint를 확인한 뒤 macOS Keychain의 별도 test keychain 또는 System keychain에서 신뢰한다. 개인 휴대폰 설치는 [ADR-0003](../adr/ADR-0003-deployment-tls.md)의 실기기 검증에 포함한다.

`curl -k`, browser의 인증서 경고 무시, 앱의 certificate validation 비활성화는 사용하지 않는다.

수동 smoke:

```bash
JIMIN_TLS_CA_FILE="$HOME/.local/state/jimin-os/jimin-os-local/ca/root.crt" \
  ./scripts/smoke-deployment.sh local deploy/env/local.env.example
```

## 중지와 재시작

일시 중지는 volume을 제거하지 않는다.

```bash
docker compose \
  --project-name jimin-os-local \
  --env-file deploy/versions.env \
  --env-file deploy/env/local.env.example \
  --file deploy/compose.yaml \
  stop
```

재시작은 `./scripts/deploy-local.sh`을 다시 실행한다. M0 검증 중 `docker compose down --volumes`, `docker volume prune`, `system prune`은 금지한다. 해당 명령은 PostgreSQL과 Codex 인증 상태를 제거한다.

## 실패 처리

- API가 unhealthy면 `jimin-api probe ready` 결과와 PostgreSQL health를 확인한다.
- `/health/live`는 성공하고 ready만 실패해야 DB 장애 계약과 일치한다.
- Agent 인증 전에도 API와 gateway health는 정상이어야 한다.
- Caddy CA가 바뀌었으면 원인을 확인하고 기기의 기존 신뢰를 무조건 우회하지 않는다.
- build 실패 시 base image digest와 `Cargo.lock` 일치 여부를 먼저 확인한다.

민감정보 없는 구조화 log만 사용한다. `docker inspect`와 `docker compose config`에 secret 원문이 보이면 즉시 실패로 기록한다.
