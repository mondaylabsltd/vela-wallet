#!/usr/bin/env bash
# Is this macOS binary one the Mac App Store may take, and one that exposes
# no developer feature? (spec 095)
#
#   ./scripts/check-store-binary.sh <path to the vela-wallet executable>
#
# Run on a RELEASE build (the bundle's Contents/MacOS/vela-wallet, any of the
# three architectures). Three assertions, each the reason a past audit or the
# App Store would refuse the build:
#
#   1. No private WindowServer import. Upstream gpui_macos links
#      `CGSMainConnectionID` / `CGSSetWindowBackgroundBlurRadius`; the
#      vendored copy (app-desktop/vendor/gpui_macos) does not. Any `_CGS*` or
#      `_SLS*` (SkyLight) undefined symbol fails.
#   2. None of the private selectors upstream sends or overrides — the strings
#      are what App Store analysis matches, wherever in the binary they sit.
#   3. No developer switch (owner rule, 2026-10-02): every `VELA_*` variable
#      the app reads through `dev_env::var!`/`var_os!`/`flag!` is compiled out
#      of a release build, name and all. The names are taken from the source,
#      so a new switch is covered the moment it is written.
#
# Pure inspection: nm and grep, no network, no signing identity — CI runs it
# on the universal bundle (desktop-macos-packages.yml), and
# build-macos-mas.sh runs it before signing anything.
set -euo pipefail

die() { echo "error: $*" >&2; exit 1; }

[[ $# -eq 1 ]] || die "usage: $0 <vela-wallet executable>"
binary="$1"
[[ -f "$binary" ]] || die "no such file: $binary"
command -v nm >/dev/null 2>&1 || die "nm not found; install the Xcode command-line tools"

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
failed=0
fail() { echo "FAIL: $*" >&2; failed=1; }

# 1. Private WindowServer API, as an import. `nm -u` lists every slice of a
#    universal binary; `-arch all` makes that explicit.
private_imports="$(nm -u -arch all "$binary" 2>/dev/null | grep -E '^_+(CGS|SLS)[A-Za-z]' | sort -u || true)"
if [[ -n "$private_imports" ]]; then
  fail "private WindowServer symbols imported:"
  while IFS= read -r symbol; do echo "    $symbol" >&2; done <<<"$private_imports"
fi

# 2. Private selectors. grep -c, not grep -q: under pipefail -q exits early
#    and the writer dies of SIGPIPE, which reads as a failure.
for selector in \
  _windowResizeNorthWestSouthEastCursor \
  _windowResizeNorthEastSouthWestCursor \
  _opaqueRectForWindowMoveWhenInTitlebar; do
  if [[ "$(LC_ALL=C grep -a -c -- "$selector" "$binary" || true)" != 0 ]]; then
    fail "private selector present: $selector"
  fi
done

# 3. Developer switches. Every literal handed to the dev_env macros.
switches="$(grep -rhoE 'dev_env::(var|var_os|flag)!\("VELA_[A-Z0-9_]+"\)' "$project_root/src" |
  sed -E 's/.*"(VELA_[A-Z0-9_]+)".*/\1/' | sort -u)"
[[ -n "$switches" ]] || die "found no dev_env switches in $project_root/src — has the macro moved?"
count=0
while IFS= read -r name; do
  count=$((count + 1))
  if [[ "$(LC_ALL=C grep -a -c -- "$name" "$binary" || true)" != 0 ]]; then
    fail "developer switch compiled in: $name (a release build must not read it — build without dev-fixtures, in --release)"
  fi
done <<<"$switches"

if (( failed )); then
  exit 1
fi
echo "ok: no private WindowServer import, no private selector, none of $count developer switches"
