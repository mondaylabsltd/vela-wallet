#!/usr/bin/env bash
# Is this iOS binary one that exposes no developer door? (native parity F0.5)
#
#   app-ios/scripts/check-ios-store-binary.sh <VelaWallet executable | VelaWallet.app | .xcarchive>
#
# Run on a RELEASE build — the archive ios-package.yml makes, or
#   xcodebuild -scheme VelaWallet -configuration Release -sdk iphonesimulator build
# — the iOS sibling of app-desktop/vela-wallet/scripts/check-store-binary.sh.
# Two assertions:
#
#   1. None of the Debug-only launch inputs is in the binary, by NAME: every
#      `VELA_*` literal in the files Release excludes
#      (`EXCLUDED_SOURCE_FILE_NAMES`: app-ios/VelaWallet/VelaWallet/Dev/*.swift)
#      — the parallel space's door, `VELA_SERVICE_ENDPOINTS`,
#      `VELA_RPC_OVERRIDES`, `VELA_PEERCONNECT_RELAY`. Taken from the source,
#      so a new input there is covered the moment it is written.
#   2. None of the parallel space's fixture symbols is linked
#      (`vela_dev_fixtures`' uniffi exports): the keyset is not in the app.
#
# Pure inspection: strings, nm and grep — no signing identity, no network.
set -euo pipefail

die() { echo "error: $*" >&2; exit 1; }

[[ $# -eq 1 ]] || die "usage: $0 <VelaWallet executable | VelaWallet.app | .xcarchive>"
target="$1"
case "$target" in
  *.xcarchive) target="$target/Products/Applications/VelaWallet.app/VelaWallet" ;;
  *.app) target="$target/VelaWallet" ;;
esac
[[ -f "$target" ]] || die "no such file: $target"
command -v nm >/dev/null 2>&1 || die "nm not found; install the Xcode command-line tools"
# A Debug build keeps its code in `VelaWallet.debug.dylib` beside a stub
# executable (Xcode's debug dylib): read both, or a Debug build reads clean.
binaries=("$target")
if [[ -f "$(dirname "$target")/VelaWallet.debug.dylib" ]]; then
  binaries+=("$(dirname "$target")/VelaWallet.debug.dylib")
fi
hits() { local n=0 b; for b in "${binaries[@]}"; do n=$((n + $(LC_ALL=C grep -a -c -- "$1" "$b" || true))); done; echo "$n"; }

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dev_dir="$here/../VelaWallet/VelaWallet/Dev"
[[ -d "$dev_dir" ]] || die "no Dev/ directory at $dev_dir — has it moved?"

failed=0
fail() { echo "FAIL: $*" >&2; failed=1; }

# A Release build excludes these files; a Debug build is not what this checks.
# The Debug-only door's own marker: if it is here, this is a Debug build.
if [[ "$(hits ParallelSpaceBinding)" != 0 ]]; then
  fail "the parallel space's binding is linked — this is a Debug build, or Release stopped excluding Dev/"
fi

# 1. The launch inputs, by name. grep -c, not -q: under pipefail -q exits
#    early and the writer dies of SIGPIPE, which reads as a failure.
names="$(grep -rhoE '"VELA_[A-Z0-9_]+"' "$dev_dir" | tr -d '"' | sort -u)"
[[ -n "$names" ]] || die "found no VELA_* names in $dev_dir — have the inputs moved?"
count=0
while IFS= read -r name; do
  count=$((count + 1))
  if [[ "$(hits "$name")" != 0 ]]; then
    fail "developer input compiled in: $name (Release must exclude Dev/ — EXCLUDED_SOURCE_FILE_NAMES)"
  fi
done <<<"$names"

# 2. The fixture keyset's exports.
fixtures="$(for b in "${binaries[@]}"; do nm -arch all "$b" 2>/dev/null; done | grep -cE 'uniffi_vela_dev_fixtures|fixture_multi_address' || true)"
if [[ "$fixtures" != 0 ]]; then
  fail "$fixtures parallel-space fixture symbol(s) linked"
fi

if (( failed )); then
  exit 1
fi
echo "ok: none of $count developer inputs, no fixture symbol, no parallel-space binding"
