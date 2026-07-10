#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
config_file="${1:-${REPO_ROOT}/deploy/env/staging.env.example}"
init_deployment staging "${config_file}"
validate_runtime_secrets
validate_staging_images

"${SCRIPT_DIR}/validate-compose.sh" staging "${config_file}"
ensure_state_directory
pending="${DEPLOY_STATE_ROOT}/desired.env"
write_desired_release "${pending}"

info "Pulling immutable staging images"
compose pull gateway api agent postgres
info "Starting staging services without local builds"
compose up --detach --remove-orphans --no-build --wait --wait-timeout 180

if [[ "${DEPLOY_TLS_MODE}" == "internal" ]]; then
  ca_file="$(export_internal_ca)"
  JIMIN_TLS_CA_FILE="${ca_file}" "${SCRIPT_DIR}/smoke-deployment.sh" staging "${config_file}"
else
  "${SCRIPT_DIR}/smoke-deployment.sh" staging "${config_file}"
fi

record_successful_release "${pending}"
rm -f "${pending}"
info "Staging deployment passed smoke checks and was recorded"
