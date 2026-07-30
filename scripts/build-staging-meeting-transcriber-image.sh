#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
[[ -z "${DEPLOY_CONFIG_FILE:-}" ]] \
  || die "DEPLOY_CONFIG_FILE is not accepted when building checked-in image pins"

registry_prefix="${1:-}"
platforms="${2:-linux/amd64}"
release_env="${3:-}"

[[ -n "${registry_prefix}" ]] \
  || die "usage: $0 <registry-prefix> [platforms] [release-env-output]"
[[ "${registry_prefix}" =~ ^[a-z0-9./_-]+$ ]] \
  || die "registry prefix contains unsupported characters"
[[ "${platforms}" =~ ^linux/(amd64|arm64)(,linux/(amd64|arm64))?$ ]] \
  || die "platforms must contain linux/amd64 and/or linux/arm64"

require_command docker
require_command git
docker buildx version >/dev/null

if [[ -n "$(git -C "${REPO_ROOT}" status --porcelain)" ]]; then
  die "meeting transcriber image must be built from a clean Git worktree"
fi

build_sha="$(git -C "${REPO_ROOT}" rev-parse HEAD)"
[[ "${build_sha}" =~ ^[0-9a-f]{40}$ ]] || die "could not resolve a full Git SHA"
short_sha="${build_sha:0:12}"
image_tag="${registry_prefix}/jimin-os-meeting-transcriber:sha-${short_sha}"
cache_args=()
if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
  cache_args+=(
    --cache-from "type=gha,scope=jimin-os-meeting-transcriber"
    --cache-to "type=gha,mode=max,scope=jimin-os-meeting-transcriber"
  )
fi

info "Building and pushing the optional meeting transcriber image"
docker buildx build \
  "${cache_args[@]}" \
  --platform "${platforms}" \
  --file "${REPO_ROOT}/services/meeting-transcriber/Dockerfile" \
  --build-arg "PYTHON_RUNTIME_IMAGE=$(effective_value PYTHON_RUNTIME_IMAGE)" \
  --build-arg "JIMIN_BUILD_SHA=${build_sha}" \
  --tag "${image_tag}" \
  --push \
  "${REPO_ROOT}"

manifest_digest=""
for attempt in {1..12}; do
  manifest_digest="$(
    docker buildx imagetools inspect "${image_tag}" 2>/dev/null \
      | awk '/^Digest:/ { print $2; exit }'
  )" || true
  if [[ "${manifest_digest}" =~ ^sha256:[0-9a-f]{64}$ ]]; then
    break
  fi
  if [[ ${attempt} -lt 12 ]]; then
    sleep 5
  fi
done
[[ "${manifest_digest}" =~ ^sha256:[0-9a-f]{64}$ ]] \
  || die "could not read meeting transcriber manifest digest"

if [[ -z "${release_env}" ]]; then
  release_env="${XDG_STATE_HOME:-${HOME}/.local/state}/jimin-os/builds/meeting-transcriber-${build_sha}.env"
fi
mkdir -p "$(dirname "${release_env}")"
umask 077
{
  printf 'JIMIN_MEETING_TRANSCRIBER_IMAGE=%s@%s\n' \
    "${image_tag}" "${manifest_digest}"
  printf 'JIMIN_MEETING_TRANSCRIBER_BUILD_SHA=%s\n' "${build_sha}"
} > "${release_env}"
chmod 600 "${release_env}"

info "Meeting transcriber image reference written to ${release_env}"
