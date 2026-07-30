#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

temporary_root="$(mktemp -d)"
trap 'rm -rf "${temporary_root}"' EXIT
DEPLOY_STATE_ROOT="${temporary_root}/state"
export DEPLOY_STATE_ROOT
ensure_state_directory

release_a="${temporary_root}/release-a.env"
release_b="${temporary_root}/release-b.env"
release_z="${temporary_root}/release-z.env"
printf 'JIMIN_BUILD_SHA=%040d\n' 1 > "${release_a}"
printf 'JIMIN_BUILD_SHA=%040d\n' 2 > "${release_b}"
printf 'JIMIN_BUILD_SHA=%040d\n' 0 > "${release_z}"

cp "${release_a}" "${DEPLOY_STATE_ROOT}/current.env"
cp "${release_z}" "${DEPLOY_STATE_ROOT}/previous.env"

record_rollback_release "${release_a}"
cmp -s "${DEPLOY_STATE_ROOT}/current.env" "${release_a}"
cmp -s "${DEPLOY_STATE_ROOT}/previous.env" "${release_z}"

record_rollback_release "${release_z}"
cmp -s "${DEPLOY_STATE_ROOT}/current.env" "${release_z}"
cmp -s "${DEPLOY_STATE_ROOT}/previous.env" "${release_a}"

record_successful_release "${release_b}"
cmp -s "${DEPLOY_STATE_ROOT}/current.env" "${release_b}"
cmp -s "${DEPLOY_STATE_ROOT}/previous.env" "${release_z}"

info "Deployment state transition tests passed"

config_file="${temporary_root}/config.env"
release_file="${temporary_root}/selected-release.env"
printf 'JIMIN_BUILD_SHA=%040d\n' 3 > "${config_file}"
printf 'JIMIN_BUILD_SHA=%040d\n' 4 > "${release_file}"
DEPLOY_CONFIG_FILE="${config_file}"
export DEPLOY_CONFIG_FILE
JIMIN_BUILD_SHA="$(printf '9%.0s' {1..40})"
CODEX_VERSION=9.9.9
export JIMIN_BUILD_SHA CODEX_VERSION
unset JIMIN_RELEASE_ENV
[[ "$(effective_value JIMIN_BUILD_SHA)" == "$(printf '0%.0s' {1..39})3" ]]
[[ "$(effective_value CODEX_VERSION)" == "0.144.1" ]]
JIMIN_RELEASE_ENV="${release_file}"
export JIMIN_RELEASE_ENV
[[ "$(effective_value JIMIN_BUILD_SHA)" == "$(printf '0%.0s' {1..39})4" ]]

info "Authoritative environment precedence tests passed"

core_release="${temporary_root}/core-release.env"
write_desired_release "${core_release}"
grep -Fq 'JIMIN_API_IMAGE=' "${core_release}"
grep -Fq 'JIMIN_AGENT_IMAGE=' "${core_release}"
grep -Fq 'JIMIN_GATEWAY_IMAGE=' "${core_release}"
if grep -Fq 'JIMIN_MEETING_TRANSCRIBER_IMAGE=' "${core_release}"; then
  die "core release state must not pin the optional meeting transcriber image"
fi

info "Core release state excludes the optional meeting transcriber image"
