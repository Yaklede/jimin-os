# ADR-0002. 모바일 runtime 실기기 판정

- 상태: Proposed
- 제안일: 2026-07-10
- 적용 단계: M0, M3

## 배경

Jimin OS의 Mac client와 모바일 client는 React/TypeScript 화면과 API 계약을 최대한 공유한다. Rust와 직접 결합되는 desktop에서는 Tauri 2가 자연스럽지만, 모바일에서는 OAuth 복귀, 보안 저장소, 앱 생명주기, SQLite, signing과 배포 경로가 실기기에서 안정적으로 동작하는지가 더 중요하다.

KMP/CMP는 현재 프로젝트의 개발 경험과 library 제약 때문에 후보에서 제외했다. Tauri 2 mobile을 우선 검증하되, 공유 자체가 목적이 되어 모바일 안정성을 낮추지 않는다.

## 후보

### A. Tauri 2 mobile

- React/TypeScript 화면과 Rust core를 desktop과 공유하기 쉽다.
- 필요한 native 기능은 Swift/Kotlin plugin 경계에 둔다.
- mobile plugin 지원과 lifecycle 문제를 직접 검증해야 한다.

### B. Expo/React Native

- 모바일 library, signing, OAuth와 lifecycle 생태계가 더 성숙하다.
- desktop Tauri와 화면 component 일부만 공유하고 Rust는 server API를 통해 사용한다.
- 별도 mobile shell과 native bridge 유지 비용이 생긴다.

## 판정 전 기본 방향

M0 probe는 Tauri 2 mobile로 먼저 만든다. 다음 조건을 모두 실기기에서 반복 통과하면 A를 Accepted로 확정한다.

1. release-equivalent signing으로 반복 설치할 수 있다.
2. private hostname에 TLS 검증을 유지한 채 HTTPS/WSS가 동작한다.
3. system browser OAuth 후 deep link/app link로 복귀한다.
4. Keychain 또는 Keystore에 fake token을 저장·조회·삭제하고 재실행 후 유지한다.
5. SQLite create/migrate/read/write와 artifact 재설치 후 migration이 동작한다.
6. background/foreground, network OFF/ON, Wi-Fi/mobile 전환 후 bounded reconnect가 동작한다.
7. 필요한 native code가 plugin adapter 안에 머물고 공통 React code에 platform 분기가 번지지 않는다.
8. 핵심 인증 경로가 검증되지 않은 abandonware나 장기 fork에 의존하지 않는다.

다음 중 하나가 재현되면 B로 전환한다.

- OAuth, secure storage 또는 lifecycle 중 하나가 실기기에서 안정적으로 구현되지 않는다.
- release build나 store signing 절차를 저장소 명령과 runbook으로 재현할 수 없다.
- 핵심 plugin을 장기간 fork해야 한다.
- native crash의 원인과 복구를 application code에서 통제할 수 없다.
- Tauri mobile이 Expo/React Native보다 명확히 많은 platform-specific code를 요구한다.

## 필요한 증거

| 항목 | 상태 | 증거 위치 |
|---|---|---|
| 대상 휴대폰 OS·version·architecture 확인 | Pending | M0 실기기 기록 |
| release-equivalent 설치 | Pending | M0 실기기 기록 |
| TLS/HTTPS/WSS | Pending | M0 실기기 기록 |
| OAuth deep link 복귀 | Pending | M0 실기기 기록 |
| secure storage round-trip | Pending | M0 실기기 기록 |
| SQLite migration | Pending | M0 실기기 기록 |
| lifecycle/reconnect | Pending | M0 실기기 기록 |
| platform-specific code 비교 | Pending | ADR 최종 갱신 |

실패도 증거다. 실패한 scenario, 재현 절차, 사용한 plugin/version, crash 또는 build error의 민감정보 제거 요약을 기록한 뒤 runtime을 결정한다.

## 확정 전 제약

- 모바일 제품 화면을 대량 구현하지 않는다.
- Tauri 전용 domain logic을 React component에 넣지 않는다.
- App Server나 ChatGPT credential을 모바일에 배포하지 않는다.
- 모바일은 항상 Jimin OS 서버에 연결하는 client로 유지한다.
- 실제 기기 증거 없이 상태를 `Accepted`로 바꾸지 않는다.

## 후속 작업

실기기 검증 후 이 ADR에 최종 선택, 근거, 거부한 대안과 유지 비용을 기록하고 상태를 `Accepted`로 바꾼다. Expo/React Native로 전환하면 desktop Tauri와 공유할 package 경계를 함께 확정한다.
