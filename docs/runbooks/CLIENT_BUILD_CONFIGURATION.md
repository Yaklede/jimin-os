# Jimin OS 개인 클라이언트 빌드 설정

## 목적

Jimin OS는 한 명이 사용하는 개인 서버 앱이다. 따라서 서버 주소는 사용자가
입력하는 값이 아니라, Mac·Android 앱을 만들 때 한 번 고정하는 배포 설정이다.

앱은 `VITE_API_BASE_URL`을 빌드에 포함한다. 값을 생략하면 개인 배포의 고정
주소인 `https://os.jimin.ai.kr`를 사용한다. 다른 개인 서버를 검증할 때만
HTTPS origin을 명시한다.

## 개인 서버 주소 설정

앱 패키지를 만드는 환경에서 다음 값을 설정한다.

```bash
export VITE_API_BASE_URL="https://<private-jimin-os-hostname>"
```

값은 HTTPS origin만 허용한다. 경로, query, fragment, 사용자 정보가 포함된
주소나 HTTP 주소는 앱이 사용하지 않는다.

이 값은 `apps/desktop/.env.production.local`에 두어도 된다.

```dotenv
VITE_API_BASE_URL=https://<private-jimin-os-hostname>
```

이 파일은 로컬 배포 설정이며 repository에 추가하지 않는다.

## 패키지 만들기

운영 주소가 포함된 Web asset:

```bash
pnpm client:build:web
```

macOS 앱과 Android debug APK:

```bash
pnpm client:build:macos
pnpm client:build:android
```

운영 Mac과 연결된 Android 실기기에 바로 설치하려면 다음 전용 script를
사용한다. 로컬 Docker를 사용하는 `install-local-*` script와 혼용하지 않는다.

```bash
./scripts/install-private-mac.sh
./scripts/install-private-android.sh
```

운영 build script는 HTTPS origin만 허용하고 loopback 주소와
`VITE_LOCAL_PHONE_TEST=1`을 거절한다. build 후 JavaScript 산출물에 기대한
서버 주소가 없거나 `http://127.0.0.1:8080`이 남아 있으면 패키징·설치를
중단한다. Android 설치 시 남아 있는 `adb reverse tcp:8080`도 제거한다.

일반 사용 화면에는 서버 주소 입력란·QR 등록·연결 코드 입력란을 제공하지
않는다.

## 개발 브라우저

`pnpm --filter @jimin-os/desktop dev` 실행 시에만 Vite `/server` proxy를
사용한다. 이 proxy target은 `JIMIN_API_DEV_TARGET`으로 바꿀 수 있으며,
Android 또는 macOS 배포 패키지의 서버 주소를 정하지 않는다.
