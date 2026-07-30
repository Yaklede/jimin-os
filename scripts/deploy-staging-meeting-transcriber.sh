#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
config_file="${1:-${REPO_ROOT}/deploy/env/staging.env.example}"
init_deployment staging "${config_file}"
[[ "$(effective_value JIMIN_MEETING_TRANSCRIBER_ENABLED)" == "1" ]] \
  || die "set JIMIN_MEETING_TRANSCRIBER_ENABLED=1 before deploying the optional worker"
validate_runtime_secrets
validate_staging_images

"${SCRIPT_DIR}/validate-compose.sh" staging "${config_file}"

info "Pulling the optional meeting transcriber image"
compose pull meeting-transcriber
info "Updating only the optional meeting transcriber service"
compose up \
  --detach \
  --no-deps \
  --no-build \
  --wait \
  --wait-timeout 180 \
  meeting-transcriber

if [[ "${DEPLOY_TLS_MODE}" == "internal" ]]; then
  ca_file="$(export_internal_ca)"
  JIMIN_SMOKE_INCLUDE_MEETING_TRANSCRIBER=1 \
    JIMIN_TLS_CA_FILE="${ca_file}" \
    "${SCRIPT_DIR}/smoke-deployment.sh" staging "${config_file}"
else
  JIMIN_SMOKE_INCLUDE_MEETING_TRANSCRIBER=1 \
    "${SCRIPT_DIR}/smoke-deployment.sh" staging "${config_file}"
fi

info "Meeting transcriber deployment passed its health checks"
