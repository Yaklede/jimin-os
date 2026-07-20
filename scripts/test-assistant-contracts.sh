#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"
cargo test -p jimin-agent
cargo test -p jimin-api
pnpm frontend:test
"${SCRIPT_DIR}/test-postgres-integration.sh"

printf 'Assistant command contract checks passed.\n'
