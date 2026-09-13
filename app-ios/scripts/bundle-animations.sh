#!/bin/bash
#
# Xcode build phase: Bundle launch animations  (spec 012-launch-animation-lottie)
#
# Copies the shipped launch animations (docs/design/onboarding/launch/*-core-*.json)
# into VelaWallet.app at build time, so the repository keeps exactly one copy of
# them — the arrangement spec 010 established for locale catalogs and spec 008
# established for Android.
#
# This script hardcodes NOTHING: not the source path, not the bundle path, not
# the file list, not the count. Everything comes from the two .xcfilelists the
# build phase declares, which Xcode resolves (expanding $(SRCROOT),
# $(TARGET_BUILD_DIR), …) and hands over as SCRIPT_INPUT_FILE_LIST_0 and
# SCRIPT_OUTPUT_FILE_LIST_0. That declaration is what makes the phase legal under
# ENABLE_USER_SCRIPT_SANDBOXING (it grants read access outside SRCROOT), what
# makes it incremental, and what defines "which animations exist" for iOS.
#
# Deliberately a sibling of bundle-catalogs.sh rather than a shared abstraction:
# the two differ in their corpus rule (catalogs pin an exact locale count;
# animations must NOT, or adding one would require a build edit — FR-004), and
# refactoring a shipped build phase to save forty lines of bash is a poor trade.
#
# Contract: specs/012-launch-animation-lottie/contracts/portable-subset.md
#
set -euo pipefail

# Xcode surfaces `error:`-prefixed lines in the Issue navigator; a bare echo
# would only reach the raw log, where a broken build phase is invisible.
fail() {
	echo "error: [bundle-animations] $1" >&2
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

# Blank lines and # comments are legal in an .xcfilelist; drop them so the two
# lists stay index-aligned.
strip() { grep -v -e '^[[:space:]]*$' -e '^[[:space:]]*#' "$1" || true; }

IFS=$'\n' read -r -d '' -a sources < <(strip "$input_list" && printf '\0')
IFS=$'\n' read -r -d '' -a targets < <(strip "$output_list" && printf '\0')

[ "${#sources[@]}" -gt 0 ] ||
	fail "input file list is empty: $input_list — run \`node app-ios/scripts/gen-animation-filelists.mjs\`"
[ "${#sources[@]}" -eq "${#targets[@]}" ] ||
	fail "declaration mismatch: ${#sources[@]} inputs vs ${#targets[@]} outputs — run \`node app-ios/scripts/gen-animation-filelists.mjs\`"

# A `-core-` framing present in the design directory but absent from the
# declaration would ship as a silently incomplete app — the launch animation
# would simply not appear for whichever appearance or form factor is missing.
source_dir="$(dirname "${sources[0]}")"
[ -d "$source_dir" ] ||
	fail "launch animation directory not found: $source_dir (expected docs/design/onboarding/launch)"

for candidate in "$source_dir"/*-core-*.json; do
	[ -e "$candidate" ] || fail "no shipped (-core-) animations found in $source_dir"
	declared=false
	for source in "${sources[@]}"; do
		if [ "$source" = "$candidate" ]; then
			declared=true
			break
		fi
	done
	[ "$declared" = true ] ||
		fail "$(basename "$candidate") exists in the design directory but is not declared — run \`node app-ios/scripts/gen-animation-filelists.mjs\`"
done

for index in "${!sources[@]}"; do
	source="${sources[$index]}"
	target="${targets[$index]}"

	[ -f "$source" ] ||
		fail "declared animation is missing: $source — run \`node app-ios/scripts/gen-animation-filelists.mjs\`"
	[ -s "$source" ] ||
		fail "declared animation is empty: $source"

	mkdir -p "$(dirname "$target")" || fail "cannot create bundle directory for $target"
	cp -f "$source" "$target" || fail "copy failed: $source -> $target"
done

echo "[bundle-animations] bundled ${#sources[@]} launch animations from $source_dir"
