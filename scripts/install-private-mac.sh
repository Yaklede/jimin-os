#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SOURCE_APP="${REPO_ROOT}/target/release/bundle/macos/Jimin OS.app"
INSTALLED_APP="/Applications/Jimin OS.app"
requested_server_url="${1:-}"
backup_dir=""
previous_app=""

source "${SCRIPT_DIR}/lib/client-build-config.sh"

server_url="$(production_server_url "${requested_server_url}")"

restore_previous_app() {
  local status=$?
  trap - EXIT
  if [[ ${status} -ne 0 && -n "${previous_app}" && -e "${previous_app}" ]]; then
    if [[ -e "${INSTALLED_APP}" ]]; then
      mv "${INSTALLED_APP}" "${backup_dir}/failed-install.app"
    fi
    mv "${previous_app}" "${INSTALLED_APP}"
    printf 'Installation failed; restored the previous macOS app.\n' >&2
  fi
  exit "${status}"
}

trap restore_previous_app EXIT
"${SCRIPT_DIR}/build-private-client.sh" macos "${server_url}"

built_identifier="$(defaults read "${SOURCE_APP}/Contents/Info" CFBundleIdentifier 2>/dev/null || true)"
if [[ "${built_identifier}" != "io.jimin.os" ]]; then
  printf 'Refusing to install a non-production macOS bundle: %s\n' "${built_identifier:-unknown}" >&2
  exit 1
fi

backup_dir="$(mktemp -d "${TMPDIR:-/tmp}/jimin-os-mac-install.XXXXXX")"
pkill -f "${INSTALLED_APP}/Contents/MacOS/jimin-desktop" 2>/dev/null || true
if [[ -e "${INSTALLED_APP}" ]]; then
  previous_app="${backup_dir}/previous.app"
  mv "${INSTALLED_APP}" "${previous_app}"
fi
ditto "${SOURCE_APP}" "${INSTALLED_APP}"
codesign --force --deep --sign - "${INSTALLED_APP}"
codesign --verify --deep --strict --verbose=2 "${INSTALLED_APP}"
open "${INSTALLED_APP}"

trap - EXIT
printf 'Installed private-server macOS app with server: %s\n' "${server_url}"
if [[ -n "${previous_app}" ]]; then
  printf 'Previous app backup: %s\n' "${previous_app}"
fi
