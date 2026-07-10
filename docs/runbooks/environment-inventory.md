# M0 환경 inventory

기준일: 2026-07-10

이 문서는 비밀정보를 기록하지 않는다. hostname은 공개 가능한 별칭만 쓰고 IP, token, credential, 개인 계정 ID는 verification의 private 보관본에만 둔다.

## 개발 Mac

| 항목 | 확인 값 | 상태 |
|---|---|---|
| macOS | 26.0.1 (25A362) | 확인 |
| CPU architecture | arm64 | 확인 |
| memory | 48 GiB | 확인 |
| workspace disk free | 약 167 GiB | 확인 |
| Xcode | 26.2 (17C52) | 확인 |
| Rust | 1.95.0 | 확인 |
| Node.js | 22.14.0 | 확인 |
| pnpm | 11.5.0 | 확인 |
| Docker client | 24.0.7 | 확인 |
| Docker Compose | 2.23.3-desktop.2 | 확인 |
| Docker daemon | 24.0.7 | 확인 |

## 로컬 Linux 서버

아직 실측하지 않았다. staging 배포 전에 다음 표를 채운다.

| 항목 | 값 | 확인 명령 또는 방법 |
|---|---|---|
| Linux distribution | 미확인 | `/etc/os-release` 확인 |
| kernel | 미확인 | `uname -r` |
| CPU architecture | 미확인 | `uname -m`; `x86_64` 또는 `aarch64` |
| Docker Engine | 미확인 | `docker version` |
| Compose plugin | 미확인 | `docker compose version` |
| CPU count | 미확인 | `getconf _NPROCESSORS_ONLN` |
| memory | 미확인 | `/proc/meminfo` |
| service disk free | 미확인 | 실제 volume filesystem의 `df` |
| timezone | 미확인 | `timedatectl` |
| NTP synchronized | 미확인 | `timedatectl show` |
| service UID/GID | 미확인 | 전용 non-root account 확인 |
| private hostname | 미확인 | Mac·휴대폰에서 같은 이름 해석 확인 |
| private network path | 미확인 | LAN 또는 사설 network route 기록 |
| gateway firewall port | 미확인 | private interface의 443 또는 선택 port |
| OpenAI outbound HTTPS | 미확인 | DNS와 TLS 연결 확인; credential 출력 금지 |
| Google outbound HTTPS | 미확인 | DNS와 TLS 연결 확인 |
| GHCR outbound HTTPS | 미확인 | image manifest 조회 확인 |

서버 architecture는 `amd64` 또는 `arm64` image manifest와 일치해야 한다. `386`, 32-bit ARM, Rosetta/emulation만 가능한 서버는 M0 대상이 아니다.

## 개인 휴대폰

아직 실측하지 않았다.

| 항목 | 값 | 확인 방법 |
|---|---|---|
| platform | 미확인 | iOS 또는 Android |
| OS version | 미확인 | 시스템 정보 |
| CPU architecture | 미확인 | build target에서 확인 |
| 사설 network 접속 | 미확인 | staging private hostname health 요청 |
| release-equivalent signing | 미확인 | 실제 설치 artifact 기록 |
| internal CA trust | 미확인 | 잘못된 CA 실패와 올바른 CA 성공 모두 확인 |
| secure storage | 미확인 | fake token round-trip |
| deep link 복귀 | 미확인 | system browser probe |
| lifecycle/reconnect | 미확인 | background, foreground, network OFF/ON |
| SQLite | 미확인 | create, migrate, read, write |

## 배포 결정

- local 기본 endpoint: `https://localhost:8443`
- staging endpoint: 실제 private hostname 확인 후 설정
- TLS: [ADR-0003](../adr/ADR-0003-deployment-tls.md)에 따라 internal CA 기본
- server secret 위치: repository 밖 또는 Git에서 제외된 환경별 directory
- runtime state: `${XDG_STATE_HOME:-$HOME/.local/state}/jimin-os/<compose-project>`

미확인 항목은 추측으로 채우지 않는다. staging과 실기기 검증 후 날짜, 방법, PASS/FAIL을 verification record에 추가한다.
