#!/usr/bin/env bash
#
# What a release package must NOT carry (spec 043 D2, spec 088 FR-003, native
# parity F0.5): the parallel space's fixed keyset and its door, and the
# harness launch inputs that re-point endpoints / RPCs / the PeerConnect relay.
#
#   app-android/vela-wallet/scripts/check-release-apk.sh [path/to/app-release*.apk]
#
# Default: app/build/outputs/apk/release/app-release-unsigned.apk (the CI
# packaging job's `-PvelaUnsignedRelease` output), else app-release.apk.
# Exit 1, naming what was found, when the package carries any of it.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APK="${1:-}"
if [ -z "$APK" ]; then
  for candidate in app/build/outputs/apk/release/app-release-unsigned.apk app/build/outputs/apk/release/app-release.apk; do
    [ -f "$HERE/$candidate" ] && APK="$HERE/$candidate" && break
  done
fi
[ -n "$APK" ] && [ -f "$APK" ] || { echo "check-release-apk: no release APK (build :app:assembleRelease first)" >&2; exit 2; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
unzip -q -o "$APK" -d "$WORK"

problems=()
# 1. The fixture keyset's library — debug jniLibs only.
if find "$WORK/lib" -name 'libvela_dev_fixtures.so' 2>/dev/null | grep -q .; then
  problems+=("libvela_dev_fixtures.so is packaged (the parallel space's keys)")
fi
# 2. Classes that exist in the debug source set only. A class name is in the
#    dex as a type descriptor, compressed or not.
for marker in \
  'Lapp/getvela/wallet/dev/ServiceLaunchInputs;' \
  'Luniffi/vela_dev_fixtures/' \
  'fixtureAssert'
do
  if cat "$WORK"/classes*.dex | LC_ALL=C grep -aqF "$marker"; then
    problems+=("the dex carries $marker")
  fi
done

if [ ${#problems[@]} -gt 0 ]; then
  echo "check-release-apk: $(basename "$APK") carries debug-only parts:" >&2
  printf '  - %s\n' "${problems[@]}" >&2
  exit 1
fi
echo "check-release-apk: ok — $(basename "$APK") carries no fixture keys, no parallel-space door, no harness launch inputs"
