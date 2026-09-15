# Quickstart: build, run, and verify 052

Everything below runs from the worktree root
`/Volumes/data/production/vela-wallet-ios` unless a step says otherwise.

## 0. A fresh checkout

```bash
# Artifacts/ is gitignored: without this, xcodebuild reports
# "local binary target 'VelaCoreFFI' does not contain a binary artifact".
rust/scripts/build-ios-xcframework.sh            # ~5 min cold
# The parallel space's keyset (phase 2 onward). Debug only.
rust/scripts/build-ios-dev-fixtures.sh           # ~2 min
```

Re-run the first **only** when `rust/crates/vela-core-uniffi/` changes. This
cut does not change it, so after the first build it should never run again —
and if it does, something has been added that the spec says is not needed.

## 1. Hermetic tests

```bash
cd app-ios/VelaWallet
xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet \
  -destination 'id=84146B7B-C679-46AC-8426-41AA42E6F403' test
```

The iPhone 15 Pro simulator is on iOS 17.5, so `OS:latest` does **not** match
it — address it by UDID. The pass line is `✔ Test run with N tests…`;
`Executed 0 tests` above it is the XCTest reporter, not a failure.

## 2. The live suites (real relay, real chain)

```bash
xcodebuild … -only-testing:VelaWalletTests/RelayLiveTests \
  -only-testing:VelaWalletTests/SendLiveTests \
  OTHER_SWIFT_FLAGS='$(inherited) -DVELA_LIVE_TESTS'
```

These reach the real bundler. They read; they do not submit.

## 3. The parallel space

```bash
# Simulator
SIMCTL_CHILD_VELA_PARALLEL_SPACE=1 \
  xcrun simctl launch 84146B7B-C679-46AC-8426-41AA42E6F403 app.getvela.VelaWallet

# Device (one automation session at a time — a stray --console holds the
# device and the next run reports "Authentication canceled").
xcrun devicectl device process launch --device 00008130-001C68C804E1401C \
  -e '{"VELA_PARALLEL_SPACE":"1"}' app.getvela.VelaWallet
```

The badge is at the top of every screen. The receive screen shows
`0x88cCA0…6894`. `-e '{"VELA_PARALLEL_SPACE":"0"}'` leaves, and the person's
own wallet is back untouched.

Fund it before expecting a send to work: the golden Safe needs xDAI on Gnosis,
and the relay's treasury needs to be able to front the fee.

## 4. On the founder's iPhone

```bash
cd app-ios/VelaWallet
xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet \
  -destination 'platform=iOS,id=00008130-001C68C804E1401C' \
  -only-testing:VelaWalletUITests/LiveWiringAcceptanceTests \
  -resultBundlePath /tmp/accept.xcresult test
xcrun xcresulttool export attachments --path /tmp/accept.xcresult \
  --output-path /tmp/accept-images
```

Preconditions: the phone unlocked, Developer Mode on, this Mac trusted. Run it
**alone** — sharing the CPU with `cargo test` makes the ten-second waits flake.

## 5. Gates

```bash
node app-ios/scripts/audit-literals.mjs     # expect 35 — the gate is "no new ones"
git diff --stat $(git merge-base origin/main HEAD) -- rust/crates/vela-core/src/app/       # empty
git diff --stat $(git merge-base origin/main HEAD) -- rust/crates/vela-core/src/i18n_catalogs/  # empty
git diff --stat $(git merge-base origin/main HEAD) -- app-web/ app-desktop/ app-android/ app-browser-extension/  # empty
git diff --stat $(git merge-base origin/main HEAD) -- app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift # empty
```

The merge base, not `origin/main`: main has moved under this branch and a plain
diff would blame somebody else's work on this cut.

## 6. Proving the Release build carries nothing

```bash
cd app-ios/VelaWallet
xcodebuild archive -project VelaWallet.xcodeproj -scheme VelaWallet \
  -configuration Release -destination 'generic/platform=iOS' \
  -archivePath /tmp/vela-release.xcarchive CODE_SIGNING_ALLOWED=NO
BIN=/tmp/vela-release.xcarchive/Products/Applications/VelaWallet.app/VelaWallet
nm -gU "$BIN" | grep -c vela_dev_fixtures     # expect 0
strings "$BIN" | grep -c 'PARALLEL SPACE'      # expect 0
```

That the Release configuration **compiles at all** is the other half of the
proof: the binding file is excluded from it, so anything that referenced the
fixtures outside the seam would fail to build rather than ship quietly.
