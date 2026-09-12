#!/usr/bin/env bash
# Cross-compile libvela_core_uniffi.so for the Android app (spec 008, research D2).
# Output goes to app-android/vela-wallet/app/src/main/jniLibs/ (gitignored — never commit).
# Prerequisites (one-time):
#   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
#   cargo install cargo-ndk
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/app-android/vela-wallet/app/src/main/jniLibs"

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  ANDROID_NDK_HOME="$(ls -d "$SDK/ndk/"*/ 2>/dev/null | sort -V | tail -1)"
  ANDROID_NDK_HOME="${ANDROID_NDK_HOME%/}"
fi
if [ -z "$ANDROID_NDK_HOME" ] || [ ! -d "$ANDROID_NDK_HOME" ]; then
  echo "error: no Android NDK found (set ANDROID_NDK_HOME, or install an NDK under $SDK/ndk)" >&2
  exit 1
fi
export ANDROID_NDK_HOME

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "error: cargo-ndk not installed — run: cargo install cargo-ndk" >&2
  exit 1
fi

# minSdk of the app is 29 (app-android/vela-wallet/app/build.gradle.kts).
cd "$ROOT/rust"
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 --platform 29 -o "$OUT" \
  build --release -p vela-core-uniffi

echo "OK: $(find "$OUT" -name 'libvela_core_uniffi.so' | wc -l | tr -d ' ') ABIs in $OUT"

# The parallel space's keyset for the DEBUG variant (spec 043, research D2):
# a second library into app/src/debug/jniLibs, which only a debug APK packages.
# VELA_SKIP_DEV_FIXTURES=1 skips it (a release-only build has no use for it).
if [ "${VELA_SKIP_DEV_FIXTURES:-0}" != "1" ]; then
  bash "$ROOT/rust/scripts/build-dev-fixtures.sh"
fi
