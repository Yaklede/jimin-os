# ADR-0003: 로컬 서버 TLS 종료 방식

- 상태: Accepted for M0
- 결정일: 2026-07-10
- 대상: local, staging 로컬 서버

## 배경

Jimin OS API는 public internet에 직접 공개하지 않지만 Mac과 개인 휴대폰에서 credential과 개인 데이터를 주고받는다. 사설 네트워크는 인증과 전송 암호화를 대신하지 않는다. M0 완료 조건은 인증서 검증을 끄지 않고 private hostname으로 `/health/live`와 `/health/ready`에 접근하는 것이다.

로컬 Linux 서버의 실제 hostname, DNS provider, 공인 인증서 발급 가능 여부는 아직 확인되지 않았다. 구성은 두 인증서 공급 방식을 지원해야 하지만 M0의 기본 검증 경로는 하나로 고정해야 한다.

## 결정

M0 local과 staging의 기본 TLS 방식은 Caddy internal CA로 한다.

- `JIMIN_TLS_MODE=internal`이 기본이다.
- gateway는 `JIMIN_OS_HOSTNAME`에 대한 leaf certificate를 internal CA로 발급한다.
- Caddy CA private key는 `caddy_data` volume에만 남기고 일반 backup, Git, log에 복사하지 않는다.
- public root certificate만 `scripts/export-caddy-root-ca.sh`로 내보낼 수 있다.
- Mac과 개인 휴대폰은 해당 root certificate를 사용자가 명시적으로 신뢰시킨다.
- client와 smoke test는 일반 TLS 검증을 유지한다. `curl -k`, invalid certificate 허용, ATS·Network Security Config 완화는 금지한다.
- plain HTTP는 gateway container의 `127.0.0.1` health listener와 container 내부 API 통신에만 사용한다.

공인 CA 또는 private DNS용 인증서가 이미 준비된 환경에서는 `JIMIN_TLS_MODE=files`를 사용할 수 있다. 이때 certificate chain과 private key는 gateway에만 Compose secret으로 mount한다. 이 선택은 검증 우회가 아니라 인증서 공급자 교체다.

## 구현

- TLS 종료: pinned Caddy 2.10.2 image 기반 non-root gateway
- 외부 endpoint: `https://<JIMIN_OS_HOSTNAME>:<JIMIN_GATEWAY_HOST_PORT>`
- 내부 upstream: `http://api:8080`
- internal CA config: `deploy/gateway/tls/internal.caddy`
- file certificate config: `deploy/gateway/tls/files.caddy`
- certificate file overlay: `deploy/compose.tls-files.yaml`

`JIMIN_TLS_MODE`은 `internal` 또는 `files`만 허용하며 배포 script가 다른 값을 거절한다.

## 결과

장점:

- private hostname과 public DNS 소유 여부에 의존하지 않고 M0를 검증할 수 있다.
- Mac과 휴대폰에서 실제 certificate chain과 hostname 검증을 수행한다.
- gateway 외 서비스는 host port를 열지 않는다.
- 추후 공인 certificate로 바꿔도 client API와 Compose 서비스 경계가 바뀌지 않는다.

비용과 위험:

- 각 기기에 root certificate를 안전하게 설치하고 폐기해야 한다.
- `caddy_data` volume을 잃으면 새 CA가 생기므로 모든 기기에서 다시 신뢰해야 한다.
- CA private key가 들어 있는 volume은 credential과 같은 수준으로 보호해야 한다.

## 검증 기준

- Mac `curl`이 export한 root CA로 hostname을 검증하고 health `200`을 받는다.
- 개인 휴대폰 browser와 release-equivalent 앱이 같은 hostname을 신뢰한다.
- 잘못된 hostname과 신뢰하지 않은 CA에서는 연결이 실패한다.
- gateway 외 service에 host port가 없다.
- rendered Compose와 container mount에 Docker socket이 없다.

실제 로컬 서버와 휴대폰 결과는 M0 verification record에 남긴다. 결과가 나오기 전까지 TLS 검증 완료로 표시하지 않는다.
