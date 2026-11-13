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
# 2. Classes that exist in the debug source set only, and the benchmark build
#    types' fixture doors (Phase 4, src/benchmark). A class name is in the
#    dex as a type descriptor, compressed or not.
for marker in \
  'Lapp/getvela/wallet/dev/ServiceLaunchInputs;' \
  'Lapp/getvela/wallet/dev/MultisigPollTrigger;' \
  'vela.pageLaunch' \
  'Luniffi/vela_dev_fixtures/' \
  'Lapp/getvela/wallet/bench/' \
  'fixtureAssert'
do
  # grep the files themselves: `cat | grep -q` under pipefail reads as "absent"
  # when grep exits early and cat takes the SIGPIPE.
  if LC_ALL=C grep -aqF "$marker" "$WORK"/classes*.dex; then
    problems+=("the dex carries $marker")
  fi
done

# 2b. R8 renames what it keeps (release is optimized since nat-android-6), so a
#     debug-only class could ride in under a short name the grep above cannot
#     see. The mapping R8 wrote beside the APK names every class it kept by its
#     ORIGINAL name: the same markers must be absent there too. A release APK
#     with no mapping beside it was not optimized, and the dex grep above is
#     the whole check.
MAPPING="${VELA_RELEASE_MAPPING:-$HERE/app/build/outputs/mapping/release/mapping.txt}"
if [ -f "$MAPPING" ]; then
  for marker in \
    'app.getvela.wallet.dev.ServiceLaunchInputs' \
    'app.getvela.wallet.dev.MultisigPollTrigger' \
    'uniffi.vela_dev_fixtures.' \
    'app.getvela.wallet.bench.' \
    'fixtureAssert'
  do
    if LC_ALL=C grep -qF "$marker" "$MAPPING"; then
      problems+=("R8's mapping keeps $marker (renamed in the dex, still in the package)")
    fi
  done
  echo "check-release-apk: R8 mapping read ($(LC_ALL=C grep -c '^[^ ]' "$MAPPING") classes kept)"
fi

# 3. The core ships stripped and inside its budget (native parity Phase 4:
#    libvela_core_uniffi.so arm64 ≤ 19.5 MB). An AGP that cannot find its NDK
#    says "Unable to strip …, packaging them as they are" and moves on — the
#    library then carries its symbol table (`.symtab`, ~40 MB arm64). The
#    section name is in the section-header string table only while the
#    section exists; `.dynsym` (always kept) does not contain it.
CORE_BUDGET=$((195 * 1024 * 1024 / 10))
CORE="$WORK/lib/arm64-v8a/libvela_core_uniffi.so"
if [ ! -f "$CORE" ]; then
  problems+=("no arm64-v8a libvela_core_uniffi.so is packaged")
else
  core_bytes=$(wc -c < "$CORE" | tr -d ' ')
  if [ "$core_bytes" -gt "$CORE_BUDGET" ]; then
    problems+=("arm64 libvela_core_uniffi.so is $core_bytes bytes, over the $CORE_BUDGET-byte (19.5 MB) budget")
  fi
  if LC_ALL=C grep -aqF '.symtab' "$CORE"; then
    problems+=("arm64 libvela_core_uniffi.so is not stripped (it carries .symtab) — is the NDK AGP strips with installed?")
  fi
  echo "check-release-apk: arm64 libvela_core_uniffi.so = $core_bytes bytes (budget $CORE_BUDGET)"
fi

# 4. §35 (review 0-1): a store release's passkeys live under getvela.app alone.
#    The core never reads a test domain for a release build; the build must
#    not carry one either. The release BuildConfig the APK was compiled from
#    says TEST_RP_DOMAIN = "" (a device-test release, DEVICE_TEST = true, is a
#    test build and may carry its own), and a domain set in this environment
#    (TEST_RP_DOMAIN / VELA_TEST_RP_DOMAIN) is nowhere in the dex.
BUILD_CONFIG="${VELA_RELEASE_BUILDCONFIG:-$HERE/app/build/generated/source/buildConfig/release/app/getvela/wallet/BuildConfig.java}"
if [ -f "$BUILD_CONFIG" ]; then
  if LC_ALL=C grep -qE 'DEVICE_TEST = false' "$BUILD_CONFIG"; then
    if ! LC_ALL=C grep -qE 'String TEST_RP_DOMAIN = "";' "$BUILD_CONFIG"; then
      problems+=("the release BuildConfig carries a test passkey domain: $(LC_ALL=C grep -E 'TEST_RP_DOMAIN' "$BUILD_CONFIG" | tr -s ' ')")
    fi
    echo "check-release-apk: release BuildConfig read (TEST_RP_DOMAIN empty, DEVICE_TEST false)"
  else
    echo "check-release-apk: a device-test release (DEVICE_TEST true) — its test passkey domain is its own"
  fi
else
  problems+=("no release BuildConfig at $BUILD_CONFIG to read TEST_RP_DOMAIN from")
fi
for domain in "${TEST_RP_DOMAIN:-}" "${VELA_TEST_RP_DOMAIN:-}"; do
  if [ -n "$domain" ] && LC_ALL=C grep -aqF "$domain" "$WORK"/classes*.dex; then
    problems+=("the dex carries the test passkey domain $domain")
  fi
done

if [ ${#problems[@]} -gt 0 ]; then
  echo "check-release-apk: $(basename "$APK") carries what a release must not:" >&2
  printf '  - %s\n' "${problems[@]}" >&2
  exit 1
fi
echo "check-release-apk: ok — $(basename "$APK") carries no fixture keys, no parallel-space door, no harness launch inputs; its core is stripped and in budget"
