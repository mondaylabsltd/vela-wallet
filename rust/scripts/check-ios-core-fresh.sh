#!/usr/bin/env bash
#
# Is the iOS app about to run the core that is in this tree?
#
#   rust/scripts/check-ios-core-fresh.sh
#
# Run it before every device test. `xcodebuild build-for-testing` compiles the
# Swift and links VelaCoreFFI.xcframework as-is: it does not know Rust exists,
# and the xcframework is gitignored, so a core fix landed hours ago can be
# absent from the phone while `git status` is clean.
#
# This is not a hypothetical. On 2026-09-22 a 44-minute iPhone run reported a
# P1 — "a refused request is never answered, and the next request re-presents
# the stale sheet" — against an xcframework built at 13:11 for a fix committed
# at 14:47. The phone was right about what it saw and wrong about what it was
# testing, and there was nothing in the loop to say so.
#
# Exit 1 when the stamp is missing or stale, with the command that fixes it.
# Exit 2 for the third case, which a stamp alone cannot see: the build matches
# the current COMMIT and the working tree has run ahead of it. Different
# answer, different instruction — rebuilding there would put somebody else's
# half-finished work into the artifact under test.
set -euo pipefail

RUST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STAMP="$RUST_DIR/../app-ios/VelaCoreKit/Artifacts/.core-fingerprint"
WANT="$("$RUST_DIR/scripts/core-fingerprint.sh")"

if [ ! -f "$STAMP" ]; then
  echo "check-ios-core-fresh: no xcframework stamp — the phone would run an unknown core." >&2
  echo "  rust/scripts/build-ios-xcframework.sh" >&2
  exit 1
fi

HAVE="$(cat "$STAMP")"
if [ "$HAVE" != "$WANT" ]; then
  HEAD_FP="$("$RUST_DIR/scripts/core-fingerprint.sh" --head)"
  if [ "$HAVE" = "$HEAD_FP" ]; then
    # The build is a faithful one of the current commit; the TREE has run
    # ahead of it. In a worktree several sessions share, that is usually
    # somebody else's work in progress, and rebuilding would bake it into the
    # artifact under test — a green gate over a core nobody has reviewed.
    echo "check-ios-core-fresh: the xcframework matches HEAD, but the tree has uncommitted changes." >&2
    echo "  built:  $HAVE  (= HEAD)" >&2
    echo "  tree:   $WANT" >&2
    echo "Uncommitted under rust/crates:" >&2
    git -C "$RUST_DIR/.." diff --name-only HEAD -- rust/crates | sed 's/^/  /' >&2
    git -C "$RUST_DIR/.." ls-files --others --exclude-standard -- rust/crates | sed 's/^/  ? /' >&2
    echo "Testing HEAD is fine — say so in the report. Do NOT rebuild to silence this" >&2
    echo "unless that work is yours and you mean to test it." >&2
    exit 2
  fi
  echo "check-ios-core-fresh: STALE. The xcframework was built from different Rust." >&2
  echo "  built:  $HAVE" >&2
  echo "  tree:   $WANT" >&2
  echo "  HEAD:   $HEAD_FP" >&2
  echo "Anything you measure on the phone is about the old core. Rebuild first:" >&2
  echo "  rust/scripts/build-ios-xcframework.sh" >&2
  exit 1
fi

echo "check-ios-core-fresh: ok — the xcframework is this tree's core ($WANT)"
