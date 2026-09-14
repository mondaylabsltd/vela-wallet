#!/usr/bin/env bash
#
# The parallel space's fixed keyset, for DEBUG iOS builds only (spec 052, D1).
#
#   rust/scripts/build-ios-dev-fixtures.sh
#
# The sibling of build-ios-xcframework.sh, for the SECOND library. It exists
# for one reason: uniffi's bindings verify every exported function against the
# library they load, so one bindings file cannot serve a library that carries
# the keyset and one that does not. A second library, linked only by the Debug
# configuration, is what lets a Release build carry neither the keys nor the
# door (FR-001) — and that is checked by inspecting an archived binary's
# symbols, not by reading the source.
#
# Writes:
#   app-ios/VelaDevFixturesKit/Artifacts/VelaDevFixturesFFI.xcframework  (gitignored)
#   app-ios/VelaWallet/VelaWallet/Dev/vela_dev_fixtures.swift            (committed)
#
# macOS only. Requires cargo, xcodebuild, and the rustup targets
# aarch64-apple-ios + aarch64-apple-ios-sim.
set -euo pipefail

RUST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RUST_DIR"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "build-ios-dev-fixtures: macOS only (needs xcodebuild and the Apple toolchains)" >&2
  exit 1
fi

KIT_DIR="$RUST_DIR/../app-ios/VelaDevFixturesKit"
DEV_SWIFT_DIR="$RUST_DIR/../app-ios/VelaWallet/VelaWallet/Dev"

echo "build-ios-dev-fixtures: building the host dylib"
cargo build --release -p vela-dev-fixtures-uniffi
LIB_FILE="target/release/libvela_dev_fixtures.dylib"

echo "build-ios-dev-fixtures: generating Swift bindings"
cargo run --release -p vela-dev-fixtures-uniffi --bin uniffi-bindgen -- generate \
  --library "$LIB_FILE" --language swift --out-dir bindings/swift-dev

echo "build-ios-dev-fixtures: building the device static library"
cargo build --release -p vela-dev-fixtures-uniffi --target aarch64-apple-ios

echo "build-ios-dev-fixtures: building the simulator static library"
cargo build --release -p vela-dev-fixtures-uniffi --target aarch64-apple-ios-sim

HDRS="target/xcframework-headers-dev"
rm -rf "$HDRS"
for SLICE in ios ios-sim; do
  mkdir -p "$HDRS/$SLICE"
  cp bindings/swift-dev/vela_dev_fixturesFFI.h "$HDRS/$SLICE/"
  cp bindings/swift-dev/vela_dev_fixturesFFI.modulemap "$HDRS/$SLICE/module.modulemap"
done

echo "build-ios-dev-fixtures: assembling VelaDevFixturesFFI.xcframework"
XCFRAMEWORK="$KIT_DIR/Artifacts/VelaDevFixturesFFI.xcframework"
rm -rf "$XCFRAMEWORK"
mkdir -p "$KIT_DIR/Artifacts"
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libvela_dev_fixtures.a \
  -headers "$HDRS/ios" \
  -library target/aarch64-apple-ios-sim/release/libvela_dev_fixtures.a \
  -headers "$HDRS/ios-sim" \
  -output "$XCFRAMEWORK"

echo "build-ios-dev-fixtures: refreshing the committed Swift bindings"
mkdir -p "$DEV_SWIFT_DIR"
cp bindings/swift-dev/vela_dev_fixtures.swift "$DEV_SWIFT_DIR/vela_dev_fixtures.swift"

echo "build-ios-dev-fixtures: done — $XCFRAMEWORK"
