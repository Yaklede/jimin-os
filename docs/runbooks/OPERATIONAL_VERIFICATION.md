# Jimin OS 운영 검증 runbook

## 목적

개인 서버에 배포한 Jimin OS를 Mac과 Android에서 실제로 사용하기 전,
기기 연결·일정·할 일·대화·재시작 복구가 모두 한 번씩 확인되도록 한다.
이 runbook은 배포를 자동 실행하지 않는다. 서버 secret, ChatGPT device-code
인증, 개인 Android 기기 연결은 운영자가 직접 승인하고 수행한다.

## 범위와 완료 기준

이번 검증의 완료 기준은 다음이다.

1. private HTTPS hostname에서 gateway, API, PostgreSQL, Agent가 기동한다.
2. Agent가 ChatGPT 구독 계정으로 로그인되어 비민감 대화 turn 하나를 완료한다.
3. Mac과 Android가 같은 개인 서버에 각각 기기 등록된다.
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

## 2. 기기 연결 QR 코드 만들기

신뢰된 서버에서 API container를 통해 일회용 기기 연결 QR 코드를 만든다. Compose
인자는 각 배포 runbook의 실제 환경 파일과 동일해야 한다.

```bash
docker compose <compose-args> exec -T api jimin-api pairing create
```

명령은 터미널에 한 번만 쓸 수 있는 QR 코드를 출력한다. 이 QR 코드는
credential로 취급한다.

- terminal history, chat, issue, screenshot, 검증 기록에 남기지 않는다.
- Android의 Jimin OS setup 화면에서 **QR 코드 스캔하기**를 눌러 화면의 QR
  코드를 읽는다.
- macOS 또는 Android의 예외 복구에는 `jimin-api pairing create --code`로 만든
  일회용 코드를 **코드 직접 입력하기**에 입력한다. 이 명령의 출력도 credential로
  취급한다.
- 만료·소비된 값은 재사용하지 않고 새로 만든다.

## 3. Mac 검증

1. 현재 commit에서 만든 `Jimin OS_0.1.0_aarch64.dmg`를 열어 앱을 실행한다.
2. 개인 서버 주소가 포함된 설치본의 setup 화면에서 기기 이름을 확인하고,
   `jimin-api pairing create --code`로 만든 일회용 코드를 **코드 직접 입력하기**에
   입력한다.
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

```bash
adb install -r \
  apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

다음을 확인한다.

1. launcher에 Jimin OS 아이콘과 앱 이름이 보인다.
2. cold start에서 setup 화면이 표시된다.
3. 새 일회용 연결 QR 코드로 Android를 등록한다. 연결 QR 코드는 Mac에서 사용한 값과
   별도로 만든다.
4. 오늘 화면에서 Mac이 만든 일정·할 일을 읽고, Android에서 한 변경이 Mac에
   반영되는지 확인한다.
5. 앱을 강제 종료하고 다시 열어 session이 유지되는지 확인한다.
6. 대화 하나를 열고 완료된 응답과 실패 안내가 이해 가능한지 확인한다.

Android 기기에서 TLS certificate를 신뢰하지 못하면 연결이 실패해야 한다.
certificate 검증을 끄거나 `http` 주소로 바꾸어 우회하지 않는다.

## 5. 기록과 실패 처리

검증 기록에는 다음만 남긴다.

- commit SHA
- server environment 이름
- Mac/Android OS와 기기 모델
- DMG/APK SHA-256
- 각 단계의 PASS/FAIL과 비밀값을 제외한 오류 분류

다음 정보는 남기지 않는다.

- pairing URI 또는 token
- access/refresh token, ChatGPT credential, secret file 내용
- 개인 일정 제목, 대화 원문, AI 응답 원문

실패 시에는 해당 기능만 중단하고 원인을 분류한다.

- API health 실패: 배포 runbook의 health와 PostgreSQL 상태를 먼저 확인한다.
- Agent auth 실패: [Codex device auth](CODEX_DEVICE_AUTH.md)를 다시 수행한다.
- Mac/Android 연결 실패: hostname, TLS trust, 일회용 연결 값의 만료 여부를
  확인한다.
- 대화가 완료되지 않음: Agent health와 비민감 turn probe를 확인한 뒤 새 요청을
  만든다. 완료되지 않은 요청을 같은 client message ID로 임의 복제하지 않는다.
