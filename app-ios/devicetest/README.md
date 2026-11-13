# A device pass on somebody's iPhone

An iPhone on the cable may hold the owner's Vela (`app.getvela.VelaWallet`)
with real wallets in it. Installing a Debug build over it replaces its code
and keeps its data — never do that. Build the **device-test** app instead,
which installs beside it:

```sh
cd app-ios/VelaWallet
xcodebuild build-for-testing -project VelaWallet.xcodeproj -scheme VelaWallet \
  -configuration Debug -destination id=<udid> -derivedDataPath /tmp/vela-dt \
  VELA_BUNDLE_ID_SUFFIX=.devicetest INFOPLIST_KEY_CFBundleDisplayName="Vela Test" \
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
| name (home screen, Settings, Safari → Extensions) | Vela Wallet | Vela Test (the display-name override above) |

What the device-test build gives up, and why that is right:

- **Real passkeys.** getvela.app's apple-app-site-association lists
  `F9W689P9NE.app.getvela.VelaWallet` under `webcredentials`; another id
  cannot be associated, so a platform passkey ceremony for getvela.app is
  refused. The parallel space (`VELA_PARALLEL_SPACE=1`, fixed throwaway keys)
  signs instead; `VELA_PARALLEL_ACCOUNT=n` makes the space fixture key n's own
  Safe, so two phones can be two members of a multisig. A real passkey on a
  phone needs the store id, which only a phone WITHOUT Vela may be given.
  §35: the relying party is the core's rule (`relying_party_id`) — a Debug
  build on a phone uses the `TEST_RP_DOMAIN` build setting (add
  `TEST_RP_DOMAIN=<domain>` to the `xcodebuild` line; the domain must serve
  its own apple-app-site-association listing this id), never getvela.app.
  Without one, a passkey ceremony fails with the core's sentence saying how
  to set it; the parallel space signs as before.
- **Universal links** open in Safari; `VELA_OPEN_LINK=<url>` hands one to the
  app at launch.
- **Widgets** read nothing (no app group — the store app's group would show
  this build's balance on the owner's widget).

**Local services.** The app sends plain `http` (and `ws`) only to localhost
— one rule, `CoreHTTP.allowsTransport` over the core's `is_localhost_http` —
so a harness on the Mac's LAN address is refused for RPC, the relay, the
multisig service and its PeerConnect rooms alike. Reach it over the cable instead, as
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

**Tell the two apart.** Without the display-name override both apps — and
both Safari extensions — are called "Vela Wallet", and Settings → Apps →
Safari → Extensions lists them side by side. Settings names each row by its
bundle id (`app.getvela.VelaWallet.devicetest.SafariExtension`): drive it by
that, never by the label. Safari keeps an extension's switch and website
access after the app is uninstalled and restores them on the next install
(real-device pass 2 found an earlier pass's "Other websites: Allow" waiting), so turn
the extension off and its sites back to "Ask" before uninstalling.

Afterwards: `xcrun devicectl device uninstall app --device <id>
app.getvela.VelaWallet.devicetest` (and the `.devicetest.xctrunner`).
