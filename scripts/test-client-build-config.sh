#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
temporary_dir="$(mktemp -d "${TMPDIR:-/tmp}/jimin-os-client-config.XXXXXX")"

source "${SCRIPT_DIR}/lib/client-build-config.sh"

cleanup() {
  rm -rf "${temporary_dir}"
}
trap cleanup EXIT

expect_rejected() {
  local value="${1}"
  if production_server_url "${value}" >/dev/null 2>&1; then
    printf 'Expected server URL to be rejected: %s\n' "${value}" >&2
    exit 1
  fi
}

[[ "$(production_server_url 'https://os.jimin.ai.kr/')" == "https://os.jimin.ai.kr" ]]
require_android_emulator 'emulator-5554'
require_android_physical_device 'R5KL20581QR'
if require_android_emulator 'R5KL20581QR' >/dev/null 2>&1; then
  printf 'Expected a physical device to be rejected for local Android testing.\n' >&2
  exit 1
fi
if require_android_physical_device 'emulator-5554' >/dev/null 2>&1; then
  printf 'Expected an emulator to be rejected for production installation.\n' >&2
  exit 1
fi

mock_apkanalyzer="${temporary_dir}/apkanalyzer"
mock_apk="${temporary_dir}/client.apk"
touch "${mock_apk}"
cat >"${mock_apkanalyzer}" <<'EOF'
#!/usr/bin/env bash
case "${1:-} ${2:-}" in
  "manifest application-id")
    printf '%s\n' "${MOCK_ANDROID_APPLICATION_ID:-io.jimin.os}"
    ;;
  "manifest version-code")
    printf '%s\n' "${MOCK_ANDROID_VERSION_CODE:-1000}"
    ;;
  "manifest debuggable")
    printf '%s\n' "${MOCK_ANDROID_DEBUGGABLE:-false}"
    ;;
  "files list")
    printf '%s\n' "${MOCK_ANDROID_NATIVE_FILES:-/lib/arm64-v8a/libjimin_desktop_lib.so}"
    ;;
  *)
    printf 'Unexpected apkanalyzer invocation: %s\n' "$*" >&2
    exit 1
    ;;
esac
EOF
chmod +x "${mock_apkanalyzer}"
mock_apksigner="${temporary_dir}/apksigner"
cat >"${mock_apksigner}" <<'EOF'
#!/usr/bin/env bash
if [[ "${1:-}" != "verify" ]]; then
  printf 'Unexpected apksigner invocation: %s\n' "$*" >&2
  exit 1
fi
if [[ "${MOCK_ANDROID_SIGNATURE_VALID:-1}" != "1" ]]; then
  printf 'DOES NOT VERIFY\n' >&2
  exit 1
fi
apk_path="${!#}"
signer_digest="${MOCK_ANDROID_SIGNER_SHA256:-1111111111111111111111111111111111111111111111111111111111111111}"
if [[ "$(head -c 9 "${apk_path}" 2>/dev/null)" == "installed" ]]; then
  signer_digest="${MOCK_ANDROID_INSTALLED_SIGNER_SHA256:-${signer_digest}}"
fi
printf 'Verifies\n'
printf 'Number of signers: 1\n'
printf 'Signer #1 certificate SHA-256 digest: %s\n' "${signer_digest}"
EOF
chmod +x "${mock_apksigner}"
export JIMIN_ANDROID_APKANALYZER="${mock_apkanalyzer}"
export JIMIN_ANDROID_APKSIGNER="${mock_apksigner}"

JIMIN_ANDROID_APKANALYZER="${mock_apkanalyzer}" \
  MOCK_ANDROID_APPLICATION_ID='io.jimin.os.dev' \
verify_android_apk_application_id "${mock_apk}" 'io.jimin.os.dev'
if JIMIN_ANDROID_APKANALYZER="${mock_apkanalyzer}" \
  MOCK_ANDROID_APPLICATION_ID='io.jimin.os.dev' \
  verify_android_apk_application_id "${mock_apk}" 'io.jimin.os' >/dev/null 2>&1; then
  printf 'Expected a development APK to be rejected by the production verifier.\n' >&2
  exit 1
fi

release_output="${temporary_dir}/android-output/arm64/release"
mkdir -p "${release_output}"
release_apk="${release_output}/app-arm64-release.apk"
touch "${release_apk}"
[[ "$(private_android_release_apk "${temporary_dir}/android-output")" == "${release_apk}" ]]

touch "${release_output}/duplicate-release.apk"
if private_android_release_apk "${temporary_dir}/android-output" >/dev/null 2>&1; then
  printf 'Expected ambiguous private Android release APK lookup to fail.\n' >&2
  exit 1
fi
rm -f "${release_output}/duplicate-release.apk"
JIMIN_ANDROID_APKANALYZER="${mock_apkanalyzer}" \
  verify_private_android_release_apk "${release_apk}"
if MOCK_ANDROID_APPLICATION_ID='io.jimin.os.dev' \
  verify_private_android_release_apk "${release_apk}" >/dev/null 2>&1; then
  printf 'Expected a development application ID to fail the private release gate.\n' >&2
  exit 1
fi
if JIMIN_ANDROID_APKANALYZER="${mock_apkanalyzer}" \
  MOCK_ANDROID_DEBUGGABLE=true \
  verify_private_android_release_apk "${release_apk}" >/dev/null 2>&1; then
  printf 'Expected a debuggable private release APK to be rejected.\n' >&2
  exit 1
fi
if JIMIN_ANDROID_APKANALYZER="${mock_apkanalyzer}" \
  MOCK_ANDROID_NATIVE_FILES=$'/lib/arm64-v8a/libjimin_desktop_lib.so\n/lib/x86_64/libjimin_desktop_lib.so' \
  verify_private_android_release_apk "${release_apk}" >/dev/null 2>&1; then
  printf 'Expected a multi-ABI private release APK to be rejected.\n' >&2
  exit 1
fi
if MOCK_ANDROID_SIGNATURE_VALID=0 \
  verify_private_android_release_apk "${release_apk}" >/dev/null 2>&1; then
  printf 'Expected an invalid APK signature to fail the private release gate.\n' >&2
  exit 1
fi
if JIMIN_ANDROID_EXPECTED_SIGNER_SHA256='2222222222222222222222222222222222222222222222222222222222222222' \
  verify_private_android_release_apk "${release_apk}" >/dev/null 2>&1; then
  printf 'Expected an unexpected APK signer to fail the private release gate.\n' >&2
  exit 1
fi

mock_adb="${temporary_dir}/adb"
cat >"${mock_adb}" <<'EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "-s" ]]; then
  shift 2
fi
case "${1:-} ${2:-} ${3:-}" in
  "shell getprop ro.product.cpu.abilist")
    printf '%s\n' "${MOCK_ANDROID_DEVICE_ABIS:-arm64-v8a}"
    ;;
  "shell pm path")
    if [[ "${MOCK_ANDROID_PACKAGE_INSTALLED:-1}" == "1" ]]; then
      printf 'package:/data/app/io.jimin.os/base.apk\n'
    fi
    ;;
  "shell dumpsys package")
    printf '  versionCode=%s minSdk=24 targetSdk=36\n' \
      "${MOCK_ANDROID_INSTALLED_VERSION_CODE:-1000}"
    ;;
  "shell pidof io.jimin.os")
    printf '%s\n' "${MOCK_ANDROID_PID-1234}"
    ;;
  "shell dumpsys activity")
    printf '%s\n' \
      "${MOCK_ANDROID_ACTIVITY_STATE:-topResumedActivity=ActivityRecord{ io.jimin.os/.MainActivity }}"
    ;;
  "pull /data/app/io.jimin.os/base.apk "*)
    printf 'installed' >"${3:?pull destination is required}"
    ;;
  *)
    printf 'Unexpected adb invocation: %s\n' "$*" >&2
    exit 1
    ;;
esac
EOF
chmod +x "${mock_adb}"
export JIMIN_ANDROID_ADB="${mock_adb}"

verify_android_device_arm64 'R5KL20581QR'
if MOCK_ANDROID_DEVICE_ABIS='x86_64' \
  verify_android_device_arm64 'R5KL20581QR' >/dev/null 2>&1; then
  printf 'Expected a device without arm64-v8a support to be rejected.\n' >&2
  exit 1
fi
verify_android_device_update_compatibility \
  "${release_apk}" 'io.jimin.os' 'R5KL20581QR'
if MOCK_ANDROID_VERSION_CODE=999 \
  MOCK_ANDROID_INSTALLED_VERSION_CODE=1000 \
  verify_android_device_update_compatibility \
    "${release_apk}" 'io.jimin.os' 'R5KL20581QR' >/dev/null 2>&1; then
  printf 'Expected an Android versionCode downgrade to be rejected.\n' >&2
  exit 1
fi
if MOCK_ANDROID_INSTALLED_SIGNER_SHA256='2222222222222222222222222222222222222222222222222222222222222222' \
  verify_android_device_update_compatibility \
    "${release_apk}" 'io.jimin.os' 'R5KL20581QR' >/dev/null 2>&1; then
  printf 'Expected an installed-app signer mismatch to be rejected.\n' >&2
  exit 1
fi
verify_android_app_running 'R5KL20581QR' 'io.jimin.os' 1
if MOCK_ANDROID_PID='' \
  verify_android_app_running 'R5KL20581QR' 'io.jimin.os' 1 >/dev/null 2>&1; then
  printf 'Expected a missing Android app process to fail launch verification.\n' >&2
  exit 1
fi
if MOCK_ANDROID_ACTIVITY_STATE='topResumedActivity=ActivityRecord{ io.jimin.os.dev/.MainActivity }' \
  verify_android_app_running 'R5KL20581QR' 'io.jimin.os' 1 >/dev/null 2>&1; then
  printf 'Expected a missing Android app activity to fail launch verification.\n' >&2
  exit 1
fi

expect_rejected 'http://os.jimin.ai.kr'
expect_rejected 'https://os.jimin.ai.kr/api'
expect_rejected 'https://os.jimin.ai.kr?mode=private'
expect_rejected 'https://user:<password>@os.jimin.ai.kr'
expect_rejected 'https://localhost:8443'
expect_rejected 'https://127.0.0.1:8443'

mkdir -p "${temporary_dir}/valid" "${temporary_dir}/local"
printf 'const server="https://os.jimin.ai.kr";\n' >"${temporary_dir}/valid/index.js"
verify_production_web_assets "${temporary_dir}/valid" 'https://os.jimin.ai.kr'

printf 'const server="http://127.0.0.1:8080";\n' >"${temporary_dir}/local/index.js"
if verify_production_web_assets "${temporary_dir}/local" 'https://os.jimin.ai.kr' >/dev/null 2>&1; then
  printf 'Expected local-test assets to be rejected.\n' >&2
  exit 1
fi

if VITE_LOCAL_PHONE_TEST=1 production_server_url 'https://os.jimin.ai.kr' >/dev/null 2>&1; then
  printf 'Expected the local-test build flag to be rejected.\n' >&2
  exit 1
fi

firebase_source="${temporary_dir}/google-services.json"
firebase_target="${temporary_dir}/android/app/google-services.json"
mkdir -p "$(dirname "${firebase_target}")"
printf '%s\n' \
  '{"project_info":{"project_number":"422017005250","project_id":"jimin-os"},"client":[{"client_info":{"android_client_info":{"package_name":"io.jimin.os"}}}]}' \
  >"${firebase_source}"
prepare_android_firebase_config "${firebase_source}" "${firebase_target}"
cmp -s "${firebase_source}" "${firebase_target}"
firebase_mode="$(stat -c '%a' "${firebase_target}" 2>/dev/null || stat -f '%Lp' "${firebase_target}")"
[[ "${firebase_mode}" == "600" ]]
cleanup_android_firebase_config "${firebase_target}"
[[ ! -e "${firebase_target}" ]]

printf '%s\n' \
  '{"project_info":{"project_number":"422017005250","project_id":"jimin-os"},"client":[{"client_info":{"android_client_info":{"package_name":"com.example.wrong"}}}]}' \
  >"${firebase_source}"
if prepare_android_firebase_config "${firebase_source}" "${firebase_target}" >/dev/null 2>&1; then
  printf 'Expected a Firebase config for another package to be rejected.\n' >&2
  exit 1
fi

prepare_android_firebase_config "${temporary_dir}/missing.json" "${firebase_target}" >/dev/null
[[ ! -e "${firebase_target}" ]]

printf 'Client build configuration checks passed.\n'
