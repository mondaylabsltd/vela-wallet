#!/usr/bin/env bash
# Rebuild one shim and rerun it alone: ./quick.sh vela [--only rotation]
set -euo pipefail
cd "$(dirname "$0")"
name=$1; shift
WASM_OPT=${WASM_OPT:-$(command -v wasm-opt || ls ~/Library/Caches/.wasm-pack/wasm-opt-*/bin/wasm-opt 2>/dev/null | head -1)}
cargo build -q --release --target wasm32-unknown-unknown -p "shim-$name"
f=shim_${name//-/_}
"$WASM_OPT" -Os --enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int --enable-mutable-globals \
  "target/wasm32-unknown-unknown/release/$f.wasm" -o "out/wasm/$f.wasm"
node bench/run.mjs --decoders "$name" "$@" 2>&1 | tail -1
node bench/report.mjs | grep -E "^\| ($name|rxing-cpp) "
