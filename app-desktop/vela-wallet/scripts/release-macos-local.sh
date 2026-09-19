#!/usr/bin/env bash
# Build, sign, notarize and upload the macOS packages for a desktop release —
# on THIS Mac, so the signing key never leaves it (spec 063, the founder's
# choice: nothing that can sign as us is stored on GitHub).
#
#   ./scripts/release-macos-local.sh v0.9.3            # build + verify, upload nothing
#   ./scripts/release-macos-local.sh v0.9.3 --upload   # …then attach to the release
#
# The tag is the one release.yml created at the head of release/v0.9.3 (spec
# 064): one tag per version, at the commit every other package was built from —
# so the images made here say the same "0.9.3 (abc1234)" as the rest.
#
# CI builds Windows and Linux from that commit and creates the release; its macOS
# job has no credentials, so it verifies the build and attaches nothing. This
# script supplies the missing third: the same three .dmg files, from the same
# tagged source, signed with the Developer ID and notarized by Apple.
#
# It decides nothing dangerous silently. Before compiling it checks — and says
# in words when it cannot continue — that:
#   * the checkout IS the tag, and clean where it matters;
#   * there is a Developer ID provisioning profile for app.getvela.VelaWallet
#     with Associated Domains (found automatically, or --profile PATH);
#   * the keychain holds the certificate THAT PROFILE was issued for (this is
#     how it picks between same-named certificates: by SHA-1, never by name);
#   * the notary credentials work (`--notary-profile NAME`, default vela-notary);
#   * with --upload: the release exists, and has no macOS packages yet.
#
# One-time setup, both done by you because both are yours:
#   1. developer.apple.com > Profiles > + > Developer ID > App ID
#      app.getvela.VelaWallet > your Developer ID Application certificate.
#      Download it; leaving it in ~/Downloads is enough.
#   2. xcrun notarytool store-credentials vela-notary \
#        --apple-id YOU@example.com --team-id F9W689P9NE
#      It ASKS for an app-specific password (account.apple.com > Sign-In and
#      Security > App-Specific Passwords) and keeps it in your login keychain.
set -euo pipefail

app_id="F9W689P9NE.app.getvela.VelaWallet"
repo="mondaylabsltd/vela-wallet"
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_root="$(cd "$project_root/../.." && pwd)"

die() { echo "error: $*" >&2; exit 1; }
note() { echo "==> $*"; }

tag=""; upload=0; replace=0; profile=""; notary_profile="vela-notary"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --upload)         upload=1; shift ;;
    --replace)        replace=1; shift ;;
    --profile)        [[ $# -ge 2 ]] || die "--profile needs a path"; profile="$2"; shift 2 ;;
    --notary-profile) [[ $# -ge 2 ]] || die "--notary-profile needs a name"; notary_profile="$2"; shift 2 ;;
    -h|--help)        sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'; exit 0 ;;
    v[0-9]*)          tag="$1"; shift ;;
    *)                die "unknown argument: $1 (expected the release's tag, e.g. v0.9.3)" ;;
  esac
done
[[ -n "$tag" ]] || die "which release? e.g. ./scripts/release-macos-local.sh v0.9.3"
[[ "$(uname -s)" == "Darwin" ]] || die "this script only runs on macOS"

# ------------------------------------------------------------ the source --
note "checking that this checkout is $tag"
tag_commit="$(git -C "$repo_root" rev-parse -q --verify "refs/tags/$tag^{commit}" 2>/dev/null)" ||
  die "no such tag here: $tag (git fetch --tags?)"
[[ "$(git -C "$repo_root" rev-parse HEAD)" == "$tag_commit" ]] ||
  die "HEAD is not $tag. The macOS packages must come from the same source as the
       Windows and Linux ones:  git checkout $tag"
[[ -z "$(git -C "$repo_root" status --porcelain -- app-desktop rust)" ]] ||
  die "app-desktop/ or rust/ has uncommitted changes; a release is built from the tag, exactly"
version="$(sed -n '/^\[package\]/,/^\[/ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$project_root/Cargo.toml" | head -1)"
[[ "v$version" == "$tag" ]] || die "Cargo.toml says $version but the tag is $tag"

# ----------------------------------------------------------- the profile --
profile_is_ours() {  # a Developer ID profile for this app, granting Associated Domains
  local plist; plist="$(security cms -D -i "$1" 2>/dev/null)" || return 1
  [[ "$(plutil -extract 'Entitlements.com\.apple\.application-identifier' raw - <<<"$plist" 2>/dev/null)" == "$app_id" ]] || return 1
  [[ "$(plutil -extract ProvisionsAllDevices raw - <<<"$plist" 2>/dev/null)" == "true" ]] || return 1
  plutil -extract 'Entitlements.com\.apple\.developer\.associated-domains' raw - <<<"$plist" >/dev/null 2>&1
}
if [[ -n "$profile" ]]; then
  [[ -f "$profile" ]] || die "no such file: $profile"
  profile_is_ours "$profile" || die "$profile is not a DEVELOPER ID profile for $app_id with Associated Domains
       (a 'Mac Team Provisioning Profile' is a development profile and cannot be distributed)"
else
  while IFS= read -r candidate; do
    if profile_is_ours "$candidate"; then profile="$candidate"; break; fi
  done < <(find "$HOME/Downloads" "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles" \
                "$HOME/Library/MobileDevice/Provisioning Profiles" -maxdepth 1 -name '*.provisionprofile' 2>/dev/null)
  [[ -n "$profile" ]] || die "no Developer ID provisioning profile for $app_id on this Mac.
       Make one (it takes two minutes and only the account holder can):
         developer.apple.com > Certificates, IDs & Profiles > Profiles > +
         > Distribution: Developer ID > App ID app.getvela.VelaWallet
         > select your Developer ID Application certificate > download
       Leave it in ~/Downloads and run this again."
fi
note "provisioning profile: ${profile/#$HOME/~}"

# ------------------------------------ the certificate the profile names --
profile_plist="$(security cms -D -i "$profile")"
identity=""; i=0
keychain_identities="$(security find-identity -v -p codesigning | awk '/Developer ID Application/ {print $2}')"
while der="$(plutil -extract "DeveloperCertificates.$i" raw - <<<"$profile_plist" 2>/dev/null)"; do
  sha="$(base64 --decode <<<"$der" | openssl sha1 | awk '{print toupper($NF)}')"
  if grep -qx "$sha" <<<"$keychain_identities"; then identity="$sha"; break; fi
  i=$((i + 1))
done
[[ -n "$identity" ]] || die "the profile was issued for a Developer ID certificate whose private key is not
       in this Mac's keychain. Either regenerate the profile and select the
       certificate you have here, or install the one it names."
signer="$(security find-identity -v -p codesigning | awk -v h="$identity" '$2 == h' | sed -E 's/^[^"]*"([^"]*)".*/\1/')"
note "signing as: $signer"

# -------------------------------------------------- the notary, up front --
xcrun notarytool history --keychain-profile "$notary_profile" >/dev/null 2>&1 ||
  die "no working notary credentials under the name '$notary_profile'. Once, on this Mac:
         xcrun notarytool store-credentials $notary_profile --apple-id YOU@example.com --team-id F9W689P9NE
       It asks for an APP-SPECIFIC password (account.apple.com > Sign-In and
       Security > App-Specific Passwords), not your Apple ID password."
note "notary credentials: $notary_profile (Apple accepted them)"

# ------------------------------------------------ the release, if upload --
if (( upload )); then
  command -v gh >/dev/null 2>&1 || die "--upload needs the GitHub CLI (gh)"
  existing="$(gh release view "$tag" --repo "$repo" --json assets -q '.assets[].name' 2>/dev/null)" ||
    die "there is no release for $tag yet. release.yml creates it (tag, notes, Windows and
       Linux packages) when release/$tag is pushed; wait for that run, then run this."
  if grep -qE '\.dmg$|^SHA256SUMS-macos$' <<<"$existing" && (( ! replace )); then
    die "$tag already has macOS packages. Replacing published files changes their
       checksums under people who downloaded them; pass --replace if you mean it."
  fi
fi

# ------------------------------------------------------------- the build --
export VELA_SIGN_IDENTITY="$identity" VELA_PROVISION_PROFILE="$profile" VELA_NOTARY_PROFILE="$notary_profile"
# The commit About shows: the tag's, said out loud rather than left to build.rs to find.
export VELA_GIT_COMMIT="$tag_commit"
rm -rf "$project_root/dist/macos"
"$project_root/scripts/build-macos-app.sh" --arch universal --notarize
"$project_root/scripts/build-macos-app.sh" --arch arm64 --skip-build --notarize
"$project_root/scripts/build-macos-app.sh" --arch x86_64 --skip-build --notarize

cd "$project_root/dist/macos"
for built in arm64 x86_64 universal; do
  # Not `strings | grep -q`: grep -q exits at the first match, strings dies of
  # SIGPIPE, and pipefail turns "found it early" into a failure. 0.9.3's three
  # good images were refused here for exactly that.
  grep -aq "${tag_commit:0:7}" "$built/Vela Wallet.app/Contents/MacOS/vela-wallet" ||
    die "the $built binary does not carry commit ${tag_commit:0:7} — About would show something else"
done
for dmg in ./*.dmg; do
  xcrun stapler validate "$dmg" >/dev/null || die "no stapled ticket on $dmg"
  spctl --assess --type open --context context:primary-signature "$dmg" || die "Gatekeeper refuses $dmg"
done
# The same shape CI writes (`sha256sum ./*`), so one verification habit fits all three platforms.
shasum -a 256 ./*.dmg > SHA256SUMS-macos
echo; cat SHA256SUMS-macos; echo

if (( upload )); then
  clobber=(); (( replace )) && clobber=(--clobber)
  gh release upload "$tag" ./*.dmg SHA256SUMS-macos --repo "$repo" ${clobber[@]+"${clobber[@]}"}
  note "attached to https://github.com/$repo/releases/tag/$tag"
  echo "Now the check no script can make: download one .dmg THROUGH A BROWSER (a file"
  echo "that never left this Mac is not quarantined), open it, make a wallet with"
  echo "\"This device\", and open the scanner."
else
  note "built and verified in ${PWD/#$HOME/~} — nothing uploaded (pass --upload)"
fi
