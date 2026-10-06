# GPT-6.1 Sol 모델 카탈로그 갱신

- 날짜: 2026-10-06
- 범위: Codex 어댑터, Agent 카탈로그 동기화, 런타임 배포 pin, schema, 설정 화면 회귀 테스트
- 운영 배포: 이번 요청에서는 실행하지 않음

## 원인

Agent가 Codex 0.144.1에 고정되어 있었으며 카탈로그도 시작할 때 한 번만 저장했다. 확인한 0.157.0의 실제 목록에도 GPT-6.1 Sol이 없었다. UI에 임의로 모델명을 추가하는 것으로는 실행 호환성을 보장할 수 없다.

## 반영

- stable Codex 0.159.0과 검증한 npm integrity로 어댑터 및 Docker pin을 일치시켰다.
- stable JSON/TypeScript schema와 binary metadata를 저장했다. 역사적 schema는 그대로 유지했다.
- Agent가 작업 사이 15분마다 런타임 목록을 동기화한다. 실패하면 기존 snapshot을 유지하고 1분 뒤 재시도한다. 활성 turn을 중단하지 않는다.
- 민감정보 없는 `probe models` 진단을 추가했다.
- 배포용 비민감 turn probe의 기본 모델을 `gpt-6.1-sol`로 변경했다. `JIMIN_AGENT_PROBE_MODEL`로 명시적인 대체 모델을 지정할 수 있다.
- 공개 모델/추론 강도는 기존 동적 UI 계약으로 전달된다. 사용자 모델 선택은 덮어쓰지 않는다. 자동 선택은 런타임 기본값을 따른다.

## 검증 증거

| 검증 | 결과 |
|---|---|
| 0.159.0 실제 `model/list` | 성공: 8개 visible 모델, `gpt-6.1-sol` 포함 |
| GPT-6.1 Sol 추론 강도 | 런타임에서 low / medium / high / xhigh / max / ultra 반환 |
| 비민감 fixture 실제 turn | completed, 최종 메시지 1개, delta 23개, 응답 129 bytes, 재시도 0개 |
| 응답 hash | `f1a83f2cfa76f2d1f448ac15674ac69a279a0c372bcb2f3bd2cee6d8725e61ef` |
| Agent / codex-client 테스트 | 82 + 19개 통과 |
| 설정 화면 테스트 | 16개 통과: GPT-6.1 Sol 및 runtime ultra 선택 표시 포함 |
| Desktop TypeScript | 통과 |
| Rust formatter / clippy `-D warnings` | 통과 |
| Agent release build | 통과 |
| 배포 state 회귀 / shell syntax | 통과 |
| `git diff --check` | 통과 |
| Backend Ultrawork scoped harness | 통과 (스캐너 지원 대상 4개; Rust는 formatter, clippy, unit test와 수동 검토로 보완) |

실제 turn은 Codex-managed 로컬 로그인으로 수행했다. 자격증명과 프롬프트/응답 본문은 보고서에 기록하지 않았다. 이번 변경에 API key provider 전환은 없다.

## 발견 및 해결

- 구형 0.157.0 조회에서는 모델이 없었다: stable 0.159.0을 격리 설치해 목록과 실제 turn을 검증했다.
- 최초 UI 테스트가 기존 화면에 표시되지 않는 모델 설명까지 기대했다: 테스트 범위를 실제 계약인 모델 option, 추론 강도 option, 자동 기본값 표시로 바로잡고 재실행했다.
- 배포 smoke 스크립트가 `gpt-5.4`를 고정 사용했다: 새 런타임의 검증 대상 모델로 갱신했다.

## 품질 게이트 및 남은 확인

- 신규 HTTP route, request body, 인증 guard, DB migration 변경 없음. 기존 카탈로그 저장은 입력 검증 후 단일 transaction을 사용한다.
- 실패 로그는 오류 코드만 포함한다. 계정 email/token과 원문을 추가로 로깅하지 않는다.
- 카탈로그는 권한 확인이 아니다. 런타임의 캐시/번들 목록일 수 있고, 15분 동기화가 모든 미래 모델의 upstream 즉시 갱신을 보장하지 않는다.
- Linux 운영 이미지 빌드, 운영 계정 turn, 운영 DB 카탈로그 반영은 미실행이다. 서버 배포 후 별도 확인해야 한다. 이번 변경은 클라이언트 재설치가 필요하지 않다.
- ITSM 프로젝트 매핑 제한 변경, 운영 앱 재시작 및 실기기 설치는 이번 범위에서 수행하지 않았다.

## 공식 근거

- [GPT-6.1 Sol](https://developers.openai.com/api/docs/models/gpt-6.1-sol)
- [Codex App Server](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)
- [Models and inference](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference)
