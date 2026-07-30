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
  (
    local firebase_target="${REPO_ROOT}/apps/desktop/src-tauri/gen/android/app/google-services.json"
    local firebase_source="${JIMIN_ANDROID_GOOGLE_SERVICES_FILE:-${REPO_ROOT}/deploy/secrets/staging/google-services.json}"
    prepare_android_firebase_config "${firebase_source}" "${firebase_target}"
    trap 'cleanup_android_firebase_config "${firebase_target}"' EXIT
    # Existing personal-device installs are debug-certificate signed. Build a
    # release-mode APK with that same certificate so Rust symbols and unused
    # Android resources are stripped without forcing an uninstall/data loss.
    ORG_GRADLE_PROJECT_jiminPrivateReleaseDebugSigning=true \
    VITE_API_BASE_URL="${server_url}" \
      VITE_LOCAL_PHONE_TEST=0 \
      pnpm --filter @jimin-os/desktop tauri android build \
        --apk --target aarch64 --split-per-abi --ci
  )
  verify_production_web_assets "${assets_dir}" "${server_url}"
  local apk_path
  apk_path="$(
    private_android_release_apk \
      "${REPO_ROOT}/apps/desktop/src-tauri/gen/android/app/build/outputs/apk"
  )"
  verify_private_android_release_apk "${apk_path}"
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
