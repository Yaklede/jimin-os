#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
requested_server_url="${1:-}"
device_serial="${2:-${ANDROID_SERIAL:-}}"
apk_path="${REPO_ROOT}/apps/desktop/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"

source "${SCRIPT_DIR}/lib/client-build-config.sh"

server_url="$(production_server_url "${requested_server_url}")"

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
"${SCRIPT_DIR}/build-private-client.sh" android "${server_url}"

"${adb_device[@]}" wait-for-device
"${adb_device[@]}" install -r "${apk_path}"
"${adb_device[@]}" reverse --remove tcp:8080 2>/dev/null || true
if "${adb_device[@]}" reverse --list | grep -q 'tcp:8080 tcp:8080'; then
  printf 'Local adb reverse is still active for %s; refusing to start the private-server client.\n' "${device_serial}" >&2
  exit 1
fi
"${adb_device[@]}" shell am force-stop io.jimin.os
"${adb_device[@]}" shell monkey -p io.jimin.os 1 >/dev/null

printf 'Installed private-server Android app on %s with server: %s\n' "${device_serial}" "${server_url}"
