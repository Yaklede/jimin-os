#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
config_file="${1:-${REPO_ROOT}/deploy/env/local.env.example}"
init_deployment local "${config_file}"
validate_runtime_secrets

"${SCRIPT_DIR}/validate-compose.sh" local "${config_file}"
info "Building pinned local images"
compose build --pull gateway api agent
info "Starting local services"
compose up --detach --remove-orphans --wait --wait-timeout 180

if [[ "${DEPLOY_TLS_MODE}" == "internal" ]]; then
  ca_file="$(export_internal_ca)"
  JIMIN_TLS_CA_FILE="${ca_file}" "${SCRIPT_DIR}/smoke-deployment.sh" local "${config_file}"
else
  "${SCRIPT_DIR}/smoke-deployment.sh" local "${config_file}"
fi

ensure_state_directory
pending="${DEPLOY_STATE_ROOT}/desired.env"
write_desired_release "${pending}"
record_successful_release "${pending}"
rm -f "${pending}"
info "Local deployment passed smoke checks"
