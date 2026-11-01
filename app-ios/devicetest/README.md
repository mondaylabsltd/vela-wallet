# A device pass on somebody's iPhone

An iPhone on the cable may hold the owner's Vela (`app.getvela.VelaWallet`)
with real wallets in it. Installing a Debug build over it replaces its code
and keeps its data — never do that. Build the **device-test** app instead,
which installs beside it:

```sh
cd app-ios/VelaWallet
xcodebuild build-for-testing -project VelaWallet.xcodeproj -scheme VelaWallet \
  -configuration Debug -destination id=<udid> -derivedDataPath /tmp/vela-dt \
  VELA_BUNDLE_ID_SUFFIX=.devicetest \
  CODE_SIGN_ENTITLEMENTS="$PWD/../devicetest/VelaWallet.devicetest.entitlements"
xcrun devicectl device install app --device <id> \
  /tmp/vela-dt/Build/Products/Debug-iphoneos/VelaWallet.app
```

| What | Store / Debug | Device test |
|---|---|---|
| app | `app.getvela.VelaWallet` | `app.getvela.VelaWallet.devicetest` |
| widgets, Safari extension | `….Widgets`, `….SafariExtension` | `….devicetest.Widgets`, `….devicetest.SafariExtension` |
| UI test runner | `app.getvela.VelaWalletUITests.xctrunner` | `app.getvela.VelaWalletUITests.devicetest.xctrunner` |
| entitlements | associated domains, app group | none (`VelaWallet.devicetest.entitlements`) |
| signing | explicit App ID profile | the team's wildcard development profile; nothing registered |

What the device-test build gives up, and why that is right:

- **Real passkeys.** getvela.app's apple-app-site-association lists
  `F9W689P9NE.app.getvela.VelaWallet` under `webcredentials`; another id
  cannot be associated, so a platform passkey ceremony for getvela.app is
  refused. The parallel space (`VELA_PARALLEL_SPACE=1`, fixed throwaway keys)
  signs instead; `VELA_PARALLEL_ACCOUNT=n` makes the space fixture key n's own
  Safe, so two phones can be two members of a multisig. A real passkey on a
  phone needs the store id, which only a phone WITHOUT Vela may be given.
- **Universal links** open in Safari; `VELA_OPEN_LINK=<url>` hands one to the
  app at launch.
- **Widgets** read nothing (no app group — the store app's group would show
  this build's balance on the owner's widget).

**Local services.** The app sends plain `http` only to localhost
(`CoreHTTP`, the core's `is_localhost_http`), so a harness on the Mac's LAN
address is refused for RPC and the relay. Reach it over the cable instead, as
Android's `adb reverse` does: start the hand with
`TEST_RUNNER_VELA_DEVICE_HAND_REVERSE="9545,9587,9590"` and run
`python3 app-ios/devicetest/reverse.py <udid> 9545 9587 9590` on the Mac;
then `http://127.0.0.1:9545` on the phone (the app, and Safari) is the Mac's
`127.0.0.1:9545`. Launch inputs go through the hand's `launch` op
(`{"op":"launch","env":{"VELA_PARALLEL_SPACE":"1",…}}`).

The Debug-only doors work as on a simulator (`DevLaunchInputs`,
`VELA_DEV_MULTISIG_POLL`, `PerfSeed` — which seeds on a phone for this id
only). `VelaWalletUITests/DeviceHandUITests` is a hand for driving the app on
the phone from the Mac (it refuses any runner but the `.devicetest` one).

Afterwards: `xcrun devicectl device uninstall app --device <id>
app.getvela.VelaWallet.devicetest` (and the `.devicetest.xctrunner`).
