#!/usr/bin/env bash
#
# Print a fingerprint of the Rust sources the native shells compile into their
# bindings. Used by build-ios-xcframework.sh to stamp what it built, and by
# check-ios-core-fresh.sh to ask whether that stamp still describes the tree.
#
# Content, not mtimes: a fresh clone, a branch switch and a `git stash` all
# rewrite mtimes without changing a line, and mtimes going BACKWARDS is how a
# build system gets talked into reusing something stale.
set -euo pipefail

RUST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RUST_DIR"

find crates -name '*.rs' -o -name 'Cargo.toml' -o -name '*.udl' \
  | LC_ALL=C sort \
  | xargs shasum -a 256 \
  | shasum -a 256 \
  | cut -d' ' -f1
