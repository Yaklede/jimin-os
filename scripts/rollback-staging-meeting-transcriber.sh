#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
config_file="${1:-${REPO_ROOT}/deploy/env/staging.env.example}"
init_deployment staging "${config_file}" meeting-transcriber
[[ "$(effective_value JIMIN_MEETING_TRANSCRIBER_ENABLED)" == "1" ]] \
  || die "set JIMIN_MEETING_TRANSCRIBER_ENABLED=1 before rolling back the optional worker"
ensure_meeting_transcriber_state_directory
state_root="$(meeting_transcriber_state_root)"

rollback_target="${2:-}"
case "${rollback_target}" in
  current)
    release_file="${state_root}/current.env"
    ;;
  previous)
    release_file="${state_root}/previous.env"
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
[[ -f "${release_file}" ]] || die "meeting transcriber rollback release not found: ${release_file}"
JIMIN_RELEASE_ENV="${release_file}"
export JIMIN_RELEASE_ENV
init_deployment staging "${config_file}" meeting-transcriber
validate_runtime_secrets meeting-transcriber
validate_staging_images meeting-transcriber

info "Pulling the meeting transcriber rollback digest"
compose pull meeting-transcriber
info "Applying the meeting transcriber rollback without changing core services"
compose up \
  --detach \
  --no-deps \
  --no-build \
  --wait \
  --wait-timeout 180 \
  meeting-transcriber

if [[ "${DEPLOY_TLS_MODE}" == "internal" ]]; then
  ca_file="$(export_internal_ca)"
  env -u JIMIN_RELEASE_ENV \
    JIMIN_SMOKE_INCLUDE_MEETING_TRANSCRIBER=1 \
    JIMIN_TLS_CA_FILE="${ca_file}" \
    "${SCRIPT_DIR}/smoke-deployment.sh" staging "${config_file}" "${release_file}"
else
  env -u JIMIN_RELEASE_ENV \
    JIMIN_SMOKE_INCLUDE_MEETING_TRANSCRIBER=1 \
    "${SCRIPT_DIR}/smoke-deployment.sh" staging "${config_file}" "${release_file}"
fi

pending="${state_root}/rollback-success.env"
write_meeting_transcriber_release "${pending}"
record_rollback_meeting_transcriber_release "${pending}"
rm -f "${pending}"
info "Meeting transcriber rollback target passed health checks"
