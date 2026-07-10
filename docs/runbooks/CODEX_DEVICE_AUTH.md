# Codex device-code 인증 runbook

## 목적

headless 로컬 서버의 Agent가 사용하는 `codex_home` volume에 ChatGPT 인증을 저장한다. 앱 client와 API에는 이 credential을 전달하지 않는다.

## 전제 조건

- Agent image가 pinned `@openai/codex@0.144.1`을 포함한다.
- gateway/API와 무관하게 Agent container를 중지·시작할 수 있다.
- 운영 host에 `jq`가 설치되어 있다.
- 사용자는 공식 OpenAI device-code page를 개인 browser에서 직접 연다.
- 화면 공유, recording, command transcript를 끈다.

## 로그인

Staging:

```bash
./scripts/codex-device-auth.sh \
  staging \
  "$HOME/.config/jimin-os/staging.env"
```

Local:

```bash
./scripts/codex-device-auth.sh local deploy/env/local.env.example
```

script 동작:

1. 실행 중 Agent를 중지해 `CODEX_HOME` 동시 쓰기를 막는다.
2. 같은 `codex_home` volume을 mount한 one-shot Agent image를 non-root로 실행한다.
3. `codex login --device-auth`를 호출한다.
4. 사용자가 browser에서 일회용 code를 승인한다.
5. `codex login status`를 실행하되 credential 원문을 출력하지 않는다.
6. 원래 실행 중이던 Agent를 다시 시작한다.

일회용 code도 credential로 취급한다. chat, issue, screenshot, verification log에 기록하지 않는다.

## 검증

Agent의 `serve` process와 probe용 Codex App Server가 같은 `CODEX_HOME`에 동시에 접근하면 안 된다. 아래 script는 Agent의 원래 실행 상태를 확인하고, `compose stop agent`로 단일 소유권을 만든 뒤 account와 turn probe를 차례로 실행한다. 종료·실패·interrupt 시 `trap`이 원래 실행 중이던 Agent를 복구하며, 원래 중지 상태였다면 중지 상태를 유지한다.

Staging:

```bash
./scripts/verify-codex-probes.sh \
  staging \
  "$HOME/.config/jimin-os/staging.env"
```

Local:

```bash
./scripts/verify-codex-probes.sh local deploy/env/local.env.example
```

검증 script의 핵심 순서는 다음과 같다. 이미지에 포함된 prompt는 개인 데이터가 없는 고정 fixture이며 운영자가 임의 경로를 대입하지 않는다.

```bash
docker compose <compose-args> stop agent
docker compose <compose-args> run --rm --no-deps agent probe account
docker compose <compose-args> run --rm --no-deps agent \
  probe turn \
  --model gpt-5.4 \
  --prompt-file /opt/jimin-agent/fixtures/generic-prompt.txt
docker compose <compose-args> up -d --no-deps --wait --wait-timeout 60 agent # 원래 실행 중이었을 때만
```

판정 기준:

1. device auth 단계의 `codex login status`가 로그인 상태를 반환한다.
2. 검증 script가 account probe의 JSON을 log에 출력하지 않고 `authenticated=true`, `accountType=chatgpt`, `runtimeState=ready`를 모두 강제한다. API key account나 `authRequired` 상태는 실패다.
3. turn probe가 `completed`, 하나 이상의 delta, 하나 이상의 authoritative agent message item과 content-free size/hash metadata를 반환한다. 검증 script는 prompt 본문이나 response 본문을 출력하지 않는다.
4. script 종료 후 원래 실행 중이던 Agent가 다시 기동하고 bounded wait 안에 health를 통과한다.
5. API `/health/live`, `/health/ready`는 로그인 전후 모두 동일하게 정상이다.

일반 log level에 prompt, response, token, auth file path를 남기지 않는다.

## 실패

- `auth_required`: 같은 volume으로 device auth를 다시 실행한다.
- account에서 device auth를 제공하지 않음: [credential recovery](CODEX_CREDENTIAL_RECOVERY.md)의 공식 fallback만 사용한다.
- `incompatible`: 인증을 반복하지 말고 Codex version/schema mismatch를 해결한다.
- `unsupportedAccount`: API key 인증을 제거하고 ChatGPT device auth로 다시 로그인한다.
- volume permission 오류: container를 root로 실행하지 말고 volume ownership과 service UID를 확인한다.
- Agent crash: API health를 유지하고 Agent만 중지한 뒤 진단한다.
