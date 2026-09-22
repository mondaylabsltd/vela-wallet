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

KIT_DIR="$RUST_DIR/../app-ios/VelaCoreKit"

echo "build-ios-xcframework: building the host dylib"
cargo build --release -p vela-core-uniffi
LIB_FILE="target/release/libvela_core_uniffi.dylib"

echo "build-ios-xcframework: generating Swift bindings"
cargo run --release -p vela-uniffi-bindgen --bin uniffi-bindgen -- generate \
  --library "$LIB_FILE" --language swift --out-dir bindings/swift

echo "build-ios-xcframework: building the device static library"
cargo build --release -p vela-core-uniffi --target aarch64-apple-ios

echo "build-ios-xcframework: building the simulator static library"
cargo build --release -p vela-core-uniffi --target aarch64-apple-ios-sim

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
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libvela_core_uniffi.a \
  -headers "$HDRS/ios" \
  -library target/aarch64-apple-ios-sim/release/libvela_core_uniffi.a \
  -headers "$HDRS/ios-sim" \
  -output "$XCFRAMEWORK"

echo "build-ios-xcframework: refreshing the committed Swift bindings"
mkdir -p "$KIT_DIR/Sources/VelaCore"
cp bindings/swift/vela_core_uniffi.swift "$KIT_DIR/Sources/VelaCore/vela_core_uniffi.swift"

echo "build-ios-xcframework: done — $XCFRAMEWORK"
