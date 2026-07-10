# Architecture decision records

구현 중 되돌리기 어렵거나 여러 단계에 영향을 주는 결정을 기록한다. 상태는 `Proposed`, `Accepted`, `Superseded` 중 하나를 사용하며, 바뀐 결정은 기존 문서를 삭제하지 않고 대체 ADR을 연결한다.

| ADR | 상태 | 결정 |
|---|---|---|
| [ADR-0001](ADR-0001-codex-app-server-compatibility.md) | Accepted | Codex App Server 호환성 경계 |
| [ADR-0002](ADR-0002-mobile-runtime.md) | Proposed | 모바일 runtime 실기기 판정 |

M0에서 배포/TLS와 모바일 runtime을 실제 환경으로 검증한 뒤 Proposed ADR을 확정한다.
