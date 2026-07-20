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

printf 'Client build configuration checks passed.\n'
