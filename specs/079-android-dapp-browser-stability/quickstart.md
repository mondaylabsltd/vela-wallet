# Quickstart — the device pass for 079

## Setup (all clients)

```sh
# test dApp, two origins (070's harness)
cd app-android/vela-wallet/dev/testdapp && python3 -m http.server 8137 & python3 -m http.server 8138 &
# fault proxy (this Mac reaches the internet through 127.0.0.1:1088)
CHAOS_UPSTREAM=127.0.0.1:1088 python3 scripts/device/chaos-proxy.py /tmp/chaos.log &
curl -s 'http://127.0.0.1:8899/__chaos?mode=pass'
```

Faults used below: `drop` (connection refused at once), `blackhole` (accepted, never answered),
`latency=6000` (slow first byte), each with `match=<host regex>`. Switching a fault on cuts matching
live tunnels.

**Android (Xiaomi `9d5f42fb`)**: `adb reverse tcp:8137 tcp:8137; adb reverse tcp:8899 tcp:8899;
adb shell settings put global http_proxy 127.0.0.1:8899`, then force-stop and start the app (OkHttp
pools). **Restore**: `adb shell settings put global http_proxy :0`.

**iPhone ("ABC")**: Settings ▸ Developer ▸ UI Automation ON (a person); the probe
`DappBrowserStabilityProbeTests` via a copied `.xctestrun` with its skip removed (placed next to
the build products). Faults: Wi-Fi ▸ Configure Proxy ▸ Manual `<mac-ip>:8899` with
`CHAOS_BIND=0.0.0.0`; turn it Off after.

**Desktop**: run the app; screenshots at desktop and phone width (memory: look before done).

## A — Page loads (Android, iOS, desktop)

| # | Do | Expect |
|---|---|---|
| L1 | `latency=6000 match=example`, type `example.com`, Go | progress within 0.5 s; old host stays until commit |
| L2 | `drop match=uniswap`, open `app.uniswap.org` | Vela panel: host + "网络不稳定，页面没能打开。"; never the engine page |
| L3 | leave L2's panel up | "正在重试…" at ~2 s, ~5 s, ~10 s, then stops; restore `pass` before the third → the page loads by itself |
| L4 | tap Retry with the fault still on | the panel stays with "正在重试…" for the whole attempt (1 s frames: 0 frames of an engine page) |
| L5 | `https://expired.badssl.com/` | certificate line, no automatic retry, no "continue anyway" |
| L6 | after L1–L5 read Recents from the store | only the pages that loaded, each with its own title and icon |

## B — Chain notice

| # | Do | Expect |
|---|---|---|
| C1 | test dApp on Gnosis, `blackhole match=gnosis\|xdai`, Block number | the one-line notice names Gnosis; the chip stays green |
| C2 | `pass`, Block number | the notice clears without a tap |

## C — Signing (Android + iOS devices; desktop and extension where present)

| # | Do | Expect |
|---|---|---|
| S1 | Sign → swipe the sheet down ×5, tap the scrim ×5, Back ×5 | sheet stays; page has no answer |
| S2 | ✕ | closes; page gets 4001 once |
| S3 | Sign → approve | "签名中…" → "已签名" tick → closes; page gets the signature |
| S4 | Send dust → approve | spinner → clock with ring → tick with short hash → closes; page gets the tx hash |
| S5 | Send dust → `drop match=vela-relay` right after the signature | cross + plain reason; page gets one error; no pending record |
| S6 | Send dust on a chain whose relay does not mine (Arbitrum, today) | after the window: the still-confirming sentence; ✕ closes without rejecting; the page is answered once |
| S7 | open Send dust with `drop match=vela-relay` | fee row: the service line; slide disabled; restore `pass` without touching → fee appears ≤ 15 s |
| S8 | tap the fee refresh | re-quote spinner → new fee |

## D — Chrome and pickers

| # | Do | Expect |
|---|---|---|
| U1 | https page and http test dApp: address bar, consent, connection panel, site menu | closed lock grey / open lock warning; no "安全站点"/"不安全"/"已加密" text anywhere (UI dump grep, zh + en) |
| U2 | Connect | title "连接到 127.0.0.1:8137"; Connect is the filled button |
| U3 | connection panel → network picker | logo + balance per network (from the home screen's figures), no zeros for unknown |
| U4 | account picker | identicon per account |
| U5 | signing header on the test dApp | the host once |
| U6 | two tabs → switcher | two different snapshots |
| U7 | Recents / favourites for app.uniswap.org | its icon; letter fallback "U" |

## E — Trusted signer (owner's account on the Xiaomi; iPhone with the trusted-signer route)

| # | Do | Expect |
|---|---|---|
| T1 | Sign from the test dApp | the app shows a "去签名页确认" button; exactly one slide, on the page |
| T2 | the page | origin line names the host as seen by Vela's browser; no "未知站点" warning; summary above the fold |
| T3 | cancel the passkey prompt | plain "你取消了…" line; slide works again without reload |
| T4 | after one visit, `drop match=sign\.getvela` → Sign | the page opens from the device and signs |
| T5 | first use with the page host dropped | the app's own sheet: "签名页没能打开…" + Retry; the request stays open |

## F — No regression

070 quickstart rows A1–A18 on the Xiaomi; Android JVM, iOS hermetic, desktop and core suites green.
