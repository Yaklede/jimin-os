#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_LIB_DIR}/../.." && pwd)"
VERSIONS_ENV="${REPO_ROOT}/deploy/versions.env"

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

info() {
  printf '==> %s\n' "$*"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

is_non_root_user() {
  local configured_user="${1:-}"
  local principal="${configured_user%%:*}"

  [[ -n "${principal}" && "${principal}" != "root" && ! "${principal}" =~ ^0+$ ]]
}

read_env_file_value() {
  local file="$1"
  local key="$2"

  awk -v wanted="${key}" '
    /^[[:space:]]*#/ || /^[[:space:]]*$/ { next }
    {
      line = $0
      sub(/^[[:space:]]*export[[:space:]]+/, "", line)
      separator = index(line, "=")
      if (separator == 0) next
      name = substr(line, 1, separator - 1)
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", name)
      if (name != wanted) next
      value = substr(line, separator + 1)
      sub(/\r$/, "", value)
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
      if ((substr(value, 1, 1) == "\"" && substr(value, length(value), 1) == "\"") ||
          (substr(value, 1, 1) == "\047" && substr(value, length(value), 1) == "\047")) {
        value = substr(value, 2, length(value) - 2)
      }
      result = value
      found = 1
    }
    END { if (found) print result }
  ' "${file}"
}

effective_value() {
  local key="$1"
  local value=""

  if [[ "${key}" == "JIMIN_SECRETS_DIR" ]] && declare -p "${key}" >/dev/null 2>&1; then
    printf '%s\n' "${!key}"
    return
  fi

  value="$(read_env_file_value "${VERSIONS_ENV}" "${key}")"
  if [[ -n "${DEPLOY_CONFIG_FILE:-}" ]]; then
    local configured
    configured="$(read_env_file_value "${DEPLOY_CONFIG_FILE}" "${key}")"
    if [[ -n "${configured}" ]]; then
      value="${configured}"
    fi
  fi
  if [[ -n "${JIMIN_RELEASE_ENV:-}" ]]; then
    local released
    released="$(read_env_file_value "${JIMIN_RELEASE_ENV}" "${key}")"
    if [[ -n "${released}" ]]; then
      value="${released}"
    fi
  fi

  printf '%s\n' "${value}"
}

resolve_repo_path() {
  local value="$1"
  if [[ "${value}" = /* ]]; then
    printf '%s\n' "${value}"
  else
    printf '%s/%s\n' "${REPO_ROOT}" "${value#./}"
  fi
}

init_deployment() {
  local environment="$1"
  local config_file="$2"
  local component_scope="${3:-all}"

  require_command docker
  [[ -f "${VERSIONS_ENV}" ]] || die "versions file not found: ${VERSIONS_ENV}"
  [[ -f "${config_file}" ]] || die "configuration file not found: ${config_file}"
  [[ "${component_scope}" =~ ^(all|core|meeting-transcriber)$ ]] \
    || die "deployment component scope must be all, core, or meeting-transcriber"

  DEPLOY_ENVIRONMENT="${environment}"
  DEPLOY_CONFIG_FILE="$(cd "$(dirname "${config_file}")" && pwd)/$(basename "${config_file}")"
  DEPLOY_PROJECT="$(effective_value JIMIN_COMPOSE_PROJECT)"
  DEPLOY_TLS_MODE="$(effective_value JIMIN_TLS_MODE)"
  DEPLOY_COMPONENT_SCOPE="${component_scope}"

  [[ "${DEPLOY_ENVIRONMENT}" =~ ^(local|staging)$ ]] || die "environment must be local or staging"
  [[ "${DEPLOY_PROJECT}" =~ ^[a-z0-9][a-z0-9_-]+$ ]] || die "invalid Compose project name"
  [[ "${DEPLOY_TLS_MODE}" =~ ^(internal|files)$ ]] || die "JIMIN_TLS_MODE must be internal or files"

  COMPOSE_ARGS=(
    docker compose
    --project-directory "${REPO_ROOT}"
    --project-name "${DEPLOY_PROJECT}"
    --env-file "${VERSIONS_ENV}"
    --env-file "${DEPLOY_CONFIG_FILE}"
    --file "${REPO_ROOT}/deploy/compose.yaml"
  )

  if [[ "${DEPLOY_ENVIRONMENT}" == "staging" ]]; then
    COMPOSE_ARGS+=(--file "${REPO_ROOT}/deploy/compose.staging.yaml")
  fi
  if [[ "${DEPLOY_TLS_MODE}" == "files" ]]; then
    COMPOSE_ARGS+=(--file "${REPO_ROOT}/deploy/compose.tls-files.yaml")
  fi
  calendar_oauth_enabled="$(effective_value JIMIN_GOOGLE_CALENDAR_OAUTH_ENABLED)"
  calendar_oauth_enabled="${calendar_oauth_enabled:-0}"
  [[ "${calendar_oauth_enabled}" =~ ^[01]$ ]] \
    || die "JIMIN_GOOGLE_CALENDAR_OAUTH_ENABLED must be 0 or 1"
  if [[ "${calendar_oauth_enabled}" == "1" ]]; then
    COMPOSE_ARGS+=(--file "${REPO_ROOT}/deploy/compose.google-calendar.yaml")
  fi
  fcm_enabled="$(effective_value JIMIN_FIREBASE_MESSAGING_ENABLED)"
  fcm_enabled="${fcm_enabled:-0}"
  [[ "${fcm_enabled}" =~ ^[01]$ ]] \
    || die "JIMIN_FIREBASE_MESSAGING_ENABLED must be 0 or 1"
  if [[ "${fcm_enabled}" == "1" ]]; then
    COMPOSE_ARGS+=(--file "${REPO_ROOT}/deploy/compose.fcm.yaml")
  fi
  meeting_transcriber_enabled="$(effective_value JIMIN_MEETING_TRANSCRIBER_ENABLED)"
  meeting_transcriber_enabled="${meeting_transcriber_enabled:-0}"
  [[ "${meeting_transcriber_enabled}" =~ ^[01]$ ]] \
    || die "JIMIN_MEETING_TRANSCRIBER_ENABLED must be 0 or 1"
  if [[ "${meeting_transcriber_enabled}" == "1" ]]; then
    COMPOSE_ARGS+=(--file "${REPO_ROOT}/deploy/compose.meeting-transcriber-client.yaml")
    if [[ "${DEPLOY_COMPONENT_SCOPE}" != "core" ]]; then
      COMPOSE_ARGS+=(--file "${REPO_ROOT}/deploy/compose.meeting-transcriber.yaml")
    fi
  elif [[ "${DEPLOY_COMPONENT_SCOPE}" == "meeting-transcriber" ]]; then
    die "meeting transcriber deployment scope requires JIMIN_MEETING_TRANSCRIBER_ENABLED=1"
  fi
  if [[ -n "${JIMIN_RELEASE_ENV:-}" ]]; then
    [[ -f "${JIMIN_RELEASE_ENV}" ]] || die "release env not found: ${JIMIN_RELEASE_ENV}"
    COMPOSE_ARGS+=(--env-file "${JIMIN_RELEASE_ENV}")
  fi

  DEPLOY_STATE_ROOT="${JIMIN_STATE_DIR:-${XDG_STATE_HOME:-${HOME}/.local/state}/jimin-os}/${DEPLOY_PROJECT}"
}

compose() {
  local -a sanitized_environment=(env)
  local key
  while IFS= read -r key; do
    [[ -n "${key}" && "${key}" != "JIMIN_SECRETS_DIR" ]] || continue
    sanitized_environment+=(-u "${key}")
  done < <(
    {
      env_file_keys "${VERSIONS_ENV}"
      env_file_keys "${DEPLOY_CONFIG_FILE}"
      if [[ -n "${JIMIN_RELEASE_ENV:-}" ]]; then
        env_file_keys "${JIMIN_RELEASE_ENV}"
      fi
    } | sort -u
  )
  "${sanitized_environment[@]}" "${COMPOSE_ARGS[@]}" "$@"
}

env_file_keys() {
  local file="$1"
  awk '
    /^[[:space:]]*#/ || /^[[:space:]]*$/ { next }
    {
      line = $0
      sub(/^[[:space:]]*export[[:space:]]+/, "", line)
      separator = index(line, "=")
      if (separator == 0) next
      name = substr(line, 1, separator - 1)
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", name)
      if (name ~ /^[A-Z_][A-Z0-9_]*$/) print name
    }
  ' "${file}"
}

reject_external_release_override() {
  [[ -z "${JIMIN_RELEASE_ENV:-}" ]] \
    || die "JIMIN_RELEASE_ENV is internal; select releases through the script arguments"
}

restore_agent_service() {
  compose up \
    --detach \
    --no-deps \
    --wait \
    --wait-timeout 60 \
    agent >/dev/null
}

validate_secret_file() {
  local file="$1"
  local label="$2"

  [[ -f "${file}" ]] || die "missing ${label}: ${file}"
  [[ -r "${file}" ]] || die "${label} is not readable: ${file}"
  [[ -s "${file}" ]] || die "${label} is empty: ${file}"
  if grep -Eqi 'replace[-_ ]with|example|fixture|changeme' "${file}"; then
    die "${label} still contains a placeholder"
  fi

  local mode
  if mode="$(stat -c '%a' "${file}" 2>/dev/null)" && [[ "${mode}" =~ ^[0-7]{3,4}$ ]]; then
    :
  else
    mode="$(stat -f '%Lp' "${file}")"
  fi
  case "${mode}" in
    400|600) ;;
    *) die "${label} must use mode 0400 or 0600; current mode is ${mode}" ;;
  esac
}

validate_runtime_secrets() {
  local scope="${1:-all}"
  local secrets_dir
  [[ "${scope}" =~ ^(all|core|meeting-transcriber)$ ]] \
    || die "secret validation scope must be all, core, or meeting-transcriber"
  secrets_dir="$(resolve_repo_path "$(effective_value JIMIN_SECRETS_DIR)")"
  [[ -d "${secrets_dir}" ]] || die "secret directory not found: ${secrets_dir}"

  if [[ "${scope}" != "meeting-transcriber" ]]; then
    validate_secret_file "${secrets_dir}/postgres_password" "PostgreSQL password file"
    validate_secret_file "${secrets_dir}/api_database_url" "API database URL file"
    validate_secret_file "${secrets_dir}/auth_signing_key" "access-token signing key file"
    validate_secret_file "${secrets_dir}/auth_verify_key" "access-token verify key file"
    validate_secret_file "${secrets_dir}/auth_refresh_pepper" "refresh-token pepper file"
    validate_secret_file "${secrets_dir}/auth_pairing_pepper" "device-pairing pepper file"
    if [[ "$(effective_value JIMIN_GOOGLE_CALENDAR_OAUTH_ENABLED)" == "1" ]]; then
      validate_secret_file "${secrets_dir}/google_calendar_client_secret" "Google Calendar client secret file"
      validate_secret_file "${secrets_dir}/calendar_encryption_key" "Calendar encryption key file"
    fi
    if [[ "$(effective_value JIMIN_FIREBASE_MESSAGING_ENABLED)" == "1" ]]; then
      validate_secret_file "${secrets_dir}/firebase_service_account" "Firebase service-account file"
    fi
    if [[ "${DEPLOY_TLS_MODE}" == "files" ]]; then
      validate_secret_file "${secrets_dir}/gateway_tls_cert" "gateway certificate"
      validate_secret_file "${secrets_dir}/gateway_tls_key" "gateway private key"
    fi
  fi
  if [[ "${scope}" != "core" ]] \
    && [[ "$(effective_value JIMIN_MEETING_TRANSCRIBER_ENABLED)" == "1" ]]; then
    validate_secret_file "${secrets_dir}/hugging_face_token" "Hugging Face token file"
  fi
  if [[ "${scope}" == "meeting-transcriber" ]] \
    && [[ "$(effective_value JIMIN_MEETING_TRANSCRIBER_ENABLED)" != "1" ]]; then
    die "meeting transcriber secret validation requires JIMIN_MEETING_TRANSCRIBER_ENABLED=1"
  fi
}

assert_digest_reference() {
  local value="$1"
  local label="$2"
  [[ "${value}" =~ ^[^[:space:]]+@sha256:[0-9a-f]{64}$ ]] || die "${label} must be an immutable sha256 digest reference"
}

validate_staging_images() {
  local scope="${1:-all}"
  [[ "${scope}" =~ ^(all|core|meeting-transcriber)$ ]] \
    || die "image validation scope must be all, core, or meeting-transcriber"

  if [[ "${scope}" != "meeting-transcriber" ]]; then
    assert_digest_reference "$(effective_value JIMIN_API_IMAGE)" "JIMIN_API_IMAGE"
    assert_digest_reference "$(effective_value JIMIN_AGENT_IMAGE)" "JIMIN_AGENT_IMAGE"
    assert_digest_reference "$(effective_value JIMIN_GATEWAY_IMAGE)" "JIMIN_GATEWAY_IMAGE"
    assert_digest_reference "$(effective_value POSTGRES_IMAGE)" "POSTGRES_IMAGE"

    local build_sha
    build_sha="$(effective_value JIMIN_BUILD_SHA)"
    [[ "${build_sha}" =~ ^[0-9a-f]{40}$ ]] \
      || die "JIMIN_BUILD_SHA must be a full 40-character Git SHA for staging"
  fi
  if [[ "${scope}" != "core" ]] \
    && [[ "$(effective_value JIMIN_MEETING_TRANSCRIBER_ENABLED)" == "1" ]]; then
    assert_digest_reference \
      "$(effective_value JIMIN_MEETING_TRANSCRIBER_IMAGE)" \
      "JIMIN_MEETING_TRANSCRIBER_IMAGE"
  fi
  if [[ "${scope}" == "meeting-transcriber" ]] \
    && [[ "$(effective_value JIMIN_MEETING_TRANSCRIBER_ENABLED)" != "1" ]]; then
    die "meeting transcriber image validation requires JIMIN_MEETING_TRANSCRIBER_ENABLED=1"
  fi
}

ensure_state_directory() {
  umask 077
  mkdir -p "${DEPLOY_STATE_ROOT}/releases" "${DEPLOY_STATE_ROOT}/ca"
  chmod 700 "${DEPLOY_STATE_ROOT}" "${DEPLOY_STATE_ROOT}/releases" "${DEPLOY_STATE_ROOT}/ca"
}

write_desired_release() {
  local target="$1"
  local temporary="${target}.tmp.$$"

  umask 077
  {
    printf 'JIMIN_API_IMAGE=%s\n' "$(effective_value JIMIN_API_IMAGE)"
    printf 'JIMIN_AGENT_IMAGE=%s\n' "$(effective_value JIMIN_AGENT_IMAGE)"
    printf 'JIMIN_GATEWAY_IMAGE=%s\n' "$(effective_value JIMIN_GATEWAY_IMAGE)"
    printf 'JIMIN_BUILD_SHA=%s\n' "$(effective_value JIMIN_BUILD_SHA)"
  } > "${temporary}"
  chmod 600 "${temporary}"
  mv "${temporary}" "${target}"
}

record_successful_release() {
  local pending="$1"
  local timestamp
  timestamp="$(date -u '+%Y%m%dT%H%M%SZ')"

  ensure_state_directory
  if [[ -f "${DEPLOY_STATE_ROOT}/current.env" ]]; then
    cp "${DEPLOY_STATE_ROOT}/current.env" "${DEPLOY_STATE_ROOT}/previous.env"
  fi
  cp "${pending}" "${DEPLOY_STATE_ROOT}/current.env"
  cp "${pending}" "${DEPLOY_STATE_ROOT}/releases/${timestamp}.env"
  chmod 600 "${DEPLOY_STATE_ROOT}/current.env" "${DEPLOY_STATE_ROOT}/releases/${timestamp}.env"
  if [[ -f "${DEPLOY_STATE_ROOT}/previous.env" ]]; then
    chmod 600 "${DEPLOY_STATE_ROOT}/previous.env"
  fi
}

record_rollback_release() {
  local target="$1"
  local timestamp
  timestamp="$(date -u '+%Y%m%dT%H%M%SZ')"

  ensure_state_directory
  if [[ -f "${DEPLOY_STATE_ROOT}/current.env" ]]; then
    cp "${DEPLOY_STATE_ROOT}/current.env" "${DEPLOY_STATE_ROOT}/releases/${timestamp}-replaced.env"
    chmod 600 "${DEPLOY_STATE_ROOT}/releases/${timestamp}-replaced.env"
    if ! cmp -s "${DEPLOY_STATE_ROOT}/current.env" "${target}"; then
      cp "${DEPLOY_STATE_ROOT}/current.env" "${DEPLOY_STATE_ROOT}/previous.env"
      chmod 600 "${DEPLOY_STATE_ROOT}/previous.env"
    fi
  fi
  cp "${target}" "${DEPLOY_STATE_ROOT}/current.env"
  cp "${target}" "${DEPLOY_STATE_ROOT}/releases/${timestamp}-rollback.env"
  chmod 600 \
    "${DEPLOY_STATE_ROOT}/current.env" \
    "${DEPLOY_STATE_ROOT}/releases/${timestamp}-rollback.env"
}

meeting_transcriber_state_root() {
  printf '%s/meeting-transcriber\n' "${DEPLOY_STATE_ROOT}"
}

ensure_meeting_transcriber_state_directory() {
  local state_root
  state_root="$(meeting_transcriber_state_root)"
  umask 077
  mkdir -p "${state_root}/releases"
  chmod 700 "${state_root}" "${state_root}/releases"
}

write_meeting_transcriber_release() {
  local target="$1"
  local temporary="${target}.tmp.$$"

  umask 077
  printf 'JIMIN_MEETING_TRANSCRIBER_IMAGE=%s\n' \
    "$(effective_value JIMIN_MEETING_TRANSCRIBER_IMAGE)" > "${temporary}"
  chmod 600 "${temporary}"
  mv "${temporary}" "${target}"
}

record_successful_meeting_transcriber_release() {
  local pending="$1"
  local state_root
  local timestamp
  state_root="$(meeting_transcriber_state_root)"
  timestamp="$(date -u '+%Y%m%dT%H%M%SZ')"

  ensure_meeting_transcriber_state_directory
  if [[ -f "${state_root}/current.env" ]]; then
    cp "${state_root}/current.env" "${state_root}/previous.env"
  fi
  cp "${pending}" "${state_root}/current.env"
  cp "${pending}" "${state_root}/releases/${timestamp}.env"
  chmod 600 "${state_root}/current.env" "${state_root}/releases/${timestamp}.env"
  if [[ -f "${state_root}/previous.env" ]]; then
    chmod 600 "${state_root}/previous.env"
  fi
}

record_rollback_meeting_transcriber_release() {
  local target="$1"
  local state_root
  local timestamp
  state_root="$(meeting_transcriber_state_root)"
  timestamp="$(date -u '+%Y%m%dT%H%M%SZ')"

  ensure_meeting_transcriber_state_directory
  if [[ -f "${state_root}/current.env" ]]; then
    cp "${state_root}/current.env" "${state_root}/releases/${timestamp}-replaced.env"
    chmod 600 "${state_root}/releases/${timestamp}-replaced.env"
    if ! cmp -s "${state_root}/current.env" "${target}"; then
      cp "${state_root}/current.env" "${state_root}/previous.env"
      chmod 600 "${state_root}/previous.env"
    fi
  fi
  cp "${target}" "${state_root}/current.env"
  cp "${target}" "${state_root}/releases/${timestamp}-rollback.env"
  chmod 600 \
    "${state_root}/current.env" \
    "${state_root}/releases/${timestamp}-rollback.env"
}

bootstrap_meeting_transcriber_release_state() {
  local state_root
  local container_id
  local running_image
  local bootstrap
  state_root="$(meeting_transcriber_state_root)"
  ensure_meeting_transcriber_state_directory
  [[ ! -f "${state_root}/current.env" ]] || return 0

  container_id="$(compose ps --quiet meeting-transcriber)"
  [[ -n "${container_id}" ]] || return 0
  running_image="$(docker inspect --format '{{.Config.Image}}' "${container_id}")"
  assert_digest_reference "${running_image}" "running meeting transcriber image"

  bootstrap="${state_root}/bootstrap.env"
  umask 077
  printf 'JIMIN_MEETING_TRANSCRIBER_IMAGE=%s\n' "${running_image}" > "${bootstrap}"
  chmod 600 "${bootstrap}"
  record_successful_meeting_transcriber_release "${bootstrap}"
  rm -f "${bootstrap}"
}

export_internal_ca() {
  local target="${1:-${DEPLOY_STATE_ROOT}/ca/root.crt}"
  local temporary="${target}.tmp.$$"

  ensure_state_directory
  compose exec -T gateway cat /data/caddy/pki/authorities/local/root.crt > "${temporary}"
  chmod 644 "${temporary}"
  mv "${temporary}" "${target}"
  printf '%s\n' "${target}"
}
