<!-- OPENDOCK:START id=files:TERMS.md dock=opendock/ux-writing-ultrawork path=TERMS.md -->
# TERMS.md

공개 UI에서 쓰는 용어와 피해야 할 내부 용어를 관리해요.

| Concept | Korean | English | Avoid |
| --- | --- | --- | --- |
| sign in | 로그인 | Sign in | auth, authentication token |
| retry | 다시 시도 | Try again | retry request, re-call endpoint |
| input error | 입력한 내용 | information | payload, schema, validation |
| access | 권한 | access | permission denied, forbidden |
| workspace | 워크스페이스 | workspace | tenant, namespace |

## Allowed Developer Terms

아래에 적은 단어는 사용자에게 노출해도 돼요.

- API

## Project Terms

서비스별 용어를 여기에 추가해요.

### 일정과 할 일 구분

- 특정 시각에 발생하는 약속, 이동, 출발, 방문은 `일정`으로 표현해요.
- 완료해야 하는 결과나 후속 조치는 `할 일`로 표현해요.
- 사용자가 `출발 시간`처럼 정확한 시각을 정하면 일감이 아니라 일정으로 확인해요.

### 비서가 정리하는 할 일

- 대화나 음성으로 요청한 문장 전체를 할 일 제목으로 복사하지 않아요.
- 제목에는 해야 할 행동이나 완료 결과를 짧게 적어요.
- 배경, 요청한 산출물, 완료 조건은 설명에 나눠 적어요.
- 고유명사, 수치, 사용자가 명시한 기한은 바꾸지 않아요.

| Concept | Korean | English | Avoid |
| --- | --- | --- | --- |
| server connection | 서버 연결 | Server connection | endpoint status, host reachability |
| current state | 현재 상태 | Current state | runtime state, health status |
| app response | 앱 응답 | App response | liveness probe |
| data store | 데이터 저장소 | Data store | database readiness |
| data structure | 데이터 구조 | Data structure | migration status, schema version |
| check again | 다시 확인하기 | Check again | retry probe, refresh endpoint |
| needs attention | 준비가 더 필요해요 | Needs attention | not ready, degraded |
| unreachable | 서버에 연결하지 못했어요 | Could not connect | connection refused, fetch failed |
<!-- OPENDOCK:END id=files:TERMS.md dock=opendock/ux-writing-ultrawork path=TERMS.md -->
