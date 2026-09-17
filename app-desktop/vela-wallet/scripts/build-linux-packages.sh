#!/usr/bin/env bash
# Build distributable Linux packages (.rpm, .deb, .tar.gz) for the GPUI desktop
# application, the way scripts/build-windows-installer.ps1 builds the Windows
# installer: compile the release binary, then wrap it with the metadata a
# desktop environment needs.
#
# Architecture: this script builds for the machine it runs on. It does NOT
# cross-compile. ThorVG's vendored C++ and gpui's Wayland/X11/Vulkan link set
# make a cross build far more trouble than running the same script on an
# aarch64 machine, an aarch64 container, or an aarch64 CI runner - all three of
# which take this script unchanged. See the README section "ARM64 packages".
# Pass --binary to wrap a release binary that was built somewhere else.
#
#   ./scripts/build-linux-packages.sh                     # rpm + deb for this host
#   ./scripts/build-linux-packages.sh --formats rpm       # just the rpm
#   ./scripts/build-linux-packages.sh --formats rpm,deb,tar
#   ./scripts/build-linux-packages.sh --skip-build        # reuse target/release
#   ./scripts/build-linux-packages.sh --binary out/vela-wallet --arch aarch64
#
# Output lands in dist/linux/, next to a SHA256SUMS file.
set -euo pipefail

appid="app.getvela.VelaWallet"
binary_name="vela-wallet"
icon_sizes=(16 24 32 48 64 96 128 256 512)

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
packaging_dir="$project_root/packaging"
dist_dir="$project_root/dist/linux"

die() { echo "error: $*" >&2; exit 1; }
note() { echo "==> $*"; }

usage() {
  sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'
}

# ---------------------------------------------------------------- arguments --

arch=""
prebuilt_binary=""
formats="rpm,deb"
skip_build=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch)     [[ $# -ge 2 ]] || die "--arch needs a value"; arch="$2"; shift 2 ;;
    --binary)   [[ $# -ge 2 ]] || die "--binary needs a path"; prebuilt_binary="$2"; shift 2 ;;
    --formats)  [[ $# -ge 2 ]] || die "--formats needs a value"; formats="$2"; shift 2 ;;
    --skip-build) skip_build=1; shift ;;
    -h|--help)  usage; exit 0 ;;
    *)          usage >&2; die "unknown option: $1" ;;
  esac
done

host_arch="$(uname -m)"
arch="${arch:-$host_arch}"

case "$arch" in
  x86_64)  deb_arch="amd64" ;;
  aarch64) deb_arch="arm64" ;;
  *)       die "unsupported architecture: $arch (expected x86_64 or aarch64)" ;;
esac

want() { [[ ",$formats," == *",$1,"* ]]; }
for f in ${formats//,/ }; do
  case "$f" in rpm|deb|tar) ;; *) die "unknown format: $f (expected rpm, deb or tar)" ;; esac
done

version="$(sed -n '/^\[package\]/,/^\[/ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' \
  "$project_root/Cargo.toml" | head -1)"
[[ -n "$version" ]] || die "could not read the package version from $project_root/Cargo.toml"

# ------------------------------------------------------------------- build --

if [[ -n "$prebuilt_binary" ]]; then
  [[ -f "$prebuilt_binary" ]] || die "no such binary: $prebuilt_binary"
  release_binary="$prebuilt_binary"
elif [[ "$arch" != "$host_arch" ]]; then
  die "cannot build $arch packages on a $host_arch host.
       Run this script on an $arch machine, in an $arch container, or on an
       $arch CI runner - or pass --binary with a $arch build made elsewhere.
       The README section \"ARM64 packages\" spells out all three."
else
  release_binary="$project_root/target/release/$binary_name"
  if (( ! skip_build )); then
    note "cargo build --release --locked ($arch)"
    ( cd "$project_root" && cargo build --release --locked )
  fi
  [[ -f "$release_binary" ]] || die "release binary not found: $release_binary
       Drop --skip-build, or build it with: cargo build --release --locked"
fi

# The binary must actually be for the architecture the packages claim, or the
# package installs and then fails to exec.
if command -v file >/dev/null 2>&1; then
  binary_arch="$(file -b "$release_binary")"
  case "$arch:$binary_arch" in
    # `file` reports "x86-64" for one and "ARM aarch64" for the other.
    x86_64:*x86-64*|aarch64:*aarch64*) ;;
    *) die "binary is not $arch: $binary_arch" ;;
  esac
fi

# ------------------------------------------------------------------- stage --

stage="$dist_dir/stage-$arch"
rm -rf "$stage"
mkdir -p "$dist_dir"

install -Dm755 "$release_binary" "$stage/usr/bin/$binary_name"
install -Dm644 "$packaging_dir/$appid.desktop" "$stage/usr/share/applications/$appid.desktop"
install -Dm644 "$packaging_dir/$appid.metainfo.xml" "$stage/usr/share/metainfo/$appid.metainfo.xml"
# The FIDO udev rule. Without it a security key enumerates and every attempt to
# OPEN it fails with EACCES, which the app reports as "your security key can't
# be opened" — accurate, but a package that ships the wallet should ship the
# rule that lets it reach the hardware it needs.
install -Dm644 "$packaging_dir/70-vela-fido.rules" \
  "$stage/usr/lib/udev/rules.d/70-vela-fido.rules"
for size in "${icon_sizes[@]}"; do
  install -Dm644 "$packaging_dir/icons/${size}x${size}/$appid.png" \
    "$stage/usr/share/icons/hicolor/${size}x${size}/apps/$appid.png"
done
install -Dm644 "$packaging_dir/icons/scalable/$appid.svg" \
  "$stage/usr/share/icons/hicolor/scalable/apps/$appid.svg"

# Symbols in a shipped binary are dead weight - and rpm's own stripping is
# disabled in the spec precisely so this runs with a stripper that understands
# the target. llvm-strip handles every architecture it was built with.
strip_tool=""
if [[ "$arch" == "$host_arch" ]] && command -v strip >/dev/null 2>&1; then
  strip_tool="strip"
elif command -v "$arch-linux-gnu-strip" >/dev/null 2>&1; then
  strip_tool="$arch-linux-gnu-strip"
elif command -v llvm-strip >/dev/null 2>&1; then
  strip_tool="llvm-strip"
fi
if [[ -n "$strip_tool" ]]; then
  "$strip_tool" "$stage/usr/bin/$binary_name"
else
  echo "warning: no usable strip for $arch; packaging an unstripped binary" >&2
fi

# ---------------------------------------------------------------- validate --

if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "$stage/usr/share/applications/$appid.desktop" \
    || die "the .desktop file is invalid"
fi
if command -v appstreamcli >/dev/null 2>&1; then
  # --no-net: screenshot URLs are checked at release time, not on every build.
  appstreamcli validate --no-net "$stage/usr/share/metainfo/$appid.metainfo.xml" \
    || die "the AppStream metainfo is invalid"
fi

installed_size_kb="$(du -ks "$stage" | cut -f1)"
note "staged $version for $arch (${installed_size_kb} KiB)"

built=()

# --------------------------------------------------------------------- rpm --

build_rpm() {
  command -v rpmbuild >/dev/null 2>&1 || die "rpmbuild not found.
       Fedora: sudo dnf install rpm-build
       Debian: sudo apt install rpm"

  local top="$dist_dir/rpmbuild"
  rm -rf "$top"
  mkdir -p "$top/SOURCES"

  tar -C "$stage" --owner=root --group=root --numeric-owner \
    -czf "$top/SOURCES/$binary_name-$version-stage.tar.gz" usr

  note "rpmbuild ($arch)"
  rpmbuild -bb "$packaging_dir/$binary_name.spec" \
    --define "_topdir $top" \
    --define "vela_version $version" \
    --target "$arch" \
    --quiet

  local produced
  produced="$(find "$top/RPMS" -name "*.rpm" -type f | head -1)"
  [[ -n "$produced" ]] || die "rpmbuild produced no package"
  mv "$produced" "$dist_dir/"
  rm -rf "$top"
  built+=("$dist_dir/$(basename "$produced")")
}

# --------------------------------------------------------------------- deb --

# Debian names shared libraries by package, not by soname, so the Depends field
# has to be mapped by hand. Anything the binary needs and this table does not
# know about is a hard error: a silently missing dependency turns into a user
# whose freshly installed app will not start.
deb_package_for_soname() {
  case "$1" in
    libc.so.6|libm.so.6|libdl.so.2|libpthread.so.0|librt.so.1) echo "libc6" ;;
    ld-linux-x86-64.so.2|ld-linux-aarch64.so.1)  echo "libc6" ;;
    libgcc_s.so.1)                      echo "libgcc-s1" ;;
    libstdc++.so.6)                     echo "libstdc++6" ;;
    libwayland-client.so.0)             echo "libwayland-client0" ;;
    libwayland-cursor.so.0)             echo "libwayland-cursor0" ;;
    libwayland-egl.so.1)                echo "libwayland-egl1" ;;
    libxkbcommon.so.0)                  echo "libxkbcommon0" ;;
    libxkbcommon-x11.so.0)              echo "libxkbcommon-x11-0" ;;
    libxcb.so.1)                        echo "libxcb1" ;;
    libxcb-cursor.so.0)                 echo "libxcb-cursor0" ;;
    libxcb-randr.so.0)                  echo "libxcb-randr0" ;;
    libxcb-render.so.0)                 echo "libxcb-render0" ;;
    libxcb-render-util.so.0)            echo "libxcb-render-util0" ;;
    libxcb-shape.so.0)                  echo "libxcb-shape0" ;;
    libxcb-shm.so.0)                    echo "libxcb-shm0" ;;
    libxcb-xfixes.so.0)                 echo "libxcb-xfixes0" ;;
    libxcb-xinput.so.0)                 echo "libxcb-xinput0" ;;
    libxcb-xkb.so.1)                    echo "libxcb-xkb1" ;;
    libX11.so.6)                        echo "libx11-6" ;;
    libX11-xcb.so.1)                    echo "libx11-xcb1" ;;
    libXcursor.so.1)                    echo "libxcursor1" ;;
    libfontconfig.so.1)                 echo "libfontconfig1" ;;
    libfreetype.so.6)                   echo "libfreetype6" ;;
    libexpat.so.1)                      echo "libexpat1" ;;
    libz.so.1)                          echo "zlib1g" ;;
    libzstd.so.1)                       echo "libzstd1" ;;
    libbz2.so.1.0)                      echo "libbz2-1.0" ;;
    libpng16.so.16)                     echo "libpng16-16t64 | libpng16-16" ;;
    libbrotlidec.so.1)                  echo "libbrotli1" ;;
    libasound.so.2)                     echo "libasound2t64 | libasound2" ;;
    libssl.so.3|libcrypto.so.3)         echo "libssl3t64 | libssl3" ;;
    libvulkan.so.1)                     echo "libvulkan1" ;;
    libgbm.so.1)                        echo "libgbm1" ;;
    libEGL.so.1)                        echo "libegl1" ;;
    libGL.so.1)                         echo "libgl1" ;;
    libdrm.so.2)                        echo "libdrm2" ;;
    libudev.so.1)                       echo "libudev1" ;;
    libsystemd.so.0)                    echo "libsystemd0" ;;
    # btleplug's BlueZ backend talks to bluetoothd over D-Bus; the .rpm gets
    # this from rpm's own dependency generator, the .deb has to be told.
    libdbus-1.so.3)                     echo "libdbus-1-3" ;;
    *)                                  return 1 ;;
  esac
}

# Libraries the binary opens with dlopen() at runtime. They are absent from the
# ELF's NEEDED list, so nothing derives them automatically — and every one of
# them is fatal at startup when missing. Keep this in step with the Requires
# block in packaging/vela-wallet.spec; re-derive both after a gpui bump with:
#
#     strings -a target/release/vela-wallet |
#       grep -oE 'lib[A-Za-z0-9_-]+\.so(\.[0-9]+)*' | sort -u
#
dlopen_sonames=(
  libwayland-client.so.0
  libwayland-egl.so.1
  libEGL.so.1
  libvulkan.so.1
)

deb_depends() {
  local reader sonames=() mapped=() unknown=() soname pkg
  if command -v readelf >/dev/null 2>&1; then
    reader=(readelf -d "$stage/usr/bin/$binary_name")
  elif command -v objdump >/dev/null 2>&1; then
    reader=(objdump -p "$stage/usr/bin/$binary_name")
  else
    die "readelf or objdump is required to compute the .deb dependencies"
  fi

  mapfile -t sonames < <(
    "${reader[@]}" | sed -n 's/.*NEEDED.*\[\(.*\)\].*/\1/p; s/^ *NEEDED *\(.*\)$/\1/p' \
      | tr -d ' ' | sort -u | grep .
  )
  (( ${#sonames[@]} )) || die "the binary records no NEEDED libraries; is it really an ELF?"
  sonames+=("${dlopen_sonames[@]}")

  # A dlopen'd name that has vanished from the binary means gpui changed how it
  # loads that library, and the dependency is now either wrong or misspelled.
  for soname in "${dlopen_sonames[@]}"; do
    grep -qF "$soname" <(strings -a "$stage/usr/bin/$binary_name" 2>/dev/null) \
      || echo "warning: $soname is declared as a runtime dependency but does not appear in the binary" >&2
  done

  for soname in "${sonames[@]}"; do
    if pkg="$(deb_package_for_soname "$soname")"; then
      mapped+=("$pkg")
    else
      unknown+=("$soname")
    fi
  done

  if (( ${#unknown[@]} )); then
    die "no Debian package is mapped for: ${unknown[*]}
       A dependency appeared that deb_package_for_soname() in this script does
       not know. Find the owning package (\`apt-file search <soname>\`) and add
       a case for it, then rebuild."
  fi

  # `paste -d', '` would cycle the comma and the space as two alternating
  # delimiters, which is not what a Depends field looks like.
  printf '%s\n' "${mapped[@]}" | sort -u |
    awk 'NR > 1 { printf ", " } { printf "%s", $0 } END { print "" }'
}

build_deb() {
  local work="$dist_dir/debbuild"
  local out="$dist_dir/${binary_name}_${version}_${deb_arch}.deb"
  local depends
  depends="$(deb_depends)"

  rm -rf "$work"
  mkdir -p "$work/control"

  sed -e "s|@VERSION@|$version|" \
      -e "s|@DEB_ARCH@|$deb_arch|" \
      -e "s|@INSTALLED_SIZE@|$installed_size_kb|" \
      -e "s|@DEPENDS@|$depends|" \
      "$packaging_dir/deb/control.in" > "$work/control/control"

  # dpkg wants paths relative to the package root, and its own verification
  # reads this file back on `dpkg -V`.
  ( cd "$stage" && find . -type f -printf '%P\0' | sort -z \
      | xargs -0 md5sum ) > "$work/control/md5sums"

  # No maintainer scripts: desktop-file-utils and hicolor-icon-theme both ship
  # dpkg triggers for /usr/share/applications and /usr/share/icons/hicolor, so
  # the menu database and icon cache refresh themselves.

  if command -v dpkg-deb >/dev/null 2>&1; then
    note "dpkg-deb ($deb_arch)"
    local root="$work/root"
    mkdir -p "$root/DEBIAN"
    cp -a "$stage/usr" "$root/"
    cp "$work/control/control" "$work/control/md5sums" "$root/DEBIAN/"
    dpkg-deb --build --root-owner-group "$root" "$out" >/dev/null
  else
    # Fedora has no dpkg. A .deb is an ar archive of exactly three members in
    # exactly this order, so assemble it directly.
    note "assembling .deb with ar ($deb_arch)"
    command -v ar >/dev/null 2>&1 || die "neither dpkg-deb nor ar is available"
    printf '2.0\n' > "$work/debian-binary"
    tar -C "$work/control" --owner=root --group=root --numeric-owner \
      --sort=name -czf "$work/control.tar.gz" ./control ./md5sums
    tar -C "$stage" --owner=root --group=root --numeric-owner \
      --sort=name -czf "$work/data.tar.gz" ./usr
    # -D keeps timestamps and uids out, so a rebuild of the same input is
    # byte-identical.
    ( cd "$work" && ar rcD "$out" debian-binary control.tar.gz data.tar.gz )
  fi

  [[ -f "$out" ]] || die "the .deb was not produced"
  rm -rf "$work"
  built+=("$out")
}

# --------------------------------------------------------------------- tar --

build_tar() {
  local out="$dist_dir/$binary_name-$version-linux-$arch.tar.gz"
  note "tarball ($arch)"
  tar -C "$stage" --owner=root --group=root --numeric-owner --sort=name \
    -czf "$out" usr
  built+=("$out")
}

want rpm && build_rpm
want deb && build_deb
want tar && build_tar

rm -rf "$stage"

# ---------------------------------------------------------------- checksums --

( cd "$dist_dir" && sha256sum "${built[@]#"$dist_dir/"}" > SHA256SUMS )

echo
note "packages in ${dist_dir#"$project_root"/}"
( cd "$dist_dir" && cat SHA256SUMS )
