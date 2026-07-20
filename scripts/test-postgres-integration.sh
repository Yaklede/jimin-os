#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
postgres_image="$(sed -n 's/^POSTGRES_IMAGE=//p' "${REPO_ROOT}/deploy/versions.env")"
container_name="jimin-os-postgres-tests-$$"
database_user="jimin_test"
database_password="jimin_test_fixture"
test_names=()

cleanup() {
  docker rm -f "${container_name}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

if [[ -z "${postgres_image}" ]]; then
  printf 'POSTGRES_IMAGE is missing from deploy/versions.env.\n' >&2
  exit 1
fi
if ! command -v docker >/dev/null 2>&1; then
  printf 'Docker is required for PostgreSQL integration tests.\n' >&2
  exit 1
fi

cd "${REPO_ROOT}"
while IFS= read -r test_name; do
  test_names+=("${test_name}")
done < <(
  cargo test -p jimin-storage --test postgres -- --list \
    | sed -n 's/: test$//p'
)
if [[ ${#test_names[@]} -eq 0 ]]; then
  printf 'No PostgreSQL integration tests were discovered.\n' >&2
  exit 1
fi

docker run --detach --rm \
  --name "${container_name}" \
  --env "POSTGRES_USER=${database_user}" \
  --env "POSTGRES_PASSWORD=${database_password}" \
  --env POSTGRES_DB=postgres \
  --publish 127.0.0.1::5432 \
  "${postgres_image}" >/dev/null

for attempt in {1..30}; do
  if docker exec "${container_name}" pg_isready -U "${database_user}" -d postgres >/dev/null 2>&1; then
    break
  fi
  if [[ ${attempt} -eq 30 ]]; then
    printf 'PostgreSQL test container did not become ready.\n' >&2
    exit 1
  fi
  sleep 1
done

mapped_port="$(docker port "${container_name}" 5432/tcp | awk -F: 'NR == 1 {print $NF}')"
for index in "${!test_names[@]}"; do
  test_name="${test_names[${index}]}"
  database_name="jimin_test_$((index + 1))"
  test_database_url="postgres://${database_user}"
  test_database_url+=":${database_password}@127.0.0.1:${mapped_port}/${database_name}"
  docker exec "${container_name}" createdb -U "${database_user}" "${database_name}"
  printf '[%s/%s] %s\n' "$((index + 1))" "${#test_names[@]}" "${test_name}"
  JIMIN_TEST_DATABASE_URL="${test_database_url}" \
    cargo test -p jimin-storage --test postgres "${test_name}" -- --exact --test-threads=1
done

printf 'PostgreSQL integration tests passed: %s isolated scenarios.\n' "${#test_names[@]}"
