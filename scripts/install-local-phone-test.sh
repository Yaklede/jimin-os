#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
config_file="${1:-/tmp/jimin-os-phone-test.env}"
device_serial="${2:-${ANDROID_SERIAL:-}}"
apk_path="${REPO_ROOT}/apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"

if [[ -z "${device_serial}" ]]; then
  physical_devices=()
  while read -r serial state; do
    if [[ "${state:-}" == "device" && "${serial}" != emulator-* ]]; then
      physical_devices+=("${serial}")
    fi
  done < <(adb devices | tail -n +2)

  if [[ ${#physical_devices[@]} -ne 1 ]]; then
    printf 'Expected one connected physical Android device; found %s. Pass its serial as the second argument.\n' "${#physical_devices[@]}" >&2
    exit 1
  fi
  device_serial="${physical_devices[0]}"
fi

adb_device=(adb -s "${device_serial}")

"${SCRIPT_DIR}/deploy-local-phone-test.sh" "${config_file}"

(
  cd "${REPO_ROOT}"
  VITE_API_BASE_URL="http://127.0.0.1:8080" \
    VITE_LOCAL_PHONE_TEST=1 \
    pnpm --filter @jimin-os/desktop tauri android build --debug --apk --target aarch64 --ci
)

"${adb_device[@]}" wait-for-device
"${adb_device[@]}" install -r "${apk_path}"
"${adb_device[@]}" reverse tcp:8080 tcp:8080
if ! "${adb_device[@]}" reverse --list | grep -q 'tcp:8080 tcp:8080'; then
  printf 'Failed to configure adb reverse for %s.\n' "${device_serial}" >&2
  exit 1
fi
"${adb_device[@]}" shell am force-stop io.jimin.os
"${adb_device[@]}" shell monkey -p io.jimin.os 1 >/dev/null

printf 'Installed local phone-test APK on %s: %s\n' "${device_serial}" "${apk_path}"
