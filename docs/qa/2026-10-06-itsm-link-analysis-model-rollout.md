# ITSM 링크 분석 및 GPT-6.1 Sol 운영 반영

## 범위와 원인

- ITSM API가 원문을 반환해도 upstream 프로젝트 ID가 현재 연결과 다르거나 여러 프로젝트 링크가 섞이면 Agent가 제목과 원문을 지웠다.
- 후보 프로젝트를 확인하기 전에는 다시 원문을 지웠다. 이는 API 읽기 권한과 앱의 일감 분류를 혼동한 제한이었다.
- 모델 카탈로그는 구형 Codex 0.144.1과 startup-only 동기화에 묶여 있었다.

## 구현

- 인증된 신뢰 ITSM 서버의 링크를 원문 분석 근거로 사용한다. upstream 프로젝트 번호와 후보 확인을 읽기의 필수 조건으로 사용하지 않는다.
- 서로 다른 ITSM 프로젝트의 링크를 독립적으로 분석한다. 하나의 조회 실패가 다른 정상 원문까지 지우지 않는다.
- ITSM 프로젝트명은 원문 메타데이터로 보존한다. 할 일의 Jimin OS 프로젝트를 자동 변경하지 않는다.
- 활성화된 프로젝트의 ITSM 연결 응답은 기존 `confirmed` 계약으로 제공한다. 오래된 후보 정보로 앱이 추가 프로젝트 확인을 요구하지 않는다.
- 소유자별 ITSM 자동 확인 활성화/해제는 유지한다. 연결이 없거나 꺼진 앱 프로젝트까지 전역 인증키로 읽도록 확대하지 않는다.
- 허용된 HTTPS origin, 숫자 이슈 경로, redirect 차단, 인증 실패, 크기/시간 제한을 유지한다. 원문 속 지시는 실행하지 않는다.
- startup에서 이전 `itsm.project_mismatch` / `itsm.confirmation_required` 원문 오류가 남은 미처리 분석만 재시도한다. 활성 ITSM 연결 및 활성 유입 소스가 있어야 한다. 이미 등록/제외한 대표 메시지, 이미 일감이 연결된 스레드, 실행 중 분석은 제외한다. sync change도 같은 transaction에 남긴다.
- stable Codex 0.159.0으로 runtime/schema/integrity를 고정하고 모델 목록을 작업 사이 15분 간격으로 동기화한다. 실패하면 기존 snapshot을 유지한다.

## 검증

- Rust API/Agent/codex-client/storage unit tests 301개 통과. 명시적 계정이 필요한 live smoke는 별도로 실행했다.
- 실제 GPT-6.1 Sol 비민감 turn 완료 및 실제 structured ITSM 분석 smoke 통과: `new_task`, MID 요구사항과 등록된 담당자 보존, 프로젝트 불일치로 거절하지 않음.
- 격리 PostgreSQL: 회사 Chat 수집·소유권·승격·제외·후속 댓글 lifecycle 및 모델 카탈로그/사용자 선택 round-trip 통과.
- PostgreSQL에서 기존 매핑 오류 재시도, disabled 연결 제외, 재시도 idempotency 검증.
- 데스크탑 설정/ITSM 연결 패널/판단함 회귀 테스트 33개와 전체 65파일/408개 테스트 통과. TypeScript typecheck도 통과했다. 실제 UI asset은 변경하지 않았다.
- API/Agent release build 통과.
- formatter, clippy `-D warnings`, 배포 state 회귀, `git diff --check`, Backend Ultrawork scoped harness 통과.
- HTTP route/body/OpenAPI 타입 추가 및 DB migration 없음. 기존 인증 guard와 optimistic version 검증 유지.

## 발견한 문제와 반영

| 발견 | 조치 |
|---|---|
| JSON 참고 자료 오류 필드는 camelCase `errorCode` | 재시도 SQL을 실제 저장 계약과 일치시킴 |
| 테스트에서 비공개 DB pool 접근 | 별도 fixture pool 사용으로 수정 |
| disable/enable fixture가 connection version을 올려 후속 검증 충돌 | 연결의 최신 version을 다시 읽어 회귀 lifecycle 유지 |
| live smoke의 Duration scope 및 clippy 단위 오류 | 명시적인 `std::time::Duration::from_mins(2)`로 수정 |
| 운영 서버 Docker Compose 2.29.7에서 `compose run --pull` 미지원 | 운영을 자동 복구하고 인프라 전환 도구의 임시 probe overlay로 pull 정책 고정 |
| Gateway 재기동 직후 health 안정화 대기 부족 | 운영을 자동 복구하고 인프라 전환 도구의 health 대기 보강. 시작된 분석은 안전한 완료 시점을 확인한 뒤 재시도 |

## 문구/스킬 검토

- Backend Ultrawork 품질 게이트를 적용했다. 스캐너가 지원하지 않는 Rust 대상은 formatter, clippy, test 및 수동 안전 검토로 보완했다.
- WRITING.md와 TERMS.md를 읽고 기존 공개 ITSM 연결 문구/상태 계약을 검토했다. 공개 문구, 화면 구조 및 interaction 변경이 없어 writing/design/interaction target은 새로 만들지 않았다.

## 배포 계획

1. 검증된 immutable commit을 origin에 올린다.
2. 사용자 지정 인프라 담당 세션 `019f1288-7a14-7121-b49c-e66b246b2382`에서 배포한다.
3. 기존 secret/인증 volume/데이터와 optional meeting-transcriber 이미지를 보존한다. Agent 실행 중 작업을 확인하고 안전한 교체 시점을 사용한다.
4. Linux 0.159.0 version, GPT-6.1 Sol 실제 목록 및 비민감 완료 turn, API health/build SHA, DB schema 57 및 카탈로그 저장을 확인한다.
5. 과거 매핑 오류 중 재분석 큐 건수와 실제 ITSM 읽기 성공 여부는 집계만 보고한다. 자격증명/원문은 로그나 보고서에 노출하지 않는다.
6. startup 재시도는 분석만 수행하며 할 일 자동 승격과 webhook 전송은 실행하지 않는다.
7. 실패하면 이전 이미지/런타임 배포를 복구한다. DB schema는 바뀌지 않는다. 이미 진행된 재분석을 되돌리기 위해 원문/일감/사용자 판단을 덮어쓰지 않는다.

운영 완료 여부는 인프라 배포 결과 확인 후 아래에 기록한다.

## 운영 결과

- 상태: 운영 배포 및 실제 설치 앱 확인 완료 (2026-10-06)
- 대상: main `2204a2301d0a8800aa7ba0ce41aca750cb2465f4` (PR #5 병합). 검증한 source `6db4a868fe96008463ffa4194c9596bb5475cc8f`와 Git tree가 같다.
- 공식 이미지 빌드: GitHub Actions run `37425861865` 성공. API/Agent linux/amd64 및 OCI revision이 대상 SHA와 일치.
- Twingate 경유 live 응답의 build SHA 일치, schema 57, 5개 운영 서비스 healthy/restart 0.
- Linux Codex 0.159.0 호환성·기존 ChatGPT 인증·모델 8개 확인. GPT-6.1 Sol 비민감 turn `completed` 확인.
- 기존 매핑 오류 재분석 대상 15건 모두 `ready`, ITSM 원문 읽기 성공 15건, 기존 매핑 오류 0건. #4245/#4247 원문 읽기와 요약 갱신 확인.
- 실제 `/Applications/Jimin OS.app`에서 새로고침 후 GPT-6.1-Sol 선택 옵션과 자동 기본 모델 6.1 확인. 사용자의 기존 GPT-5.6-Sol/medium 설정은 변경하지 않음.
- 실제 앱 #4247에서 프로젝트 불일치 문구 제거, 원문 전체·첨부 링크·구체적인 요약 표시 확인. 승격/제외/완료/Chat 전송은 검증 중 실행하지 않음.
- 기존 inflow 변경·삭제 0건, 정상 수집으로 1건 추가. mount/설정/네트워크와 일감·모델 선택 보존 확인.
- Gateway/PostgreSQL/meeting-transcriber의 기존 이미지 및 컨테이너 보존. 전사 이미지 다운로드/재빌드 없음.
- 전환 도구 문제 두 차례 자동 rollback 후 수정된 도구로 최종 전환 성공. 최초 실패는 제품 코드 및 DB migration 문제가 아님.
- macOS/Android: UI asset 변경 없음. 서버 반영을 위해 재설치할 필요 없음.
