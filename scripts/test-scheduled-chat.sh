#!/usr/bin/env bash
# Isolated PostgreSQL only. Never uses production credentials or a developer DB.
set -Eeuo pipefail
cd "$(dirname "$0")/.."
image="$(sed -n 's/^POSTGRES_IMAGE=//p' deploy/versions.env)"
name="jimin-scheduled-chat-test-$$"
created=false
cleanup() { if $created; then docker rm -f "$name" >/dev/null; fi; }
trap cleanup EXIT
docker run --detach --rm --name "$name" --env POSTGRES_USER=jimin_test --env POSTGRES_PASSWORD=jimin_test_fixture --env POSTGRES_DB=postgres --publish 127.0.0.1::5432 "$image" >/dev/null
created=true
for attempt in {1..30}; do
    if docker exec "$name" pg_isready -U jimin_test >/dev/null 2>&1; then break; fi
    if [[ $attempt == 30 ]]; then exit 1; fi
    sleep 1
done
port="$(docker port "$name" 5432/tcp | awk -F: 'NR==1 {print $NF}')"
for target in scheduled_chat_reply scheduled_work task_chat_routing; do
    docker exec "$name" createdb -U jimin_test "$target"
    JIMIN_TEST_DATABASE_URL="postgres://jimin_test:jimin_test_fixture@127.0.0.1:$port/$target" cargo test -p jimin-storage --test "$target" -- --ignored --test-threads=1
done
