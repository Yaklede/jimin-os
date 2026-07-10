# M0 진단 클라이언트 검증 기록

검증일: 2026-07-10
검증 환경: macOS 개발 Mac, Vite 8.1.4, React 19.2.7, local Docker Compose

## 검증 범위

- 실제 API `/health/live`, `/health/ready` 연동
- 연결 확인 중, 연결됨, 준비 필요, 연결 안 됨 상태
- 1280x900 데스크톱 레이아웃
- 390x844 휴대폰 레이아웃
- Design Ultrawork와 UX Writing Ultrawork
- 키보드 초점 계약, 44px 동작 영역, `aria-live`, reduced motion

## 실행 결과

| 항목 | 결과 |
| --- | --- |
| TypeScript typecheck | 통과 |
| Vitest | 3개 통과 |
| Vite production build | 통과 |
| Design Ultrawork | 통과 |
| UX Writing Ultrawork | 통과 |
| Local Compose smoke | 통과 |
| 1280px horizontal overflow | 없음, viewport와 scroll width 모두 1280px |
| 390px horizontal overflow | 없음, viewport와 scroll width 모두 390px |
| Primary action height | 44px |
| Browser console warning/error | 없음 |

## 실제 상태 검증

1. Docker stack을 내린 상태에서 클라이언트를 열어 `서버에 연결하지 못했어요`와 각 항목의 복구 문구를 확인했다.
2. `scripts/deploy-local.sh`로 gateway, API, Agent, PostgreSQL을 시작했다.
3. 같은 화면의 `다시 확인하기`를 눌러 `서버에 연결됐어요`로 전환되는 것을 확인했다.
4. 앱 응답, 앱 준비, 데이터 저장소, 데이터 구조가 모두 `준비됨`으로 표시되는 것을 확인했다.
5. 서버가 반환한 build SHA와 schema version만 서버 정보에 표시되는 것을 확인했다.

## 대비 확인

| 조합 | 대비 |
| --- | ---: |
| 본문 / surface | 10.67:1 |
| muted text / surface | 4.81:1 |
| primary button text / accent | 4.67:1 |
| focus / surface | 6.01:1 |
| warning / surface | 4.79:1 |
| destructive / surface | 5.49:1 |

## 아직 검증하지 않은 범위

- Tauri macOS native shell
- iOS/Android Tauri shell
- 개인 휴대폰 실기기에서 터치, safe area, 사설망 TLS 확인
- Google 로그인과 Calendar 화면
- Linux 개인 서버 배포 후 외부 사설망 접근

이번 결과는 브라우저에서 직접 확인할 수 있는 첫 사용자 표면의 완료 근거이며, M0 전체 완료를 의미하지 않는다.
