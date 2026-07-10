#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/lib/deploy-common.sh"

reject_external_release_override
environment="${1:-local}"
config_file="${2:-${REPO_ROOT}/deploy/env/${environment}.env.example}"
target="${3:-}"
init_deployment "${environment}" "${config_file}"
[[ "${DEPLOY_TLS_MODE}" == "internal" ]] || die "CA export is only available for internal TLS mode"

if [[ -n "${target}" ]]; then
  export_internal_ca "${target}"
else
  export_internal_ca
fi
