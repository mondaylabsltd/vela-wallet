#!/usr/bin/env bash
#
# The Rust core for ONE Android ABI — the CI emulator's — instead of the three
# rust/scripts/build-android.sh cross-compiles for a shipped APK (~6 minutes
# of it is the two ARM ABIs an x86_64 emulator never loads).
#
#   scripts/ci/android-core-emulator.sh [abi]      (default x86_64)
#
# Same output directory, platform level and 16 KB page alignment as
# build-android.sh, so Gradle then runs with -PvelaSkipRustBuild and packages
# what is there. The Kotlin bindings must already be generated
# (rust/scripts/build-kotlin-bindings.sh): Gradle's freshness check refuses to
# compile against bindings from another core.
#
# Needs: an NDK under $ANDROID_HOME/ndk (GitHub's ubuntu image ships one),
# cargo-ndk, and the abi's Rust target (x86_64-linux-android).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ABI="${1:-x86_64}"
OUT="$ROOT/app-android/vela-wallet/app/src/main/jniLibs"

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  ANDROID_NDK_HOME="$(printf '%s\n' "$SDK/ndk/"*/ 2>/dev/null | grep -v '\*' | sort -V | tail -1)"
  ANDROID_NDK_HOME="${ANDROID_NDK_HOME%/}"
fi
[ -d "${ANDROID_NDK_HOME:-}" ] || { echo "android-core-emulator: no NDK under $SDK/ndk" >&2; exit 1; }
export ANDROID_NDK_HOME

PAGE16K="-C link-arg=-Wl,-z,max-page-size=16384"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="${CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS:-} $PAGE16K"
export CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS="${CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS:-} $PAGE16K"

cd "$ROOT/rust"
# minSdk 29, as build-android.sh (app/build.gradle.kts).
cargo ndk -t "$ABI" --platform 29 -o "$OUT" build --release -p vela-core-uniffi
ls -l "$OUT/$ABI/libvela_core_uniffi.so"
