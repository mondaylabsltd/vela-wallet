#!/usr/bin/env bash
#
# Native parity Phase 4: measure the iOS app on its RELEASE build.
#
#   app-ios/scripts/perf/run-perf.sh <simulator-udid> [derived-data-dir] [result.xcresult]
#
# 1. Builds the Debug app and launches it once with
#      VELA_PARALLEL_SPACE=1  VELA_PERF_SEED=200  VELA_PERF_ADDRESS=<golden Safe>
#    so the simulator holds the parallel space's account, 200 history records
#    and a cached home (`Dev/PerfSeed.swift`; Debug only, simulator only).
# 2. Builds the `VelaWalletPerf` scheme (Release) and runs `PerfTests` on the
#    same simulator — installing the Release app over the Debug one keeps the
#    data, and a Release build reads no launch input of its own.
# 3. Prints every metric beside its target (`report-perf.mjs`).
#
# Non-blocking by design: a figure over its target is printed as OVER and the
# script still exits 0; only a build, or a test that could not run, fails it.
# Simulator numbers are indicative — the floor that counts is a real phone.
#
# Use a simulator of your own (`xcrun simctl clone "iPhone 17 Pro" Vela-Perf`):
# step 1 writes into that simulator's copy of the app's storage. Needs the
# core (rust/scripts/build-ios-xcframework.sh) and, for the Debug seed, the
# dev fixtures (rust/scripts/build-ios-dev-fixtures.sh). The Confirm → passkey
# test reads a real chain and skips without a network; it signs nothing (see
# PerfTests.swift).
#
# PERF_SKIP_SEED=1 skips step 1 (a simulator seeded within the last day).
#
# A live Confirm → passkey: point the seed at the multisig fork harness
# (scripts/onchain/multisig-fork/up.sh), which funds the golden Safe on its
# Gnosis fork, and the test no longer skips for want of a balance:
#
#   PERF_HARNESS_RPC=http://127.0.0.1:10045 PERF_HARNESS_RELAY=http://127.0.0.1:10090 \
#     app-ios/scripts/perf/run-perf.sh <udid>
#
# The Debug seed launch writes them as the person's own node for Gnosis and
# the relay (`DevLaunchInputs`: `VELA_RPC_OVERRIDES`, `VELA_SERVICE_ENDPOINTS`);
# they persist, so the Release build reads the fork too. Without them the seed
# REMOVES both, so a simulator once pointed at a harness is not measured
# against a dead one.
set -euo pipefail

[[ $# -ge 1 ]] || { echo "usage: $0 <simulator-udid> [derived-data-dir] [result.xcresult]" >&2; exit 2; }
udid="$1"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project="$here/../../VelaWallet/VelaWallet.xcodeproj"
dd="${2:-${TMPDIR:-/tmp}/vela-perf-dd}"
result="${3:-${TMPDIR:-/tmp}/vela-perf-$(date +%Y%m%d-%H%M%S).xcresult}"
bundle=app.getvela.VelaWallet
golden=0x88cCA0EeDbF2C4426110bbFc998F048689266894
common=(-project "$project" -destination "id=$udid" -derivedDataPath "$dd"
        CODE_SIGNING_ALLOWED=NO COMPILER_INDEX_STORE_ENABLE=NO)

xcrun simctl boot "$udid" 2>/dev/null || true
xcrun simctl bootstatus "$udid" -b >/dev/null

if [[ "${PERF_SKIP_SEED:-0}" != 1 ]]; then
  echo "run-perf: 1/3 seeding the wallet (Debug launch)"
  xcodebuild build -scheme VelaWallet -configuration Debug "${common[@]}" -quiet
  xcrun simctl install "$udid" "$dd/Build/Products/Debug-iphonesimulator/VelaWallet.app"
  if [[ -n "${PERF_HARNESS_RPC:-}" && -n "${PERF_HARNESS_RELAY:-}" ]]; then
    echo "run-perf: Gnosis through $PERF_HARNESS_RPC, the relay at $PERF_HARNESS_RELAY"
    rpc_overrides="{\"100\":{\"rpcURL\":\"$PERF_HARNESS_RPC\",\"onlyMine\":true}}"
    endpoints="{\"bundlerServiceURL\":\"$PERF_HARNESS_RELAY\"}"
  else
    rpc_overrides='{"100":null}'
    endpoints='{"bundlerServiceURL":null}'
  fi
  SIMCTL_CHILD_VELA_PARALLEL_SPACE=1 SIMCTL_CHILD_VELA_PERF_SEED=200 \
  SIMCTL_CHILD_VELA_PERF_ADDRESS="$golden" SIMCTL_CHILD_VELA_SKIP_LAUNCH_ANIMATION=1 \
  SIMCTL_CHILD_VELA_RPC_OVERRIDES="$rpc_overrides" SIMCTL_CHILD_VELA_SERVICE_ENDPOINTS="$endpoints" \
    xcrun simctl launch --terminate-running-process "$udid" "$bundle" >/dev/null
  # The space's door opens after the first frame; give it, and the store, time.
  sleep 15
  xcrun simctl terminate "$udid" "$bundle" || true
  sleep 2
  plist="$(xcrun simctl get_app_container "$udid" "$bundle" data)/Library/Preferences/$bundle.plist"
  if ! plutil -p "$plist" 2>/dev/null | grep -q "perf-199"; then
    echo "error: the seed did not land in $plist" >&2
    exit 1
  fi
fi

echo "run-perf: 2/3 PerfTests on the Release build"
xcodebuild build-for-testing -scheme VelaWalletPerf "${common[@]}" -quiet
rm -rf "$result"
set +e
xcodebuild test-without-building -scheme VelaWalletPerf "${common[@]}" -resultBundlePath "$result"
status=$?
set -e

echo "run-perf: 3/3 report ($result)"
node "$here/report-perf.mjs" "$result"
# A figure over its target is not a failure here; a test that could not run is.
exit $status
