# ADR-0001. Codex App Server 호환성 경계

- 상태: Accepted
- 결정일: 2026-07-10
- 적용 단계: M0, M4

## 배경

Jimin OS는 로컬 서버에서 ChatGPT 구독 계정으로 Codex를 실행하되, 일정 API와 개인 데이터 저장소가 Codex process의 상태에 종속되지 않아야 한다. App Server protocol은 Codex 버전마다 생성되는 schema와 함께 바뀔 수 있고 WebSocket transport는 아직 안정 계약이 아니다.

## 결정

1. Rust `agent` process가 Codex App Server를 child process로 실행한다.
2. transport는 기본 `stdio`의 newline-delimited JSON만 사용한다.
3. `experimentalApi` capability를 보내지 않고 stable API만 사용한다.
4. 연결마다 `initialize` 성공 응답을 확인한 뒤 `initialized` notification을 한 번 보낸다.
5. App Server request/notification type은 `codex-client` adapter 안에 가두고 Jimin OS domain type으로 변환한다.
6. 지원하는 Codex CLI 버전과 해당 버전으로 생성한 JSON/TypeScript schema를 저장소에 고정한다.
7. 시작할 때 실제 `codex --version`을 검사하고 지원 목록에 없으면 Agent를 `incompatible`로 둔다.
8. Agent 실패와 인증 실패는 일정 API의 liveness/readiness를 실패시키지 않는다.
9. headless 서버 로그인은 `codex login --device-auth`를 우선하며, 인증 cache는 Agent 전용 volume에 둔다.
10. Agent serve는 `account/read`의 `accountType=chatgpt`만 `ready`로 인정한다. API key account는 `unsupportedAccount` terminal state로 두고 turn을 실행하지 않는다.

## 최초 호환성 기준

| 항목 | 값 |
|---|---|
| Codex CLI | `0.144.1` |
| 생성 schema | `schemas/codex/0.144.1` |
| transport | `stdio` JSONL |
| API surface | stable only |
| 로컬 생성 binary | `aarch64-apple-darwin` |
| SHA-256 | `29915529b97697def1a957b0505e770aa6a45744435d62fc263e98d7619e167a` |
| npm integrity | `sha512-Xir1zqPfpenhdoAoshN53uonzbBXj18COyzRkFlVZpSNyEl5XtkuYu9oddELePFN7K/0sXUcSO34Ad5IeCXPbw==` |

Linux Agent image의 binary checksum과 image digest는 image build 후 별도 검증 기록에 추가한다. macOS checksum을 Linux binary 검증값으로 재사용하지 않는다.

최초 검토에 사용한 0.142.3은 schema 생성과 handshake에는 성공했지만 2026-07-10의 실제 account model이 더 새로운 Codex를 요구해 turn이 HTTP 400으로 실패했다. 0.144.1에서는 compatibility와 `account/read`가 통과하고 provider가 해당 version을 수용하는 단계까지 확인해 호환성 기준으로 채택했다. 비민감 turn의 최종 완료는 adapter가 `usageLimitExceeded`를 안전하게 분류한 상태에서 계정 한도 복구 후 다시 검증한다.

## protocol 성공 조건

다음 흐름이 한 연결에서 순서대로 성공해야 호환되는 것으로 판정한다.

```text
initialize
→ initialized
→ account/read
→ thread/start
→ turn/start
→ item/agentMessage/delta*
→ item/completed
→ turn/completed(status = completed)
```

`item/completed`의 item을 최종 item 상태로 사용한다. `turn/completed`가 `failed` 또는 `interrupted`이면 성공으로 간주하지 않는다. 알 수 없는 notification은 무시할 수 있지만 알 수 없는 response ID, 최대 크기를 넘은 line, malformed JSON과 stdout EOF는 protocol 오류다.

## 결과

- 모바일과 Mac client는 App Server에 직접 연결하지 않고 Jimin OS API/stream 계약만 사용한다.
- Codex 업그레이드 시 schema 재생성, metadata 갱신, fixture test와 실제 turn smoke test가 필요하다.
- App Server 기능을 직접 노출하는 것보다 adapter 유지 비용이 생기지만, Codex 변경이 일정·기억 domain으로 전파되는 범위를 제한한다.
- WebSocket 또는 Unix socket transport는 별도 ADR 없이는 도입하지 않는다.

## 검증할 증거

- [x] Codex CLI 0.144.1 stable schema와 metadata 생성
- [ ] macOS에서 adapter handshake fixture 통과
- [ ] Linux Agent image에서 version/checksum 확인
- [ ] fresh Agent volume에서 `auth_required` 확인
- [ ] device-code 로그인 후 `account/read`가 `chatgpt` 반환
- [ ] 실제 한 turn의 delta와 completed 수신
- [ ] Agent 재시작 뒤 인증 유지
- [ ] child crash와 지원하지 않는 version 회귀 테스트

## 근거

- [Codex App Server](https://developers.openai.com/codex/app-server/)
- [Codex authentication](https://developers.openai.com/codex/auth/)
