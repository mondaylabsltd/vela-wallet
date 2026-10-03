#!/usr/bin/env bash
#
# Are the Kotlin bindings in this checkout the ones this tree's core generates?
#
#   rust/scripts/check-android-core-fresh.sh
#
# The Android twin of check-ios-core-fresh.sh, and for the same reason: the
# bindings are gitignored, generated once, and then silently describe whatever
# core was in the tree that day. Gradle runs this before it compiles Kotlin, so
# the answer arrives as one sentence instead of forty `Unresolved reference`
# lines that read like a code bug.
#
# Exit 1 when the stamp is missing or stale, with the command that fixes it.
# Exit 2 for the case a stamp alone cannot see: the bindings match the current
# COMMIT and the working tree has run ahead of it — usually somebody else's
# work in progress in a shared checkout, where regenerating is the wrong move.
set -euo pipefail

RUST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STAMP="$RUST_DIR/bindings/kotlin/.core-fingerprint"
WANT="$("$RUST_DIR/scripts/core-fingerprint.sh")"

if [ ! -f "$STAMP" ]; then
  echo "check-android-core-fresh: no bindings stamp — the app would compile against an unknown core." >&2
  echo "  rust/scripts/build-kotlin-bindings.sh" >&2
  exit 1
fi

HAVE="$(cat "$STAMP")"
if [ "$HAVE" != "$WANT" ]; then
  HEAD_FP="$("$RUST_DIR/scripts/core-fingerprint.sh" --head)"
  if [ "$HAVE" = "$HEAD_FP" ]; then
    echo "check-android-core-fresh: the bindings match HEAD, but the tree has uncommitted changes." >&2
    echo "  generated:  $HAVE  (= HEAD)" >&2
    echo "  tree:       $WANT" >&2
    echo "Uncommitted under rust/crates:" >&2
    git -C "$RUST_DIR/.." diff --name-only HEAD -- rust/crates | sed 's/^/  /' >&2
    git -C "$RUST_DIR/.." ls-files --others --exclude-standard -- rust/crates | sed 's/^/  ? /' >&2
    echo "Building HEAD is fine — say so in the report. Do NOT regenerate to silence this" >&2
    echo "unless that work is yours and you mean to build it." >&2
    exit 2
  fi
  echo "check-android-core-fresh: STALE. The Kotlin bindings were generated from different Rust." >&2
  echo "  generated:  $HAVE" >&2
  echo "  tree:       $WANT" >&2
  echo "  HEAD:       $HEAD_FP" >&2
  echo "Kotlin will fail with 'Unresolved reference' about functions that exist. Regenerate:" >&2
  echo "  rust/scripts/build-kotlin-bindings.sh" >&2
  exit 1
fi

echo "check-android-core-fresh: ok — the Kotlin bindings are this tree's core ($WANT)"
