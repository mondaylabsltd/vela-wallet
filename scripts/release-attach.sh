#!/usr/bin/env bash
# Attach packages signed on THIS machine to a GitHub Release — any subset, one
# command (spec 065, Part B).
#
#   ./scripts/release-attach.sh v0.9.3 app-desktop/vela-wallet/dist/macos/*.dmg
#   ./scripts/release-attach.sh v0.9.3 ~/Downloads/VelaWallet-Setup-0.9.3-x64.exe
#   ./scripts/release-attach.sh v0.9.3 --replace one.dmg     # swap a published file
#   ./scripts/release-attach.sh v0.9.3 --dry-run *.dmg       # every check, no upload
#
# Why a script and not `gh release upload`: spec 063's rule is that nothing sits
# on a Release that a person cannot install, and a file attached by hand is
# exactly where that rule is easiest to break. So before ANYTHING is uploaded,
# every file must pass, and a refusal says in words what is wrong:
#
#   1. it is what it claims   .dmg: notarized, stapled, accepted by Gatekeeper
#                             .exe: carries a valid Authenticode signature
#                             anything else: refused by name, not guessed at
#   2. it belongs here        its name carries this tag's version — in the exact
#                             shape the download page looks for — and, where the
#                             format lets us look (.dmg), the binary inside
#                             carries this tag's commit (spec 064 §3)
#   3. it replaces nothing    unless --replace: a published file changing under
#                             people who already downloaded it changes a checksum
#                             they may have verified
#   4. it is not a phone app  .apk / .aab / .ipa never go on a Release
#                             (spec 065 §0.3); those go to Play / App Store
#
# All files are checked before the first is uploaded: a batch is attached whole
# or not at all. Then the platform's checksum file on the Release is updated
# (SHA256SUMS-macos for .dmg, SHA256SUMS for .exe) — the download mirror verifies
# against those, so a --replace also evicts the old copy from it, by itself.
set -euo pipefail

repo="mondaylabsltd/vela-wallet"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

die() { echo "error: $*" >&2; exit 1; }
note() { echo "==> $*"; }

tag=""; replace=0; dry_run=0; files=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --replace) replace=1; shift ;;
    --dry-run) dry_run=1; shift ;;
    -h|--help) sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'; exit 0 ;;
    --*)       die "unknown option: $1" ;;
    *)         if [[ -z "$tag" ]]; then tag="$1"; else files+=("$1"); fi; shift ;;
  esac
done
[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
  die "the first argument is the release's tag, e.g. v0.9.3 (got: ${tag:-nothing})"
(( ${#files[@]} )) || die "which files? e.g. ./scripts/release-attach.sh $tag dist/macos/*.dmg"
version="${tag#v}"

command -v gh >/dev/null || die "the GitHub CLI (gh) is needed to read and update the release"
published="$(gh release view "$tag" --repo "$repo" --json assets -q '.assets[].name')" ||
  die "there is no release $tag on $repo — release.yml creates it when release/$tag is pushed"
commit="$(git -C "$repo_root" rev-parse "refs/tags/$tag^{commit}" 2>/dev/null)" ||
  die "tag $tag is not in this checkout — run: git fetch origin --tags"
commit7="${commit:0:7}"

# ------------------------------------------------------------- the checks --

check_dmg() {
  local file="$1" name="$2"
  [[ "$name" =~ ^VelaWallet-${version//./\\.}-macos-(arm64|x86_64|universal)\.dmg$ ]] ||
    die "$name is not named for $tag. Expected VelaWallet-$version-macos-{arm64,x86_64,universal}.dmg —
       the download page finds files by that shape, so another name is a file nobody is offered"
  [[ "$(uname -s)" == "Darwin" ]] || die "$name: a .dmg can only be verified on macOS"

  xcrun stapler validate "$file" >/dev/null 2>&1 ||
    die "$name has no stapled notarization ticket — it would be refused on a Mac that is offline.
       Build it with: app-desktop/vela-wallet/scripts/release-macos-local.sh $tag"
  spctl --assess --type open --context context:primary-signature "$file" >/dev/null 2>&1 ||
    die "Gatekeeper refuses $name — it is not signed with a Developer ID and notarized"

  local mount; mount="$(mktemp -d)"
  hdiutil attach -nobrowse -readonly -mountpoint "$mount" "$file" >/dev/null ||
    die "$name does not mount"
  local binary="$mount/Vela Wallet.app/Contents/MacOS/vela-wallet" carries=1
  # grep on the file, never `strings | grep -q`: that fails under pipefail
  # exactly when it finds the commit early (it refused 0.9.3's good images).
  [[ -f "$binary" ]] && grep -aq "$commit7" "$binary" || carries=0
  hdiutil detach "$mount" >/dev/null 2>&1 || true
  rmdir "$mount" 2>/dev/null || true
  (( carries )) ||
    die "the app inside $name does not carry commit $commit7 ($tag) — it was built from another
       commit, and its About would say so. Check out $tag and build again"
}

check_exe() {
  local file="$1" name="$2"
  [[ "$name" =~ ^VelaWallet-Setup-${version//./\\.}-(x64|arm64)\.exe$ ]] ||
    die "$name is not named for $tag. Expected VelaWallet-Setup-$version-{x64,arm64}.exe"
  command -v osslsigncode >/dev/null ||
    die "$name: checking an Authenticode signature needs osslsigncode (brew install osslsigncode)"
  osslsigncode verify -in "$file" >/dev/null 2>&1 ||
    die "$name carries no valid Authenticode signature. The unsigned installer is already on the
       release, built by CI; the only reason to attach one by hand is that it is signed"
  # An Inno Setup installer compresses its payload: the commit cannot be read
  # out of it, so this check is the name's alone. Said rather than skipped quietly.
  echo "    note: the commit inside an installer cannot be inspected; trusted from the name"
}

for file in "${files[@]}"; do
  name="$(basename "$file")"
  [[ -f "$file" ]] || die "no such file: $file"
  note "checking $name"
  case "$name" in
    *.apk|*.aab|*.ipa)
      die "$name is a phone package. Phones are never attached to a GitHub Release (spec 065 §0.3):
       Android goes to Google Play, iOS to App Store Connect — different tools, not this one" ;;
    *.dmg) check_dmg "$file" "$name" ;;
    *.exe) check_exe "$file" "$name" ;;
    *)     die "$name: this script only knows how to verify .dmg and .exe, and does not attach what it
       cannot verify. Everything else on a release is built and attached by release.yml" ;;
  esac
  if grep -qxF "$name" <<<"$published" && (( ! replace )); then
    die "$name is already published on $tag. Replacing it changes a checksum under people who
       already downloaded it; if that is what you mean, say --replace"
  fi
done

# ------------------------------------------------------------- the upload --

sums_for() { case "$1" in *.dmg) echo SHA256SUMS-macos ;; *) echo SHA256SUMS ;; esac; }

work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
for file in "${files[@]}"; do
  name="$(basename "$file")"; sums="$(sums_for "$name")"
  if [[ ! -f "$work/$sums" ]]; then
    : > "$work/$sums"
    if grep -qxF "$sums" <<<"$published"; then
      gh release download "$tag" --repo "$repo" -p "$sums" -D "$work" --clobber
    fi
  fi
  # The shape CI writes (`sha256sum ./*`): one verification habit for every platform.
  { grep -vF "  ./$name" "$work/$sums" || true; echo "$(shasum -a 256 "$file" | cut -d' ' -f1)  ./$name"; } |
    sort -k2 > "$work/$sums.next"
  mv "$work/$sums.next" "$work/$sums"
done

if (( dry_run )); then
  note "every check passed — nothing uploaded (--dry-run). The checksum files would become:"
  for sums in "$work"/SHA256SUMS*; do echo "--- $(basename "$sums")"; cat "$sums"; done
  exit 0
fi

clobber=(); (( replace )) && clobber=(--clobber)
gh release upload "$tag" "${files[@]}" --repo "$repo" ${clobber[@]+"${clobber[@]}"}
gh release upload "$tag" "$work"/SHA256SUMS* --repo "$repo" --clobber
note "attached to https://github.com/$repo/releases/tag/$tag"
echo "getvela.app/get-started offers them within five minutes (its list is cached that long)."
