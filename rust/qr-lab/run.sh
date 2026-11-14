#!/usr/bin/env bash
# The whole experiment: corpus -> wasm shims -> sizes -> decode -> report.
#   ./run.sh                 everything (corpus is generated once, then reused)
#   ./run.sh --only moire    one category
#   REGEN=1 ./run.sh         regenerate the corpus
set -euo pipefail
cd "$(dirname "$0")"

WASM_OPT=${WASM_OPT:-$(command -v wasm-opt || ls ~/Library/Caches/.wasm-pack/wasm-opt-*/bin/wasm-opt 2>/dev/null | head -1 || true)}
[ -x "$WASM_OPT" ] || { echo "wasm-opt not found (set WASM_OPT)"; exit 1; }

# rxing, patched (see Cargo.toml): from the cargo registry, or RXING_SRC.
if [ ! -d out/vendor/rxing ]; then
  src=${RXING_SRC:-$(ls -d ~/.cargo/registry/src/*/rxing-0.9.* 2>/dev/null | tail -1)}
  [ -d "$src" ] || { echo "rxing source not found: set RXING_SRC to an rxing 0.9 checkout"; exit 1; }
  mkdir -p out/vendor && cp -R "$src" out/vendor/rxing && chmod -R u+w out/vendor/rxing
  perl -pi -e 's/chrono::Utc::now\(\)\.timestamp_millis\(\) as u128/0u128/g' out/vendor/rxing/src/rxing_result.rs out/vendor/rxing/src/aztec/aztec_reader.rs
fi

cargo build --release -p qr-corpus
if [ -n "${REGEN:-}" ] || [ ! -f out/corpus/manifest.json ]; then
  rm -rf out/corpus out/preview
  ./target/release/qr-corpus --n 30 --seed 1
fi

# Every shim-* crate is a contestant.
shims=$(ls -d shim-*/ | tr -d / | sed 's/^/-p /' | tr '\n' ' ')
cargo build --release --target wasm32-unknown-unknown $shims
mkdir -p out/wasm
rm -f out/wasm/*.wasm
for src in target/wasm32-unknown-unknown/release/shim_*.wasm; do
  name=$(basename "$src" .wasm)
  # -Os is what vela-core-wasm ships with; -Oz for the floor.
  flags="--enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int --enable-mutable-globals"
  "$WASM_OPT" -Os $flags "$src" -o "out/wasm/$name.wasm"
  "$WASM_OPT" -Oz $flags "$src" -o "out/wasm/$name.Oz.wasm"
done

node bench/sizes.mjs
node bench/run.mjs "$@"
node bench/report.mjs
