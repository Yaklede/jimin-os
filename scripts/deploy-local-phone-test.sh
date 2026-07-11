#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
config_file="${1:-${REPO_ROOT}/deploy/env/local.env.example}"
init_deployment local "${config_file}"
COMPOSE_ARGS+=(--file "${REPO_ROOT}/deploy/compose.local-phone-test.yaml")
validate_runtime_secrets

JIMIN_LOCAL_PHONE_TEST_PORT="${JIMIN_LOCAL_PHONE_TEST_PORT:-8080}"
[[ "${JIMIN_LOCAL_PHONE_TEST_PORT}" =~ ^[1-9][0-9]{0,4}$ ]] \
  && (( JIMIN_LOCAL_PHONE_TEST_PORT <= 65535 )) \
  || die "JIMIN_LOCAL_PHONE_TEST_PORT must be a valid TCP port"
export JIMIN_LOCAL_PHONE_TEST_PORT

"${SCRIPT_DIR}/validate-compose.sh" local "${config_file}"
compose config --quiet
info "Building local phone-test images"
compose build --pull gateway api agent
info "Starting local phone-test services"
# Local images intentionally keep stable development tags. Recreate services
# after every build so the emulator never tests an older API container.
compose up --detach --remove-orphans --force-recreate --wait --wait-timeout 180

curl --fail --silent --show-error \
  "http://127.0.0.1:${JIMIN_LOCAL_PHONE_TEST_PORT}/health/ready" >/dev/null
info "Local phone-test API is ready on USB loopback port ${JIMIN_LOCAL_PHONE_TEST_PORT}"
