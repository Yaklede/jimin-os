#!/usr/bin/env bash

set -Eeuo pipefail

JIMIN_OS_DEFAULT_SERVER_URL="https://os.jimin.ai.kr"
JIMIN_OS_LOCAL_TEST_SERVER_URL="http://127.0.0.1:8080"

require_android_emulator() {
  local serial="${1:?Android serial is required}"
  if [[ "${serial}" != emulator-* ]]; then
    printf 'Local Android builds may only be installed on an emulator; refusing device: %s\n' "${serial}" >&2
    return 1
  fi
}

require_android_physical_device() {
  local serial="${1:?Android serial is required}"
  if [[ "${serial}" == emulator-* ]]; then
    printf 'Production Android builds may only be installed on a physical device; refusing emulator: %s\n' "${serial}" >&2
    return 1
  fi
}

android_apkanalyzer() {
  local candidates=(
    "${JIMIN_ANDROID_APKANALYZER:-}"
    "${ANDROID_HOME:-}/cmdline-tools/latest/bin/apkanalyzer"
    "${ANDROID_SDK_ROOT:-}/cmdline-tools/latest/bin/apkanalyzer"
    "${HOME:-}/Library/Android/sdk/cmdline-tools/latest/bin/apkanalyzer"
  )
  local candidate

  for candidate in "${candidates[@]}"; do
    if [[ -n "${candidate}" && -x "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  if command -v apkanalyzer >/dev/null 2>&1; then
    command -v apkanalyzer
    return 0
  fi

  printf 'Android apkanalyzer was not found; refusing an unverified APK installation.\n' >&2
  return 1
}

verify_android_apk_application_id() {
  local apk_path="${1:?APK path is required}"
  local expected_application_id="${2:?expected Android application ID is required}"
  local analyzer actual_application_id

  if [[ ! -f "${apk_path}" ]]; then
    printf 'Android APK does not exist: %s\n' "${apk_path}" >&2
    return 1
  fi
  analyzer="$(android_apkanalyzer)"
  actual_application_id="$("${analyzer}" manifest application-id "${apk_path}")"
  if [[ "${actual_application_id}" != "${expected_application_id}" ]]; then
    printf 'Refusing to install Android APK %s; expected application ID %s, found %s.\n' \
      "${apk_path}" "${expected_application_id}" "${actual_application_id:-unknown}" >&2
    return 1
  fi
}

production_server_url() {
  local configured="${1:-${VITE_API_BASE_URL:-${JIMIN_OS_DEFAULT_SERVER_URL}}}"
  local normalized="${configured%/}"
  local authority=""
  local authority_lower=""

  if [[ "${VITE_LOCAL_PHONE_TEST:-}" == "1" ]]; then
    printf 'VITE_LOCAL_PHONE_TEST=1 is reserved for local test builds and cannot be used for a private-server client.\n' >&2
    return 1
  fi
  if [[ ! "${normalized}" =~ ^https://[^/?#]+$ ]]; then
    printf 'Private-server clients require one HTTPS origin without a path, query, or fragment: %s\n' "${configured}" >&2
    return 1
  fi

  authority="${normalized#https://}"
  authority_lower="$(printf '%s' "${authority}" | tr '[:upper:]' '[:lower:]')"
  if [[ "${authority}" == *"@"* ]] ||
    [[ "${authority_lower}" =~ ^localhost(:[0-9]+)?$ ]] ||
    [[ "${authority_lower}" =~ ^127\.0\.0\.1(:[0-9]+)?$ ]] ||
    [[ "${authority_lower}" =~ ^0\.0\.0\.0(:[0-9]+)?$ ]] ||
    [[ "${authority_lower}" =~ ^\[::1\](:[0-9]+)?$ ]]; then
    printf 'Private-server clients cannot use credentials or a loopback server: %s\n' "${configured}" >&2
    return 1
  fi

  printf '%s\n' "${normalized}"
}

verify_production_web_assets() {
  local assets_dir="${1:?assets directory is required}"
  local expected_server_url="${2:?expected server URL is required}"
  local javascript_files=()

  if [[ ! -d "${assets_dir}" ]]; then
    printf 'Client assets directory does not exist: %s\n' "${assets_dir}" >&2
    return 1
  fi
  while IFS= read -r file; do
    javascript_files+=("${file}")
  done < <(find "${assets_dir}" -type f -name '*.js' -print)
  if [[ ${#javascript_files[@]} -eq 0 ]]; then
    printf 'No JavaScript asset was produced in %s.\n' "${assets_dir}" >&2
    return 1
  fi
  if ! rg -a -F -q "${expected_server_url}" "${javascript_files[@]}"; then
    printf 'Built client does not contain the expected private server origin: %s\n' "${expected_server_url}" >&2
    return 1
  fi
  if rg -a -F -q "${JIMIN_OS_LOCAL_TEST_SERVER_URL}" "${javascript_files[@]}"; then
    printf 'Built client still contains the local test server origin: %s\n' "${JIMIN_OS_LOCAL_TEST_SERVER_URL}" >&2
    return 1
  fi
}

prepare_android_firebase_config() {
  local source_file="${1:-}"
  local target_file="${2:?Firebase target path is required}"

  rm -f "${target_file}"
  if [[ -z "${source_file}" || ! -f "${source_file}" ]]; then
    printf 'Firebase Android config is absent; building with local reminders only.\n'
    return 0
  fi
  local size
  size="$(wc -c < "${source_file}" | tr -d '[:space:]')"
  if [[ ! "${size}" =~ ^[0-9]+$ ]] || (( size < 100 || size > 65536 )); then
    printf 'Firebase Android config has an invalid size.\n' >&2
    return 1
  fi
  if ! grep -Eq '"package_name"[[:space:]]*:[[:space:]]*"io\.jimin\.os"' "${source_file}"; then
    printf 'Firebase Android config is not registered for io.jimin.os.\n' >&2
    return 1
  fi
  install -m 600 "${source_file}" "${target_file}"
}

cleanup_android_firebase_config() {
  local target_file="${1:?Firebase target path is required}"
  rm -f "${target_file}"
}
