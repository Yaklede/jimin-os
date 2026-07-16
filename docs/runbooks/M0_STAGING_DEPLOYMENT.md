# M0 staging 배포 runbook

## 목적

로컬 Linux 서버에 registry digest로 고정된 image를 배포하고 Mac과 개인 휴대폰이 private hostname의 TLS를 검증하게 한다.

## 배포 전 확인

1. [환경 inventory](environment-inventory.md)의 Linux server 항목을 모두 채운다.
2. `amd64` 또는 `arm64` image manifest가 서버 architecture를 포함하는지 확인한다.
3. private DNS와 gateway firewall은 사설 network에서만 접근 가능하게 한다.
4. actual config는 repository 밖에 둔다. 예: `$HOME/.config/jimin-os/staging.env`.
5. actual secret은 repository 밖 또는 Git에서 제외된 directory에 mode `0600`으로 둔다.
6. `JIMIN_TLS_MODE`에 맞는 certificate 경로를 준비한다.
7. 현재 성공 release와 rollback target이 state directory에 있는지 확인한다.

`staging.env`는 `deploy/env/staging.env.example`을 바탕으로 만들되 다음을 실제 값으로 교체한다.

- private hostname과 bind port
- host Nginx가 TLS를 종료하는 구성에서는 gateway를 loopback port에만 bind하고,
  `JIMIN_SMOKE_PORT=443`, `JIMIN_SMOKE_TLS_MODE=public`,
  `JIMIN_SMOKE_RESOLVE_IP=127.0.0.1`로 외부 ingress를 검증한다.
- API, Agent, gateway의 `@sha256:<64-hex>` registry reference
- full 40-character source Git SHA
- secret directory
- 필요하면 smoke용 resolve IP

tag만 있는 image, `latest`, placeholder digest는 배포 script가 거절한다.

## Image build와 registry push

M7 CI가 준비되기 전 M0에서는 clean commit에서만 다음 script로 image를 push한다.

```bash
./scripts/build-staging-images.sh \
  ghcr.io/<owner> \
  linux/amd64,linux/arm64
```

로컬 서버 platform 하나만 검증할 때는 두 번째 인자를 `linux/amd64` 또는 `linux/arm64`로 제한할 수 있다. Script는 dirty worktree를 거절하고 API, Agent, gateway를 source SHA tag로 push한 뒤 registry manifest digest가 담긴 release env를 user state directory에 만든다.

생성된 digest를 actual `staging.env`에 복사한다. Tag를 복사하거나 image를 다시 build하지 않는다. Registry 로그인 credential은 Docker credential store에서 관리하고 script argument나 env file에 넣지 않는다.

## Secret

필수 파일과 형식은 `deploy/secrets/README.md`를 따른다. `api_database_url`의 host는 Compose service 이름 `postgres`다. 실제 password와 DSN을 environment file에 넣지 않는다.

## 사전 검증

```bash
./scripts/validate-compose.sh staging "$HOME/.config/jimin-os/staging.env"
```

다음을 별도로 확인한다.

```bash
docker buildx imagetools inspect '<api-image@sha256:digest>'
docker buildx imagetools inspect '<agent-image@sha256:digest>'
docker buildx imagetools inspect '<gateway-image@sha256:digest>'
```

명령 결과에 서버 platform이 있어야 한다. digest는 verification record에 기록한다.

## 배포

```bash
./scripts/deploy-staging.sh "$HOME/.config/jimin-os/staging.env"
```

script는 image를 다시 build하지 않고 지정 digest를 pull하며 `--no-build`로 기동한다. 성공 전에는 current release state를 바꾸지 않는다.

성공 후 확인:

- gateway만 host port를 publish한다.
- API·Agent·PostgreSQL은 non-root/read-only다.
- API ready는 PostgreSQL과 migration을 확인한다.
- Agent의 Codex version은 `deploy/versions.env`와 같다.
- Agent가 미인증이어도 API health는 정상이다.
- Mac과 휴대폰에서 TLS hostname 검증을 유지한다.

## ChatGPT 인증

Agent image와 volume이 준비된 뒤 [Codex device auth runbook](CODEX_DEVICE_AUTH.md)을 실행한다. ChatGPT credential은 image, config, log, verification 문서에 포함하지 않는다.

## 실패

- 배포 script가 smoke 전에 실패하면 current release state는 그대로다.
- 새 container가 올라왔지만 smoke가 실패하면 원인을 기록한 뒤 마지막으로 검증된 `current`를 명시해 복구한다.

```bash
./scripts/rollback-staging.sh \
  "$HOME/.config/jimin-os/staging.env" \
  current
```

- 이미 성공 기록된 현재 release를 그 이전 release로 되돌릴 때만 `previous`를 선택한다.
- migration이 이미 적용됐다면 image rollback 호환성을 먼저 확인한다. M0 baseline 이후의 destructive DB rollback은 자동화하지 않는다.
- `down --volumes` 또는 volume prune으로 해결하지 않는다.

완료 증거는 `docs/verification/M0_VERIFICATION_TEMPLATE.md`를 복사해 채운다.
