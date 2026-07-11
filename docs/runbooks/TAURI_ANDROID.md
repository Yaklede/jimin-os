# Tauri Android 개발·실기기 검증

이 runbook은 Jimin OS Android 클라이언트를 생성, 빌드, 개인 Android 기기에서 검증할 때 사용한다. 앱 identity는 QR 기기 연결 코드이며 Google Cloud Console은 Calendar 연결 기능을 구현하기 전까지 필요하지 않다.

## 사전 조건

- Android Studio와 Android SDK Platform, Platform-Tools, Build-Tools, Command-line Tools, NDK가 설치되어 있다.
- Java 17을 사용한다.
- `aarch64-linux-android` Rust target이 설치되어 있다.
- 개인 Android 기기에서 개발자 옵션과 USB debugging을 켜고, USB 또는 무선 debugging으로 연결했다.
- 서버의 private HTTPS hostname과 CA를 Android 기기가 신뢰한다.

```bash
export JAVA_HOME="$(/usr/libexec/java_home -v 17)"
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/<installed-ndk-version>"

rustup target add aarch64-linux-android
"$ANDROID_HOME/platform-tools/adb" devices -l
```

`adb devices -l`에 개인 기기가 `device` 상태로 표시되지 않으면 APK build나 설치 검증을 통과로 기록하지 않는다.

## 프로젝트 생성과 개발 실행

Android project는 최초 한 번만 생성한다. 생성물은 `apps/desktop/src-tauri/gen/android`에 포함한다.

```bash
pnpm --filter @jimin-os/desktop tauri:android:init
pnpm --filter @jimin-os/desktop tauri android dev --target aarch64
```

개발 서버는 같은 LAN에서 Android 기기가 접근할 수 있어야 한다. 사설망·방화벽·TLS 신뢰 문제를 해결하려고 API를 public internet에 직접 노출하지 않는다.

## Debug APK build

```bash
pnpm --filter @jimin-os/desktop tauri android build --debug --apk --target aarch64 --ci
```

생성된 APK 경로와 SHA-256은 검증 기록에 남긴다. release signing, Play 배포, production Android client ID는 운영 검증이 끝날 때까지 수행하지 않는다.

## 최소 실기기 검증

1. 앱 cold start에서 기기 연결 화면이 보이는지 확인한다.
2. 서버가 만든 QR 기기 연결 코드를 교환하고, 앱을 강제 종료한 뒤 다시 열어 session이 유지되는지 확인한다.
3. 오늘 일정과 열린 할 일을 읽고, 할 일 하나와 일정 하나를 추가·완료한다.
4. 서버를 잠시 차단했다가 복구해 오류 문구와 재연결을 확인한다.
5. Android 기기에서 서버 TLS 인증서를 신뢰하지 않을 때 안전하게 연결을 거부하는지 확인한다.
6. 화면 캡처, 기기 모델, Android version, APK SHA-256, PASS/FAIL을 `docs/verification/`의 private 검증 기록에 남긴다. 연결 코드·access token·refresh token·일정 상세 내용은 기록하지 않는다.

## 현재 알려진 검증 제한

2026-07-11 기준 개발 Mac은 Android SDK와 NDK를 찾았고 Android Studio project까지 생성했다. 연결된 `adb` 개인 기기가 없으며, 설치된 NDK가 Apple Silicon native host toolchain으로 인식되지 않아 Android APK build는 아직 통과하지 않았다. NDK를 host architecture에 맞게 설치한 뒤 위 명령으로 다시 검증한다.
