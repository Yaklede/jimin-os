#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
environment="${1:-local}"
config_file="${2:-${REPO_ROOT}/deploy/env/${environment}.env.example}"
temporary_root="$(mktemp -d)"
trap 'rm -rf "${temporary_root}"' EXIT

mkdir -p "${temporary_root}/secrets"
printf 'compose-validation-fixture\n' > "${temporary_root}/secrets/postgres_password"
printf 'postgres://jimin_api:compose-validation-fixture@postgres:5432/jimin_os\n' > "${temporary_root}/secrets/api_database_url"
printf 'compose-validation-certificate\n' > "${temporary_root}/secrets/gateway_tls_cert"
printf 'compose-validation-private-key\n' > "${temporary_root}/secrets/gateway_tls_key"
chmod 600 "${temporary_root}/secrets/"*

export JIMIN_SECRETS_DIR="${temporary_root}/secrets"
export CODEX_VERSION=9.9.9
export JIMIN_API_IMAGE=caller.invalid/jimin-os-api:unexpected
if [[ "${environment}" == "staging" ]]; then
  digest="$(printf '0%.0s' {1..64})"
  release_env="${temporary_root}/release.env"
  {
    printf 'JIMIN_API_IMAGE=registry.invalid/jimin-os-api@sha256:%s\n' "${digest}"
    printf 'JIMIN_AGENT_IMAGE=registry.invalid/jimin-os-agent@sha256:%s\n' "${digest}"
    printf 'JIMIN_GATEWAY_IMAGE=registry.invalid/jimin-os-gateway@sha256:%s\n' "${digest}"
    printf 'JIMIN_BUILD_SHA=%s\n' "$(printf '0%.0s' {1..40})"
  } > "${release_env}"
  chmod 600 "${release_env}"
  export JIMIN_RELEASE_ENV="${release_env}"
fi

init_deployment "${environment}" "${config_file}"
rendered="${temporary_root}/compose.yaml"
compose config --quiet
compose config > "${rendered}"

expected_services=$'agent\napi\ngateway\npostgres'
actual_services="$(compose config --services | sort)"
[[ "${actual_services}" == "${expected_services}" ]] || die "unexpected Compose service set"

if grep -Fq '/var/run/docker.sock' "${rendered}"; then
  die "Docker socket mount is forbidden"
fi
if compose config --images | grep -Eq '(^|:)latest($|@)'; then
  die "floating latest image is forbidden"
fi
if grep -Fq 'compose-validation-fixture' "${rendered}"; then
  die "Compose rendered output exposed secret contents"
fi
if grep -Eq '9\.9\.9|caller\.invalid' "${rendered}"; then
  die "caller environment overrode an authoritative Compose value"
fi

assert_service_setting() {
  local service="$1"
  local pattern="$2"
  awk -v service="${service}" -v pattern="${pattern}" '
    $0 == "  " service ":" { in_service = 1; next }
    in_service && /^  [a-zA-Z0-9_-]+:$/ { exit }
    in_service && index($0, pattern) { found = 1 }
    END { exit(found ? 0 : 1) }
  ' "${rendered}" || die "service ${service} is missing setting: ${pattern}"
}

assert_service_without_ports() {
  local service="$1"
  if awk -v service="${service}" '
    $0 == "  " service ":" { in_service = 1; next }
    in_service && /^  [a-zA-Z0-9_-]+:$/ { exit }
    in_service && $0 == "    ports:" { found = 1 }
    END { exit(found ? 0 : 1) }
  ' "${rendered}"; then
    die "service ${service} must not publish host ports"
  fi
}

assert_service_non_root_user() {
  local service="$1"
  local configured_user

  configured_user="$(awk -v service="${service}" '
    $0 == "  " service ":" { in_service = 1; next }
    in_service && /^  [a-zA-Z0-9_-]+:$/ { exit }
    in_service && /^[[:space:]]+user:[[:space:]]*/ {
      value = $0
      sub(/^[[:space:]]+user:[[:space:]]*/, "", value)
      print value
      exit
    }
  ' "${rendered}")"
  configured_user="${configured_user#\"}"
  configured_user="${configured_user%\"}"
  configured_user="${configured_user#\'}"
  configured_user="${configured_user%\'}"

  is_non_root_user "${configured_user}" \
    || die "service ${service} has a root or empty user: ${configured_user:-<empty>}"
}

assert_dependency_condition() {
  local service="$1"
  local dependency="$2"
  local condition="$3"

  awk -v service="${service}" -v dependency="${dependency}" -v condition="${condition}" '
    $0 == "  " service ":" { in_service = 1; next }
    in_service && /^  [a-zA-Z0-9_-]+:$/ { exit }
    in_service && $0 == "    depends_on:" { in_dependencies = 1; next }
    in_dependencies && $0 == "      " dependency ":" { in_dependency = 1; next }
    in_dependency && $0 == "        condition: " condition { found = 1 }
    END { exit(found ? 0 : 1) }
  ' "${rendered}" || die "service ${service} dependency ${dependency} must use ${condition}"
}

for service in gateway api agent postgres; do
  assert_service_setting "${service}" 'read_only: true'
  assert_service_setting "${service}" 'no-new-privileges:true'
  assert_service_non_root_user "${service}"
done

for service in api agent postgres; do
  assert_service_without_ports "${service}"
done

assert_dependency_condition gateway api service_started
assert_dependency_condition api postgres service_started
assert_dependency_condition agent postgres service_started

for dockerfile in \
  "${REPO_ROOT}/deploy/docker/api.Dockerfile" \
  "${REPO_ROOT}/deploy/docker/agent.Dockerfile" \
  "${REPO_ROOT}/deploy/gateway/Dockerfile"; do
  grep -Eq '^USER [^[:space:]]+' "${dockerfile}" || die "Dockerfile has no non-root USER: ${dockerfile}"
  if grep -Eq '(^|[:=])latest([@[:space:]]|$)' "${dockerfile}"; then
    die "Dockerfile contains a floating latest reference: ${dockerfile}"
  fi
done

info "Compose validation passed for ${environment}"
