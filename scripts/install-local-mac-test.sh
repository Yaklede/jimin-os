#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SOURCE_APP="${REPO_ROOT}/target/release/bundle/macos/Jimin OS Dev.app"
INSTALLED_APP="/Applications/Jimin OS Dev.app"

cd "${REPO_ROOT}"

VITE_API_BASE_URL="${VITE_API_BASE_URL:-http://127.0.0.1:8080}" \
  VITE_LOCAL_PHONE_TEST=1 \
  pnpm --filter @jimin-os/desktop tauri build \
    --config src-tauri/tauri.dev.conf.json

pkill -f "${INSTALLED_APP}/Contents/MacOS/jimin-desktop" 2>/dev/null || true
rm -rf "${INSTALLED_APP}"
ditto "${SOURCE_APP}" "${INSTALLED_APP}"
codesign --force --deep --sign - "${INSTALLED_APP}"
codesign --verify --deep --strict --verbose=2 "${INSTALLED_APP}"
open "${INSTALLED_APP}"

printf 'Installed Jimin OS Dev without replacing Jimin OS; server: %s\n' "${VITE_API_BASE_URL:-http://127.0.0.1:8080}"
