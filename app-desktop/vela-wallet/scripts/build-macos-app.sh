#!/usr/bin/env bash
# Build "Vela Wallet.app" and its .dmg installer - Apple silicon, Intel, or a
# universal binary that runs on both.
#
#   ./scripts/build-macos-app.sh                   # host architecture
#   ./scripts/build-macos-app.sh --arch arm64      # Apple silicon (M1 and later)
#   ./scripts/build-macos-app.sh --arch x86_64     # Intel
#   ./scripts/build-macos-app.sh --arch universal  # one binary, both architectures
#   ./scripts/build-macos-app.sh --skip-build      # reuse target/<triple>/release
#   ./scripts/build-macos-app.sh --zip             # also produce a .zip
#   ./scripts/build-macos-app.sh --no-dmg          # bundle only, for iteration
#   ./scripts/build-macos-app.sh --distribution    # sign as for the public, no Apple round trip
#   ./scripts/build-macos-app.sh --notarize        # the package a person can actually open
#
# The only .dmg that may be PUBLISHED is a --notarize one (spec 063 §3): signed
# with a Developer ID, hardened runtime, the Developer ID provisioning profile
# embedded, notarized and stapled. Anything less is refused by Gatekeeper as
# "damaged" once it has been downloaded through a browser. It needs:
#
#   VELA_SIGN_IDENTITY       "Developer ID Application: …" — or its SHA-1, which
#                            is the only unambiguous form on a Mac whose
#                            keychain holds two certificates of the same name
#   VELA_PROVISION_PROFILE   the Developer ID profile for app.getvela.VelaWallet
#                            with Associated Domains (without it: no passkeys,
#                            and newer macOS kills the bundle at launch)
#   VELA_NOTARY_PROFILE      a `notarytool store-credentials` profile name
#     [VELA_NOTARY_KEYCHAIN] …and the keychain it lives in, when not the login one
#   — or VELA_NOTARY_KEY (.p8 path) + VELA_NOTARY_KEY_ID + VELA_NOTARY_ISSUER
#
# Outputs land in dist/macos/: the bundle at <arch>/Vela Wallet.app and the
# installer at VelaWallet-<version>-macos-<arch>.dmg, with a SHA-256 checksum
# printed for publishing next to the download.
#
# Every variant builds on either kind of Mac: the Apple SDK carries both
# architectures, so - unlike the Linux packages - no second machine is needed.
# The only per-architecture prerequisite is the Rust target, and the script
# stops with the exact `rustup target add` command when one is missing.
#
# Runs on macOS only: iconutil, lipo, codesign and hdiutil are Apple tools with
# no Linux equivalent, and a bundle assembled without them is not something to
# hand to anyone.
#
# Why a bundle at all, rather than shipping the bare executable: macOS takes the
# Dock icon, the application name and the high-DPI opt-in from Contents/, so an
# unbundled binary appears as a generic executable named "vela-wallet" and
# renders at 1x. There is no per-window API to fix that from inside gpui.
set -euo pipefail

app_name="Vela Wallet"
binary_name="vela-wallet"

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
packaging="$project_root/packaging"
dist_dir="$project_root/dist/macos"

die() { echo "error: $*" >&2; exit 1; }
note() { echo "==> $*"; }

arch="$(uname -m)"   # arm64 on Apple silicon, x86_64 on Intel
skip_build=0
make_zip=0
make_dmg=1
distribution=0   # sign the way a published package must be signed
notarize=0       # …and have Apple agree
while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch)       [[ $# -ge 2 ]] || die "--arch needs a value: arm64, x86_64 or universal"
                  arch="$2"; shift 2 ;;
    --arch=*)     arch="${1#--arch=}"; shift ;;
    --skip-build) skip_build=1; shift ;;
    --zip)        make_zip=1; shift ;;
    --no-dmg)     make_dmg=0; shift ;;
    --distribution) distribution=1; shift ;;
    --notarize)   distribution=1; notarize=1; shift ;;
    -h|--help)    sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'; exit 0 ;;
    *)            die "unknown option: $1" ;;
  esac
done

[[ "$(uname -s)" == "Darwin" ]] || die "this script only runs on macOS (uname reports $(uname -s)).
       The icon set it consumes, packaging/icons/macos/AppIcon.iconset, is
       generated on any platform by ./scripts/generate-desktop-icons.sh - but
       turning it into an .icns needs Apple's iconutil."

command -v iconutil >/dev/null 2>&1 || die "iconutil not found; install the Xcode command-line tools"

# Distribution preflight — BEFORE the build, so a missing credential costs
# seconds rather than the half hour the compile takes.
notary_auth=()
if (( distribution )); then
  [[ -n "${VELA_SIGN_IDENTITY:-}" ]] || die "--distribution/--notarize need VELA_SIGN_IDENTITY (a Developer ID Application identity)"
  # A warning in the local branch below; an ERROR here. A team signature that
  # claims associated-domains without a profile granting it is a bundle newer
  # macOS kills at launch — and one without the claim has no platform passkeys.
  # Neither is something to hand to the public.
  [[ -n "${VELA_PROVISION_PROFILE:-}" ]] || die "--distribution/--notarize need VELA_PROVISION_PROFILE
       (the Developer ID provisioning profile for app.getvela.VelaWallet, with
       Associated Domains — developer.apple.com > Profiles > Developer ID)"
  [[ -f "$VELA_PROVISION_PROFILE" ]] || die "no such provisioning profile: $VELA_PROVISION_PROFILE"
  (( make_dmg )) || die "--distribution/--notarize produce the .dmg; drop --no-dmg"
fi
if (( notarize )); then
  xcrun --find notarytool >/dev/null 2>&1 || die "notarytool not found; it ships with Xcode 13 and later"
  if [[ -n "${VELA_NOTARY_PROFILE:-}" ]]; then
    notary_auth=(--keychain-profile "$VELA_NOTARY_PROFILE")
    [[ -n "${VELA_NOTARY_KEYCHAIN:-}" ]] && notary_auth+=(--keychain "$VELA_NOTARY_KEYCHAIN")
  elif [[ -n "${VELA_NOTARY_KEY:-}" ]]; then
    [[ -f "$VELA_NOTARY_KEY" ]] || die "no such App Store Connect key: $VELA_NOTARY_KEY"
    [[ -n "${VELA_NOTARY_KEY_ID:-}" && -n "${VELA_NOTARY_ISSUER:-}" ]] ||
      die "VELA_NOTARY_KEY needs VELA_NOTARY_KEY_ID and VELA_NOTARY_ISSUER beside it"
    notary_auth=(--key "$VELA_NOTARY_KEY" --key-id "$VELA_NOTARY_KEY_ID" --issuer "$VELA_NOTARY_ISSUER")
  else
    die "--notarize needs notary credentials: VELA_NOTARY_PROFILE, or
       VELA_NOTARY_KEY + VELA_NOTARY_KEY_ID + VELA_NOTARY_ISSUER"
  fi
fi

# Submit one file to Apple's notary service and wait for the verdict. On
# anything but "Accepted" the notary's own log is printed — it names the exact
# binary and the exact reason, which the one-word status never does.
notarize_file() {
  local file="$1" out id status
  note "notarizing ${file##*/} (Apple usually answers in a few minutes)"
  out="$(xcrun notarytool submit "$file" "${notary_auth[@]}" --wait --output-format json)" ||
    die "notarytool submit failed for ${file##*/}: $out"
  id="$(/usr/bin/plutil -extract id raw -o - - <<<"$out" 2>/dev/null || true)"
  status="$(/usr/bin/plutil -extract status raw -o - - <<<"$out" 2>/dev/null || true)"
  if [[ "$status" != "Accepted" ]]; then
    echo "$out" >&2
    if [[ -n "$id" ]]; then
      xcrun notarytool log "$id" "${notary_auth[@]}" >&2 || true
    fi
    die "Apple did not accept ${file##*/} (status: ${status:-unknown})"
  fi
  echo "  accepted ($id)"
}

# --arch to Rust target triple(s). "universal" builds both and merges with lipo.
triples=()
case "$arch" in
  arm64)     triples=(aarch64-apple-darwin) ;;
  x86_64)    triples=(x86_64-apple-darwin) ;;
  universal) triples=(aarch64-apple-darwin x86_64-apple-darwin) ;;
  *)         die "unknown --arch '$arch' (expected arm64, x86_64 or universal)" ;;
esac

# Fail on the missing Rust target before spending minutes in the build. Plain
# cargo without rustup manages targets its own way, so only rustup is asked.
if command -v rustup >/dev/null 2>&1; then
  installed="$(rustup target list --installed)"
  for triple in "${triples[@]}"; do
    grep -qx "$triple" <<<"$installed" || die "the Rust target $triple is not installed.
       Install it with: rustup target add $triple"
  done
fi

version="$(sed -n '/^\[package\]/,/^\[/ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' \
  "$project_root/Cargo.toml" | head -1)"
[[ -n "$version" ]] || die "could not read the package version from Cargo.toml"

for triple in "${triples[@]}"; do
  if (( ! skip_build )); then
    note "cargo build --release --locked --target $triple"
    ( cd "$project_root" && cargo build --release --locked --target "$triple" )
  fi
  [[ -f "$project_root/target/$triple/release/$binary_name" ]] ||
    die "release binary not found: target/$triple/release/$binary_name
       Run without --skip-build, or build it first:
       cargo build --release --locked --target $triple"
done

# The binary that goes into the bundle. Universal is a lipo merge of the two
# single-architecture binaries; target/universal-apple-darwin is not a real
# triple, so cargo never writes there and the path cannot collide.
if [[ "$arch" == "universal" ]]; then
  release_binary="$project_root/target/universal-apple-darwin/release/$binary_name"
  mkdir -p "$(dirname "$release_binary")"
  note "lipo -create (${triples[*]})"
  lipo -create \
    "$project_root/target/aarch64-apple-darwin/release/$binary_name" \
    "$project_root/target/x86_64-apple-darwin/release/$binary_name" \
    -output "$release_binary"
else
  release_binary="$project_root/target/${triples[0]}/release/$binary_name"
fi

# Refuse to package a binary whose real architecture disagrees with --arch, so
# a mislabelled .dmg cannot reach a user. `lipo -archs` prints the slices of
# thin and fat binaries alike.
case "$arch" in
  universal) expected="arm64 x86_64" ;;
  *)         expected="$arch" ;;
esac
actual="$(lipo -archs "$release_binary" | tr ' ' '\n' | sort | xargs)"
[[ "$actual" == "$expected" ]] ||
  die "refusing to package: the binary contains [$actual] but --arch $arch expects [$expected]"

app="$dist_dir/$arch/$app_name.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

note "assembling $app_name.app ($version, $arch)"
install -m755 "$release_binary" "$app/Contents/MacOS/$binary_name"
sed "s|@VERSION@|$version|g" "$packaging/macos/Info.plist.in" > "$app/Contents/Info.plist"

note "AppIcon.icns"
iconset="$packaging/icons/macos/AppIcon.iconset"
[[ -d "$iconset" ]] || die "missing $iconset - run ./scripts/generate-desktop-icons.sh"
iconutil --convert icns "$iconset" --output "$app/Contents/Resources/AppIcon.icns"

# Signing. With VELA_SIGN_IDENTITY set (a "Developer ID Application: …" or
# "Apple Development: …" identity from the F9W689P9NE team), the bundle is
# signed with the associated-domains entitlement — which is what unlocks the
# macOS platform authenticator ("This device": Touch ID / iCloud passkeys for
# getvela.app). Without it, an ad-hoc signature: not a Developer ID signature
# and does not avoid Gatekeeper, but an unsigned bundle is refused outright on
# Apple silicon, so it is the difference between "warns on first launch" and
# "will not run" — and the entitlement is carried but ignored.
if command -v codesign >/dev/null 2>&1; then
  entitlements="$packaging/macos/entitlements.plist"
  # associated-domains is a RESTRICTED entitlement: under a real team signature
  # it only takes effect with a provisioning profile (portal: App ID
  # app.getvela.VelaWallet + Associated Domains capability + a Developer ID
  # profile) embedded in the bundle. Without one, newer macOS may refuse to
  # launch a team-signed bundle that claims the entitlement.
  if [[ -n "${VELA_PROVISION_PROFILE:-}" ]]; then
    note "embedding provisioning profile: $VELA_PROVISION_PROFILE"
    install -m644 "$VELA_PROVISION_PROFILE" "$app/Contents/embedded.provisionprofile"
  fi
  if [[ -n "${VELA_SIGN_IDENTITY:-}" ]]; then
    # The team-signed entitlements add the identifier claims the embedded
    # profile validates against; see entitlements-signed.plist. NOT --deep:
    # the proven recipe (gpui-demo/bundle.sh) signs the app seal only, and
    # deep-signing re-signs nested code with the app's entitlements, which is
    # never what a restricted entitlement should spread onto.
    note "code signature: $VELA_SIGN_IDENTITY (+ associated-domains entitlement)"
    # The notary refuses a signature without a SECURE timestamp; a local
    # development signature does not need one and should not need the network.
    if (( distribution )); then timestamp=(--timestamp); else timestamp=(--timestamp=none); fi
    codesign --force "${timestamp[@]}" --options runtime \
      --entitlements "$packaging/macos/entitlements-signed.plist" \
      --sign "$VELA_SIGN_IDENTITY" "$app"
  else
    note "ad-hoc code signature (set VELA_SIGN_IDENTITY for platform passkeys)"
    codesign --force --deep --entitlements "$entitlements" --sign - "$app"
  fi
  # `|| die`, not `&& echo`: under set -e a failure on the left of && does not
  # abort, and an unverifiable bundle must never reach the .dmg.
  codesign --verify --strict "$app" || die "the signature does not verify"
  echo "  signature verifies"
fi

# The APP is notarized and stapled before it goes into the image, not only the
# image after: a ticket stapled to the image stays on the image, and the thing
# a person launches is the copy they dragged out of it. Without its own ticket
# that copy needs Apple to be reachable at first launch.
if (( notarize )); then
  app_zip="$(mktemp -d "${TMPDIR:-/tmp}/vela-notary.XXXXXX")/$app_name.zip"
  ditto -c -k --keepParent "$app" "$app_zip"
  notarize_file "$app_zip"
  rm -rf "$(dirname "$app_zip")"
  xcrun stapler staple "$app" >/dev/null || die "could not staple the ticket to the bundle"
  echo "  ticket stapled to the bundle"
fi

checksums=()

# The .dmg is the artifact to distribute - the drag-to-/Applications window is
# the macOS install experience, the way the Setup.exe is Windows' and the
# .rpm/.deb are Linux's.
if (( make_dmg )); then
  dmg="$dist_dir/VelaWallet-$version-macos-$arch.dmg"
  note "dmg: ${dmg#"$project_root"/}"
  staging="$(mktemp -d "${TMPDIR:-/tmp}/vela-dmg.XXXXXX")"
  trap 'rm -rf "$staging"' EXIT
  # ditto, not cp: it preserves the metadata the ad-hoc signature covers.
  ditto "$app" "$staging/$app_name.app"
  ln -s /Applications "$staging/Applications"
  # HFS+ rather than hdiutil's default: APFS images refuse to mount before
  # 10.13, and an installer should not be the thing that surprises an old Mac.
  # Three attempts: GitHub's macOS runners intermittently fail hdiutil create
  # with "Resource busy" while XProtect scans the staging folder
  # (actions/runner-images#7522), and one transient hit should not abort a
  # release run that just spent half an hour compiling.
  created=0
  for attempt in 1 2 3; do
    if hdiutil create -volname "$app_name" -srcfolder "$staging" \
      -fs HFS+ -format UDZO -imagekey zlib-level=9 -ov -quiet "$dmg"; then
      created=1
      break
    fi
    echo "  hdiutil create failed (attempt $attempt of 3); retrying" >&2
    sleep 2
  done
  (( created )) || die "hdiutil create failed three times"
  rm -rf "$staging"
  trap - EXIT
  hdiutil verify -quiet "$dmg" || die "the image fails hdiutil verify: $dmg"
  echo "  image verifies"
  if (( distribution )); then
    codesign --force --timestamp --sign "$VELA_SIGN_IDENTITY" "$dmg" ||
      die "could not sign the image"
    echo "  image signed"
  fi
  if (( notarize )); then
    notarize_file "$dmg"
    xcrun stapler staple "$dmg" >/dev/null || die "could not staple the ticket to the image"
    echo "  ticket stapled to the image"
    # The question that matters, asked the way Gatekeeper asks it of a download.
    spctl --assess --type open --context context:primary-signature -v "$dmg" ||
      die "Gatekeeper would still refuse this image"
    spctl --assess --type execute -v "$app" ||
      die "Gatekeeper would still refuse the application"
  fi
  # AFTER stapling: the ticket changes the file, and a checksum of the
  # unstapled image matches nothing a person can download.
  checksums+=("$dmg")
fi

if (( make_zip )); then
  zip_path="$dist_dir/VelaWallet-$version-macos-$arch.zip"
  note "zip: ${zip_path#"$project_root"/}"
  # ditto, not zip: it preserves the resource forks and signature metadata that
  # a plain zip silently discards.
  ( cd "$dist_dir/$arch" && ditto -c -k --keepParent "$app_name.app" "$zip_path" )
  checksums+=("$zip_path")
fi

echo
note "bundle: ${app#"$project_root"/}"
echo "Run it with: open '$app'"
if (( ${#checksums[@]} )); then
  echo
  echo "SHA-256 checksums to publish next to the downloads:"
  ( cd "$dist_dir" && shasum -a 256 "${checksums[@]/#"$dist_dir"\//}" )
fi
echo
if (( notarize )); then
  echo "Signed, notarized and stapled: this is a package a person can open."
elif (( distribution )); then
  echo "NOTE: signed for distribution but NOT notarized (a rehearsal). Gatekeeper"
  echo "refuses it once downloaded; publish only what --notarize produced."
else
  echo "NOTE: not signed for distribution and not notarized. Gatekeeper reports a"
  echo "downloaded copy as damaged - fine on the Mac that built it, never"
  echo "something to publish (spec 063). See --notarize."
fi
