#!/usr/bin/env bash
#
# Replay the conformance corpus through the uniffi-generated Kotlin bindings.
#
#   rust/scripts/smoke-kotlin.sh
#
# Builds the cdylib, generates Kotlin bindings from the compiled library, then
# compiles and runs rust/harness/kotlin/Harness.kt against the committed
# vectors. Third surface of spec SC-001 (Rust, Web, Kotlin, Swift).
#
# Requires: cargo, kotlinc, java. JNA and org.json are fetched from Maven
# Central into rust/target/harness-libs on first run (both are compile+runtime
# dependencies of the generated bindings and the harness respectively).
set -euo pipefail

RUST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RUST_DIR"

LIB_DIR="target/harness-libs"
JNA_VERSION="5.14.0"
JSON_VERSION="20240303"
JNA_JAR="$LIB_DIR/jna-$JNA_VERSION.jar"
JSON_JAR="$LIB_DIR/json-$JSON_VERSION.jar"

mkdir -p "$LIB_DIR"
if [ ! -f "$JNA_JAR" ]; then
  echo "smoke-kotlin: fetching JNA $JNA_VERSION"
  curl -fsSL -o "$JNA_JAR" \
    "https://repo1.maven.org/maven2/net/java/dev/jna/jna/$JNA_VERSION/jna-$JNA_VERSION.jar"
fi
if [ ! -f "$JSON_JAR" ]; then
  echo "smoke-kotlin: fetching org.json $JSON_VERSION"
  curl -fsSL -o "$JSON_JAR" \
    "https://repo1.maven.org/maven2/org/json/json/$JSON_VERSION/json-$JSON_VERSION.jar"
fi

echo "smoke-kotlin: building the cdylib"
cargo build --release -p vela-core-uniffi

# .dylib on macOS, .so on Linux — JNA resolves the library by name from
# jna.library.path, so only the directory matters at run time.
case "$(uname -s)" in
  Darwin) LIB_FILE="target/release/libvela_core_uniffi.dylib" ;;
  *)      LIB_FILE="target/release/libvela_core_uniffi.so" ;;
esac
if [ ! -f "$LIB_FILE" ]; then
  echo "smoke-kotlin: expected library at $LIB_FILE — did the build target change?" >&2
  exit 1
fi

echo "smoke-kotlin: generating Kotlin bindings"
cargo run --release -p vela-uniffi-bindgen --bin uniffi-bindgen -- generate \
  --library "$LIB_FILE" --language kotlin --out-dir bindings/kotlin --no-format

OUT="target/harness-kotlin"
rm -rf "$OUT" && mkdir -p "$OUT"

echo "smoke-kotlin: compiling the harness"
kotlinc \
  bindings/kotlin/uniffi/vela_core_uniffi/vela_core_uniffi.kt \
  harness/kotlin/Harness.kt \
  -classpath "$JNA_JAR:$JSON_JAR" \
  -include-runtime -d "$OUT/harness.jar" \
  -nowarn

echo "smoke-kotlin: running"
java \
  -Djna.library.path="$RUST_DIR/target/release" \
  -classpath "$OUT/harness.jar:$JNA_JAR:$JSON_JAR" \
  HarnessKt \
  "$RUST_DIR/crates/vela-core/tests/vectors"
