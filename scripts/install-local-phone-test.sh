#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
config_file="${1:-/tmp/jimin-os-phone-test.env}"
device_serial="${2:-${ANDROID_SERIAL:-}}"
apk_path="${REPO_ROOT}/apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"
dev_package="io.jimin.os.dev"

source "${SCRIPT_DIR}/lib/client-build-config.sh"

if [[ -z "${device_serial}" ]]; then
  emulators=()
  while read -r serial state; do
    if [[ "${state:-}" == "device" && "${serial}" == emulator-* ]]; then
      emulators+=("${serial}")
    fi
  done < <(adb devices | tail -n +2)

  if [[ ${#emulators[@]} -ne 1 ]]; then
    printf 'Expected one running Android emulator; found %s. Pass its serial as the second argument.\n' "${#emulators[@]}" >&2
    exit 1
  fi
  device_serial="${emulators[0]}"
fi
require_android_emulator "${device_serial}"

adb_device=(adb -s "${device_serial}")

"${SCRIPT_DIR}/deploy-local-phone-test.sh" "${config_file}"

(
  cd "${REPO_ROOT}"
  firebase_target="${REPO_ROOT}/apps/desktop/src-tauri/gen/android/app/google-services.json"
  cleanup_android_firebase_config "${firebase_target}"
  trap 'cleanup_android_firebase_config "${firebase_target}"' EXIT
  VITE_API_BASE_URL="http://127.0.0.1:8080" \
    VITE_LOCAL_PHONE_TEST=1 \
    ORG_GRADLE_PROJECT_jiminDevPackage=true \
    pnpm --filter @jimin-os/desktop tauri android build --debug --apk \
      --target aarch64 --ci --config src-tauri/tauri.android-dev.conf.json
)
verify_android_apk_application_id "${apk_path}" "${dev_package}"

"${adb_device[@]}" wait-for-device
"${adb_device[@]}" install -r "${apk_path}"
"${adb_device[@]}" reverse tcp:8080 tcp:8080
if ! "${adb_device[@]}" reverse --list | grep -q 'tcp:8080 tcp:8080'; then
  printf 'Failed to configure adb reverse for %s.\n' "${device_serial}" >&2
  exit 1
fi
"${adb_device[@]}" shell am force-stop "${dev_package}"
"${adb_device[@]}" shell monkey -p "${dev_package}" 1 >/dev/null

printf 'Installed Jimin OS Dev on emulator %s: %s\n' "${device_serial}" "${apk_path}"
