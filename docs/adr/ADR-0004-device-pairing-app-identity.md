# ADR-0004. Jimin OS 앱 신원은 QR 기기 등록으로 관리한다

- 상태: Accepted
- 결정일: 2026-07-11
- 적용 단계: M1 이후 전체 client/API

## 결정

Jimin OS의 앱 로그인은 Google OAuth가 아니라 개인 서버가 발급하는 단일
소유자용 QR 기기 등록으로 한다.

1. 신뢰된 서버 명령 또는 이미 등록된 기기가 짧은 수명의 일회용 pairing
   token을 발급한다.
2. 새 Android 또는 macOS client가 token을 소비하면 서버가 device별 access
   token과 회전형 refresh token을 발급한다.
3. pairing token 원문은 QR 전달 중에만 존재한다. PostgreSQL에는 별도 HMAC
   pepper로 만든 verifier만 저장하고, 한 번 소비하거나 새 token을 만들면
   이전 token은 쓸 수 없다.
4. Jimin OS session과 ChatGPT/Codex credential은 전혀 공유하지 않는다.
   ChatGPT 계정은 개인 서버의 Codex App Server가 managed device-code OAuth로
   보관·갱신하며 모바일 client에는 전달하지 않는다.
5. Google OAuth는 M2의 Google Calendar 연결을 위해서만 도입한다. 따라서
   앱 가입이나 기기 복구에 Google Cloud Console 설정은 필요하지 않다.

## 이유

- 개인 서버가 상시 실행되는 구조에서는 서버가 기기 접근을 직접 폐기·추적할
  수 있어야 한다.
- 앱 신원, 외부 캘린더 권한, ChatGPT 구독 인증은 수명과 권한이 다른 세
  credential이므로 한 로그인 흐름에 합치면 복구와 권한 검토가 복잡해진다.
- Android와 macOS의 browser/deep-link OAuth 차이를 앱 접근 제어의 필수
  경로에서 제거한다.

## 운영 경계

- 최초 기기 등록은 신뢰된 개인 서버에서 `jimin-api pairing create`를
  실행해 시작한다. 명령은 일회용 QR 코드를 터미널에 렌더링하고 원문 URI를
  출력하지 않는다. QR과 그 화면은 credential로 취급하며 terminal history,
  issue, log에 보관하지 않는다.
- macOS 또는 Android의 예외 복구는 `jimin-api pairing create --code`를 명시적으로
  호출해 일회용 연결 코드를 받는다. 이 경로는 QR을 스캔할 수 없을 때에만 쓰며,
  출력한 코드도 동일한 credential 취급을 받는다.
- 등록된 기기는 `POST /v1/device-pairings`를 통해 새 pairing token을 만들
  수 있다. 이 endpoint의 응답과 session token 응답은 `Cache-Control:
no-store`를 사용한다.
- pairing은 인증이 아니라 기기 등록이다. 서버 주소의 TLS 검증과 이미
  등록된 기기의 session 검증을 대체하지 않는다.

## 결과

- `POST /v1/auth/google/exchange`는 제거한다.
- M1 profile email은 nullable이며, local-device owner는 email을 앱 신원의
  근거로 사용하지 않는다.
- 기존 Google adapter는 Calendar integration 경계로 보존하되, M1 API
  startup과 deployment secret에서 제거한다.
