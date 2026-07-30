#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
config_file="${1:-${REPO_ROOT}/deploy/env/staging.env.example}"
init_deployment staging "${config_file}" core
ensure_state_directory

rollback_target="${2:-}"
case "${rollback_target}" in
  current)
    release_file="${DEPLOY_STATE_ROOT}/current.env"
    ;;
  previous)
    release_file="${DEPLOY_STATE_ROOT}/previous.env"
    ;;
  /*)
    release_file="${rollback_target}"
    ;;
  "")
    die "rollback target is required: current, previous, or an absolute release file"
    ;;
  *)
    die "rollback target must be current, previous, or an absolute release file"
    ;;
esac
[[ -f "${release_file}" ]] || die "rollback release file not found: ${release_file}"
JIMIN_RELEASE_ENV="${release_file}"
export JIMIN_RELEASE_ENV
init_deployment staging "${config_file}" core
validate_runtime_secrets core
validate_staging_images core

info "Pulling rollback image digests"
services=(gateway api agent)
compose pull "${services[@]}"
info "Applying application-only rollback; database volumes are not replaced"
compose up \
  --detach \
  --no-build \
  --wait \
  --wait-timeout 180 \
  "${services[@]}"

if [[ "${DEPLOY_TLS_MODE}" == "internal" ]]; then
  ca_file="$(export_internal_ca)"
  env -u JIMIN_RELEASE_ENV \
    JIMIN_SMOKE_INCLUDE_MEETING_TRANSCRIBER=0 \
    JIMIN_TLS_CA_FILE="${ca_file}" \
    "${SCRIPT_DIR}/smoke-deployment.sh" staging "${config_file}" "${release_file}"
else
  env -u JIMIN_RELEASE_ENV \
    JIMIN_SMOKE_INCLUDE_MEETING_TRANSCRIBER=0 \
    "${SCRIPT_DIR}/smoke-deployment.sh" staging "${config_file}" "${release_file}"
fi

pending="${DEPLOY_STATE_ROOT}/rollback-success.env"
write_desired_release "${pending}"
record_rollback_release "${pending}"
rm -f "${pending}"
info "Rollback target passed smoke checks"
