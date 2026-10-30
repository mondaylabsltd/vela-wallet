#!/usr/bin/env bash
#
# Type-check the Windows passkey path from any machine.
#
# The desktop app itself CANNOT be cross-checked: its dependency tree compiles C
# (ThorVG, resvg's helpers, hidapi's vendored backends), so `cargo check
# --target x86_64-pc-windows-gnu` dies in a build script long before it reaches
# any Rust. That is why `vela-passkey-win` is a separate crate with no C in it
# at all — including the mapping into the core's wire types, which would
# otherwise be the one part of the Windows path nobody could check.
#
# What this does NOT do is run anything: every line here is checked and none of
# its BEHAVIOUR is confirmed — no ceremony has been run against a real key.
#
# It also checks this crate STANDALONE, which is the point (it runs anywhere)
# and also its blind spot: it cannot see how the desktop app depends on it. The
# app once declared this crate under
# `[target.'cfg(target_os = "macos")'.dependencies]` and left the Windows path
# unlinked, with this gate green throughout.
set -euo pipefail
# Resolved BEFORE the cd below, while "$0" still points where it was invoked
# from — the timezone lift near the bottom needs the app's own source.
APP="$(cd "$(dirname "$0")/.." && pwd)"
cd "$(dirname "$0")/../../vela-passkey-win"

TARGET=x86_64-pc-windows-gnu
# The rust/ workspace pins its toolchain, and `rustup target add` without
# --toolchain adds the target to the DEFAULT one — which is why this fails with
# "can't find crate for core" if the pin is ever bumped without re-adding.
TOOLCHAIN=$(sed -n "s/^channel = \"\(.*\)\"/\\1/p" ../../rust/rust-toolchain.toml)

if ! rustup target list --toolchain "$TOOLCHAIN" --installed | grep -qx "$TARGET"; then
  echo "adding $TARGET to toolchain $TOOLCHAIN"
  rustup target add --toolchain "$TOOLCHAIN" "$TARGET"
fi

echo "checking the Windows passkey path ($TARGET)"
cargo clippy --target "$TARGET" --all-targets -- -D warnings

# The app's OWN Windows-only code, which nothing above can see.
#
# `local_utc_offset_seconds` and its arithmetic live in the app crate, and the
# app crate cannot be cross-checked at all — the reason this script exists in
# the first place. So lift those functions verbatim into a throwaway crate that
# has no C in it and check THEM. Text extraction is deliberately literal: if
# somebody renames the function, this fails loudly rather than quietly checking
# nothing.
#
# Found this way, before it could ship: windows-sys 0.59 exports only
# TIME_ZONE_ID_INVALID of the four zone ids.
MOD="$APP/src/executor/mod.rs"
LIFTED=$(mktemp -d)
trap 'rm -rf "$LIFTED"' EXIT
mkdir -p "$LIFTED/src"
cat > "$LIFTED/Cargo.toml" <<'TOML'
[package]
name = "vela-windows-lift"
version = "0.0.0"
edition = "2024"

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = ["Win32_System_Time"] }
TOML
awk '
  /^#\[cfg\(windows\)\]$/ { hold = 1 }
  /^\/\/\/ Local midnight for an instant/ { exit }
  hold { print }
' "$MOD" | sed 's/^#\[cfg(any(windows, test))\]$/#[cfg(windows)]/' \
  > "$LIFTED/src/lib.rs.body"
if ! grep -q "GetTimeZoneInformation" "$LIFTED/src/lib.rs.body"; then
  echo "windows: could not lift the timezone functions out of src/executor/mod.rs" >&2
  echo "         (renamed? re-cfg'd? fix this script rather than deleting it)" >&2
  exit 1
fi
{ echo '#![allow(dead_code)]'; cat "$LIFTED/src/lib.rs.body"; } > "$LIFTED/src/lib.rs"
( cd "$LIFTED" && cargo clippy --target "$TARGET" -- -D warnings )
echo "windows: the app's timezone call type-checks too"

# N-D3: the notification-area icon (`src/tray/windows.rs`, windows-sys only
# by design) and the Windows half of `ui::motion`'s Reduce Motion read, lifted
# the same way: the tray file whole (its `super::` stubbed with the three
# names it uses), the motion function by its text.
TRAY="$APP/src/tray/windows.rs"
MOTION="$APP/src/ui/motion.rs"
TRAY_LIFT=$(mktemp -d)
trap 'rm -rf "$LIFTED" "$TRAY_LIFT"' EXIT
mkdir -p "$TRAY_LIFT/src"
cat > "$TRAY_LIFT/Cargo.toml" <<'TOML'
[package]
name = "vela-windows-tray-lift"
version = "0.0.0"
edition = "2024"

[workspace]

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = ["Win32_Foundation", "Win32_Graphics_Gdi", "Win32_UI_WindowsAndMessaging", "Win32_UI_Shell", "Win32_System_LibraryLoader"] }
TOML
MOTION_FN=$(awk '
  /^#\[cfg\(windows\)\]$/ { held = $0; next }
  held != "" && /^fn system_reduces_motion/ { print held; on = 1 }
  { held = "" }
  on { print }
  on && /^}$/ { exit }
' "$MOTION")
if ! grep -q "SPI_GETCLIENTAREAANIMATION" <<<"$MOTION_FN"; then
  echo "windows: could not lift system_reduces_motion out of src/ui/motion.rs" >&2
  exit 1
fi
cat > "$TRAY_LIFT/src/lib.rs" <<RUST
#![cfg(windows)]
#![allow(dead_code)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayModel {
    pub figure: Option<String>,
    pub updated: Option<String>,
    pub send: String,
    pub receive: String,
    pub open: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    Open,
    Send,
    Receive,
}
pub(crate) fn press(_: TrayCommand) {}
#[path = "$TRAY"]
mod windows;
$MOTION_FN
RUST
( cd "$TRAY_LIFT" && cargo clippy --target "$TARGET" -- -D warnings )
echo "windows: the tray icon and the Reduce Motion read type-check too"

echo "windows: type-checked (not run — see the crate docs)"
