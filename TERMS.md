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
- Webhook

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

### 일정 충돌과 판단함

- 새 일정이나 일정 변경이 기존 일정과 겹치면 바로 실행하지 않고 현재 대화에서 먼저 알려요.
- 충돌 안내에는 겹치는 일정, 실행하지 않은 결과, 선택할 수 있는 가까운 빈 시간을 함께 보여줘요.
- 사용자가 다른 시간을 선택하거나 겹침을 명시적으로 허용하면 대화에서 바로 처리해요.
- 아직 정하지 않은 일정 충돌만 판단함에 남기고, 대화에서 해결되면 해당 제안을 자동으로 종료해요.

### 목표와 진행 근거

- 목표는 할 일 목록이 아니라 원하는 결과와 우선순위를 정하는 기준이에요.
- 목표 진행률은 연결된 프로젝트의 실제 할 일 완료 결과로 계산해요.
- 진행률과 함께 최근 7일 완료, 기한이 지난 일, 다음 행동을 보여줘요.
- 모든 일을 마쳐도 원하는 결과를 달성했는지는 사용자가 확인해요.
- 기한이 더 급한 일이 없다면 활성 목표에 연결된 일을 먼저 제안해요.
- 근거 없이 목표 달성 확률을 숫자로 만들지 않아요.

### 외부 자동화 연결

- 사용자가 직접 설정하는 프로젝트 자동화 기능에는 `웹훅`을 사용해요.
- URL, payload, endpoint 같은 하위 구현 용어는 공개 문구에 노출하지 않아요.

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
