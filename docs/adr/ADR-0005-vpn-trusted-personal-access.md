# ADR-0005. VPN 전용 개인 서버는 앱 등록 없이 단일 소유자를 신뢰한다

- 상태: Accepted
- 결정일: 2026-07-12
- 적용 단계: M1 이후 client/API

## 결정

Jimin OS는 한 명이 사용하는 개인 서비스이며, 서버 ingress는 소유자 VPN으로만
제한한다. 따라서 Android와 macOS 앱은 QR·일회용 코드·기기 이름 입력 없이
고정된 개인 서버에 바로 연결한다.

1. 배포 환경에서 `JIMIN_TRUSTED_NETWORK=1`을 명시한 경우에만
   `POST /v1/access/session`이 device session을 자동으로 발급한다.
2. 앱은 설치 직후와 내부 refresh session이 만료된 경우 이 endpoint를 조용히
   호출한다. 발급받은 access·refresh token은 OS 보안 저장소에만 보관한다.
3. VPN ingress가 아닌 배포에서는 flag의 기본값 `0`을 유지한다. 이 경우 route는
   404로 동작하며 자동 session 발급이 불가능하다.
4. 이후 일정·대화·도구 API는 기존 bearer session으로 보호한다. VPN은 네트워크
   경계이며, 앱 내부 session은 실행 중인 요청과 기기별 폐기를 구분하는 수단이다.
5. ChatGPT/Codex credential은 개인 서버의 Codex App Server가 managed
   device-code OAuth로 보관·갱신하며 앱 client에는 전달하지 않는다.

## 이유

- 사용자와 서버가 모두 한 명의 개인 환경에 속하는데 QR 스캔과 수동 코드는
  매 설치마다 불필요한 진입 장벽이 된다.
- 사설 VPN이 이미 원격 접근을 제한하므로 앱 자체의 가입·기기 등록 화면은
  보안 경계를 추가하지 못하고 사용성만 악화시킨다.
- 앱의 고정 서버 주소와 VPN 경계가 유지되면 Mac, Android, 이후 다른 개인
  기기에서도 같은 대화와 데이터를 자연스럽게 사용한다.

## 운영 경계

- `JIMIN_TRUSTED_NETWORK=1`은 VPN 또는 같은 수준의 private ingress가 실제로
  검증된 서버에서만 허용한다.
- 이 flag는 공개 인터넷에 직접 노출된 gateway, 포트 포워딩, 임의 LAN 공개에
  사용하지 않는다.
- 로컬 Android 검증은 Mac loopback Docker API를 `adb reverse`로만 연결해 같은
  private-network 경계를 재현한다. Android에 개발 인증서를 설치할 필요가 없다.

## 결과

- Android QR scanner와 앱의 QR·수동 코드 등록 화면을 제거한다.
- 앱은 개인 서버가 응답하지 않을 때 VPN과 서버 상태를 확인한 뒤 다시 연결할
  수 있는 단일 recovery 화면만 표시한다.
- Google Cloud Console은 앱 진입에 필요하지 않으며, 향후 Calendar 연결 scope에만
  사용한다.
