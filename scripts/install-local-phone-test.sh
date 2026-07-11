#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
config_file="${1:-/tmp/jimin-os-phone-test.env}"
apk_path="${REPO_ROOT}/apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"

"${SCRIPT_DIR}/deploy-local-phone-test.sh" "${config_file}"

(
  cd "${REPO_ROOT}"
  VITE_API_BASE_URL="http://127.0.0.1:8080" \
    VITE_LOCAL_PHONE_TEST=1 \
    pnpm --filter @jimin-os/desktop tauri android build --debug --apk --target aarch64 --ci
)

adb wait-for-device
adb reverse tcp:8080 tcp:8080
adb install -r "${apk_path}"
adb shell am force-stop io.jimin.os
adb shell monkey -p io.jimin.os 1 >/dev/null

printf 'Installed local phone-test APK: %s\n' "${apk_path}"
