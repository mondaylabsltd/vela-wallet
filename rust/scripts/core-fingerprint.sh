#!/usr/bin/env bash
#
# Print a fingerprint of the Rust sources the native shells compile into their
# bindings. Used by build-ios-xcframework.sh to stamp what it built, and by
# check-ios-core-fresh.sh to ask whether that stamp still describes the tree.
#
#   rust/scripts/core-fingerprint.sh           # the working tree
#   rust/scripts/core-fingerprint.sh --head    # the same files as COMMITTED
#
# Content, not mtimes: a fresh clone, a branch switch and a `git stash` all
# rewrite mtimes without changing a line, and mtimes going BACKWARDS is how a
# build system gets talked into reusing something stale.
#
# `--head` exists because a mismatch has two very different causes and the
# stamp alone cannot tell them apart: the build is BEHIND the tree (rebuild),
# or the tree has run AHEAD of the commit the build was made from, because
# somebody else's uncommitted work is sitting in this worktree. The second is
# routine when several sessions share a checkout, and rebuilding then is the
# wrong move — it bakes half-finished work into the artifact under test. See
# check-ios-core-fresh.sh, which uses this to say which case it is.
set -euo pipefail

RUST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RUST_DIR"

files() {
  find crates -name '*.rs' -o -name 'Cargo.toml' -o -name '*.udl' | LC_ALL=C sort
}

if [ "${1:-}" = "--head" ]; then
  # The same list, hashed from the committed blobs. An untracked file has no
  # committed content and is skipped — which is right: it is not in HEAD.
  files() {
    git ls-files -- 'crates/**/*.rs' 'crates/**/Cargo.toml' 'crates/**/*.udl' \
      'crates/*.rs' 'crates/*/Cargo.toml' | LC_ALL=C sort
  }
  files | while IFS= read -r path; do
    printf '%s  %s\n' "$(git show "HEAD:rust/$path" | shasum -a 256 | cut -d' ' -f1)" "$path"
  done | shasum -a 256 | cut -d' ' -f1
  exit 0
fi

files | xargs shasum -a 256 | shasum -a 256 | cut -d' ' -f1
