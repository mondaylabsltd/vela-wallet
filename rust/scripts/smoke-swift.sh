#!/usr/bin/env bash
#
# Replay the conformance corpus through the uniffi-generated Swift bindings.
#
#   rust/scripts/smoke-swift.sh
#
# Builds the cdylib, generates Swift bindings from the compiled library, then
# compiles and runs rust/harness/swift/main.swift against the committed
# vectors. Fourth surface of spec SC-001 (Rust, Web, Kotlin, Swift).
#
# macOS only (needs the Swift toolchain + a dylib to link against). Requires:
# cargo, swiftc.
set -euo pipefail

RUST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RUST_DIR"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "smoke-swift: macOS only (needs the Swift toolchain and a linkable dylib)" >&2
  exit 1
fi

echo "smoke-swift: building the cdylib"
cargo build --release -p vela-core-uniffi
LIB_FILE="target/release/libvela_core_uniffi.dylib"

echo "smoke-swift: generating Swift bindings"
cargo run --release -p vela-uniffi-bindgen --bin uniffi-bindgen -- generate \
  --library "$LIB_FILE" --language swift --out-dir bindings/swift

OUT="target/harness-swift"
rm -rf "$OUT" && mkdir -p "$OUT"

# The generated Swift imports the C shim through a module map; point swiftc at
# the header directory and hand it the modulemap as an implicit module.
cp bindings/swift/vela_core_uniffiFFI.h "$OUT/"
cp bindings/swift/vela_core_uniffiFFI.modulemap "$OUT/module.modulemap"

echo "smoke-swift: compiling the harness"
swiftc \
  -swift-version 5 \
  -I "$OUT" \
  -L target/release \
  -lvela_core_uniffi \
  -Xlinker -rpath -Xlinker "$RUST_DIR/target/release" \
  bindings/swift/vela_core_uniffi.swift \
  harness/swift/main.swift \
  -o "$OUT/harness" \
  -suppress-warnings

echo "smoke-swift: running"
"$OUT/harness" "$RUST_DIR/crates/vela-core/tests/vectors"
