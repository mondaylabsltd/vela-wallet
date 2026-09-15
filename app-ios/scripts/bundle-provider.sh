#!/bin/bash
#
# Xcode build phase: Bundle dApp provider  (spec 053-ios-dapp-browser-signing)
#
# Copies the page-side EIP-1193 provider out of the web tree and into
# VelaWallet.app, so the repository keeps exactly one copy of it — the
# arrangement spec 010 established for locale catalogs and spec 012 for launch
# animations.
#
# A sibling of bundle-animations.sh rather than a shared abstraction, and for a
# sharper reason than that script had: bundle-catalogs.sh aborts when a glob
# matches nothing, because a missing locale is a shipping bug. Here the file
# list is exact and there is no directory to scan, so the corpus rule would be
# dead code pretending to be a safety net.
#
# The bytes matter. `ProviderBundleTests` compares what lands here with what
# the web tree holds, and the provider is assembled at runtime by stripping
# exactly two module keywords — so a transform in this script (minifying,
# rewriting, concatenating) would break the comparison AND the assembly.
# Copy, nothing else.
#
# Contract: specs/053-ios-dapp-browser-signing/contracts/page-envelope.md
#
set -euo pipefail

# Xcode surfaces `error:`-prefixed lines in the Issue navigator; a bare echo
# would only reach the raw log, where a broken build phase is invisible.
fail() {
	echo "error: [bundle-provider] $1" >&2
	exit 1
}

[ "${SCRIPT_INPUT_FILE_LIST_COUNT:-0}" -eq 1 ] ||
	fail "expected exactly 1 input file list, got ${SCRIPT_INPUT_FILE_LIST_COUNT:-0} — the build phase is misconfigured"
[ "${SCRIPT_OUTPUT_FILE_LIST_COUNT:-0}" -eq 1 ] ||
	fail "expected exactly 1 output file list, got ${SCRIPT_OUTPUT_FILE_LIST_COUNT:-0} — the build phase is misconfigured"

input_list="${SCRIPT_INPUT_FILE_LIST_0}"
output_list="${SCRIPT_OUTPUT_FILE_LIST_0}"

[ -f "$input_list" ] || fail "resolved input file list not found: $input_list"
[ -f "$output_list" ] || fail "resolved output file list not found: $output_list"

strip() { grep -v -e '^[[:space:]]*$' -e '^[[:space:]]*#' "$1" || true; }

IFS=$'\n' read -r -d '' -a sources < <(strip "$input_list" && printf '\0')
IFS=$'\n' read -r -d '' -a targets < <(strip "$output_list" && printf '\0')

[ "${#sources[@]}" -eq 2 ] ||
	fail "the provider is exactly two files (protocol.js, inpage.js); the declaration has ${#sources[@]}"
[ "${#sources[@]}" -eq "${#targets[@]}" ] ||
	fail "declaration mismatch: ${#sources[@]} inputs vs ${#targets[@]} outputs"

for index in "${!sources[@]}"; do
	source="${sources[$index]}"
	target="${targets[$index]}"

	[ -f "$source" ] ||
		fail "the provider script is missing: $source — the web tree is where it lives, and this build phase is the only copy"
	[ -s "$source" ] ||
		fail "the provider script is empty: $source"

	mkdir -p "$(dirname "$target")" || fail "cannot create bundle directory for $target"
	cp -f "$source" "$target" || fail "copy failed: $source -> $target"
done

echo "[bundle-provider] bundled ${#sources[@]} provider scripts"
