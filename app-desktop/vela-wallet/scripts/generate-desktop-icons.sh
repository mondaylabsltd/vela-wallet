#!/usr/bin/env bash
# Render the desktop application icons - Linux hicolor, the Windows .ico and the
# macOS .iconset - from the canonical vector source in docs/design/icon/, the same
# file every mobile and web icon comes from (scripts/gen-app-icons.sh).
#
# The results are committed under packaging/icons/ so that building a package
# needs no image tooling; only this script does. Re-run it after editing
# docs/design/icon/app-icon.svg, and commit the diff:
#
#     ./scripts/generate-desktop-icons.sh
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_root="$(cd "$project_root/../.." && pwd)"

appid="app.getvela.VelaWallet"
icon_src="$repo_root/docs/design/icon"
source_svg="$icon_src/app-icon.svg"
out_dir="$project_root/packaging/icons"

# hicolor's standard sizes, plus 96: GNOME's app grid asks for 96px, and with no
# exact match the lookup falls through to scalable/ and depends on SVG
# rasterization working - which is a needless way to lose the icon.
sizes=(16 24 32 48 64 96 128 256 512)

# Windows ICO members. 256 is the largest Explorer uses.
ico_sizes=256,128,64,48,32,16

# macOS draws no mask and adds no padding of its own, so the artwork must carry
# both. Apple's icon grid puts the rounded square at 824 of 1024 units; the rest
# is transparent margin the system uses for the drop shadow.
macos_inset=0.805

svg_px=68              # the source SVG's intrinsic size, for render density

die() { echo "error: $*" >&2; exit 1; }

if command -v magick >/dev/null 2>&1; then im=(magick)
elif command -v convert >/dev/null 2>&1; then im=(convert)
else
  die "ImageMagick is required.
       Fedora: sudo dnf install ImageMagick
       Debian: sudo apt install imagemagick
       macOS:  brew install imagemagick"
fi

[[ -f "$source_svg" ]] || die "canonical source not found: $source_svg"

# -density is what makes this a vector render at the target size. Without it
# ImageMagick rasterizes at the SVG's intrinsic 68x68 and upscales, and every
# icon above 68px comes out visibly blurred. 96 is ImageMagick's default DPI, so
# scaling it by size/68 renders exactly `size` pixels.
density_for() { awk -v s="$1" -v n="$svg_px" 'BEGIN { printf "%.4f", 96 * s / n }'; }

render() {
  # -strip drops the date chunks that would otherwise make every regeneration a
  # non-empty diff.
  "${im[@]}" -background none -density "$(density_for "$1")" "$source_svg" \
    -resize "${1}x${1}" -depth 8 -strip -define png:color-type=6 "$2"
}

echo "==> Linux: hicolor PNGs"
for size in "${sizes[@]}"; do
  mkdir -p "$out_dir/${size}x${size}"
  render "$size" "$out_dir/${size}x${size}/$appid.png"
  echo "  ${size}x${size}"
done

echo "==> Linux: scalable SVG"
mkdir -p "$out_dir/scalable"
# A copy of the canonical file, not a second drawing of it, so the two can never
# disagree. Note the canonical SVG keeps its <svg> tag on line 2 deliberately:
# gdk-pixbuf sniffs only the first few hundred bytes to identify an SVG, and a
# comment above the tag makes the file unrecognizable as an image.
cp "$source_svg" "$out_dir/scalable/$appid.svg"

echo "==> Windows: multi-resolution .ico"
# gpui loads the window icon with LoadImageW(module, MAKEINTRESOURCE(1), ...),
# so this file has to reach the executable as resource id 1 - see build.rs.
"${im[@]}" -background none -density "$(density_for 256)" "$source_svg" \
  -resize 256x256 -depth 8 -strip \
  -define "icon:auto-resize=$ico_sizes" "$out_dir/$appid.ico"

echo "==> macOS: AppIcon.iconset"
iconset="$out_dir/macos/AppIcon.iconset"
rm -rf "$iconset"
mkdir -p "$iconset"
# name:pixel-size pairs; iconutil derives the @2x variants from the file names.
for entry in \
  icon_16x16:16      icon_16x16@2x:32 \
  icon_32x32:32      icon_32x32@2x:64 \
  icon_128x128:128   icon_128x128@2x:256 \
  icon_256x256:256   icon_256x256@2x:512 \
  icon_512x512:512   icon_512x512@2x:1024
do
  name="${entry%%:*}"; px="${entry##*:}"
  inner="$(awk -v s="$px" -v f="$macos_inset" 'BEGIN { printf "%d", s * f }')"
  "${im[@]}" -background none -density "$(density_for "$inner")" "$source_svg" \
    -resize "${inner}x${inner}" \
    -gravity center -background none -extent "${px}x${px}" \
    -depth 8 -strip -define png:color-type=6 "$iconset/$name.png"
done
echo "  10 files, mark inset to ${macos_inset} of the canvas"

echo
echo "Written under ${out_dir#"$repo_root"/}:"
printf '  %-34s %s\n' "$appid.ico" \
  "$("${im[@]}" identify "$out_dir/$appid.ico" 2>/dev/null | wc -l) members"
printf '  %-34s %s\n' "scalable/$appid.svg" "copied from docs/design/icon/app-icon.svg"
printf '  %-34s %s\n' "macos/AppIcon.iconset/" "$(find "$iconset" -name '*.png' | wc -l) PNGs"
printf '  %-34s %s\n' "hicolor PNGs" "${#sizes[@]} sizes"
