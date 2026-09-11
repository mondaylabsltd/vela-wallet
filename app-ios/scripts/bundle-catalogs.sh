#!/bin/bash
#
# Xcode build phase: Bundle locale catalogs  (spec 010-ios-catalog-bundling)
#
# Copies the merged runtime catalogs (assets/i18n/<lng>.json) into
# VelaWallet.app at build time, so the repository keeps exactly one copy of
# them — the arrangement Android has had since spec 008.
#
# This script deliberately hardcodes NOTHING: not the corpus path, not the
# bundle path, not the locale list, not the locale count. Everything comes from
# the two .xcfilelists the build phase declares, which Xcode resolves (expanding
# $(SRCROOT), $(TARGET_BUILD_DIR), …) and hands over as SCRIPT_INPUT_FILE_LIST_0
# and SCRIPT_OUTPUT_FILE_LIST_0. That is what makes the declaration
# authoritative (research D3): a locale that is not declared cannot be shipped,
# and — with ENABLE_USER_SCRIPT_SANDBOXING=YES — cannot even be read.
#
# Contract: specs/010-ios-catalog-bundling/contracts/build-phase.md
#
set -euo pipefail

# Xcode surfaces `error:`-prefixed lines in the Issue navigator; a bare echo
# would only reach the raw log, where a broken localization build is invisible.
fail() {
	echo "error: [bundle-catalogs] $1" >&2
	exit 1
}

[ "${SCRIPT_INPUT_FILE_LIST_COUNT:-0}" -eq 1 ] ||
	fail "expected exactly 1 input file list, got ${SCRIPT_INPUT_FILE_LIST_COUNT:-0} — the build phase is misconfigured (see contracts/build-phase.md)"
[ "${SCRIPT_OUTPUT_FILE_LIST_COUNT:-0}" -eq 1 ] ||
	fail "expected exactly 1 output file list, got ${SCRIPT_OUTPUT_FILE_LIST_COUNT:-0} — the build phase is misconfigured (see contracts/build-phase.md)"

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
	fail "input file list is empty: $input_list — run \`node app-ios/scripts/gen-catalog-filelists.mjs\`"
[ "${#sources[@]}" -eq "${#targets[@]}" ] ||
	fail "declaration mismatch: ${#sources[@]} inputs vs ${#targets[@]} outputs — run \`node app-ios/scripts/gen-catalog-filelists.mjs\`"

# Every declared input lives in the corpus directory; take it from entry 0
# rather than reconstructing a path, so this stays free of repo layout
# knowledge. Used only for the extra-locale check below.
corpus_dir="$(dirname "${sources[0]}")"
[ -d "$corpus_dir" ] ||
	fail "locale corpus directory not found: $corpus_dir (expected the repo's assets/i18n) — regenerate it with \`node scripts/gen-i18n.mjs\`"

# A locale added to the corpus but not to the declaration would otherwise ship
# silently as a 15-of-16 app. Fail instead (spec US3).
for candidate in "$corpus_dir"/*.json; do
	[ -e "$candidate" ] || fail "locale corpus directory is empty: $corpus_dir"
	declared=false
	for source in "${sources[@]}"; do
		if [ "$source" = "$candidate" ]; then
			declared=true
			break
		fi
	done
	[ "$declared" = true ] ||
		fail "$(basename "$candidate") exists in the corpus but is not declared in the build's file lists — run \`node app-ios/scripts/gen-catalog-filelists.mjs\`"
done

for index in "${!sources[@]}"; do
	source="${sources[$index]}"
	target="${targets[$index]}"

	[ -f "$source" ] ||
		fail "declared catalog is missing: $source — run \`node scripts/gen-i18n.mjs\`, then \`node app-ios/scripts/gen-catalog-filelists.mjs\`"
	[ -s "$source" ] ||
		fail "declared catalog is empty: $source"

	mkdir -p "$(dirname "$target")" || fail "cannot create bundle directory for $target"
	cp -f "$source" "$target" || fail "copy failed: $source -> $target"
done

echo "[bundle-catalogs] bundled ${#sources[@]} locale catalogs from $corpus_dir"
