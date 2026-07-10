#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
[[ -z "${DEPLOY_CONFIG_FILE:-}" ]] \
  || die "DEPLOY_CONFIG_FILE is not accepted when building checked-in image pins"

registry_prefix="${1:-}"
platforms="${2:-linux/amd64,linux/arm64}"
release_env="${3:-}"

[[ -n "${registry_prefix}" ]] || die "usage: $0 <registry-prefix> [platforms] [release-env-output]"
[[ "${registry_prefix}" =~ ^[a-z0-9./_-]+$ ]] || die "registry prefix contains unsupported characters"
[[ "${platforms}" =~ ^linux/(amd64|arm64)(,linux/(amd64|arm64))?$ ]] || die "platforms must contain linux/amd64 and/or linux/arm64"

require_command docker
require_command git
docker buildx version >/dev/null

if [[ -n "$(git -C "${REPO_ROOT}" status --porcelain)" ]]; then
  die "staging images must be built from a clean Git worktree"
fi

build_sha="$(git -C "${REPO_ROOT}" rev-parse HEAD)"
[[ "${build_sha}" =~ ^[0-9a-f]{40}$ ]] || die "could not resolve a full Git SHA"
short_sha="${build_sha:0:12}"

api_tag="${registry_prefix}/jimin-os-api:sha-${short_sha}"
agent_tag="${registry_prefix}/jimin-os-agent:sha-${short_sha}"
gateway_tag="${registry_prefix}/jimin-os-gateway:sha-${short_sha}"

info "Building and pushing API image"
docker buildx build \
  --platform "${platforms}" \
  --file "${REPO_ROOT}/deploy/docker/api.Dockerfile" \
  --build-arg "RUST_BUILDER_IMAGE=$(effective_value RUST_BUILDER_IMAGE)" \
  --build-arg "DEBIAN_RUNTIME_IMAGE=$(effective_value DEBIAN_RUNTIME_IMAGE)" \
  --build-arg "JIMIN_BUILD_SHA=${build_sha}" \
  --tag "${api_tag}" \
  --push \
  "${REPO_ROOT}"

info "Building and pushing Agent image"
docker buildx build \
  --platform "${platforms}" \
  --file "${REPO_ROOT}/deploy/docker/agent.Dockerfile" \
  --build-arg "RUST_BUILDER_IMAGE=$(effective_value RUST_BUILDER_IMAGE)" \
  --build-arg "NODE_RUNTIME_IMAGE=$(effective_value NODE_RUNTIME_IMAGE)" \
  --build-arg "CODEX_VERSION=$(effective_value CODEX_VERSION)" \
  --build-arg "CODEX_NPM_INTEGRITY=$(effective_value CODEX_NPM_INTEGRITY)" \
  --build-arg "JIMIN_BUILD_SHA=${build_sha}" \
  --tag "${agent_tag}" \
  --push \
  "${REPO_ROOT}"

info "Building and pushing gateway image"
docker buildx build \
  --platform "${platforms}" \
  --file "${REPO_ROOT}/deploy/gateway/Dockerfile" \
  --build-arg "CADDY_BASE_IMAGE=$(effective_value CADDY_BASE_IMAGE)" \
  --build-arg "JIMIN_BUILD_SHA=${build_sha}" \
  --tag "${gateway_tag}" \
  --push \
  "${REPO_ROOT}/deploy/gateway"

manifest_digest() {
  local image="$1"
  docker buildx imagetools inspect "${image}" | awk '/^Digest:/ { print $2; exit }'
}

api_digest="$(manifest_digest "${api_tag}")"
agent_digest="$(manifest_digest "${agent_tag}")"
gateway_digest="$(manifest_digest "${gateway_tag}")"
[[ "${api_digest}" =~ ^sha256:[0-9a-f]{64}$ ]] || die "could not read API manifest digest"
[[ "${agent_digest}" =~ ^sha256:[0-9a-f]{64}$ ]] || die "could not read Agent manifest digest"
[[ "${gateway_digest}" =~ ^sha256:[0-9a-f]{64}$ ]] || die "could not read gateway manifest digest"

if [[ -z "${release_env}" ]]; then
  release_env="${XDG_STATE_HOME:-${HOME}/.local/state}/jimin-os/builds/${build_sha}.env"
fi
mkdir -p "$(dirname "${release_env}")"
umask 077
{
  printf 'JIMIN_API_IMAGE=%s@%s\n' "${api_tag}" "${api_digest}"
  printf 'JIMIN_AGENT_IMAGE=%s@%s\n' "${agent_tag}" "${agent_digest}"
  printf 'JIMIN_GATEWAY_IMAGE=%s@%s\n' "${gateway_tag}" "${gateway_digest}"
  printf 'JIMIN_BUILD_SHA=%s\n' "${build_sha}"
} > "${release_env}"
chmod 600 "${release_env}"

info "Release image references written to ${release_env}"
