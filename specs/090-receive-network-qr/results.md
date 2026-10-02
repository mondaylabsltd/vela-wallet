# 090 results: receive code with opt-in network (ERC-681)

Status 2026-10-02: implemented on `090-receive-network-qr`, in eight commits on top of `origin/main` `ec033f231`. Nothing is pushed.

## What changed

| Layer | Change |
|---|---|
| Core | `payment_request` adds `IncludeNetworkChanged { include }` plus the view fields `network_switch`, `include_network` and `network_hint`.<br>Address-mode `qr_value` is `ethereum:<address>@<chain>` while the switch is on; otherwise it is the bare address.<br>`copy_payload` stays the bare address.<br>`Start` turns the switch off. |
| Corpus | `receive.includeNetwork` and `receive.includeNetworkHint` are in all 15 locales. zh: 「附带网络信息」 / 「部分钱包无法识别。对方扫不出来时请关掉。」.<br>Path pin is now 1788.<br>SC-005 budget goes from 138,800 to 139,800 (owner-approved). |
| Web + extension | The route runs a `payment_request` session while a receive screen is up and sends the on-screen asset (`receiveAssetPicked`).<br>`liveReceiveQr` encodes `qr_value` on screen and in the share card.<br>New `Switch.svelte` primitive. |
| Desktop | New `switch_row`, drawn in `receive_qr`.<br>`keep_receive_asset` syncs `AssetPicked` from `receive_chain` / the DR3 token.<br>The resident is forgotten on each receive visit. The share card already used `qr_value`. |
| iOS | New `VelaSwitchRow` and `FlowsLive.receiveCode`. `shareCard(pay:)` now follows the machine's asset.<br>`enterReceive` starts the session once per visit. It used to start on every R1 step, which turned the switch off on the way back to the list. |
| Android | New `FlowSwitchRow`.<br>`ReceiveQrModel.code` is the core's value; the screen used to re-join the address lines.<br>Wire event and fields, plus `WalletController.includeNetwork`. `shareCard(code)`. |
| Docs | `specs/086-issue-sweep/results.md` now records two owner rulings:<br>#312: ERC-681 opt-in, default bare address (this spec).<br>#333: refusing non-UTF-8 files is kept. |

There is no new Receive surface beyond the four shells. The extension renders the web wallet UI. `ContactQr` (web, Android) encodes a contact's address, not the user's own, so it is out of scope.

## Round trip

The URI the core produces reads back as the same address and chain in every scanner:

- **Core:** `the_network_code_round_trips_through_the_scanner`. The send machine gets `request_chain_id == 100` and the same recipient.
- **Desktop:** `eip681::parse`.
- **Web:** `parseEIP681`.
- **iOS:** `Eip681.parse`.
- **Android:** `Eip681.parse`.

The web e2e also decodes the rendered code off the screen at both widths.

## Suites

| Suite | Result |
|---|---|
| core `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,319 passed / 0 failed (`app_payment_request` 27, of which 8 are new; `qr_scan` 4) |
| core clippy `-D warnings`, `cargo fmt --check` | clean |
| core `build-web --check`, `gen-onboarding-types --check`, `gen-core-types --check` | current |
| web `npx vitest run` | 171 files, 2,398 passed, 5 skipped |
| web `pnpm check` | 0 errors, 0 warnings |
| web e2e `receive-code.e2e.ts` (chromium) | 4/4. Two tests are new; the two existing ones had a stale selector on `main` and are fixed here |
| desktop `cargo test` / clippy / fmt | 883 passed, 49 ignored / no new warnings / clean |
| Android `:app:testDebugUnitTest` | 916 passed, 0 failed (`ReceiveNetworkSwitchTest` 3 new; `QrDecoderTest` 3 new) |
| iOS focused (6 suites) | 67 passed |
| iOS UI `ReceiveNetworkSwitchUITests` (parallel space) | passed |
| iOS full `VelaWalletTests` (cloned iPhone 16 Pro sim) | 1,105 tests in 143 suites passed |
| `check-native-reachability`, `check-event-payloads`, `check-dead-controls` | pass (0 mismatches, 532 sites) |

## Vela reads its own code from a picture (lead's device finding, 2026-10-02)

On a Xiaomi, picking a screenshot of a Vela receive code from the album gave 「在所选图片中未找到二维码。」. Each module is about 24 px across in a 1080×2400 screenshot.

Fixtures: the lead's Android crop (945×948) of a 1080×2400 screenshot, network on (`@137`) and bare. Both are re-encoded losslessly: 17 KB and 22 KB, against 97 KB for the original. Shrinking them makes them decode, so they cannot be downscaled and still reproduce the bug.

| Shell | Decoder | As is (crops and full 1080×2400 / 750×1624 screenshots) | Change |
|---|---|---|---|
| Android | ZXing 3.5.3, HybridBinarizer | **Fails** on all the Android images. The iPhone screenshots read. | **Ladder added**: `QrDecoder.decodeStill` climbs the core's `stillQrSizes` with a pure-Kotlin area-average downscale. Bare crop reads at 400; network-on crop at 640. |
| Web + extension | `decodeImage`: zbar at 1200/1000/800/600/400 wide, then jsQR | Raw jsQR fails, but `decodeImage` reads crops and full screenshots: its existing photo ladder shrinks before zbar. | None. Fixture test added; it asserts raw jsQR fails and `decodeImage` reads the core's value. |
| Desktop | rqrr | Reads all of them as they are; rqrr's threshold window scales with the image. | None. Fixture test through `decode_first`. |
| iOS | CoreImage `CIDetector` | Reads all of them as they are. | None. Fixture test through `QrDecoder.decode(image:)`. |

- **Core:** `vela_core::qr_scan::still_qr_sizes` (`STILL_QR_LADDER` = 1024 / 640 / 400) is exported over UniFFI as `still_qr_sizes` → `[QrScanSize]`.
- **Not exported over wasm:** the web would not use it, because its measured zbar ladder already covers this case. An unused export would be dead code.
- **Camera frames:** unchanged everywhere. No test showed the failure there.

## i18n bytes

Two new keys in each of the 15 locales:

- **JSON bytes:** en +133, ja +205, so ja + en is +338.
- **Runtime-JSON residency (ja + en):** now 139,006 against the new budget of 139,800. Before this change it was about 138,668, so the old 138,800 line would have failed.
- **Per-locale halves:** 134,980.

## Screenshots

These are in the session scratchpad `090-shots/`.

- **iOS simulator** (iPhone 16 Pro clone, zh, parallel space, live): `ios-zh-switch-off.png`, `ios-zh-switch-on.png`.
- **Web phone** (390 px, e2e): `web-390-switch-off.png`, `web-390-switch-on.png`.
- **Web desktop** (1440 px, e2e): `web-1440-switch-off.png`, `web-1440-switch-on.png`.

## Not done

- **Desktop gpui screenshot.** The machine was in use (HID idle under 90 s), and the GUI-harness rule forbids launching windows then. The desktop panel is covered by unit tests. The mock can be viewed with `VELA_PAGE=wallet VELA_FLOW=DR2` (switch off).
- **Android screenshot.** There is no Robolectric or Compose screenshot infrastructure. An emulator run needs adb, which is off-limits in this task, and `connectedAndroidTest` would target any attached phone.

## Decisions to confirm with the owner

1. **The switch is session-scoped.** Every receive visit starts with it off, and the choice is not saved. Within a visit it stays as the person left it when they pick another network.
2. **A token's code names only its network** (`@8453`). It does not use the ERC-20 `transfer` form, which even fewer wallets read.
3. **The switch is monochrome.** "On" uses the ink track and no accent, because the web's rule 4 reserves the accent for the money-moving action.
