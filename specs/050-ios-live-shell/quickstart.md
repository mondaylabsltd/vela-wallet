# Quickstart — 050 iOS Live Shell

## 0. A fresh checkout cannot build the app yet

`app-ios/VelaCoreKit/Artifacts/` is gitignored (`app-ios/.gitignore:30`), so the
`.xcframework` the SPM package points at does not exist until you build it:

```bash
rust/scripts/build-ios-xcframework.sh          # ~5 min cold, macOS only
```

It builds the host dylib, regenerates the Swift bindings, cross-compiles the device
and simulator slices, assembles the xcframework, and refreshes the **committed**
`app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift`.

Run it again after **any** change to `rust/crates/vela-core-uniffi/` — including
adding a `bridge_object!` line. `git diff` on the regenerated Swift file is how you
confirm the export landed; never edit that file by hand.

Requires the `aarch64-apple-ios` and `aarch64-apple-ios-sim` rustup targets for the
toolchain `rust/rust-toolchain.toml` pins.

## 1. Build and test on the simulator

The iPhone 15 Pro simulator is on iOS 17.5, so `OS:latest` does **not** match it —
address it by id or pin the OS:

```bash
cd app-ios/VelaWallet
xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet \
  -destination 'id=84146B7B-C679-46AC-8426-41AA42E6F403' build      # iPhone 15 Pro, 17.5
xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet \
  -destination 'id=84146B7B-C679-46AC-8426-41AA42E6F403' test
```

`xcodebuild -list -project VelaWallet.xcodeproj` prints the current destinations if
that id has gone stale.

## 2. Look at a state without signing in

The env pins render any drawn state directly, and are the fastest way to compare a
live builder against its fixture sibling:

```bash
VELA_PAGE=contacts          # contacts home
VELA_PAGE=contacts-gallery  # every contacts state, side by side
VELA_PAGE=settings          # settings home
VELA_PAGE=settings-gallery  VELA_SETTINGS_STATE=st10c   # the 不兼容 wizard
VELA_PAGE=wallet | gallery | flows-gallery | explore | signing
VELA_STATE=<id>             # which state the overridden page opens on
VELA_THEME=light|dark   VELA_LANG=<locale>   VELA_GALLERY=1
```

Set them in the scheme's Run → Arguments → Environment Variables, or pass them
through `launchEnvironment` from a UI test.

**These are never production navigation.** A live builder must render correctly
without any of them.

## 3. Verify on the device

Founder decision: every P1 scenario is confirmed on hardware. The simulator run is
preparation, never proof (SC-008).

```bash
xcrun devicectl list devices                     # shelchin's iPhone → available (paired)
xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet \
  -destination 'id=00008130-001C68C804E1401C' build
xcrun devicectl device install app --device 00008130-001C68C804E1401C <path/to/VelaWallet.app>
xcrun devicectl device process launch --device 00008130-001C68C804E1401C app.getvela.VelaWallet
```

Signing needs no setup: `DEVELOPMENT_TEAM = F9W689P9NE`, automatic signing, and the
`webcredentials:getvela.app` entitlement whose AASA half is live in production.

### The three things to look at

1. **通讯录 opens.** Tap the tab. It must show *your* book — or the drawn empty state
   if you have no contacts — never fixture contacts. Delete one through the row swipe
   and its confirm, force-quit, relaunch: still gone.
2. **A network you add is still there.** 设置 → 网络 → 添加. Type a chain id and an
   RPC URL. 添加 must stay disabled until the core's probes come back compatible;
   point the RPC at a different chain and the refusal must appear and write nothing.
   Add it, force-quit, relaunch: still listed, every field intact.
3. **The currency sticks and does not lie.** 设置 → 显示货币. Pick one, relaunch. Any
   fiat figure must render the core's degraded presentation — a missing rate is never
   shown as 1:1.

### Cross-client check

Export a `vela.contacts` written by the web client and read it on iOS, and the
reverse. Same key, same JSON text, same field names — that is the contract
(data-model §2).

## 4. Gates at every phase boundary

```bash
xcodebuild … build                                   # green
xcodebuild … test                                    # count strictly increased
node app-ios/scripts/audit-literals.mjs              # no literals
git diff --stat -- rust/crates/vela-core/src/app/    # empty
git diff --stat -- rust/crates/vela-core/src/i18n_catalogs/   # empty
```

Plus the XCUITest screenshot sweep, which is what proves the fixtures still render
exactly as they did (FR-003 / SC-005).
