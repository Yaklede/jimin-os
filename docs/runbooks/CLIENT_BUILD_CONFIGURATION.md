# Jimin OS 개인 클라이언트 빌드 설정

## 목적

Jimin OS는 한 명이 사용하는 개인 서버 앱이다. 따라서 서버 주소는 사용자가
입력하는 값이 아니라, Mac·Android 앱을 만들 때 한 번 고정하는 배포 설정이다.

앱은 `VITE_API_BASE_URL`을 빌드에 포함한다. 이 값은 credential이 아니지만,
개인 인프라의 주소이므로 repository에 커밋하지 않는다.

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

Android debug APK:

```bash
VITE_API_BASE_URL="$VITE_API_BASE_URL" \
pnpm --filter @jimin-os/desktop tauri android build --debug --apk --target aarch64 --ci
```

macOS 앱:

```bash
VITE_API_BASE_URL="$VITE_API_BASE_URL" \
pnpm --filter @jimin-os/desktop tauri build
```

서버 주소가 없는 production 패키지는 개인 서버 정보를 찾을 수 없다는 안내를
표시한다. 일반 사용 화면에는 서버 주소 입력란·QR 등록·연결 코드 입력란을
제공하지 않는다.

## 개발 브라우저

`pnpm --filter @jimin-os/desktop dev` 실행 시에만 Vite `/server` proxy를
사용한다. 이 proxy target은 `JIMIN_API_DEV_TARGET`으로 바꿀 수 있으며,
Android 또는 macOS 배포 패키지의 서버 주소를 정하지 않는다.
