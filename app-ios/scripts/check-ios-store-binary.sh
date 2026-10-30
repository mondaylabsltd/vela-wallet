#!/usr/bin/env bash
# Is this iOS binary one that exposes no developer door? (native parity F0.5)
#
#   app-ios/scripts/check-ios-store-binary.sh <VelaWallet executable | VelaWallet.app | .xcarchive>
#
# Run on a RELEASE build — the archive ios-package.yml makes, or
#   xcodebuild -scheme VelaWallet -configuration Release -sdk iphonesimulator build
# — the iOS sibling of app-desktop/vela-wallet/scripts/check-store-binary.sh.
# Three assertions:
#
#   1. None of the Debug-only launch inputs is in the binary, by NAME: every
#      `VELA_*` literal in the files Release excludes
#      (`EXCLUDED_SOURCE_FILE_NAMES`: app-ios/VelaWallet/VelaWallet/Dev/*.swift)
#      — the parallel space's door, `VELA_SERVICE_ENDPOINTS`,
#      `VELA_RPC_OVERRIDES`, `VELA_PEERCONNECT_RELAY`. Taken from the source,
#      so a new input there is covered the moment it is written.
#   2. None of the parallel space's fixture symbols is linked
#      (`vela_dev_fixtures`' uniffi exports): the keyset is not in the app.
#   3. The widget extension (native parity N-I2) links no core and no network,
#      keychain, passkey or camera framework, calls no URL session, keychain
#      item or biometry, and — when signed — is entitled to its App Group only.
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

# 3. The widget extension (native parity N-I2) holds no key, links no core
#    and makes no request: it reads the snapshot the app writes into the App
#    Group and draws it. Read from the built extension, so a convenience
#    import in a shared file cannot slip a network stack or a keychain into
#    a process that runs on the Lock Screen.
appex_dir="$(dirname "$target")/PlugIns/VelaWidgetsExtension.appex"
appex="$appex_dir/VelaWidgetsExtension"
if [[ ! -f "$appex" ]]; then
  fail "the widget extension is not embedded (PlugIns/VelaWidgetsExtension.appex)"
else
  ext=("$appex")
  [[ -f "$appex_dir/VelaWidgetsExtension.debug.dylib" ]] && ext+=("$appex_dir/VelaWidgetsExtension.debug.dylib")
  ext_hits() { local n=0 b; for b in "${ext[@]}"; do n=$((n + $(LC_ALL=C grep -a -c -- "$1" "$b" || true))); done; echo "$n"; }
  # The core: none of its exports, and not its Swift module.
  core="$(for b in "${ext[@]}"; do nm -arch all "$b" 2>/dev/null; done | grep -cE 'uniffi_vela_core|ffi_vela_core' || true)"
  [[ "$core" == 0 ]] || fail "the widget extension links the core ($core symbol(s))"
  [[ "$(ext_hits VelaCoreFFI)" == 0 ]] || fail "the widget extension carries VelaCoreFFI"
  # Linked frameworks: nothing that talks to a network, a key or a passkey.
  linked="$(for b in "${ext[@]}"; do otool -L "$b" 2>/dev/null; done)"
  for framework in Network CFNetwork WebKit AuthenticationServices LocalAuthentication CryptoTokenKit \
                   CoreBluetooth AVFoundation VisionKit; do
    if grep -q "/$framework.framework/" <<<"$linked"; then
      fail "the widget extension links $framework.framework"
    fi
  done
  # Symbols it calls: no URL session, no keychain item, no biometry.
  undefined="$(for b in "${ext[@]}"; do nm -u -arch all "$b" 2>/dev/null; done)"
  for symbol in NSURLSession URLSession SecItemCopyMatching SecItemAdd SecKeyCreate LAContext ASAuthorization \
                nw_connection nw_endpoint CFStreamCreatePairWithSocket; do
    if grep -q -- "$symbol" <<<"$undefined"; then
      fail "the widget extension calls $symbol"
    fi
  done
  # Signed: its only entitlement is the App Group (no keychain group, no
  # associated domains). An unsigned simulator build carries none to read.
  if entitlements="$(codesign -d --entitlements - --xml "$appex_dir" 2>/dev/null)" && [[ -n "$entitlements" ]]; then
    for key in keychain-access-groups com.apple.developer.associated-domains \
               com.apple.developer.authentication-services.autofill-credential-provider; do
      if grep -q "$key" <<<"$entitlements"; then
        fail "the widget extension is entitled to $key"
      fi
    done
  fi
fi

if (( failed )); then
  exit 1
fi
echo "ok: none of $count developer inputs, no fixture symbol, no parallel-space binding;" \
  "the widget extension links no core, no network stack, no keychain"
