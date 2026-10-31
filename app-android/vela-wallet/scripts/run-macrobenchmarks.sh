#!/usr/bin/env bash
#
# Native parity Phase 4: run the Macrobenchmark journeys on ONE device and
# pull their JSON results — without Gradle's connected tasks, which run on
# every attached device (a phone on the cable is somebody's wallet).
#
#   ANDROID_SERIAL=emulator-5554 app-android/vela-wallet/scripts/run-macrobenchmarks.sh [out-dir] [test-filter]
#
#   test-filter: an instrumentation `class` argument, e.g.
#     app.getvela.wallet.macrobenchmark.StartupBenchmark
#     app.getvela.wallet.macrobenchmark.JourneyBenchmarks#activityHistoryScroll
#   (default: every benchmark in the module).
#
# Builds nothing: run `./gradlew :app:assembleBenchmarkRelease
# :macrobenchmark:assembleBenchmarkRelease` first (CI: the same two tasks,
# then this script against its emulator; see macrobenchmark/README.md).
# Exit 1 when an instrumentation test fails.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
: "${ANDROID_SERIAL:?set ANDROID_SERIAL to the one device to run on}"
ADB="${ADB:-$(command -v adb || echo "$HOME/Library/Android/sdk/platform-tools/adb")}"
OUT="${1:-$HERE/build/macrobenchmark-results}"
FILTER="${2:-}"
APP_APK="$HERE/app/build/outputs/apk/benchmarkRelease/app-benchmarkRelease.apk"
TEST_APK="$HERE/macrobenchmark/build/outputs/apk/benchmarkRelease/macrobenchmark-benchmarkRelease.apk"
TEST_PKG="app.getvela.wallet.macrobenchmark"
for apk in "$APP_APK" "$TEST_APK"; do
  [ -f "$apk" ] || { echo "run-macrobenchmarks: missing $apk (assemble the benchmarkRelease variants first)" >&2; exit 2; }
done

a() { "$ADB" -s "$ANDROID_SERIAL" "$@"; }
a install -r -d "$APP_APK" >/dev/null
a install -r -d -t "$TEST_APK" >/dev/null
DEVICE_DIR="/sdcard/Android/media/$TEST_PKG"
a shell rm -rf "$DEVICE_DIR" >/dev/null 2>&1 || true

ARGS=(-w -r -e androidx.benchmark.suppressErrors EMULATOR)
[ -n "$FILTER" ] && ARGS+=(-e class "$FILTER")
mkdir -p "$OUT"
LOG="$OUT/instrumentation.log"
a shell am instrument "${ARGS[@]}" "$TEST_PKG/androidx.test.runner.AndroidJUnitRunner" | tee "$LOG"
a pull "$DEVICE_DIR/." "$OUT/" >/dev/null 2>&1 || true
echo "run-macrobenchmarks: results in $OUT"
if grep -qE "FAILURES!!!|INSTRUMENTATION_FAILED|Process crashed" "$LOG"; then exit 1; fi
