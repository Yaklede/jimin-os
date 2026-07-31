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

unset JIMIN_RELEASE_ENV
worker_digest_a="$(printf 'a%.0s' {1..64})"
worker_digest_b="$(printf 'b%.0s' {1..64})"
worker_release_a="${temporary_root}/meeting-transcriber-a.env"
worker_release_b="${temporary_root}/meeting-transcriber-b.env"
printf 'JIMIN_MEETING_TRANSCRIBER_IMAGE=registry.invalid/transcriber@sha256:%s\n' \
  "${worker_digest_a}" > "${worker_release_a}"
printf 'JIMIN_MEETING_TRANSCRIBER_IMAGE=registry.invalid/transcriber@sha256:%s\n' \
  "${worker_digest_b}" > "${worker_release_b}"
worker_state_root="$(meeting_transcriber_state_root)"

record_successful_meeting_transcriber_release "${worker_release_a}"
cmp -s "${worker_state_root}/current.env" "${worker_release_a}"
[[ ! -f "${worker_state_root}/previous.env" ]]

record_successful_meeting_transcriber_release "${worker_release_b}"
cmp -s "${worker_state_root}/current.env" "${worker_release_b}"
cmp -s "${worker_state_root}/previous.env" "${worker_release_a}"

record_rollback_meeting_transcriber_release "${worker_release_a}"
cmp -s "${worker_state_root}/current.env" "${worker_release_a}"
cmp -s "${worker_state_root}/previous.env" "${worker_release_b}"

info "Meeting transcriber state transition tests passed"

scope_secrets="${temporary_root}/scope-secrets"
mkdir -p "${scope_secrets}"
for secret_name in \
  postgres_password \
  api_database_url \
  auth_signing_key \
  auth_verify_key \
  auth_refresh_pepper \
  auth_pairing_pepper; do
  printf 'audit-secret-value\n' > "${scope_secrets}/${secret_name}"
done
chmod 600 "${scope_secrets}/"*

scope_config="${temporary_root}/scope-config.env"
core_digest="$(printf 'c%.0s' {1..64})"
{
  printf 'JIMIN_API_IMAGE=registry.invalid/api@sha256:%s\n' "${core_digest}"
  printf 'JIMIN_AGENT_IMAGE=registry.invalid/agent@sha256:%s\n' "${core_digest}"
  printf 'JIMIN_GATEWAY_IMAGE=registry.invalid/gateway@sha256:%s\n' "${core_digest}"
  printf 'JIMIN_BUILD_SHA=%s\n' "$(printf 'd%.0s' {1..40})"
  printf 'JIMIN_GOOGLE_CALENDAR_OAUTH_ENABLED=0\n'
  printf 'JIMIN_FIREBASE_MESSAGING_ENABLED=0\n'
  printf 'JIMIN_ITSM_ENABLED=1\n'
  printf 'JIMIN_ITSM_BASE_URL=https://itsm.bix.bz\n'
  printf 'JIMIN_MEETING_TRANSCRIBER_ENABLED=1\n'
  printf 'JIMIN_MEETING_TRANSCRIBER_IMAGE=invalid-worker-reference\n'
} > "${scope_config}"
DEPLOY_CONFIG_FILE="${scope_config}"
JIMIN_SECRETS_DIR="${scope_secrets}"
DEPLOY_TLS_MODE=internal
export DEPLOY_CONFIG_FILE JIMIN_SECRETS_DIR DEPLOY_TLS_MODE

if (validate_runtime_secrets core >/dev/null 2>&1); then
  die "core secret scope accepted a missing ITSM read credential"
fi
printf 'audit-itsm-read-credential\n' > "${scope_secrets}/itsm_read_credential"
chmod 600 "${scope_secrets}/itsm_read_credential"
validate_runtime_secrets core
validate_staging_images core
if (validate_runtime_secrets meeting-transcriber >/dev/null 2>&1); then
  die "meeting transcriber secret scope accepted a missing Hugging Face token"
fi
if (validate_staging_images meeting-transcriber >/dev/null 2>&1); then
  die "meeting transcriber image scope accepted an invalid worker digest"
fi

printf 'audit-hugging-face-token\n' > "${scope_secrets}/hugging_face_token"
chmod 600 "${scope_secrets}/hugging_face_token"
printf 'JIMIN_MEETING_TRANSCRIBER_IMAGE=registry.invalid/transcriber@sha256:%s\n' \
  "${worker_digest_a}" >> "${scope_config}"
validate_runtime_secrets meeting-transcriber
validate_staging_images meeting-transcriber

info "Core Agent and meeting transcriber preflight scopes are isolated"

itsm_disabled_config="${temporary_root}/itsm-disabled.env"
itsm_enabled_config="${temporary_root}/itsm-enabled.env"
itsm_invalid_config="${temporary_root}/itsm-invalid.env"
for target in \
  "${itsm_disabled_config}" \
  "${itsm_enabled_config}" \
  "${itsm_invalid_config}"; do
  {
    printf 'JIMIN_COMPOSE_PROJECT=jimin-os-deploy-test\n'
    printf 'JIMIN_TLS_MODE=internal\n'
    printf 'JIMIN_SECRETS_DIR=%s\n' "${scope_secrets}"
    printf 'JIMIN_GOOGLE_CALENDAR_OAUTH_ENABLED=0\n'
    printf 'JIMIN_FIREBASE_MESSAGING_ENABLED=0\n'
    printf 'JIMIN_MEETING_TRANSCRIBER_ENABLED=0\n'
  } > "${target}"
done
printf 'JIMIN_ITSM_ENABLED=0\n' >> "${itsm_disabled_config}"
{
  printf 'JIMIN_ITSM_ENABLED=1\n'
  printf 'JIMIN_ITSM_BASE_URL=https://itsm.bix.bz\n'
} >> "${itsm_enabled_config}"
printf 'JIMIN_ITSM_ENABLED=yes\n' >> "${itsm_invalid_config}"

unset JIMIN_RELEASE_ENV
init_deployment local "${itsm_disabled_config}" core
if printf '%s\n' "${COMPOSE_ARGS[@]}" | grep -Fq 'compose.itsm.yaml'; then
  die "disabled ITSM integration selected the Compose overlay"
fi
init_deployment local "${itsm_enabled_config}" core
[[ "$(printf '%s\n' "${COMPOSE_ARGS[@]}" | grep -Fc 'compose.itsm.yaml')" == "1" ]] \
  || die "enabled ITSM integration did not select exactly one Compose overlay"
if (init_deployment local "${itsm_invalid_config}" core >/dev/null 2>&1); then
  die "invalid ITSM enable flag was accepted"
fi

info "ITSM Compose overlay selection is explicit and validated"

for core_script in \
  "${REPO_ROOT}/scripts/deploy-staging.sh" \
  "${REPO_ROOT}/scripts/rollback-staging.sh"; do
  if grep -Fq -- '--remove-orphans' "${core_script}"; then
    die "core lifecycle must not remove the separately managed meeting transcriber: ${core_script}"
  fi
done

info "Core lifecycle preserves separately managed optional services"
