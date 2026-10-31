#!/bin/bash
# Put the Safari package into the Safari Web Extension's bundle (native AUDIT §6.5).
#
# Xcode run-script phase of the VelaSafariExtension target. The package is
# BUILT by the web app — `pnpm build:extension:safari` in app-web/vela-wallet —
# exactly as the core's xcframework is built by rust/scripts before Xcode
# links it: this phase only copies, so an Xcode build never needs node.
#
#   Debug   → extension/dist-safari          (the development package: the
#                                             parallel space and the harness's
#                                             doorway, for the simulator e2e)
#   Release → extension/dist-safari-release  (no developer page; what ships)
#
# A Release build without the package, or with the development one, or with a
# version other than the app's, FAILS: the App Store must never receive an
# extension that is empty, carries a developer door, or says another version.
# A Debug build without it warns and goes on (the unit tests do not need it,
# and CI's test job builds no web app); Safari then lists Vela with nothing in
# it, which is what the warning says.
set -euo pipefail

web="$SRCROOT/../../app-web/vela-wallet/extension"
case "$CONFIGURATION" in
  Release*) package="$web/dist-safari-release" ;;
  *) package="$web/dist-safari" ;;
esac
dest="$TARGET_BUILD_DIR/$UNLOCALIZED_RESOURCES_FOLDER_PATH"

if [[ ! -f "$package/manifest.json" ]]; then
  message="no Safari package at $package — run \`pnpm build:extension:safari\` in app-web/vela-wallet"
  if [[ "$CONFIGURATION" == Release* ]]; then
    echo "error: $message" >&2
    exit 1
  fi
  echo "warning: $message; the extension is embedded EMPTY in this Debug build"
  exit 0
fi

if [[ "$CONFIGURATION" == Release* ]]; then
  for developer in dev-seed.html dev-probe.html dev-handoff.html dev-bridge.js; do
    [[ ! -e "$package/$developer" ]] || { echo "error: $package carries $developer — a development package" >&2; exit 1; }
  done
  if compgen -G "$package/*/parallel.html" >/dev/null; then
    echo "error: $package carries the parallel space — a development package" >&2
    exit 1
  fi
  version=$(/usr/bin/plutil -extract version raw -o - "$package/manifest.json")
  if [[ "$version" != "$MARKETING_VERSION" ]]; then
    echo "error: the Safari package is version $version, the app $MARKETING_VERSION — rebuild it" >&2
    exit 1
  fi
fi

mkdir -p "$dest"
# Contents, not the folder: Safari reads manifest.json at the bundle's root.
# No --delete: the bundle root also holds Info.plist and the executable.
/usr/bin/rsync -a --exclude '.DS_Store' "$package/" "$dest/"
echo "bundled $(basename "$package") ($(du -sh "$package" | cut -f1)) into $(basename "$TARGET_BUILD_DIR/$WRAPPER_NAME")"
