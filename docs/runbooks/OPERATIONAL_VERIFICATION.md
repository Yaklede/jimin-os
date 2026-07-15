# Jimin OS 운영 검증 runbook

## 목적

개인 서버에 배포한 Jimin OS를 Mac과 Android에서 실제로 사용하기 전,
개인 서버 자동 연결·일정·할 일·대화·재시작 복구가 모두 한 번씩 확인되도록 한다.
이 runbook은 배포를 자동 실행하지 않는다. 서버 secret, ChatGPT device-code
인증, 개인 Android 기기 연결은 운영자가 직접 승인하고 수행한다.

## 범위와 완료 기준

이번 검증의 완료 기준은 다음이다.

1. private HTTPS hostname에서 gateway, API, PostgreSQL, Agent가 기동한다.
2. Agent가 ChatGPT 구독 계정으로 로그인되어 비민감 대화 turn 하나를 완료한다.
3. Mac과 Android가 같은 VPN 전용 개인 서버에 자동으로 연결된다.
4. 두 기기에서 일정과 할 일의 생성·완료 결과가 서버 기준으로 확인된다.
5. 대화 요청이 완료되고, 앱을 다시 열어도 같은 대화와 마지막 요청 상태를 읽는다.

현재 대화 기능은 read-only, no-approval Agent turn을 제공한다. terminal·파일
수정·승인 작업은 이 검증 범위에 포함하지 않는다.

## 1. 서버 사전 조건

1. 실제 환경 파일과 secret file을 repository 밖에 준비한다.
2. 환경에 맞는 배포 runbook을 수행한다.

   - local: [M0 local deployment](M0_LOCAL_DEPLOYMENT.md)
   - private server: [M0 staging deployment](M0_STAGING_DEPLOYMENT.md)

3. Agent volume에 ChatGPT device-code 인증을 완료하고 비민감 fixture turn을
   확인한다. [Codex device auth](CODEX_DEVICE_AUTH.md)를 따른다.
4. Mac과 Android가 server hostname의 TLS certificate를 신뢰하는지 확인한다.

Agent가 `auth_required` 상태이면 일정과 할 일은 계속 사용할 수 있지만 AI
요청은 처리되지 않는다. 대화 검증 전에 device auth와 turn probe가 성공해야
한다.

## 2. 개인 서버 자동 연결 확인

실제 환경 파일에서 `JIMIN_TRUSTED_NETWORK=1`을 설정한다. 이 값은 gateway
ingress가 소유자 VPN으로 제한된 배포에서만 설정한다. 외부 인터넷에 공개된
서버에서는 기본값 `0`을 유지한다.

앱은 고정된 private HTTPS hostname으로 개인 서버에 연결하고, 최초 실행과
내부 session 만료 시 session을 자동으로 갱신한다. 사용자에게 QR, 기기 이름,
연결 코드, 서버 주소 입력을 요구하지 않는다.

## 3. Mac 검증

1. 현재 commit에서 만든 `Jimin OS_0.1.0_aarch64.dmg`를 열어 앱을 실행한다.
2. 대화 화면이 바로 열리고 QR·기기 등록 화면이 보이지 않는지 확인한다.
3. 연결 후 오늘 화면에서 할 일 하나와 일정 하나를 만든다.
4. 할 일을 완료하고 일정·할 일이 화면에 맞게 갱신되는지 확인한다.
5. 앱을 완전히 종료한 뒤 다시 열어 session과 오늘 데이터가 유지되는지 확인한다.
6. 대화 화면에서 비민감 요청 하나를 보낸다. 예: `오늘 일정과 열린 할 일을
간단히 정리해줘`.
7. 응답이 끝난 뒤 앱을 다시 열고 같은 대화를 선택한다. 마지막 message와 job
   terminal state가 유지되어야 한다.

## 4. Android 검증

Android phone을 USB 또는 wireless debugging으로 연결한 뒤, 생성 APK를
설치한다. release signing이나 Play 배포는 이 단계의 범위가 아니다.

### 맥 Docker 로컬 테스트

개인 서버 배포 전의 Android 검증은 아래 스크립트로 수행한다. 이 경로는 맥의
Docker API를 `adb reverse`로만 휴대폰에 연결하고, debug APK에만
`http://127.0.0.1:8080`을 고정한다. LAN·인터넷에는 HTTP 포트를 열지 않으며,
Android에 개발 CA 인증서를 설치할 필요도 없다.

```bash
./scripts/install-local-phone-test.sh /tmp/jimin-os-phone-test.env
```

에뮬레이터와 실기기가 함께 연결되어 있거나 실기기가 여러 대라면 두 번째 인자로
대상 serial을 지정한다. 스크립트는 설치가 끝난 뒤 해당 기기의 `adb reverse`를
다시 설정하고 연결 여부를 검증한다.

```bash
./scripts/install-local-phone-test.sh \
  /tmp/jimin-os-phone-test.env R5KL20581QR
```

USB 또는 wireless debugging 연결이 끊기면 이 테스트 APK는 서버에 연결할 수
없다. 이는 배포본의 동작이 아니라 맥 loopback을 쓰는 테스트 경계다.

### 개인 서버 배포 검증

```bash
adb install -r \
  apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

다음을 확인한다.

1. launcher에 Jimin OS 아이콘과 앱 이름이 보인다.
2. cold start에서 대화 화면이 표시되고 QR·기기 등록 화면이 보이지 않는다.
3. 오늘 화면에서 Mac이 만든 일정·할 일을 읽고, Android에서 한 변경이 Mac에
   반영되는지 확인한다.
4. 앱을 강제 종료하고 다시 열어 session이 유지되는지 확인한다.
5. 대화 하나를 열고 완료된 응답과 실패 안내가 이해 가능한지 확인한다.

개인 서버 배포본에서 Android 기기가 TLS certificate를 신뢰하지 못하면 연결이
실패해야 한다. certificate 검증을 끄거나 `http` 주소로 바꾸어 우회하지 않는다.

## 5. 기록과 실패 처리

검증 기록에는 다음만 남긴다.

- commit SHA
- server environment 이름
- Mac/Android OS와 기기 모델
- DMG/APK SHA-256
- 각 단계의 PASS/FAIL과 비밀값을 제외한 오류 분류

다음 정보는 남기지 않는다.

- access/refresh token, ChatGPT credential, secret file 내용
- 개인 일정 제목, 대화 원문, AI 응답 원문

실패 시에는 해당 기능만 중단하고 원인을 분류한다.

- API health 실패: 배포 runbook의 health와 PostgreSQL 상태를 먼저 확인한다.
- Agent auth 실패: [Codex device auth](CODEX_DEVICE_AUTH.md)를 다시 수행한다.
- Mac/Android 연결 실패: hostname, TLS trust, 일회용 연결 값의 만료 여부를
  확인한다.
- 대화가 완료되지 않음: Agent health와 비민감 turn probe를 확인한 뒤 새 요청을
  만든다. 완료되지 않은 요청을 같은 client message ID로 임의 복제하지 않는다.
