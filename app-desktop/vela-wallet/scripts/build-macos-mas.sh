#!/usr/bin/env bash
# Build the Mac App Store package for TestFlight / App Store review (spec 095):
# "Vela Wallet.app" universal, sandboxed, signed for the App Store, wrapped in
# a signed installer .pkg, and validated by Apple. Nothing is uploaded.
#
#   VELA_MAS_PROFILE=PATH ./scripts/build-macos-mas.sh   # the store build
#   ./scripts/build-macos-mas.sh --profile PATH          # the same, as a flag
#   ./scripts/build-macos-mas.sh --dev                   # sandboxed DEVELOPMENT build, this Mac only
#   ./scripts/build-macos-mas.sh --skip-build            # reuse target/<triple>/release
#   ./scripts/build-macos-mas.sh --no-validate           # stop at the signed .pkg
#
# THE ONE INPUT is the provisioning profile:
#
#   store  a "Mac App Store Connect" distribution profile for App ID
#          app.getvela.VelaWallet (team F9W689P9NE) with Associated Domains —
#          developer.apple.com > Profiles > + > Distribution > Mac App Store
#          Connect > app.getvela.VelaWallet > the Apple Distribution
#          certificate. Found automatically in ~/Downloads and the two profile
#          folders, or named with VELA_MAS_PROFILE / --profile.
#   --dev  the "Mac Team Provisioning Profile: app.getvela.VelaWallet"
#          development profile that lists THIS Mac (Xcode keeps it current).
#
# Everything else is derived, never guessed:
#   * the app signing identity is the keychain certificate THE PROFILE names
#     (by SHA-1 — this Mac holds several certificates with the same name);
#   * the installer identity is the team's "3rd Party Mac Developer Installer"
#     (or "Mac Installer Distribution") certificate, unless
#     VELA_MAS_INSTALLER_IDENTITY names one;
#   * CFBundleShortVersionString is Cargo.toml's version, CFBundleVersion is
#     `git rev-list --count HEAD` — monotonic, so every upload is a new build.
#
# Validation (`xcrun altool --validate-app`) needs App Store Connect
# credentials, one of:
#   VELA_ASC_KEY_ID + VELA_ASC_ISSUER   an App Store Connect API key; altool
#                                       finds AuthKey_<id>.p8 in
#                                       ~/.appstoreconnect/private_keys (or set
#                                       VELA_ASC_KEY_FILE to its path)
#   VELA_ASC_APPLE_ID + VELA_ASC_PASSWORD_ITEM
#                                       an Apple ID and the name of a keychain
#                                       item holding an app-specific password
#                                       (altool --store-password-in-keychain-item)
# To upload once it validates: Transporter (drag the .pkg in), or the same
# command with --upload-app in place of --validate-app. This script never
# uploads.
#
# Outputs (dist/macos/):
#   mas/Vela Wallet.app, mas/VelaWallet-<version>-<build>-mas.pkg   (store)
#   mas-dev/Vela Wallet.app                                          (--dev)
#
# What makes the bundle different from the Developer ID one (spec 063), and
# why each is here, is in packaging/macos/: entitlements-mas.plist (the
# sandbox, and nothing the app does not use), PrivacyInfo.xcprivacy (measured
# required-reason APIs), container-migration.plist (a .dmg tester keeps their
# wallets; store build only). The binary must pass check-store-binary.sh — no
# private API, no developer switch — before anything is signed.
set -euo pipefail

app_name="Vela Wallet"
binary_name="vela-wallet"
app_id="F9W689P9NE.app.getvela.VelaWallet"
team_id="F9W689P9NE"

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_root="$(cd "$project_root/../.." && pwd)"
packaging="$project_root/packaging/macos"

die() { echo "error: $*" >&2; exit 1; }
note() { echo "==> $*"; }

dev=0; skip_build=0; validate=1; profile="${VELA_MAS_PROFILE:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dev)          dev=1; shift ;;
    --skip-build)   skip_build=1; shift ;;
    --no-validate)  validate=0; shift ;;
    --profile)      [[ $# -ge 2 ]] || die "--profile needs a path"; profile="$2"; shift 2 ;;
    --profile=*)    profile="${1#--profile=}"; shift ;;
    -h|--help)      sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'; exit 0 ;;
    *)              die "unknown option: $1" ;;
  esac
done
[[ "$(uname -s)" == "Darwin" ]] || die "this script only runs on macOS"
for tool in codesign productbuild plutil security xcrun ditto; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool not found; install Xcode"
done

# -------------------------------------------------------------- the profile --
# A profile is CMS-signed XML; `security cms -D` gives the plist.
profile_field() { plutil -extract "$2" raw - <<<"$1" 2>/dev/null; }
this_mac_udid="$(system_profiler SPHardwareDataType 2>/dev/null | awk -F': ' '/Provisioning UDID/ {print $2; exit}')"

# Which kind a profile is, from what it provisions: a development profile
# lists devices, a Developer ID one provisions all of them, a Mac App Store
# one neither.
profile_kind() {
  local plist="$1"
  if [[ "$(profile_field "$plist" ProvisionsAllDevices)" == "true" ]]; then echo developer-id
  elif profile_field "$plist" ProvisionedDevices >/dev/null; then echo development
  else echo app-store; fi
}
want_kind="app-store"; (( dev )) && want_kind="development"

profile_fits() {  # $1 = path; prints nothing, answers with the exit status
  local plist; plist="$(security cms -D -i "$1" 2>/dev/null)" || return 1
  [[ "$(profile_field "$plist" 'Entitlements.com\.apple\.application-identifier')" == "$app_id" ]] || return 1
  profile_field "$plist" 'Entitlements.com\.apple\.developer\.associated-domains' >/dev/null || return 1
  [[ "$(profile_kind "$plist")" == "$want_kind" ]] || return 1
  if (( dev )); then
    grep -qx "$this_mac_udid" < <(plutil -extract ProvisionedDevices json -o - - <<<"$plist" | tr -d '[]"' | tr ',' '\n') || return 1
  fi
  # Not expired: a plist date's raw form is ISO 8601, which sorts as text.
  local expires; expires="$(profile_field "$plist" ExpirationDate)"
  [[ "$(date -u +%Y-%m-%dT%H:%M:%SZ)" < "$expires" ]]
}

if [[ -n "$profile" ]]; then
  [[ -f "$profile" ]] || die "no such provisioning profile: $profile"
  if ! profile_fits "$profile"; then
    plist="$(security cms -D -i "$profile" 2>/dev/null)" || die "$profile is not a provisioning profile"
    die "$profile does not fit a $want_kind build:
       kind:             $(profile_kind "$plist") (need $want_kind)
       app id:           $(profile_field "$plist" 'Entitlements.com\.apple\.application-identifier') (need $app_id)
       associated domains: $(profile_field "$plist" 'Entitlements.com\.apple\.developer\.associated-domains' >/dev/null && echo granted || echo MISSING)
       expires:          $(profile_field "$plist" ExpirationDate)"
  fi
else
  while IFS= read -r candidate; do
    if profile_fits "$candidate"; then profile="$candidate"; break; fi
  done < <(find "$HOME/Downloads" "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles" \
                "$HOME/Library/MobileDevice/Provisioning Profiles" -maxdepth 1 \
                \( -name '*.provisionprofile' -o -name '*.mobileprovision' \) 2>/dev/null)
  if [[ -z "$profile" ]]; then
    if (( dev )); then
      die "no development profile for $app_id that lists this Mac ($this_mac_udid).
       Open any project in Xcode signed for team $team_id with this Mac
       registered, or create one at developer.apple.com > Profiles > macOS
       App Development, and run this again."
    fi
    die "no Mac App Store provisioning profile for $app_id on this Mac.
       developer.apple.com > Certificates, IDs & Profiles > Profiles > +
       > Distribution: Mac App Store Connect > App ID app.getvela.VelaWallet
       > the Apple Distribution certificate > download, then
       VELA_MAS_PROFILE=<the file> $0
       (--dev builds the sandboxed development bundle in the meantime)"
  fi
fi
profile_plist="$(security cms -D -i "$profile")"
note "provisioning profile ($want_kind): ${profile/#$HOME/~} — $(profile_field "$profile_plist" Name)"

# ------------------------------------------- the identities, by SHA-1 only --
identity=""; i=0
keychain_identities="$(security find-identity -v -p codesigning | awk '{print $2}')"
while der="$(profile_field "$profile_plist" "DeveloperCertificates.$i")"; do
  sha="$(base64 --decode <<<"$der" | openssl sha1 | awk '{print toupper($NF)}')"
  if grep -qx "$sha" <<<"$keychain_identities"; then identity="$sha"; break; fi
  i=$((i + 1))
done
[[ -n "$identity" ]] || die "the profile names no certificate whose private key is in this keychain.
       Regenerate it for a certificate this Mac holds, or install the one it names."
signer="$(security find-identity -v -p codesigning | awk -v h="$identity" '$2 == h' | sed -E 's/^[^"]*"([^"]*)".*/\1/')"
note "app signature: $signer ($identity)"

installer=""
if (( ! dev )); then
  if [[ -n "${VELA_MAS_INSTALLER_IDENTITY:-}" ]]; then
    installer="$VELA_MAS_INSTALLER_IDENTITY"
  else
    installer="$(security find-identity -v | grep -E "\"(3rd Party Mac Developer Installer|Mac Installer Distribution): .*\($team_id\)\"" |
      awk '{print $2}' | head -1)"
  fi
  [[ -n "$installer" ]] || die "no installer identity for team $team_id in this keychain.
       developer.apple.com > Certificates > + > Mac Installer Distribution,
       or name one with VELA_MAS_INSTALLER_IDENTITY"
  note "installer signature: $(security find-identity -v | awk -v h="$installer" '$2 == h' | sed -E 's/^[^"]*"([^"]*)".*/\1/') ($installer)"
fi

# Validation credentials, checked before the half-hour build rather than after.
asc_auth=()
if (( ! dev && validate )); then
  if [[ -n "${VELA_ASC_KEY_ID:-}" && -n "${VELA_ASC_ISSUER:-}" ]]; then
    asc_auth=(--api-key "$VELA_ASC_KEY_ID" --api-issuer "$VELA_ASC_ISSUER")
    [[ -n "${VELA_ASC_KEY_FILE:-}" ]] && asc_auth+=(--p8-file-path "$VELA_ASC_KEY_FILE")
  elif [[ -n "${VELA_ASC_APPLE_ID:-}" && -n "${VELA_ASC_PASSWORD_ITEM:-}" ]]; then
    asc_auth=(-u "$VELA_ASC_APPLE_ID" -p "@keychain:$VELA_ASC_PASSWORD_ITEM")
  else
    die "validation needs App Store Connect credentials: VELA_ASC_KEY_ID + VELA_ASC_ISSUER
       (an API key), or VELA_ASC_APPLE_ID + VELA_ASC_PASSWORD_ITEM. --no-validate
       stops at the signed .pkg."
  fi
fi

# ------------------------------------------------------------- the build --
version="$(sed -n '/^\[package\]/,/^\[/ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' \
  "$project_root/Cargo.toml" | head -1)"
[[ -n "$version" ]] || die "could not read the package version from Cargo.toml"
build_number="$(git -C "$repo_root" rev-list --count HEAD)"
[[ "$build_number" =~ ^[0-9]+$ ]] || die "could not count commits for CFBundleVersion"

# The universal bundle, exactly as the Developer ID pipeline assembles it —
# release profile, no dev-fixtures — ad-hoc signed, re-signed below. The
# identity variables are cleared so that script cannot sign it its own way.
build_args=(--arch universal --no-dmg)
(( skip_build )) && build_args+=(--skip-build)
env -u VELA_SIGN_IDENTITY -u VELA_PROVISION_PROFILE "$project_root/scripts/build-macos-app.sh" "${build_args[@]}"

out_dir="$project_root/dist/macos/mas"; (( dev )) && out_dir="$project_root/dist/macos/mas-dev"
app="$out_dir/$app_name.app"
rm -rf "$out_dir"
mkdir -p "$out_dir"
ditto "$project_root/dist/macos/universal/$app_name.app" "$app"

note "store-binary check"
"$project_root/scripts/check-store-binary.sh" "$app/Contents/MacOS/$binary_name"

# --------------------------------------------------------- the bundle --
note "Info.plist: $version ($build_number), macOS 12+, export compliance"
plist="$app/Contents/Info.plist"
pb() { /usr/libexec/PlistBuddy -c "$1" "$plist"; }
pb_set() {  # key type value — set, or add when absent
  pb "Set :$1 $3" 2>/dev/null || pb "Add :$1 $2 $3"
}
pb_set CFBundleVersion string "$build_number"
pb_set CFBundleShortVersionString string "$version"
# Passkeys on the platform authenticator (AuthenticationServices) are 12+.
pb_set LSMinimumSystemVersion string 12.0
# Standard HTTPS/TLS and authentication only — the rationale is in
# docs/store-submission/mac-app-store.md ("Export compliance").
pb_set ITSAppUsesNonExemptEncryption bool false
# What Xcode records about the toolchain; App Store Connect reads the SDK from
# here as well as from the binary's LC_BUILD_VERSION.
pb "Delete :CFBundleSupportedPlatforms" 2>/dev/null || true
pb "Add :CFBundleSupportedPlatforms array"
pb "Add :CFBundleSupportedPlatforms:0 string MacOSX"
sdk_version="$(xcrun --sdk macosx --show-sdk-version)"
pb_set DTPlatformName string macosx
pb_set DTPlatformVersion string "$sdk_version"
pb_set DTPlatformBuild string "$(xcrun --sdk macosx --show-sdk-build-version)"
pb_set DTSDKName string "macosx$sdk_version"
pb_set DTSDKBuild string "$(xcrun --sdk macosx --show-sdk-build-version)"
pb_set DTXcode string "$(xcodebuild -version | awk '/^Xcode/ {split($2, v, "."); printf "%d%d%d", v[1], v[2], (v[3] == "" ? 0 : v[3])}')"
pb_set DTXcodeBuild string "$(xcodebuild -version | awk '/^Build version/ {print $3}')"
pb_set DTCompiler string com.apple.compilers.llvm.clang.1_0
pb_set BuildMachineOSBuild string "$(sw_vers -buildVersion)"
plutil -lint "$plist" >/dev/null || die "Info.plist does not lint"

install -m644 "$packaging/PrivacyInfo.xcprivacy" "$app/Contents/Resources/PrivacyInfo.xcprivacy"
if (( ! dev )); then
  install -m644 "$packaging/container-migration.plist" "$app/Contents/Resources/container-migration.plist"
fi
install -m644 "$profile" "$app/Contents/embedded.provisionprofile"

# ------------------------------------------------------------- the seal --
# Extended attributes (a quarantine flag, Finder info) are refused by codesign
# and by App Store ingestion alike.
xattr -cr "$app"
note "codesign ($([[ $dev == 1 ]] && echo development || echo App Store), sandbox, hardened runtime)"
# Not --deep: one executable, nothing nested, and deep signing would spread
# the app's entitlements onto anything that ever is.
if (( dev )); then timestamp=(--timestamp=none); else timestamp=(--timestamp); fi
codesign --force "${timestamp[@]}" --options runtime \
  --entitlements "$packaging/entitlements-mas.plist" \
  --sign "$identity" "$app"
codesign --verify --strict --verbose=2 "$app" || die "the signature does not verify"

# What was signed must be exactly what the file says — a typo in a key name
# is a silent missing capability.
want="$(plutil -convert json -o - "$packaging/entitlements-mas.plist")"
got="$(codesign -d --entitlements - --xml "$app" 2>/dev/null | plutil -convert json -o - -)"
[[ "$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]) == json.loads(sys.argv[2]))' "$want" "$got")" == True ]] ||
  die "the signed entitlements differ from entitlements-mas.plist:
$got"
echo "  entitlements: exactly entitlements-mas.plist"

if (( dev )); then
  echo
  note "sandboxed development bundle: ${app#"$project_root"/}"
  echo "Runs on this Mac only (the profile lists it). It has its own container,"
  echo "$HOME/Library/Containers/app.getvela.VelaWallet — no migration file, so a"
  echo "Developer ID copy's wallet on this Mac is left where it is."
  exit 0
fi

# ---------------------------------------------------------- the package --
pkg="$out_dir/VelaWallet-$version-$build_number-mas.pkg"
note "productbuild: ${pkg#"$project_root"/}"
productbuild --component "$app" /Applications --sign "$installer" "$pkg"
pkgutil --check-signature "$pkg" | sed 's/^/  /'

if (( validate )); then
  note "altool --validate-app (Apple checks the package; nothing is uploaded)"
  xcrun altool --validate-app -f "$pkg" -t macos "${asc_auth[@]}" ||
    die "App Store Connect did not validate the package (its words are above)"
  echo "  validated"
fi

echo
note "package: ${pkg#"$project_root"/}"
echo "To upload: open it in Transporter, or"
echo "  xcrun altool --upload-app -f '$pkg' -t macos <the same credentials>"
