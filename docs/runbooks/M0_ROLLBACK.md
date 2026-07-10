# M0 staging rollback runbook

## 범위

이 절차는 API, Agent, gateway를 이전 image digest로 되돌리는 application-only rollback이다. PostgreSQL과 named volume을 교체하거나 삭제하지 않는다.

## 실행 조건

- 이전 image가 현재 schema version을 읽을 수 있다.
- 실패 release가 destructive migration을 실행하지 않았다.
- 실패한 candidate 배포 복구에는 `current.env`, 이미 성공한 현재 release의 되돌리기에는 `previous.env`가 존재한다.
- rollback 전 현재 장애와 선택한 release file을 verification record에 남겼다.

조건을 만족하지 않으면 자동 rollback을 실행하지 않는다. M7 backup/restore 절차 또는 forward fix가 필요하다.

## 실행

실패한 candidate 배포를 마지막 검증 성공 release로 복구:

```bash
./scripts/rollback-staging.sh \
  "$HOME/.config/jimin-os/staging.env" \
  current
```

이미 성공 기록된 현재 release를 그 이전 성공 release로 되돌리기:

```bash
./scripts/rollback-staging.sh \
  "$HOME/.config/jimin-os/staging.env" \
  previous
```

특정 성공 release:

```bash
./scripts/rollback-staging.sh \
  "$HOME/.config/jimin-os/staging.env" \
  "$HOME/.local/state/jimin-os/jimin-os-staging/releases/<release>.env"
```

script는 다음을 확인한다.

1. rollback target의 API, Agent, gateway가 모두 digest reference인지 확인
2. image pull
3. `--no-build` Compose 갱신
4. TLS health와 API/Codex/non-root smoke
5. 성공 target을 current release로 기록

Target은 생략할 수 없다. `current`, `previous`, 절대 경로 중 하나를 명시해야 하며 상대 경로와 모호한 기본값은 거절한다. Target이 이미 `current`와 같으면 실패 candidate 복구로 보고 `previous`를 유지한다. 다른 target을 적용하면 적용 전 `current`를 새 `previous`로 보존한다.

## 검증

- `/health/live`와 `/health/ready`가 TLS 검증 아래 `200`이다.
- 실행 중 container의 image digest가 target과 같다.
- PostgreSQL volume과 `codex_home` volume ID가 rollback 전후 같다.
- 일정 API가 아직 없는 M0에서는 baseline schema version이 유지된다.
- Agent 오류가 API readiness를 실패시키지 않는다.

## 중단 조건

- target image가 registry에 없음
- schema compatibility 불명
- secret·volume path가 현재 환경과 다름
- health 실패 원인이 DB corruption 또는 disk 장애임
- rollback 중 새 migration이 필요함

이 경우 반복 실행하지 않고 incident 기록과 현재 volume을 보존한다. `docker volume rm`, `down --volumes`, `git reset`, database drop은 이 runbook의 일부가 아니다.
