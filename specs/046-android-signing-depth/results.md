# Results — 046 Android Signing Depth

**Branch**: `046-android-signing-depth` (stacked on 045) · **Device**: Xiaomi
alioth `9d5f42fb`, parallel space, Gnosis, Safe `0x88cC…6894`.

## What landed, phase by phase

| Phase | Delivered | Device evidence |
| --- | --- | --- |
| 0 | CameraX 1.4.1 + ZXing 3.5.3 (no Google services), `CAMERA` permission; `Eip681` (the web's tokenizer, the desktop's two refusals, pinned by `Eip681Test`); `SimDeltas` (`executor/sim.rs` ported: payload, succeeded calls' logs, netted Transfer deltas, the native sentinel) | — |
| 1 simulation | `WalletController.judgeSimDeltas` (held tokens + registry pushed, then `SimDeltasComputed`, read on `sim.ready`); `SigningController.sim` (`Ready`/`Unavailable`) set on open for tx methods; `SigningLive.simBlocks` (Balances rows from the judgments; unverified received tokens say so; no change; unavailable warning); the app's `simulate` port (pool `eth_simulateV1` → deltas → judge) | SC-001: the test dApp's dust transfer showed 「余额变化 · xDAI −0.001」 before signing; after the slide the feed's new row read 「−0.001 XDAI」 |
| 2 messages | `SignExecutor` signs `eth_sign` over EIP-1474's envelope (`[address, data]`); `DappSignMachineTest`: the danger class, the gate, one signature. The in-app browser keeps refusing `eth_sign` (4200) as the extension and the desktop's router do — the sheet presents it when it arrives by another transport. SIWE verdict words now carry the domain and the origin (044 showed `{{domain}}` raw — device-found here) | SC-002: SIWE on its origin → 「登录」, 站点 127.0.0.1, 声明, 发起方, the OK verdict; another domain → the phishing warning; `eth_sign` from the page → refused by policy (no sheet), test-covered |
| 3 scanner | `QrDecoder` (one luminance path), `CameraScanner` (preview + analysis, first hit wins, torch), `LiveScanSurface` (permission through the activity's launcher, the photo tool through the picker, status line), `ScanSurface` preview slot; `SendController.openScanner/closeScanner/scanned` (`scanOf` → `SendScan.Request|Text`); `flowState` S1; the home's 扫码 opens the send with the scanner on top; `SendExecutor.AddNetwork` → `SettingsController.addNetworkByChainId` (Added on `last_added_chain_id`, else NotFound) | SC-003: `qr-pay-gnosis.png` picked → the Send locked on Gnosis, 0.001 XDAI to the founder. SC-004: the preview draws live frames inside the brackets (`p46-3-scanner.png`), closes on Back, no FATAL in logcat; the permission was already granted on this MIUI build, so the system prompt was not exercised. SC-005: `qr-pay-arbitrum.png` → Arbitrum added by the settings machine and the Send locked there (「发送 ETH · Arbitrum · 余额 0 · 总额超过你的余额」) |

## Gates

| Gate | Result |
| --- | --- |
| Unit suite (`testDebugUnitTest`) | 493 run, 0 failures (480 at the start of 046) |
| `CoreWireDriftTest` | green — no wire change in 046 |
| `check-native-reachability.mjs` | green (`scan` is reached from the send flow's Scan state) |
| `build-web.mjs --check` | current — no Rust change in 046 |
| Device `.so` arm64 | 19297248 bytes (unchanged); debug APK 110951826 bytes with CameraX + ZXing |

## Owed / not done here
- A QR held to the lens: the phone sits on a desk under adb; the decoder is
  the same object for frames and photos (`QrDecoderTest` reads a ZXing-written
  code through both paths) and the photo path resolved on the device.
- The camera permission prompt itself (granted before the first open).
- `eth_sign` in the in-app browser stays refused (parity with the extension
  and the desktop router); the sheet's danger surface is test-covered.
- The Balances block sits on the signing sheet only (037's placement); Send
  confirm keeps 043's facts.
- Flip camera: the tool is drawn, inert (back camera only).

## Findings worth keeping
- 044's SIWE verdicts printed the corpus placeholders (`{{domain}}`); the
  words need the domain and the origin filled in.
- `eth_simulateV1` on the Gnosis pool endpoint answers with `traceTransfers`
  logs; the native sentinel `0xeeee…` carried the −0.001.
- DocumentsUI needs a media scan for a pushed file and its search to find a
  PNG in Downloads.
