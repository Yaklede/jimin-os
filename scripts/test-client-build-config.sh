#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
temporary_dir="$(mktemp -d "${TMPDIR:-/tmp}/jimin-os-client-config.XXXXXX")"

source "${SCRIPT_DIR}/lib/client-build-config.sh"

cleanup() {
  rm -rf "${temporary_dir}"
}
trap cleanup EXIT

expect_rejected() {
  local value="${1}"
  if production_server_url "${value}" >/dev/null 2>&1; then
    printf 'Expected server URL to be rejected: %s\n' "${value}" >&2
    exit 1
  fi
}

[[ "$(production_server_url 'https://os.jimin.ai.kr/')" == "https://os.jimin.ai.kr" ]]
expect_rejected 'http://os.jimin.ai.kr'
expect_rejected 'https://os.jimin.ai.kr/api'
expect_rejected 'https://os.jimin.ai.kr?mode=private'
expect_rejected 'https://user:<password>@os.jimin.ai.kr'
expect_rejected 'https://localhost:8443'
expect_rejected 'https://127.0.0.1:8443'

mkdir -p "${temporary_dir}/valid" "${temporary_dir}/local"
printf 'const server="https://os.jimin.ai.kr";\n' >"${temporary_dir}/valid/index.js"
verify_production_web_assets "${temporary_dir}/valid" 'https://os.jimin.ai.kr'

printf 'const server="http://127.0.0.1:8080";\n' >"${temporary_dir}/local/index.js"
if verify_production_web_assets "${temporary_dir}/local" 'https://os.jimin.ai.kr' >/dev/null 2>&1; then
  printf 'Expected local-test assets to be rejected.\n' >&2
  exit 1
fi

if VITE_LOCAL_PHONE_TEST=1 production_server_url 'https://os.jimin.ai.kr' >/dev/null 2>&1; then
  printf 'Expected the local-test build flag to be rejected.\n' >&2
  exit 1
fi

firebase_source="${temporary_dir}/google-services.json"
firebase_target="${temporary_dir}/android/app/google-services.json"
mkdir -p "$(dirname "${firebase_target}")"
printf '%s\n' \
  '{"project_info":{"project_number":"422017005250","project_id":"jimin-os"},"client":[{"client_info":{"android_client_info":{"package_name":"io.jimin.os"}}}]}' \
  >"${firebase_source}"
prepare_android_firebase_config "${firebase_source}" "${firebase_target}"
cmp -s "${firebase_source}" "${firebase_target}"
firebase_mode="$(stat -c '%a' "${firebase_target}" 2>/dev/null || stat -f '%Lp' "${firebase_target}")"
[[ "${firebase_mode}" == "600" ]]
cleanup_android_firebase_config "${firebase_target}"
[[ ! -e "${firebase_target}" ]]

printf '%s\n' \
  '{"project_info":{"project_number":"422017005250","project_id":"jimin-os"},"client":[{"client_info":{"android_client_info":{"package_name":"com.example.wrong"}}}]}' \
  >"${firebase_source}"
if prepare_android_firebase_config "${firebase_source}" "${firebase_target}" >/dev/null 2>&1; then
  printf 'Expected a Firebase config for another package to be rejected.\n' >&2
  exit 1
fi

prepare_android_firebase_config "${temporary_dir}/missing.json" "${firebase_target}" >/dev/null
[[ ! -e "${firebase_target}" ]]

printf 'Client build configuration checks passed.\n'
