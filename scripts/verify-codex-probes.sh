#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
environment="${1:-staging}"
config_file="${2:-${REPO_ROOT}/deploy/env/${environment}.env.example}"
init_deployment "${environment}" "${config_file}"
require_command jq

agent_was_running=0
if [[ -n "$(compose ps --status running --quiet agent)" ]]; then
  agent_was_running=1
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

info "Stopping Agent so one process owns CODEX_HOME"
compose stop agent >/dev/null

info "Probing the authenticated account without exposing credentials"
if ! account_summary="$(compose run --rm --no-deps agent probe account)"; then
  die "account probe command failed"
fi
if ! jq -e '
  .ok == true and
  .probe == "account" and
  .result.authenticated == true and
  .result.accountType == "chatgpt" and
  .result.runtimeState == "ready"
' >/dev/null <<<"${account_summary}"; then
  die "account probe did not confirm an authenticated ChatGPT account"
fi
info "Authenticated ChatGPT account confirmed"

info "Running the pinned non-personal turn fixture"
if ! turn_summary="$(compose run --rm --no-deps agent \
    probe turn \
    --model gpt-5.4 \
    --prompt-file /opt/jimin-agent/fixtures/generic-prompt.txt)"; then
  die "turn probe command failed"
fi
if ! jq -e '
  .ok == true and
  .probe == "turn" and
  .result.status == "completed" and
  .result.promptBytes > 0 and
  .result.deltaNotifications > 0 and
  .result.agentMessageItems > 0 and
  .result.responseBytes > 0 and
  (.result.responseSha256 | test("^[0-9a-f]{64}$"))
' >/dev/null <<<"${turn_summary}"; then
  die "turn probe did not complete successfully"
fi
info "Pinned turn fixture completed without logging prompt or response content"
