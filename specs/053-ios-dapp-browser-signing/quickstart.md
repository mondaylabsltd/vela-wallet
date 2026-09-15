# Quickstart — 053

```bash
# Build once per fresh checkout (Artifacts/ is gitignored). This cut changes
# NEITHER crate — if you are re-running these, something was added.
rust/scripts/build-ios-xcframework.sh
rust/scripts/build-ios-dev-fixtures.sh

cd app-ios/VelaWallet

# hermetic
xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet \
  -destination 'id=84146B7B-C679-46AC-8426-41AA42E6F403' test

# the device (run it ALONE — sharing CPU with cargo makes the waits flake)
xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet \
  -destination 'platform=iOS,id=00008030-001A75961445802E' \
  -only-testing:VelaWalletUITests/LiveWiringAcceptanceTests \
  -resultBundlePath /tmp/accept.xcresult test
xcrun xcresulttool export attachments --path /tmp/accept.xcresult \
  --output-path /tmp/accept-images

# the browser, by hand, in the parallel space
xcrun devicectl device process launch --device 00008030-001A75961445802E \
  -e '{"VELA_PARALLEL_SPACE":"1","VELA_PAGE":"explore-live"}' app.getvela.VelaWallet

# gallery states are unchanged
SIMCTL_CHILD_VELA_PAGE=explore SIMCTL_CHILD_VELA_STATE=e7 \
  xcrun simctl launch 84146B7B-C679-46AC-8426-41AA42E6F403 app.getvela.VelaWallet
SIMCTL_CHILD_VELA_PAGE=signing SIMCTL_CHILD_VELA_STATE=cs5 \
  xcrun simctl launch 84146B7B-C679-46AC-8426-41AA42E6F403 app.getvela.VelaWallet

# gates
node app-ios/scripts/audit-literals.mjs            # baseline 35, gate is "no new"
B=$(git merge-base 052-ios-money-wiring HEAD)      # the BRANCH POINT, not origin/main
git diff --stat $B -- rust/crates/vela-core/src/app/
git diff --stat $B -- rust/crates/vela-core/src/i18n_catalogs/
git diff --stat $B -- app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift
git diff --stat $B -- app-web/ app-desktop/ app-android/ app-browser-extension/
```

## Safari Web Inspector is this cut's DevTools

Debug builds set `isInspectable = true`. On the Mac: Safari ▸ 开发 ▸ <device> ▸
the page. It is the only way to see what the page's own JavaScript thinks, and
it is the equivalent of the `adb`-attached DevTools Android used.

## The test dApp

Served by the UI-test runner over loopback at `http://127.0.0.1:8137/`. Not
`file://`: `dappOriginOf` gives a file URL no origin, so every signature would
fail closed and the harness would prove nothing.
