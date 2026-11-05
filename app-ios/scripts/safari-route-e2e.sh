#!/bin/bash
# The Safari route's simulator e2e (SafariRouteDeviceTests), against the LOCAL
# fork harness (scripts/onchain/multisig-fork/up.sh), the LOCAL signing page
# (app-web/trusted-signer/samples/serve-dist.mjs) and the local passkey index
# (samples/mock-index.mjs). Nothing here reaches a production service or a
# real chain: every endpoint is loopback, and the multisig it creates is
# counterfactual (its genesis record goes to the harness's own service).
#
#   UDID=<a simulator YOU created> DD=<derived data> SHOTS=<dir> \
#   FORK_PORT=… UNREAD_PORT=… GENESIS_PORT=… RELAY_PORT=… PEERCONNECT_PORT=… \
#   PAGE_PORT=… INDEX_PORT=… DAPP_PORT=… [LOC=en] [NOBUILD=1] \
#     app-ios/scripts/safari-route-e2e.sh test01_enableExtension test02_createOnThePage \
#       test03_connect test07_createMultisigFromTheHeader test08_createMultisigFromSettings
#
# Before: `pnpm build:extension:safari` in app-web/vela-wallet (the Debug app
# bundles extension/dist-safari, the development package with dev-seed.html),
# and rust/scripts/build-ios-xcframework.sh. Face ID is enrolled here and a
# match posted every 2 s while the tests run.
set -u
: "${UDID:?a simulator you created}" "${DD:?a derived data path}" "${SHOTS:?where screenshots go}"
here=$(cd "$(dirname "$0")" && pwd)
LOGS=${LOGS:-$SHOTS/logs}
mkdir -p "$SHOTS" "$LOGS"
cd "$here/../VelaWallet" || exit 1

if [ -z "${NOBUILD:-}" ]; then
  echo "[$(date +%T)] build-for-testing"
  xcodebuild build-for-testing -project VelaWallet.xcodeproj -scheme VelaWallet -configuration Debug \
    -destination "id=$UDID" -derivedDataPath "$DD" >"$LOGS/build.log" 2>&1
  code=$?
  echo "[$(date +%T)] build exit $code"
  [ $code -eq 0 ] || { grep -E "error:" "$LOGS/build.log" | head -5; exit $code; }
fi

xcrun simctl spawn "$UDID" notifyutil -s com.apple.BiometricKit.enrollmentChanged 1
xcrun simctl spawn "$UDID" notifyutil -p com.apple.BiometricKit.enrollmentChanged
stop="$LOGS/.faceid.stop"; rm -f "$stop"
( while [ ! -f "$stop" ]; do
    xcrun simctl spawn "$UDID" notifyutil -p com.apple.BiometricKit_Sim.pearl.match >/dev/null 2>&1; sleep 2
  done ) &
loop=$!

failed=0
for t in "$@"; do
  echo "[$(date +%T)] $t"
  TEST_RUNNER_VELA_SAFARI_ROUTE_E2E=1 \
  TEST_RUNNER_VELA_HARNESS_FORK="$FORK_PORT" TEST_RUNNER_VELA_HARNESS_UNREAD="$UNREAD_PORT" \
  TEST_RUNNER_VELA_HARNESS_GENESIS="$GENESIS_PORT" TEST_RUNNER_VELA_HARNESS_RELAY="$RELAY_PORT" \
  TEST_RUNNER_VELA_HARNESS_PEER="$PEERCONNECT_PORT" \
  TEST_RUNNER_VELA_SIGNER_PAGE="http://localhost:$PAGE_PORT/" TEST_RUNNER_VELA_INDEX="http://127.0.0.1:$INDEX_PORT" \
  TEST_RUNNER_VELA_DAPP_PORT="$DAPP_PORT" TEST_RUNNER_VELA_SAFARI_LOCALE="${LOC:-en}" \
  TEST_RUNNER_VELA_SAFARI_SHOTS="$SHOTS" ${MS_NAME:+TEST_RUNNER_VELA_MS_NAME="$MS_NAME"} \
  xcodebuild test-without-building -project VelaWallet.xcodeproj -scheme VelaWallet \
    -destination "id=$UDID" -derivedDataPath "$DD" \
    -only-testing:"VelaWalletUITests/SafariRouteDeviceTests/$t" >"$LOGS/$t.log" 2>&1
  code=$?
  echo "[$(date +%T)] $t exit $code $(grep -E 'Test Case.*(passed|failed|skipped)' "$LOGS/$t.log" | tail -1 | cut -c1-140)"
  grep -E "error: .*XCT|failed -" "$LOGS/$t.log" | head -3 | cut -c1-300
  [ $code -eq 0 ] || { failed=1; [ -n "${KEEP_GOING:-}" ] || break; }
done

touch "$stop"; sleep 3; rm -f "$stop"
kill $loop 2>/dev/null
exit $failed
