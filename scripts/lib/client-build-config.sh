#!/usr/bin/env bash

set -Eeuo pipefail

JIMIN_OS_DEFAULT_SERVER_URL="https://os.jimin.ai.kr"
JIMIN_OS_LOCAL_TEST_SERVER_URL="http://127.0.0.1:8080"

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
