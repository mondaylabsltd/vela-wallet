#!/usr/bin/env bash
# Build a Flatpak bundle of the desktop application from
# packaging/flatpak/app.getvela.VelaWallet.yml.
#
#   ./scripts/build-flatpak.sh                 # build + export dist/flatpak/*.flatpak
#   ./scripts/build-flatpak.sh --install       # also install it for the current user
#   ./scripts/build-flatpak.sh --worktree      # build uncommitted changes too
#   ./scripts/build-flatpak.sh --arch aarch64  # needs an aarch64 host or binfmt/qemu
#
# By default the manifest sources the LOCAL GIT REPOSITORY, not the working
# tree, so a release bundle always corresponds to a commit. Uncommitted changes
# under the paths the build reads are therefore an error rather than a silently
# missing change - pass --worktree to build them anyway.
#
# Everything here runs per-user. Nothing needs root, and nothing touches the
# system flatpak installation.
set -euo pipefail

appid="app.getvela.VelaWallet"

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$project_root/packaging/flatpak/$appid.yml"
dist_dir="$project_root/dist/flatpak"

die() { echo "error: $*" >&2; exit 1; }
note() { echo "==> $*"; }

arch="$(uname -m)"
do_install=0
worktree=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch)     [[ $# -ge 2 ]] || die "--arch needs a value"; arch="$2"; shift 2 ;;
    --install)  do_install=1; shift ;;
    --worktree) worktree=1; shift ;;
    -h|--help)  sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'; exit 0 ;;
    *)          die "unknown option: $1" ;;
  esac
done

command -v flatpak >/dev/null 2>&1 || die "flatpak is not installed.
       Fedora: sudo dnf install flatpak"

# org.flatpak.Builder is the upstream-recommended way to get flatpak-builder and
# needs no root, so prefer it over a distro package that may not be installed.
if flatpak info org.flatpak.Builder >/dev/null 2>&1; then
  builder=(flatpak run org.flatpak.Builder)
elif command -v flatpak-builder >/dev/null 2>&1; then
  builder=(flatpak-builder)
else
  die "flatpak-builder is not available. Install it without root:
           flatpak install --user flathub org.flatpak.Builder
       or with the distro package:
           sudo dnf install flatpak-builder"
fi

[[ -f "$manifest" ]] || die "manifest not found: $manifest"

# Keep the runtime version in one place - the manifest - so this script cannot
# install a different one than the build asks for.
runtime_version="$(sed -n "s/^runtime-version:[[:space:]]*'\{0,1\}\([^']*\)'\{0,1\}[[:space:]]*$/\1/p" \
  "$manifest" | head -1)"
[[ -n "$runtime_version" ]] || die "could not read runtime-version from $manifest"

if ! flatpak remote-list --user | grep -q '^flathub'; then
  note "adding the flathub remote for this user"
  flatpak remote-add --user --if-not-exists flathub \
    https://dl.flathub.org/repo/flathub.flatpakrepo
fi

note "runtime org.freedesktop.Platform//$runtime_version ($arch)"
flatpak install --user --arch "$arch" --noninteractive flathub \
  "org.freedesktop.Platform//$runtime_version" \
  "org.freedesktop.Sdk//$runtime_version" \
  "org.freedesktop.Sdk.Extension.rust-stable//$runtime_version" \
  "org.freedesktop.Sdk.Extension.llvm20//$runtime_version"

mkdir -p "$dist_dir"
build_dir="$dist_dir/build-$arch"
repo_dir="$dist_dir/repo"
# The version is in the file's name, like every other package on a release.
# It was not, through 0.9.3 (`app.getvela.VelaWallet-x86_64.flatpak`, the usual
# Flatpak spelling): two versions downloaded side by side were one name, and
# nobody could tell which was which.
version="$(sed -n '/^\[package\]/,/^\[/ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' \
  "$project_root/Cargo.toml" | head -1)"
[[ -n "$version" ]] || die "could not read the version from Cargo.toml"
bundle="$dist_dir/$appid-$version-$arch.flatpak"

# ------------------------------------------------------- which tree to build --

repo_root="$(git -C "$project_root" rev-parse --show-toplevel)"

# Everything the build reads: the crate, the vela-core path dependency, and the
# Lottie assets that launch_animation.rs pulls in with include_bytes!.
build_inputs=(app-desktop/vela-wallet rust design)

manifest_to_build="$manifest"

if (( worktree )); then
  # flatpak-builder copies a `dir` source wholesale, so the generated build
  # artefacts have to be skipped by name - target/ alone is several gigabytes.
  manifest_to_build="$dist_dir/$appid.worktree.yml"
  note "building the working tree (generated manifest, not for release)"
  python3 - "$manifest" "$manifest_to_build" "$repo_root" <<'PY'
import sys

try:
    import yaml
except ModuleNotFoundError:
    sys.exit("error: --worktree needs PyYAML.\n"
             "       Fedora: sudo dnf install python3-pyyaml")

src, dst, root = sys.argv[1], sys.argv[2], sys.argv[3]

with open(src) as f:
    manifest = yaml.safe_load(f)

# An absolute path so the generated manifest does not depend on where it is
# written, unlike the committed one's repo-relative path.
manifest["modules"][0]["sources"] = [{
    "type": "dir",
    "path": root,
    "skip": [
        ".git",
        "node_modules",
        "dist",
        "app-desktop/vela-wallet/target",
        "app-desktop/vela-wallet/dist",
        "rust/target",
    ],
}]

with open(dst, "w") as f:
    yaml.safe_dump(manifest, f, sort_keys=False, default_flow_style=False)
PY
else
  # The default git source builds HEAD. Uncommitted work would be missing from
  # the bundle with no visible sign, so refuse rather than build the wrong tree.
  dirty="$(git -C "$repo_root" status --porcelain -- "${build_inputs[@]}")"
  if [[ -n "$dirty" ]]; then
    printf '%s\n' "$dirty" >&2
    die "the manifest builds from git HEAD, and the paths above have uncommitted
       changes that would NOT be in the bundle. Commit them, or pass --worktree
       to build the working tree as it stands."
  fi
fi

note "flatpak-builder ($arch)"
"${builder[@]}" \
  --force-clean \
  --arch="$arch" \
  --repo="$repo_dir" \
  --state-dir="$dist_dir/.builder" \
  "$build_dir" "$manifest_to_build"

# Spec 064 §3, asserted in the package: the sandbox has neither CI's
# environment nor, necessarily, a usable git, so this is the one build where
# "About shows the commit" has to be looked at rather than assumed.
if (( ! worktree )); then
  want="${VELA_GIT_COMMIT:-$(git -C "$repo_root" rev-parse HEAD)}"
  want="${want:0:7}"
  grep -aq "$want" "$build_dir/files/bin/vela-wallet" ||
    die "the Flatpak's binary does not carry commit $want — About would show something else"
  note "vela-wallet carries $want"
fi

note "exporting bundle"
flatpak build-bundle --arch="$arch" "$repo_dir" "$bundle" "$appid"

if (( do_install )); then
  note "installing for the current user"
  flatpak install --user --noninteractive --reinstall "$bundle"
  echo "Run it with: flatpak run $appid"
fi

echo
note "bundle: ${bundle#"$project_root"/}"
sha256sum "$bundle"
echo "Install elsewhere with: flatpak install --user $(basename "$bundle")"
