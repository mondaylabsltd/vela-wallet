#!/usr/bin/env bash
#
# Generate the Kotlin bindings the Android app compiles against — and stamp
# WHICH core they came from.
#
#   rust/scripts/build-kotlin-bindings.sh
#
# `rust/bindings/kotlin/` is gitignored and consumed in place as a
# `kotlin.srcDir`, so every checkout generates its own copy. Nothing rebuilt it
# when the core changed: Gradle cross-compiles the three .so files itself
# (`cargoNdkBuild`) but has never regenerated the Kotlin beside them, so the
# bindings quietly described an older core while `git status` stayed clean. The
# failure arrives as a wall of `Unresolved reference` from Kotlin — a compiler
# error that reads like a code bug and is not one. It cost two releases in a
# row (0.9.5 and 0.9.6) before it was named.
#
# The stamp is what makes it visible: check-android-core-fresh.sh compares it
# with the tree, and Gradle refuses to compile when they differ.
set -euo pipefail

RUST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RUST_DIR"

case "$(uname -s)" in
  Darwin) LIB="target/release/libvela_core_uniffi.dylib" ;;
  *)      LIB="target/release/libvela_core_uniffi.so" ;;
esac

echo "build-kotlin-bindings: building the host library"
cargo build --release -p vela-core-uniffi

echo "build-kotlin-bindings: generating Kotlin from $LIB"
cargo run --release -p vela-uniffi-bindgen --bin uniffi-bindgen -- generate \
  --library "$LIB" --language kotlin --out-dir bindings/kotlin --no-format

# Last, and only on success: a stamp that names a half-written generation would
# be worse than none.
"$RUST_DIR/scripts/core-fingerprint.sh" > bindings/kotlin/.core-fingerprint
echo "build-kotlin-bindings: done — $(cat bindings/kotlin/.core-fingerprint)"
