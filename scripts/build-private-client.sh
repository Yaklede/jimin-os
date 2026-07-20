#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
target="${1:-all}"
requested_server_url="${2:-}"

source "${SCRIPT_DIR}/lib/client-build-config.sh"

server_url="$(production_server_url "${requested_server_url}")"
assets_dir="${REPO_ROOT}/apps/desktop/dist/assets"

build_macos() {
  VITE_API_BASE_URL="${server_url}" \
    VITE_LOCAL_PHONE_TEST=0 \
    pnpm --filter @jimin-os/desktop tauri:build
  verify_production_web_assets "${assets_dir}" "${server_url}"
}

build_android() {
  VITE_API_BASE_URL="${server_url}" \
    VITE_LOCAL_PHONE_TEST=0 \
    pnpm --filter @jimin-os/desktop tauri android build \
      --debug --apk --target aarch64 --ci
  verify_production_web_assets "${assets_dir}" "${server_url}"
}

cd "${REPO_ROOT}"
case "${target}" in
  web)
    VITE_API_BASE_URL="${server_url}" \
      VITE_LOCAL_PHONE_TEST=0 \
      pnpm --filter @jimin-os/desktop build
    verify_production_web_assets "${assets_dir}" "${server_url}"
    ;;
  macos)
    build_macos
    ;;
  android)
    build_android
    ;;
  all)
    build_macos
    build_android
    ;;
  *)
    printf 'Usage: %s [web|macos|android|all] [https://private-server-origin]\n' "$0" >&2
    exit 1
    ;;
esac

printf 'Built %s private-server client with server: %s\n' "${target}" "${server_url}"
