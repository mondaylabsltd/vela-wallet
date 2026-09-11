#!/usr/bin/env bash
# Render every app icon in the repository from the canonical vector source in
# docs/design/icon/. Run it after editing those SVGs, and commit the result:
#
#     ./scripts/gen-app-icons.sh
#
# Covers the two native projects (app-ios, app-android) and the marketing
# site. Desktop icons (Linux hicolor, Windows .ico, macOS .iconset)
# come from the same SVGs via
# app-desktop/vela-wallet/scripts/generate-desktop-icons.sh.
#
# Platform rules that drive the shapes below. Each one fails quietly - the icon
# just looks wrong on a device nobody happened to test:
#
#   iOS       Full-bleed SQUARE, NO alpha. iOS applies its own corner mask, so
#             pre-rounded corners stay baked in underneath it, and an alpha
#             channel is an App Store rejection. The dark and tinted variants
#             are the opposite: transparent, because the system draws its own
#             backdrop behind them.
#   Android   An adaptive icon is TWO layers, and only the inner 66.7% circle is
#             guaranteed visible. The foreground must be transparent apart from
#             the mark, or it hides the background layer and launcher parallax
#             drags a visible edge around.
#   macOS     Not handled here. The mark is inset inside a ~80% squircle per
#             Apple's icon grid - see the desktop script.
#   Web       Favicons are drawn as-is, so they keep the rounded tile.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
icon_src="$repo_root/docs/design/icon"
ios_iconset="$repo_root/app-ios/VelaWallet/VelaWallet/Assets.xcassets/AppIcon.appiconset"
android_res="$repo_root/app-android/vela-wallet/app/src/main/res"
site_static="$repo_root/app-web/getvela.app/static"

tile_color="#f46d50"   # must match the <rect> fill in docs/design/icon/app-icon.svg
svg_px=68              # the SVGs' intrinsic size, for the density calculation

# Android's adaptive icon (API 26+) is the VECTOR in app-android's drawable/,
# maintained there; only the pre-26 mipmap rasters are rendered here. The
# 66.7% safe-zone rule still applies to that vector: the mark's furthest point
# must stay inside the inner 170.7px of a 512 canvas or the sail tips and the
# hull get clipped on a circular launcher (the retired Expo layers used a 0.68
# inset for exactly this).

die() { echo "error: $*" >&2; exit 1; }
step() { echo "==> $*"; }

if command -v magick >/dev/null 2>&1; then im=(magick)
elif command -v convert >/dev/null 2>&1; then im=(convert)
else
  die "ImageMagick is required.
       Fedora: sudo dnf install ImageMagick
       Debian: sudo apt install imagemagick
       macOS:  brew install imagemagick"
fi

for f in app-icon app-mark app-mark-mono; do
  [[ -f "$icon_src/$f.svg" ]] || die "missing canonical source: $icon_src/$f.svg"
done

# ImageMagick rasterizes an SVG at its intrinsic size and then upscales unless
# the render density is raised to match the target; without this every icon
# above 68px is visibly soft.
density_for() { awk -v s="$1" -v n="$svg_px" 'BEGIN { printf "%.4f", 96 * s / n }'; }

# render <svg> <size> <out> - transparent, 8-bit RGBA, no date chunks.
# png:color-type=6 forces RGBA; without it a silhouette is written as
# greyscale+alpha, which is valid PNG but a surprise for anything downstream.
render() {
  "${im[@]}" -background none -density "$(density_for "$2")" "$icon_src/$1.svg" \
    -resize "${2}x${2}" -depth 8 -strip -define png:color-type=6 "$3"
}

# flat_square <size> <out> - the mark over an opaque full-bleed tile. Used
# wherever the platform masks corners itself and rejects alpha.
flat_square() {
  "${im[@]}" -size "${1}x${1}" "xc:$tile_color" \
    \( -background none -density "$(density_for "$1")" "$icon_src/app-mark.svg" \
       -resize "${1}x${1}" \) \
    -composite -alpha remove -alpha off -depth 8 -strip -define png:color-type=2 "$2"
}

# ------------------------------------------------------- native iOS project --

step "app-ios: AppIcon.appiconset (default / dark / tinted)"
mkdir -p "$ios_iconset"
flat_square 1024 "$ios_iconset/icon-1024.png"
# Dark and tinted variants are transparent on purpose: iOS composites them onto
# its own dark backdrop, and tinted is desaturated because the system applies
# the user's tint to luminance.
render app-mark 1024 "$ios_iconset/icon-1024-dark.png"
"${im[@]}" -background none -density "$(density_for 1024)" "$icon_src/app-mark-mono.svg" \
  -resize 1024x1024 -depth 8 -strip -define png:color-type=6 \
  "$ios_iconset/icon-1024-tinted.png"

cat > "$ios_iconset/Contents.json" <<'JSON'
{
  "images" : [
    {
      "filename" : "icon-1024.png",
      "idiom" : "universal",
      "platform" : "ios",
      "size" : "1024x1024"
    },
    {
      "appearances" : [
        {
          "appearance" : "luminosity",
          "value" : "dark"
        }
      ],
      "filename" : "icon-1024-dark.png",
      "idiom" : "universal",
      "platform" : "ios",
      "size" : "1024x1024"
    },
    {
      "appearances" : [
        {
          "appearance" : "luminosity",
          "value" : "tinted"
        }
      ],
      "filename" : "icon-1024-tinted.png",
      "idiom" : "universal",
      "platform" : "ios",
      "size" : "1024x1024"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}
JSON

# --------------------------------------------------- native Android project --

step "app-android: legacy mipmap rasters"
# API 26+ uses the vector adaptive icon in drawable/; these rasters are the
# pre-26 fallback, which is why they bake the tile in rather than layering.
declare -A dpi=( [mdpi]=48 [hdpi]=72 [xhdpi]=96 [xxhdpi]=144 [xxxhdpi]=192 )
for d in "${!dpi[@]}"; do
  size="${dpi[$d]}"
  mkdir -p "$android_res/mipmap-$d"
  flat_square "$size" "$android_res/mipmap-$d/ic_launcher.png"
  "${im[@]}" "$android_res/mipmap-$d/ic_launcher.png" \
    \( +clone -alpha extract -fill black -colorize 100 \
       -fill white -draw "circle $((size/2)),$((size/2)) $((size/2)),0" \) \
    -alpha off -compose copy_opacity -composite \
    -depth 8 -strip -define png:color-type=6 \
    "$android_res/mipmap-$d/ic_launcher_round.png"
  # The project stores these as .webp; convert and drop the intermediate PNGs.
  for n in ic_launcher ic_launcher_round; do
    "${im[@]}" "$android_res/mipmap-$d/$n.png" -define webp:lossless=true \
      "$android_res/mipmap-$d/$n.webp"
    rm -f "$android_res/mipmap-$d/$n.png"
  done
done

# -------------------------------------------------------------- getvela.app --

step "getvela.app: favicons, manifest icons and header logo"
render app-icon 96  "$site_static/favicon-96x96.png"
render app-icon 512 "$site_static/web-app-manifest-512x512.png"
render app-icon 192 "$site_static/web-app-manifest-192x192.png"
render app-icon 1254 "$site_static/icon.png"
render app-icon 1254 "$site_static/vela-logo.png"
# favicon-32 and apple-touch-icon are opaque: Safari draws the touch icon on a
# white sheet if it has alpha, which haloes the rounded corners.
"${im[@]}" -background "$tile_color" -density "$(density_for 32)" "$icon_src/app-icon.svg" \
  -resize 32x32 -alpha remove -alpha off -depth 8 -strip "$site_static/favicon-32.png"
flat_square 180 "$site_static/apple-touch-icon.png"
cp "$icon_src/app-icon.svg" "$site_static/favicon.svg"
"${im[@]}" -background none -density "$(density_for 256)" "$icon_src/app-icon.svg" \
  -resize 256x256 -define icon:auto-resize=48,32,16 "$site_static/favicon.ico"

# ---------------------------------------------------------------- summary --

echo
echo "Wrote:"
while IFS= read -r f; do
  [[ -f "$f" ]] || continue
  printf '  %-58s %s\n' "${f#"$repo_root"/}" \
    "$("${im[@]}" identify -format '%wx%h %[channels]' "${f}[0]" 2>/dev/null)"
done <<EOF
$ios_iconset/icon-1024.png
$ios_iconset/icon-1024-dark.png
$ios_iconset/icon-1024-tinted.png
$android_res/mipmap-xxxhdpi/ic_launcher.webp
$site_static/favicon-96x96.png
$site_static/apple-touch-icon.png
$site_static/web-app-manifest-512x512.png
$site_static/vela-logo.png
EOF

# Guard the mistake that is invisible until a store submission, long after
# the commit that caused it.
[[ "$("${im[@]}" identify -format '%[channels]' "$ios_iconset/icon-1024.png")" == *a* ]] &&
  die "the iOS icon has an alpha channel; App Store submission will reject it"
PY

echo "iOS icon has no alpha channel."
