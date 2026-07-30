#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
config_file="${1:-${REPO_ROOT}/deploy/env/staging.env.example}"
init_deployment staging "${config_file}" meeting-transcriber
[[ "$(effective_value JIMIN_MEETING_TRANSCRIBER_ENABLED)" == "1" ]] \
  || die "set JIMIN_MEETING_TRANSCRIBER_ENABLED=1 before deploying the optional worker"
validate_runtime_secrets meeting-transcriber
validate_staging_images meeting-transcriber

"${SCRIPT_DIR}/validate-compose.sh" staging "${config_file}" meeting-transcriber
ensure_meeting_transcriber_state_directory
bootstrap_meeting_transcriber_release_state
state_root="$(meeting_transcriber_state_root)"
pending="${state_root}/desired.env"
write_meeting_transcriber_release "${pending}"

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

record_successful_meeting_transcriber_release "${pending}"
rm -f "${pending}"
info "Meeting transcriber deployment passed its health checks"
