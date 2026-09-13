#!/usr/bin/env bash
# The parallel space's fixed keyset for DEBUG Android builds (spec 043, D2):
# the host library + its Kotlin bindings (rust/bindings/kotlin-dev, gitignored,
# consumed by the debug source set only), and the three ABIs into
# app/src/debug/jniLibs (packaged in debug only). Release builds never see
# either — that is the whole point of a second library.
#
#   bash rust/scripts/build-dev-fixtures.sh          # host + bindings + ABIs
#   bash rust/scripts/build-dev-fixtures.sh --host   # host + bindings only (JVM tests)
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/app-android/vela-wallet/app/src/debug/jniLibs"
cd "$ROOT/rust"

cargo build --release -p vela-dev-fixtures-uniffi
case "$(uname -s)" in
  Darwin) LIB="target/release/libvela_dev_fixtures.dylib" ;;
  *)      LIB="target/release/libvela_dev_fixtures.so" ;;
esac
cargo run --release -p vela-dev-fixtures-uniffi --bin uniffi-bindgen -- generate \
  --library "$LIB" --language kotlin --out-dir bindings/kotlin-dev --no-format
echo "OK: bindings/kotlin-dev from $LIB"

if [ "${1:-}" = "--host" ]; then
  exit 0
fi

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  ANDROID_NDK_HOME="$(ls -d "$SDK/ndk/"*/ 2>/dev/null | sort -V | tail -1)"
  ANDROID_NDK_HOME="${ANDROID_NDK_HOME%/}"
fi
export ANDROID_NDK_HOME
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 --platform 29 -o "$OUT" \
  build --release -p vela-dev-fixtures-uniffi
echo "OK: $(find "$OUT" -name 'libvela_dev_fixtures.so' | wc -l | tr -d ' ') ABIs in $OUT"
