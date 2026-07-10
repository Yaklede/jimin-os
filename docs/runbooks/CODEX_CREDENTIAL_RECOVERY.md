# Codex credential 복구 runbook

## 원칙

ChatGPT credential은 backup artifact로 복원하지 않는다. 기본 복구는 같은 `codex_home` volume에서 공식 device-code login을 다시 수행하는 것이다.

다음 행동은 금지한다.

- `auth.json`을 Git, issue, chat, 일반 backup에 첨부
- credential을 environment variable이나 command argument로 전달
- Agent와 API가 같은 credential volume을 mount
- 인증 오류를 해결하기 위해 TLS 검증 또는 Codex version 검사를 끔
- 확인 없이 `codex_home` volume 삭제

## 1차 복구: 재인증

1. API health가 Agent와 독립적으로 정상인지 확인한다.
2. Agent 오류 상태와 Codex version만 기록한다.
3. [device auth runbook](CODEX_DEVICE_AUTH.md)을 다시 실행한다.
4. account, turn probe와 Agent restart 지속성을 확인한다.

대부분의 만료·폐기 상황은 기존 volume에 재로그인하는 것으로 복구한다.

## 2차 복구: 공식 browser login cache import

device-code가 계정 또는 workspace 정책에서 제공되지 않을 때만 사용한다.

1. 신뢰하는 Mac에서 동일한 pinned Codex version과 별도 임시 `CODEX_HOME`을 사용한다.
2. 공식 `codex login` browser flow를 완료한다.
3. `codex login status`로 account를 확인한다.
4. 서버 Agent를 중지한다.
5. 임시 `auth.json`을 stdin으로 non-root one-shot Agent container에 전달한다. command line과 log에는 내용이 나타나지 않게 한다.
6. destination은 기존 Agent의 `$CODEX_HOME/auth.json`, mode는 `0600`, owner는 Agent UID/GID여야 한다.
7. one-shot container 종료 후 `./scripts/verify-codex-probes.sh`로 Agent를 중지한 단일 소유자 상태에서 account와 고정 fixture turn probe를 실행한다.
8. Mac의 임시 `CODEX_HOME`을 일반 backup과 동기화 대상에서 제거한다.

전달 형태 예시는 다음과 같다. 실행 전에 Compose 인자와 source file을 운영자가 직접 확인한다.

```bash
docker compose <staging-compose-args> run --rm -T --no-deps \
  --entrypoint /bin/sh agent \
  -c 'umask 077; mkdir -p "$CODEX_HOME"; cat > "$CODEX_HOME/auth.json"; chmod 600 "$CODEX_HOME/auth.json"' \
  < '/trusted/temporary/CODEX_HOME/auth.json'
```

이 명령은 service의 non-root `user`를 그대로 사용한다. `--user root`를 추가하지 않는다.

## 손상된 volume

auth cache가 아닌 volume 자체가 손상됐다고 의심되면 다음을 지킨다.

- 현재 volume을 삭제하지 않고 Agent를 중지한다.
- volume 이름, mount, filesystem 오류를 민감정보 없이 기록한다.
- 새 이름의 replacement volume에서 재로그인을 먼저 검증한다.
- thread 원본이 필요하면 credential file과 분리해서 복구 범위를 결정한다.
- replacement가 검증된 뒤에도 기존 volume 삭제는 별도 사용자 승인으로 수행한다.

복구 성공은 account 상태만이 아니라 실제 비민감 turn probe와 container restart 후 지속성으로 판정한다.
