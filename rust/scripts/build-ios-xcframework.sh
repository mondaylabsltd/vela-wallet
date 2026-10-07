#!/usr/bin/env bash
#
# Build the iOS XCFramework + Swift sources for the VelaCoreKit package.
#
#   rust/scripts/build-ios-xcframework.sh
#
# Builds the host dylib, generates the Swift bindings from it (same recipe as
# smoke-swift.sh), cross-compiles the static library for device + simulator,
# and assembles app-ios/VelaCoreKit/Artifacts/VelaCoreFFI.xcframework. Also
# refreshes the committed app-ios/VelaCoreKit/Sources/VelaCore/
# vela_core_uniffi.swift.
#
# macOS only (needs xcodebuild + the Apple toolchains). Requires: cargo,
# xcodebuild, and the rustup targets aarch64-apple-ios + aarch64-apple-ios-sim.
set -euo pipefail

RUST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RUST_DIR"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "build-ios-xcframework: macOS only (needs xcodebuild and the Apple toolchains)" >&2
  exit 1
fi

# VELA_IOS_SIM_ONLY=1 (CI's simulator tests, ci.yml `ios`): the simulator slice
# alone, in the `ci-ios` profile — no LTO, parallel codegen. The device slice and
# `release`'s optimisation are for what ships (ios-package.yml, device runs),
# and they were most of the slowest CI job's time. Unset: exactly as before.
if [ "${VELA_IOS_SIM_ONLY:-0}" = 1 ]; then
  PROFILE=ci-ios
else
  PROFILE=release
fi

KIT_DIR="$RUST_DIR/../app-ios/VelaCoreKit"

echo "build-ios-xcframework: building the host dylib"
cargo build --profile "$PROFILE" -p vela-core-uniffi
LIB_FILE="target/$PROFILE/libvela_core_uniffi.dylib"

echo "build-ios-xcframework: generating Swift bindings"
cargo run --profile "$PROFILE" -p vela-uniffi-bindgen --bin uniffi-bindgen -- generate \
  --library "$LIB_FILE" --language swift --out-dir bindings/swift

if [ "$PROFILE" = release ]; then
  echo "build-ios-xcframework: building the device static library"
  cargo build --profile "$PROFILE" -p vela-core-uniffi --target aarch64-apple-ios
fi

echo "build-ios-xcframework: building the simulator static library"
cargo build --profile "$PROFILE" -p vela-core-uniffi --target aarch64-apple-ios-sim

# Each xcframework slice needs its own copy of the C headers, with the
# modulemap renamed to the clang-conventional module.modulemap.
HDRS="target/xcframework-headers"
rm -rf "$HDRS"
for SLICE in ios ios-sim; do
  mkdir -p "$HDRS/$SLICE"
  cp bindings/swift/vela_core_uniffiFFI.h "$HDRS/$SLICE/"
  cp bindings/swift/vela_core_uniffiFFI.modulemap "$HDRS/$SLICE/module.modulemap"
done

echo "build-ios-xcframework: assembling VelaCoreFFI.xcframework"
XCFRAMEWORK="$KIT_DIR/Artifacts/VelaCoreFFI.xcframework"
rm -rf "$XCFRAMEWORK"
mkdir -p "$KIT_DIR/Artifacts"
SLICES=(-library "target/aarch64-apple-ios-sim/$PROFILE/libvela_core_uniffi.a" -headers "$HDRS/ios-sim")
if [ "$PROFILE" = release ]; then
  SLICES=(-library "target/aarch64-apple-ios/$PROFILE/libvela_core_uniffi.a" -headers "$HDRS/ios" "${SLICES[@]}")
fi
xcodebuild -create-xcframework "${SLICES[@]}" -output "$XCFRAMEWORK"

echo "build-ios-xcframework: refreshing the committed Swift bindings"
mkdir -p "$KIT_DIR/Sources/VelaCore"
cp bindings/swift/vela_core_uniffi.swift "$KIT_DIR/Sources/VelaCore/vela_core_uniffi.swift"

# Stamp WHAT was built, so check-ios-core-fresh.sh can answer the only
# question a device test needs answered first: is this the core in my tree?
"$RUST_DIR/scripts/core-fingerprint.sh" > "$KIT_DIR/Artifacts/.core-fingerprint"

echo "build-ios-xcframework: done — $XCFRAMEWORK"
