#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
environment="${1:-local}"
config_file="${2:-${REPO_ROOT}/deploy/env/${environment}.env.example}"
release_file="${3:-}"
if [[ -n "${release_file}" ]]; then
  [[ "${release_file}" == /* && -f "${release_file}" ]] \
    || die "smoke release file must be an existing absolute path"
  JIMIN_RELEASE_ENV="${release_file}"
  export JIMIN_RELEASE_ENV
fi
init_deployment "${environment}" "${config_file}"
require_command curl

hostname="$(effective_value JIMIN_OS_HOSTNAME)"
port="$(effective_value JIMIN_SMOKE_PORT)"
port="${port:-$(effective_value JIMIN_GATEWAY_HOST_PORT)}"
smoke_tls_mode="$(effective_value JIMIN_SMOKE_TLS_MODE)"
smoke_tls_mode="${smoke_tls_mode:-${DEPLOY_TLS_MODE}}"
resolve_ip="$(effective_value JIMIN_SMOKE_RESOLVE_IP)"
[[ "${hostname}" =~ ^[a-zA-Z0-9.-]+$ ]] || die "invalid smoke hostname"
[[ "${port}" =~ ^[0-9]+$ ]] || die "invalid gateway port"
[[ "${smoke_tls_mode}" =~ ^(internal|files|public)$ ]] || die "invalid smoke TLS mode"

curl_args=(--fail --silent --show-error --connect-timeout 5 --max-time 15)
if [[ -n "${resolve_ip}" ]]; then
  curl_args+=(--resolve "${hostname}:${port}:${resolve_ip}")
fi

if [[ "${smoke_tls_mode}" == "internal" ]]; then
  ca_file="${JIMIN_TLS_CA_FILE:-${DEPLOY_STATE_ROOT}/ca/root.crt}"
  [[ -f "${ca_file}" ]] || die "internal CA root not found: ${ca_file}"
  curl_args+=(--cacert "${ca_file}")
elif [[ "${smoke_tls_mode}" == "files" && -n "${JIMIN_TLS_CA_FILE:-}" ]]; then
  [[ -f "${JIMIN_TLS_CA_FILE}" ]] || die "CA file not found: ${JIMIN_TLS_CA_FILE}"
  curl_args+=(--cacert "${JIMIN_TLS_CA_FILE}")
fi

base_url="https://${hostname}:${port}"
live_body="$(curl "${curl_args[@]}" "${base_url}/health/live")"
ready_body="$(curl "${curl_args[@]}" "${base_url}/health/ready")"
grep -Eq '"status"[[:space:]]*:[[:space:]]*"ok"' <<<"${live_body}" || die "unexpected liveness response"
grep -Eq '"status"[[:space:]]*:[[:space:]]*"ready"' <<<"${ready_body}" || die "unexpected readiness response"

compose exec -T api jimin-api probe live
compose exec -T api jimin-api probe ready

codex_version="$(effective_value CODEX_VERSION)"
compose exec -T agent codex --version | grep -F "${codex_version}" >/dev/null || die "Codex version mismatch"
compose exec -T agent /usr/bin/test -r /opt/jimin-agent/fixtures/generic-prompt.txt \
  || die "Agent verification fixture is missing or unreadable"

for service in gateway api agent postgres; do
  container_id="$(compose ps --quiet "${service}")"
  [[ -n "${container_id}" ]] || die "service is not running: ${service}"
  configured_user="$(docker inspect --format '{{.Config.User}}' "${container_id}")"
  is_non_root_user "${configured_user}" || die "service runs as root or has no configured user: ${service}"
  if docker inspect --format '{{range .Mounts}}{{println .Source "->" .Destination}}{{end}}' "${container_id}" | grep -Fq '/var/run/docker.sock'; then
    die "service mounts Docker socket: ${service}"
  fi
done

info "TLS, API readiness, Codex version, non-root, and socket checks passed"
