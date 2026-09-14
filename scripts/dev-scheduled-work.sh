#!/usr/bin/env bash
# Isolated local API only. Never reads production credentials or launches clients.
set -euo pipefail
umask 077
runtime=/tmp/jimin-scheduled-work-dev
mkdir -p "$runtime"
openssl_bin="${OPENSSL_BIN:-/opt/homebrew/opt/openssl@3/bin/openssl}"
if [[ ! -s "$runtime/signing.pem" || ! -s "$runtime/verify.pem" ]]; then
  "$openssl_bin" genpkey -algorithm ED25519 -out "$runtime/signing.pem"
  "$openssl_bin" pkey -in "$runtime/signing.pem" -pubout -out "$runtime/verify.pem"
  "$openssl_bin" rand -hex -out "$runtime/refresh" 32
  "$openssl_bin" rand -hex -out "$runtime/pairing" 32
fi
export DATABASE_URL="${JIMIN_SCHEDULED_DEV_DATABASE_URL:?Set the isolated scheduled_dev database URL}"
case "$DATABASE_URL" in
  postgres://*@127.0.0.1:15456/scheduled_dev) ;;
  *) printf 'Only the isolated loopback database is allowed.\n' >&2; exit 1 ;;
esac
export JIMIN_API_BIND_ADDR=127.0.0.1:18220
export JIMIN_AUTH_ISSUER=http://127.0.0.1:18220
export JIMIN_AUTH_KEY_ID=scheduled-work-dev
export JIMIN_AUTH_SIGNING_KEY_FILE="$runtime/signing.pem"
export JIMIN_AUTH_VERIFY_KEY_FILE="$runtime/verify.pem"
export JIMIN_AUTH_REFRESH_PEPPER_FILE="$runtime/refresh"
export JIMIN_AUTH_PAIRING_PEPPER_FILE="$runtime/pairing"
export JIMIN_TRUSTED_NETWORK=1
export JIMIN_BUILD_SHA=scheduled-work-dev
exec "${JIMIN_DEV_API_BINARY:-target/debug/jimin-api}"
