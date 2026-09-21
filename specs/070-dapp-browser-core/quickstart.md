# Quickstart — the device pass for 070

## Serve the test dApp (two origins)

```sh
cd app-android/vela-wallet/dev/testdapp
python3 -m http.server 8137 &      # the dApp
python3 -m http.server 8138 &      # the "attacker" frame's origin (port + 1)
```

Android: `adb reverse tcp:8137 tcp:8137 && adb reverse tcp:8138 tcp:8138`, then
`adb shell am start -n app.getvela.wallet/.MainActivity --ez vela.parallelSpace true --es vela.openUrl http://127.0.0.1:8137/`.

iOS: the phone reaches the Mac's LAN address (`http://<mac-ip>:8137/`), or the
UI-test `LocalDappServer`.

## A — Android checklist (parallel space, Gnosis dust)

| # | Do | Expect |
|---|---|---|
| A1 | Open the test dApp | `#out` shows one EIP-6963 announcement, `isVela: true`; the attacker frame line says `provider in frame: false` |
| A2 | Connect | one consent sheet naming `127.0.0.1:8137`; Approve → `eth_requestAccounts` = `[0x88cC…6894]`; chip turns green |
| A3 | Chain / Block | `0x64` (grant made on Gnosis? else `0x1`) / a block number |
| A4 | Add Base | `null`, `chainChanged` in events, Chain now `0x2105` |
| A5 | Add Polygon | error 4902 "Add chain 137 in Vela's network settings first" |
| A6 | eth_sign / Unknown method | 4200 |
| A7 | Coinbase / Watch asset | the address / `false` |
| A8 | Switch to Gnosis, Sign | signing sheet → slide → a signature; Verify → `valid: true` |
| A9 | Sign, then reload the page before sliding | the sheet closes; the page's promise: 4900 |
| A10 | Two tabs of the dApp; Connect in tab 2 while tab 1 is in front… | answers land in tab 2 only |
| A11 | `adb shell am start … --ez vela.crashRenderer true` | the app stays up; "This page stopped working" + Reload; Reload brings the page back |
| A12 | alert() | a dialog over the page |
| A13 | System Back on a page with history | goes back in the page; at its first page → start page |
| A14 | Address bar: tap the host, type `uniswap`, Go | a DuckDuckGo search |
| A15 | Site menu: Copy link / Share / Open in system browser / Disconnect | each does it; Disconnect → page hears `accountsChanged []` + `disconnect` |
| A16 | Settings → Storage → Connections | one row per connected site; Disconnect removes it and the open page hears it |
| A17 | Revoke (page button) | `null`; the site is gone from Settings |
| A18 | Kill and relaunch the app, reopen the dApp | still connected, still on the chain it picked |
| A19 | Explore scan: a QR of an address | Send opens with the recipient; a `wc:` code → the "not supported" line |

## B — iOS checklist

Same rows as A, with `webViewWebContentProcessDidTerminate` for A11 (debug
seam) and the iPhone's Back swipe for A13.

The tap rows run as `BrowserAcceptanceTests` (skipped by name in the shared
scheme; run them through an `.xctestrun` with the skip list removed). Stop any
server on 8137 first — the suite's `LocalDappServer` binds that port, and a
page cached from another server there fails every connect.

Without UI automation on the phone (Settings → Developer → UI Automation, a
switch only a person can flip), the non-tap rows still run over Web Inspector:

```sh
xcrun devicectl device process launch --device <id> --terminate-existing \
  --environment-variables '{"VELA_PARALLEL_SPACE":"1","VELA_URL":"http://<mac-ip>:8137/"}' \
  app.getvela.VelaWallet            # a debug launch with VELA_URL opens on Explore
ios_webkit_debug_proxy -c <udid>:9230 &   # pages at http://127.0.0.1:9231/json
```

then `Target.sendMessageToTarget` → `Runtime.evaluate` in the page: the
announcement, reads, and the 4200 / 4100 / 4902 / -32602 refusals of A1–A7.
