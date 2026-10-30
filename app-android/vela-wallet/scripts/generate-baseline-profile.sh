#!/usr/bin/env bash
#
# Native parity Phase 4: generate the baseline + startup profiles on ONE device
# and write them where :app ships them from
# (app/src/main/generated/baselineProfiles/{baseline,startup}-prof.txt).
#
#   ANDROID_SERIAL=emulator-5554 app-android/vela-wallet/scripts/generate-baseline-profile.sh
#
# The same thing `./gradlew :app:generateBaselineProfile` does, without
# Gradle's connected-device fan-out (a phone on the cable is somebody's
# wallet). Needs API 33+ (or a rooted device). Builds the two
# nonMinifiedRelease APKs first unless SKIP_BUILD=1.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
: "${ANDROID_SERIAL:?set ANDROID_SERIAL to the one device to run on}"
ADB="${ADB:-$(command -v adb || echo "$HOME/Library/Android/sdk/platform-tools/adb")}"
APP_APK="$HERE/app/build/outputs/apk/nonMinifiedRelease/app-nonMinifiedRelease.apk"
GEN_APK="$HERE/baselineprofile/build/outputs/apk/nonMinifiedRelease/baselineprofile-nonMinifiedRelease.apk"
GEN_PKG="app.getvela.wallet.baselineprofile"
OUT="$HERE/app/src/main/generated/baselineProfiles"

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  (cd "$HERE" && ./gradlew :app:assembleNonMinifiedRelease :baselineprofile:assembleNonMinifiedRelease -PvelaSkipRustBuild -q)
fi

a() { "$ADB" -s "$ANDROID_SERIAL" "$@"; }
a install -r -d "$APP_APK" >/dev/null
a install -r -d -t "$GEN_APK" >/dev/null
DEVICE_DIR="/sdcard/Android/media/$GEN_PKG"
a shell rm -rf "$DEVICE_DIR" >/dev/null 2>&1 || true

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
a shell am instrument -w -r -e androidx.benchmark.suppressErrors EMULATOR \
  "$GEN_PKG/androidx.test.runner.AndroidJUnitRunner" | tee "$WORK/instrumentation.log"
grep -qE "FAILURES!!!|INSTRUMENTATION_FAILED|Process crashed" "$WORK/instrumentation.log" && { echo "generate-baseline-profile: the generator failed" >&2; exit 1; }
a pull "$DEVICE_DIR/." "$WORK/" >/dev/null

# Every collected rule, de-duplicated and sorted so a regeneration diffs cleanly,
# less the benchmark build types' own doors (src/benchmark), which release has not.
# The startup profile is the launch journey's alone (includeInStartupProfile).
mkdir -p "$OUT"
cat "$WORK"/*-baseline-prof.txt | grep -v 'Lapp/getvela/wallet/bench/' | LC_ALL=C sort -u > "$OUT/baseline-prof.txt"
if ls "$WORK"/*-startup-prof.txt >/dev/null 2>&1; then
  cat "$WORK"/*-startup-prof.txt | grep -v 'Lapp/getvela/wallet/bench/' | LC_ALL=C sort -u > "$OUT/startup-prof.txt"
fi
wc -l "$OUT"/*.txt
