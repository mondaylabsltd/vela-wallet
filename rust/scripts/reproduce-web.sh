#!/usr/bin/env bash
#
# Rebuild the web core from a clean export of a commit, in the pinned
# container (rust/scripts/web-build.Dockerfile), and compare the bytes with
# what that commit carries (§38, review 1-9; docs/security/verify.md).
#
#   rust/scripts/reproduce-web.sh [<commit>]       default: HEAD
#
# Environment:
#   DOCKER             the container CLI (default: docker; podman works)
#   VELA_PLATFORM      the container platform (default: linux/amd64, what CI
#                      runs; a Mac runs it under Rosetta / qemu)
#   VELA_BASE_IMAGE    the base image, by the SAME index digest the
#                      Dockerfile pins (a registry mirror); a different digest
#                      is refused
#   VELA_NODE_DIST, VELA_BINARYEN_DIST
#                      where the node and binaryen tarballs are fetched from
#                      (a mirror); their sha256 is pinned in the Dockerfile
#   VELA_BUILD_ARGS    extra `docker build` arguments, word-split (e.g. to
#                      blank a proxy Docker Desktop injects into builds:
#                      "--build-arg HTTPS_PROXY= --build-arg HTTP_PROXY=")
#   VELA_KEEP=1        keep the export and the image afterwards
#   VELA_REPRO_OUT     where the export goes (default: a fresh mktemp dir)
#
# Compared byte for byte: assets/wasm/vela_core_bg.<source>.wasm and every
# file of rust/pkg-web. Exit 0 only when every one is identical. The sha256 of
# each is printed, so the line a person compares against a release is here.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMMIT="${1:-HEAD}"
DOCKER="${DOCKER:-docker}"
PLATFORM="${VELA_PLATFORM:-linux/amd64}"
SHA="$(git -C "$ROOT" rev-parse --verify "$COMMIT^{commit}")"
OUT="${VELA_REPRO_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/vela-repro.XXXXXX")}"
IMAGE="vela-web-build:${PLATFORM//\//-}"

cleanup() {
  if [[ "${VELA_KEEP:-}" != 1 ]]; then
    rm -rf "$OUT"
    "$DOCKER" image rm -f "$IMAGE" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

echo "reproduce-web: commit $SHA, platform $PLATFORM, export $OUT"
mkdir -p "$OUT/tree"
# A clean export: no target/, no untracked file, nothing the checkout built.
git -C "$ROOT" archive --format=tar "$SHA" rust assets/wasm | tar -x -C "$OUT/tree"
mkdir -p "$OUT/committed"
cp -R "$OUT/tree/rust/pkg-web" "$OUT/committed/pkg-web"
cp -R "$OUT/tree/assets/wasm" "$OUT/committed/wasm"

PINNED="$(sed -n 's/^ARG BASE=.*@\(sha256:[0-9a-f]*\)$/\1/p' "$OUT/tree/rust/scripts/web-build.Dockerfile")"
BASE_ARGS=()
if [[ -n "${VELA_BASE_IMAGE:-}" ]]; then
  [[ "$VELA_BASE_IMAGE" == *"@$PINNED" ]] || { echo "VELA_BASE_IMAGE must end in @$PINNED" >&2; exit 2; }
  BASE_ARGS=(--build-arg "BASE=$VELA_BASE_IMAGE")
fi
[[ -n "${VELA_NODE_DIST:-}" ]] && BASE_ARGS+=(--build-arg "NODE_DIST=$VELA_NODE_DIST")
[[ -n "${VELA_BINARYEN_DIST:-}" ]] && BASE_ARGS+=(--build-arg "BINARYEN_DIST=$VELA_BINARYEN_DIST")
# shellcheck disable=SC2206
[[ -n "${VELA_BUILD_ARGS:-}" ]] && BASE_ARGS+=($VELA_BUILD_ARGS)
"$DOCKER" build --platform "$PLATFORM" -t "$IMAGE" ${BASE_ARGS[@]+"${BASE_ARGS[@]}"} \
  -f "$OUT/tree/rust/scripts/web-build.Dockerfile" "$OUT/tree/rust/scripts" >"$OUT/image.log" 2>&1 \
  || { tail -40 "$OUT/image.log"; exit 2; }

# The build writes rust/pkg-web and assets/wasm in the export, as it does in a
# checkout. CARGO_HOME is the image's; the mount is /vela either way.
"$DOCKER" run --rm --platform "$PLATFORM" -v "$OUT/tree:/vela" -w /vela "$IMAGE" \
  node rust/scripts/build-web.mjs >"$OUT/build.log" 2>&1 \
  || { tail -40 "$OUT/build.log"; exit 2; }

status=0
compare() {
  local committed="$1" built="$2" name="$3"
  if [[ ! -f "$built" ]]; then
    echo "  MISSING  $name (the build did not write it)"
    status=1
  elif [[ ! -f "$committed" ]]; then
    echo "  EXTRA    $name ($(shasum -a 256 "$built" | cut -c1-64))"
    status=1
  elif cmp -s "$committed" "$built"; then
    echo "  same     $name $(shasum -a 256 "$built" | cut -c1-64)"
  else
    echo "  DIFFERS  $name committed $(shasum -a 256 "$committed" | cut -c1-64) built $(shasum -a 256 "$built" | cut -c1-64)"
    status=1
  fi
}
while read -r name; do
  compare "$OUT/committed/wasm/$name" "$OUT/tree/assets/wasm/$name" "assets/wasm/$name"
done < <( (ls "$OUT/committed/wasm"; ls "$OUT/tree/assets/wasm") | sort -u)
for f in "$OUT/committed/pkg-web"/*; do
  compare "$f" "$OUT/tree/rust/pkg-web/$(basename "$f")" "rust/pkg-web/$(basename "$f")"
done
if [[ $status == 0 ]]; then
  echo "reproduce-web: the committed web core is exactly what $SHA builds"
else
  echo "reproduce-web: the committed web core is NOT what $SHA builds (build log: $OUT/build.log)" >&2
  VELA_KEEP=1
fi
exit $status
