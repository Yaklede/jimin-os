#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
environment="${1:-staging}"
config_file="${2:-${REPO_ROOT}/deploy/env/${environment}.env.example}"
init_deployment "${environment}" "${config_file}"

agent_was_running=0
if [[ -n "$(compose ps --quiet agent)" ]]; then
  agent_was_running=1
  info "Stopping Agent to avoid concurrent writes to CODEX_HOME"
  compose stop agent
fi

restore_agent() {
  local exit_status=$?
  trap - EXIT

  if [[ "${agent_was_running}" -eq 1 ]]; then
    info "Restoring the Agent service"
    if ! restore_agent_service; then
      printf 'error: failed to restore a healthy Agent service\n' >&2
      if [[ "${exit_status}" -eq 0 ]]; then
        exit_status=1
      fi
    fi
  fi

  exit "${exit_status}"
}
trap restore_agent EXIT

info "Starting the official Codex device-code login"
compose run --rm --no-deps --entrypoint /usr/local/bin/codex agent login --device-auth
info "Checking the authenticated account without printing credentials"
compose run --rm --no-deps --entrypoint /usr/local/bin/codex agent login status
