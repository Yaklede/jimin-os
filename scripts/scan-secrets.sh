#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
candidate_list="$(mktemp)"
findings="$(mktemp)"
trap 'rm -f "${candidate_list}" "${findings}"' EXIT

git -C "${REPO_ROOT}" ls-files --cached --others --exclude-standard -z > "${candidate_list}"

while IFS= read -r -d '' relative_file; do
  file="${REPO_ROOT}/${relative_file}"
  [[ -f "${file}" ]] || continue

  case "${relative_file}" in
    auth.json|*/auth.json|credentials|*/credentials|credentials.toml|*/credentials.toml|\
    *.pem|*.key|*.p12|*.pfx|*.jks|*.keystore|*.mobileprovision|*.tfstate|*.tfstate.*|\
    *service-account*.json|.env|*/.env|.envrc|*/.envrc|.netrc|*/.netrc)
      printf '%s\n' "${relative_file}" >> "${findings}"
      continue
      ;;
  esac

  grep -Iq . "${file}" || continue
  if rg -q \
    '(-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----|ghp_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,}|sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{30,}|xox[baprs]-[0-9A-Za-z-]{20,}|npm_[A-Za-z0-9]{30,}|eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,})' \
    -- "${file}"; then
    printf '%s\n' "${relative_file}" >> "${findings}"
    continue
  fi
  if rg -n '://[^/@[:space:]:]+:[^/@[:space:]]+@' -- "${file}" \
    | rg -qv 'compose-validation-fixture|<encoded-password>|<password>'; then
    printf '%s\n' "${relative_file}" >> "${findings}"
    continue
  fi
  if [[ "$(basename "${relative_file}")" == ".npmrc" ]] \
    && rg -q '(_authToken|_auth|password)[[:space:]]*=' -- "${file}"; then
    printf '%s\n' "${relative_file}" >> "${findings}"
  fi
done < "${candidate_list}"

if [[ -s "${findings}" ]]; then
  printf 'error: potential credential material found in:\n' >&2
  sort -u "${findings}" >&2
  exit 1
fi

printf 'Secret scan passed.\n'
