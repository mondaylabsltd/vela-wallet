# Quickstart — the device pass after the 082 fixes

This is the verification pass for 082: the Mac desktop app, the Chrome extension and the iPhone 11
("ABC"), plus an Android smoke for the rules the four clients share. Every finding (G#) that had
a device reproduction during the pass is re-run here as a regression row. Research ids are in
[research.md](research.md); the states the rows exercise are in [data-model.md](data-model.md).

This file **replaces** `device-pass-plan.md` §1–§2 wherever those change a system proxy (§1
step 4, the "Switch the Mac to chaos" line, DX3, the iPhone Wi-Fi proxy and Shadowrocket lines,
IX3, IX4, IX5). The other rows are carried here with their post-fix expectations.

👆 = the owner's real passkey (Touch ID, Face ID, or the passkey in Chrome). The rows in §2–§5
run in the **parallel space** (fixed keys, fixture Safes, dust amounts) and need no 👆. The
flows fixed keys cannot cover are gathered in one short owner batch at the end (§6).

## 0. Ground rules

**Faults touch only the app under test.** This is owner ruling 6 and this run's request:
不能影响设备上的其他流量. [RH1]

| Client | The only allowed switch | Never |
|---|---|---|
| Desktop | a **dev-fixtures** build launched with `VELA_DEV_PROXY=127.0.0.1:8899` (that build's page and wallet traffic, nothing else) | `networksetup`, `scutil` edits, network locations, the proxy client's settings |
| iPhone | a **Debug** build launched with `"VELA_DEV_PROXY":"192.168.50.9:8899"` in the devicectl JSON (per-app `ProxyConfiguration`) | Wi-Fi ▸ Configure Proxy, Shadowrocket toggles, airplane mode |
| Extension | **Chrome for Testing** in its own `--user-data-dir`, started with its own `--proxy-server` | the owner's Chrome profile or flags |
| Android | no fault rows (a device-wide proxy is the only switch that exists) [RH3] | `adb shell settings put global http_proxy` |

- **One client at a time.** Every client pointed at the proxy shares port 8899. Run
  `chaos mode=pass` between rows, and use `match=''` (every host) only while one client is
  pointed at it.
- **Before and after.** Record `scutil --proxy > $S/proxy-before.txt` before the first row, and
  diff it in §7. The owner confirms once that the iPhone's Wi-Fi proxy setting was never touched.
- **Money.** Only dust (0.001 xDAI on Gnosis). Fault rows use the parallel space's fixture Safe.
  Before re-sending anything after a fault, check the explorer or the Safe nonce first. The nonce
  is `EntryPoint.getNonce(address,uint192)`: calldata `0x35567e1a` + the Safe address left-padded
  to 32 bytes + a 32-byte zero key (64 hex zeros; a 24-byte key reverts):

  ```sh
  SAFE=88cca0eedbf2c4426110bbfc998f048689266894   # desktop fixture Safe (extension: d400866e00b055b20752a826cd5c89b811de130b), lower-case, no 0x
  curl -s https://rpc.gnosischain.com -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"to":"0x0000000071727De22E5E9d8BAf0edAc6f37da032","data":"0x35567e1a000000000000000000000000'$SAFE'0000000000000000000000000000000000000000000000000000000000000000"},"latest"]}'
  ```
- **Evidence.** Put screenshots and log excerpts in `evidence/<client>/post-<row>.jpg` / `.txt`.
  Every failure row names the log line it must produce (FR-018). §7 runs one secret scan over
  all logs (FR-019).

## 1. Setup

### 1.1 Gates and freshness (before any device)

The phase gates in [plan.md](plan.md) are green, and:

```sh
cd /Volumes/data/production/vela-wallet-082
bash rust/scripts/check-ios-core-fresh.sh                 # must print "ok …"
grep WASM_URL rust/pkg-web/vela_core_wasm_url.js; ls app-web/vela-wallet/extension/dist/*.wasm   # same hash
git log -1 --format=%h                                    # the commit every build below is made from
git status --porcelain rust/pkg-web app-web/vela-wallet   # must be empty: build from a committed tree (G72)
```

### 1.2 Shared: test dApp, fault proxy, logs

```sh
S=/private/tmp/claude-501/-Volumes-data-production-vela-wallet/d4a496f4-ab96-4481-916a-65326d48067e/scratchpad
mkdir -p $S/logs; T=$(date +%m%d-%H%M)
scutil --proxy > $S/proxy-before.txt
# test dApp on two origins (070 harness)
(cd app-android/vela-wallet/dev/testdapp && python3 -m http.server 8137 --bind 0.0.0.0 & python3 -m http.server 8138 --bind 0.0.0.0 &)
# fault proxy. Upstream = the proxy this Mac itself needs. It serves only the clients pointed at it.
# CHAOS_BIND=0.0.0.0 only while the iPhone rows run.
CHAOS_UPSTREAM=127.0.0.1:1088 CHAOS_BIND=0.0.0.0 python3 scripts/device/chaos-proxy.py $S/logs/chaos.log &
chaos(){ curl -s "http://127.0.0.1:8899/__chaos?$1"; echo; }
chaos mode=pass; tail -F $S/logs/chaos.log
```

| Shorthand | Means | What it reproduces |
|---|---|---|
| `drop <re>` | `chaos 'mode=drop&match=<re>'`: the CONNECT is closed before any byte | refused, provably not delivered |
| `blackhole <re>` | the CONNECT hangs | a site or node that never answers |
| `mute <re>` | the request goes through, the reply never comes back | **the lost relay reply (G21)** |
| `reset_mid <re>` | the tunnel is cut 1.5 s in | a proxy that drops tunnels |
| `latency N <re>` | N ms before the upstream connect | slow first byte |
| `stall <re>` (only if RH2 adds it) | CONNECT answered 200, upstream never opened | TUN-shaped hang (W24) |

Switching a fault on cuts every live tunnel it matches. Loopback is never proxied.

### 1.3 Desktop (dev-fixtures build, macOS 14+)

```sh
cd app-desktop/vela-wallet && cargo build --release --features dev-fixtures
env -u all_proxy -u http_proxy -u https_proxy -u ALL_PROXY -u HTTP_PROXY -u HTTPS_PROXY \
  VELA_DEV_PROXY=127.0.0.1:8899 VELA_STATE_DIR=$S/desktop-state VELA_PARALLEL_SPACE=1 \
  VELA_LANG=zh VELA_SECTION=explore ./target/release/vela-wallet 2> $S/logs/desktop-$T.err &
tail -F $S/logs/desktop-$T.err       # first line: "[vela-wallet] dev proxy: all traffic via 127.0.0.1:8899"
log stream --style compact --predicate 'process == "vela-wallet" AND subsystem == "com.apple.WebKit" AND category IN {"Loading","Process"}' | tee $S/logs/webkit-$T.log
```

- Always use a scratch `VELA_STATE_DIR`: a dev build must never rewrite the owner's
  `wallet.json`.
- Quit every other copy first (`pgrep -fl vela-wallet`).
- Screenshots: `python3 $S/gui.py shot <name>`. Send no input while the owner is at the Mac.

### 1.4 Chrome extension (Chrome for Testing 151)

```sh
cd app-web/vela-wallet && pnpm build && pnpm build:extension
CFT="$HOME/Library/Caches/ms-playwright/chromium-1234/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
"$CFT" --user-data-dir=$S/cft-082 --proxy-server=http://127.0.0.1:8899 \
  --load-extension=$PWD/extension/dist --disable-extensions-except=$PWD/extension/dist \
  --remote-debugging-port=9334 --no-first-run --no-default-browser-check &
```

- Enter the parallel space from `chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl/zh/parallel.html`.
- Reload every dApp tab after any extension reload.
- For lifecycle rows (EX3, EX4, EX5, EX8), **do not attach CDP or DevTools to the worker**:
  attaching keeps it alive and hides the bug. Read the worker's log afterwards from the panel's
  DevTools with `chrome.storage.session.get('vela.sw.log')` [RB14].
- Keep ⌥⌘I open on the dApp tab with Preserve log on: the test dApp prints `{ok, code, message}`
  for every call, and that is the ground truth.

### 1.5 iPhone (Debug build)

```sh
xcodebuild -project app-ios/VelaWallet/VelaWallet.xcodeproj -scheme VelaWallet -configuration Debug \
  -destination 'platform=iOS,id=00008030-001A75961445802E' -derivedDataPath $S/dd build
xcrun devicectl device install app --device F30282CB-FA41-589E-A3BD-31855EB64414 $S/dd/Build/Products/Debug-iphoneos/VelaWallet.app
xcrun devicectl device process launch --device F30282CB-FA41-589E-A3BD-31855EB64414 --terminate-existing \
  -e '{"VELA_PARALLEL_SPACE":"1","VELA_LANG":"zh","VELA_DEV_PROXY":"192.168.50.9:8899"}' app.getvela.VelaWallet
idevicesyslog -u 00008030-001A75961445802E -p VelaWallet | tee $S/logs/ios-$T.syslog
# afterwards
log collect --device-udid 00008030-001A75961445802E --last 30m --output $S/logs/ios-$T.logarchive
log show $S/logs/ios-$T.logarchive --predicate 'subsystem == "app.getvela.VelaWallet"' --info
```

- `-e` replaces every child variable, so put every knob in the JSON.
- The switch took when `chaos.log` shows the phone's CONNECTs as soon as the app starts.
- No `--console` session during any 👆 row: it cancels the Face ID sheet (050/052). The
  `os.Logger` lines [RE11] make it unnecessary.
- Page console: Safari ▸ Develop ▸ ABC.

### 1.6 Android (smoke only, no proxy)

Run `bash rust/scripts/build-dev-fixtures.sh`, then install the debug build on the Xiaomi
`9d5f42fb` and launch it in the parallel space.

## 2. Desktop

Unless a row says otherwise: the §1.3 build, the parallel space, `zh`, and
`http://127.0.0.1:8137` as the test dApp.

| id | do | expect | 👆 |
|---|---|---|---|
| DX0 | Launch; open `https://example.com` | `.err` starts with `dev proxy: all traffic via 127.0.0.1:8899`, then timestamped `browser: asked host=example.com gen=1 how=navigation`, `browser: committed …`, `browser: finished …` [RD12]. chaos.log shows `PASS example.com:443`. | |
| DX-G3 · **G3** | Address bar: ⌘V a URL, Backspace ×3, Delete, ⌘X, ⌘V, Enter | No abort; the page loads. | |
| DX13 · **G7** | Type `127.0.0.1:8137` + Enter; then focus the bar and press Esc | Right after Enter the bar shows lock + host: no magnifier, nothing selected [RD5]. Esc gives the same. | |
| DX14 · **G29 G30** | New tab; `drop uniswap` → `https://app.uniswap.org` | No abort. Within 3–8 s the panel shows the host, 网络不稳定，页面没能打开。 and 重试. The bar reads `app.uniswap.org` with **no lock** [RE1]; the tab title is the failed host. Clicking the bar edits `https://app.uniswap.org/`. Auto-retries at +2/+5/+10 s. `.err`: `browser: probe … verdict=refused`, `browser: failed … class=refused`. | |
| DX12 · **G6** | `gui.py shot` of the toolbar on the start page and on a live page | The strip's bottom edge and the address field sit at the same y (0 pt difference) [RD8]. | |
| DX11 · **G2** | Quit with a Uniswap tab selected; relaunch into Explore | The start page shows; the Uniswap tab waits **unlit**; Back, Forward and Reload are disabled. Type `127.0.0.1:8137` + Enter **once**: it loads in a new tab and the Uniswap tab is untouched. `.err` has `browser: navigate asked host=127.0.0.1:8137 view=…` (settles G2's first-Enter question). Clicking the Uniswap tab then loads it [RD6]. | |
| L1 | `latency 6000 example` → `example.com` | Hairline within 0.5 s; the old host stays until commit. | |
| DX1 · W7 | `latency 9000 app\.uniswap\.org` → Go; watch the WebKit log | **One** `loadRequest`, with no second one at ~10 s. A watchdog panel, if any, clears by itself at commit. If a retry fell due, `.err` has `browser: retry … skipped (engine still loading)` [RD3]. | |
| L2–L4 | `blackhole uniswap` → open it; leave the panel up; tap Retry mid-attempt | Panel within 3–8 s. 正在重试… (busy, full colour) at +2/+5/+10 s, then it stops. `pass` before the third attempt → the page loads by itself. No WebKit error page in any shot. | |
| L5 | `https://expired.badssl.com/` | The certificate sentence; no auto-retry and no "continue anyway". | |
| DX2 | `blackhole app\.uniswap\.org` 60 s, then `pass` | The page loads (tap 重试 if the schedule has ended). Recents gains nothing during the fault. | |
| DX4 · W18 | Test dApp loaded; `blackhole example` → dApp console `location.href='https://example.org/'`; afterwards Back to that entry | Hairline within 0.5 s while the bar keeps `127.0.0.1:8137`. The panel names `example.org`, with manual Retry only [RD7]. Back/Forward to a faulted entry behaves the same. | |
| DX5 | `blackhole 'googleapis\|gstatic\|googletagmanager'` → `app.uniswap.org` | Usable, no panel: progress passed the live threshold, so give-up never fires. | |
| DX3′ · W6 W8 | Relaunch with `VELA_DEV_PROXY=127.0.0.1:9` (a dead proxy) → open a dApp | Within ~3 s the panel says 代理没有响应。 (`explore.loadProxy`). Balances and fees do **not** load: no direct bypass. `.err` has `proxy: 127.0.0.1:9 unreachable …` [RD2, RD9]. System PAC and exception lists are unit-tested only, because testing them on a device means touching system settings. | |
| C1 · G33 | Test dApp on Gnosis; `blackhole 'gnosis\|xdai\|1rpc'` → Block number; tap the notice's Retry | Within the first pass (≈ live endpoints × 8 s, ~15–25 s), while the call is still pending, a one-line notice names Gnosis; the chip is unchanged [RF1, RJ11]. Retry is busy until its read settles [RF4]. chaos.log keeps showing `HOLE` for those hosts (no direct bypass). `.err`: `rpc: chain=100 … outcome=not connected` (a black-holed CONNECT never connects), then `chain notice: shown chain=100` **before** the call's `gave up` line. | |
| C2 | `pass` → Block number | The notice clears within 5 s of a good read, with no tap; `chain notice: cleared chain=100`. | |
| S7 / S8 | `drop vela-relay` → Send dust (do not approve) → `pass` without touching; then tap refresh | The fee row names the cause and the slide is shut. The fee is back within 15 s with no tap. `.err`: `fee: quote failed … re-quote #n`, then `fee: quote back`. Refresh → spinner → fee. | |
| U1 / U2 · **G11** | Test dApp → Connect | The title names the site once. An account row (identicon, name, short address) and a network row with its logo [RD11]. Change the network to Gnosis → Connect → the dApp's `eth_chainId` is `0x64`. No 安全站点 / 不安全 / 已加密 text anywhere. | |
| U5 · S1 · S2 | Sign → header; Esc ×5 and click the webview ×5; then ✕ | The host appears once; the column stays; ✕ → 4001 once. | |
| DX-G14 · **G14** | dApp console: `ethereum.request({method:'eth_sendTransaction',params:[{from:ethereum.selectedAddress,to:'0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141',value:'0x38d7ea4c68000'}]})` on Gnosis; then again after `wallet_switchEthereumChain 0x1` | 发送 / −0.001 xDAI (amount card) / 接收方 0x7687…D141; the slide says 确认发送. No 合约交互, 无法解码 or 未验证合约 [RC1]. On Ethereum −0.001 ETH shows with no simulation. ✕ → 4001 once. | |
| G14-zero / G14-num | The same with `value:'0x0'`, then with `value` omitted; then with `value:1000` (a JSON number) | 0 and omitted: 发送 / 0 xDAI (no minus) / 接收方, slide 确认 [RC3]. Number: the contract card with **no** amount, never "0 xDAI" [RC4, RC6]. | |
| DX-W1 · **G21 G22** | Fee shown → `mute vela-relay` → slide the dust send | 正在准备交易… covers the whole network wait (never 等待生物识别…) [RA9]. Then 提交至网络…. After ~30–46 s the title stays 提交至网络…, captioned 可能已经发出。Vela 会继续查看，请不要重复发送。, with the op hash and 关闭 · 后台继续, and **no 重试** [RA10]. The dApp gets **one** ok answer equal to the op hash, within ~120 s of the slide [RA2, RA12]. Activity shows `dApp 交易 · 处理中 · 127.0.0.1:8137` [RG2]. `.err`: `relay: submit verdict=maybe_sent hash=0x… attempts=n`. Then `pass` → within ~12 s it reads 已确认 with the tx hash, no tap. The explorer agrees and the fixture Safe's nonce moved by exactly 1. | |
| DX6 · W1 | `reset_mid vela-relay` → slide dust; repeat once | Never 失败 · 请重试 while the dust lands. After `pass`, each ends 已确认, or 失败 with the nonce unchanged. | |
| DX7 | Slide dust; `blackhole vela-relay` as soon as 提交至网络… shows; wait 100 s; `pass` | The may-have-been-sent or still-confirming words, never 失败. After `pass`, 已确认 with no tap. | |
| S5 · NotSent | Fee shown → `drop vela-relay` → slide | An immediate cross: 失败 · 交易未能提交。您的资金安全无虞——请重试。 No Activity row. The dApp gets one -32603 with the fixed "relay unreachable; nothing was sent", never "All bundler endpoints failed". The Safe nonce is unchanged. `.err`: `submit verdict=not_sent`. | |
| DX-G13 · **G13** | `cd app-desktop/vela-wallet && cargo test live_an_unknown_hash_is_pending_not_unreachable -- --ignored`; during DX-W1, grep `.err` for `tracker:` | The live test passes: the relay answers `not_found`, not -32601 [RA7]. Tracker lines carry relay statuses; none says the status method is missing. Optional, with Arbitrum funds: an op the relay rejects ends 失败 within ~12 s of the rejected status (S6). | |
| DX-W3 · W3 | Opportunistic: a dApp tx that reverts on chain (min-slippage swap, or a call to a reverting contract) | 失败 + 转账在链上被回滚… + explorer, never 已确认 [RA8]. The Activity row is failed. The dApp gets the tx hash once. | |
| DX8 · W14 | Tab 1 test dApp, tab 2 `example.com`. In tab 1 start Send dust (unapproved) → click tab 2; click tab 1; close tab 2; then ✕ | The tab does not switch; the signing column comes forward; the bar says 请先完成或取消这个请求 for ~2.5 s [RD1]. The dApp log has no 4900. Clicking tab 1 does not reload it, and closing tab 2 does not reload tab 1. ✕ → 4001 once, and after that switching works. | |
| DX9 · W17 | `latency 20000 vela-relay` → slide dust → ⌘W during 提交至网络…; ⌘W again within 5 s; relaunch | The first ⌘W is held: `.err` `window: close held (submit in flight)` [RD14]. The second quits. After relaunch Activity lists the dApp tx, pending and then 已确认 [RG3]. | |
| DX-LD3 · L-D3 | After DX-W1 and S4: open Activity within 5 s; relaunch; open the row | The row appears within 5 s, survives the relaunch, and its detail shows the title dApp 交易, the status chip, and 发起方 127.0.0.1:8137 [RG1–RG3]. | |
| DX-LD5 · L-D5 | dApp `eth_sendTransaction` on Arbitrum (sheet only, reject with ✕); then on Gnosis, USDC `transfer(<any>, 10^30)` | Arbitrum: caution-styled Vela 未能检查这笔交易的结果，请核对后再签名。, and afterwards no Arbitrum chain notice or RPC banner [RG6, RG7]. Revert: danger 预计会失败… [RG8]. | |
| DX-LD6 · L-D6 | Connect; switch account in the wallet; switch back | Every address the dApp logs (`eth_requestAccounts`, `accountsChanged`, `eth_accounts`) is EIP-55 [RG10]; compare `p30-dapp-log`. | |
| DX-LD7 · **G1** L-D7 | An account with no history: home; History on 全部网络; History on Gnosis; sidebar filtered to one chain | Home Activity shows a skeleton, then 暂无交易记录 / 收款将实时显示在这里。 [RD10]. History reads 暂无交易; filtered, 此网络暂无交易. Home filtered reads 该网络暂无交易记录 [RG5]. | |
| DX-T5 · W16 | Optional, Trusted-Signer account only: `blackhole sign\.getvela` → Sign → 去签名页确认 → close the browser tab without signing → back to Vela | The card reads 签名页没能打开，请检查网络。 with 重试 primary; the request stays open [RD13]. The browser is not behind the dev proxy (ruling 6), so this checks the app's half only; the browser half is unit-tested. | 👆 if it signs |
| SC-006a/b | No fault (launch without `VELA_DEV_PROXY`): 10 × `https://vela-tN.invalid`, then 10 real dApps | Each `.invalid` ends on a panel with no auto-retry when its class is `not_found`. Record the words: behind a proxy, a DNS failure can surface as the network sentence (RX). Recents holds exactly the pages that loaded, each with its own title and icon. | |
| SC-007-en | Relaunch with `VELA_LANG=en`: bar, consent, connection panel, site menu | No visible "Secure site", "Not secure" or "Encrypted". | |

## 3. Chrome extension

§1.4 profile, parallel space, tab A `http://127.0.0.1:8137`, tab B `http://127.0.0.1:8138`.

| id | do | expect | 👆 |
|---|---|---|---|
| EX0 | Freshness (§1.1); load; reload the dApp tabs | `dist/manifest.json` version and the wasm hash match `rust/pkg-web`. | |
| U2 · **G16** | Tab A → Connect | The panel card: Cancel is a bordered button and Connect is filled accent with a white label, both at least `--size-control-lg` tall [RB12]. No `eth_requestAccounts` line. Screenshot in zh and en at panel width. | |
| EX2 | Panel open; dApp console `setTimeout(()=>ethereum.request({method:'personal_sign',params:['0x48656c6c6f',ethereum.selectedAddress]}),6000)` | The request shows **in the panel**; no window pops up [RB8]. It is answered once. | |
| EX4 · **G18** | Panel serving tab A → tab B Connect | B's card within 1 s, or right after A's open request is answered. B is answered once [RB7]. | |
| EX4b | In the panel: Wallet → Settings → Wallet → tab B Connect | The card shows [RB9]. | |
| EX5 · **G17** | Tab A Sign (sheet up) → reload tab A without deciding → Sign again | The first sheet is gone within 1 s; only the new one shows → approve → one signature. `storage.session` holds no `vela.req.*`. The worker log has `req.settled cause=page_left` [RB3]. | |
| EX6 | Sign → close the side panel with Chrome's ✕ | The dApp gets 4900 "The browser closed before the request finished" at once; `req.settled cause=surface_closed` [RB10]. | |
| EX7 ⏱ 5 min | Sign → wait 5 min 10 s with DevTools closed | 4900 at ~305 s; the sheet is gone; nothing can be approved [RB11]. | |
| EX8 · **G19** | Sign → chrome://serviceworker-internals → Stop the Vela worker (DevTools closed) → approve in the panel | The dApp shows no error and no "message channel closed" text. Approve → the signature arrives once. Worker log: `sw.start … recovered=1`, `req.resumed` [RB4, RB5]. | |
| EX8b · **G23** | After EX8: reload tab A → Connect → Sign. Then leave everything idle ≥ 60 s with DevTools closed until Chrome stops the worker by itself, and repeat | Each request shows in the panel within 1 s. `storage.session` holds only live records, and no `vela.req.*` is left in `storage.local`. If anything is missed, the `sw.start` / `req.arrived` lines say which branch (closes G23(d)). | |
| EX3-idle | A sign sheet left open 60 s with DevTools closed → approve | The answer arrives. | |
| EX9 | `blackhole ''` (only CfT is pointed at 8899) → a fresh origin: Connect → switch to Gnosis → Chain | All local: the card is instant; switching returns null plus `chainChanged`; Chain answers. | |
| EX10 / EX11 · **G20 G33** | `blackhole 'gnosis\|xdai\|1rpc'` → Block number, twice, timed; then `pass` → Block number | Each faulted call ends within ~3 × 8 s with `-32603 "Vela could not reach a node for chain Gnosis (100)"`, with no "Failed to fetch" [RF2]. After `pass` the first call answers in under 3 s (it was 6.3 s). Worker log: `read.fail kind=timeout host=…`, `read.exhausted chain=100 tried=3`. | |
| S7 / S8 | As on the desktop | As on the desktop. | |
| EX-W1 · **G21 G22** | Fee shown → `mute vela-relay` → slide dust | As DX-W1: 正在准备交易…, then the may-have-been-sent caption; one ok answer (the op hash) within ~120 s of the slide, well before content.js's 300 s. The panel's Activity shows the pending dApp row. After `pass`: 已确认, and the dApp's `eth_getTransactionReceipt(<op hash>)` returns the real receipt [RF3]. Panel console: `submit verdict=maybe_sent`. | |
| EX-S5 · NotSent (web) | Fee shown → `drop vela-relay` → slide → 30 s → `pass` | **Expected difference** [RA1, RA4]: the browser cannot prove a refusal, so it reads may-have-been-sent. After `pass` the relay answers `not_found` twice, ≥ 60 s after the submit, and within ~90 s of the slide the sheet or row turns 失败 and the record is failed. The Safe nonce is unchanged. | |
| EX12 | `latency 6000 vela-relay` → dust | submitting → ring → landed tick. `eth_getTransactionReceipt(hash)` returns a receipt. | |
| EX13 · W12 | Panel DevTools `vela.silentReceipt(100)` → dust → wait > 120 s → dApp polls `eth_getTransactionReceipt(<returned hash>)` every 10 s | The panel says still confirming, then landed. The dApp's receipt read for the op hash returns the real receipt once the op lands; it no longer stays null forever [RF3]. | |
| S3 · L-PANEL | Sign in the panel → approve; and `cd app-web/vela-wallet && npx playwright test -c playwright.isolated.config.ts e2e/extension-live-provider.e2e.ts e2e/extension-lifecycle.e2e.ts` | The 已签名 tick shows in the panel and clears by itself. The e2e suites are green on their own port (4174), including the signed-tick test [RG12, RH5]. | |
| E-G14 · **G14** | The DX-G14 console request, then zero, then numeric | No red 盲签 box: 发送 / −0.001 xDAI / 接收方 0x7687…D141, slide 确认发送. ✕ → 4001 once. Zero and numeric as on the desktop. | |
| EX-LD6 · L-D6 | Connect; switch account; switch back | Every address is EIP-55; old lower-case grants were rewritten at the wallet's first boot [RG10]. | |
| EX-G1 · **G1** L-D7 | `wallet.html` in a wide tab, account with no history | Activity shows a skeleton, then 暂无交易记录 / 收款将实时显示在这里。 [RB13]. History wording as DX-LD7. | |
| EX-LOG · W23 | After EX5, EX6, EX8: `chrome.storage.session.get('vela.sw.log')`; Settings → 反馈 preview | `req.arrived` / `claim` / `settled` lines with host-only fields. The preview lists e.g. `sw:req.settled.page_left ×1` and no URL or address. | |
| EX-W3 · W3 | Opportunistic revert, as DX-W3 | 失败 + failedHint. The dApp gets the tx hash, not -32603 [RA8]. | |
| SC-007-en | Wallet language English → U2, connection list, signing header | No visible "Secure site", "Not secure" or "Encrypted". | |

## 4. iPhone

§1.5 launch, parallel space, test dApp `http://192.168.50.9:8137`.

| id | do | expect | 👆 |
|---|---|---|---|
| IX0 / IX1 | Freshness gate; Debug install; launch with the dev proxy | chaos.log shows the phone's CONNECTs at once. The syslog shows `app.getvela.VelaWallet` lines. | |
| E-L1 · SC-003 | On `jumper.exchange`: `latency 6000 example` → `example.com` Go, with screen recording | Count frames: hairline within 0.5 s of Go. The bar keeps 🔒 jumper.exchange until commit at ~6–7 s [RE1]. | |
| E-G28a · **G28** | On `jumper.exchange`: `blackhole uniswap` → type `app.uniswap.org` | The bar stays on jumper.exchange with a moving hairline. At 20 ± 2 s the panel shows 无法加载此页面 / 网络不稳定，页面没能打开。/ app.uniswap.org / 重试, with **no lock** [RE1, RE2]. Automatic attempts at ~+2/+5/+10 s show 正在重试…, busy and not dimmed [RE5]. Log: requested, stalled, 3 retries. | |
| E-G28b · G28 (spoof) | On the test dApp: `blackhole uniswap` → Web Inspector `location.href='https://app.uniswap.org/'` | The bar **never** names app.uniswap.org over the live test page; it keeps 192.168.50.9:8137 until the panel at 20 s. | |
| IX2 · **G32** | On the connected test dApp: `blackhole example` → type `example.org` in the same tab, with a stopwatch | The panel appears at 20 ± 2 s (it was ~75 s). Until then the bar reads 192.168.50.9:8137, never 🔒 example.org. | |
| E-G28c | `mute uniswap` → new tab → `app.uniswap.org` | The fresh tab's bar shows `app.uniswap.org` with no lock and a hairline; the panel appears at 20 ± 2 s. | |
| E-Stop | During E-G28a's hang: ⋯ → 停止 | The hairline goes; the page underneath stays usable; the bar is unchanged; no panel. ⋯ shows 刷新 again [RE5]. | |
| E-W5-back · W5 | `drop ''` → open `app.uniswap.org` → panel, 3 retries, stops (~17 s) → `pass` → switch to 钱包 → back to 探索 | The page has loaded without a tap. Log line: `net came back` [RE3]. | |
| E-G31 · **G31** | `drop uniswap` → `app.uniswap.org` | 网络不稳定，页面没能打开。, never 找不到这个网站. Retries at 2/5/10 s. Log: `NSURLErrorDomain -1000 → offline` [RE4]. | |
| L5 | `https://expired.badssl.com/` | The certificate line; no retry. | |
| C1 / C2 · **G33** | Test dApp on Gnosis: `blackhole 'gnosis\|xdai\|1rpc'` → Block number; Retry; `pass` → Block number | A notice naming Gnosis within ~25 s (none within 60 s before the fix) [RF1]. Retry is busy until its read settles. After `pass` the notice clears without a tap. | |
| IX6 · W10 | Relaunch with `blackhole 'gnosis\|xdai\|1rpc'` → Send dust from the dApp; watch the fee row 60 s; `pass` | The fee row shows the service-unreachable reason and re-quotes on its own [RF5]. After `pass` the fee shows within 15 s. | |
| S7 / S8 | `drop vela-relay` → Send dust → `pass`; then refresh | As before (it passed during the pass, `ios-i34`): the reason is shown, and the fee is back within 15 s with no tap. | |
| IX-W1 · **G21 G22** | Fee shown → `mute vela-relay` → slide dust | 正在准备交易… (before the fix: 提交至网络… for 50 s, then 失败). Then the may-have-been-sent caption with the op hash, and no 重试. One ok answer = the op hash. Activity shows the pending row. `pass` → 已确认 within ~12 s. Log: `sign: submit verdict=maybe_sent`. | |
| IX8 · W1 | `reset_mid vela-relay` → slide; 3 times | None of the three says 失败 / 请重试 while the dust lands. After `pass`, each is 已确认, or 失败 with the nonce unchanged. | |
| S5 · NotSent | Fee shown → `drop vela-relay` → slide | An immediate cross; no pending row; the nonce is unchanged. | |
| IX7 · W3 | Opportunistic revert | 失败 + 转账在链上被回滚… + explorer, never 已确认 ✓ [RA8]. | |
| I-G14 · G14 | The DX-G14 request (Web Inspector console), then `value:'0x0'` | The look is unchanged (发送 −0.001 xDAI / 接收方 0x7687…D141), now drawn from the core. The zero-value card reads 发送 / 0 xDAI, slide 确认. ✕ → 4001 once. | |
| E-G9/G10 · **G9 G10** | Test dApp → Connect, in zh and en | The sheet names the host once: one header "连接到 192.168.50.9:8137", then the account and network rows, then one sentence 该网站想查看你的地址… above 拒绝 / 连接, and nothing below [RE6]. | |
| E-G12 · **G12** | Sign on Ethereum, then Gnosis; repeat at the largest text size | The header shows `192.168.50.9:8137` whole (wrapping if needed), and the chip reads Ethereum / Gnosis whole at 375 pt [RE13]. | |
| E-G25 · **G25** | Two tabs; force-quit; relaunch; open the switcher without showing the second tab | Card heights are equal and top-aligned. The dormant tab shows its avatar and host, with no fake bars. The tile reads 新建标签页 whole [RE12]. | |
| E-G8 · **G8** | Recents after visiting the test dApp (its title is its host) | The host shows once [RE7]. | |
| E-G26 · **G26** | Send dust from the dApp → stay in 探索 until 已确认 → go to 钱包 | The xDAI balance reflects the send within 5 s, without a pull. Log: `tracker confirmed`, then `balance refresh (holdings_moved)` [RE8]. | |
| E-W20 · W20 | Force-quit → `drop` on the logo hosts (read them from chaos.log first) → launch → letters and dots → `pass` → wait 60 s, or trigger E-W5-back | The logos come back without a relaunch [RE10]. | |
| E-LD3/6/7 | As DX-LD3, DX-LD6, DX-LD7 on the phone | Same expectations. | |
| E-W23 · W23 | For every failure row above: find its line in the collected syslog; Settings → 反馈 preview | Each failure row has a line naming the host or service and the kind. The preview shows 最近失败: browser: timeout; … with no hosts [RE11]. | |
| E-probe · T004 | Claude, hermetic: build-for-testing → copy the `.xctestrun`, remove `DappBrowserStabilityProbeTests` from SkipTestIdentifiers → `test-without-building` for `testProbeTheBrowserCheckpoints`, `testProbeALoadThatFails` and the new never-answering-listener test. Needs UI Automation ON and a trusted developer certificate. | All pass. The panel shows at 20 ± 2 s and the busy Retry label is asserted. No proxy is involved [RH4]. | |
| SC-006a/b | No fault: 10 × `https://vela-tN.invalid`, then 10 real dApps | 找不到这个网站 with no retry. Most real dApps now commit or fail within 20 s (8 of 10 did not before, G28). Recents holds exactly the pages that loaded. | |
| SC-007-en | Relaunch with `"VELA_LANG":"en"` | No visible "Secure site" or "not encrypted". The http lock's VoiceOver label is allowed. | |

## 5. Android smoke (shared rules, FR-020)

This is not a fault pass [RH3]. The JVM suite (plan phase 5) is the gate. With the Xiaomi
attached, run once in the parallel space with no proxy:

| id | do | expect | 👆 |
|---|---|---|---|
| A-G14 | The DX-G14 request in the Android dApp browser | The look is unchanged (发送 / −0.001 xDAI / 接收方), now drawn from the core's `PlainSend`. | |
| A-G28 | New tab → type `https://example.com` | Until commit the bar shows `example.com` with no lock. Never the fixture host `app.uniswap.org` or an open warning lock [RE1]. | |
| A-LD3 | Send dust from the test dApp → open Activity | The dApp row appears within 5 s (it used to wait for a tick), then turns confirmed [RG3]. | |
| A-LD5/6/7 | Arbitrum sheet; account switch; empty History | Caution "could not check"; EIP-55; 暂无交易 on 全部网络. | |
| A-G26 | After A-LD3 confirms → home | The balance moved without a pull [RE8]. | |

## 6. Owner batch (real passkeys, at the end)

These are the flows the parallel space cannot cover. **No faults here**: release and owner
builds carry no dev proxy, and the owner's Chrome runs with no flags. Each row is short.

| id | client | do | expect | 👆 |
|---|---|---|---|---|
| O-D1 | Mac, `/Applications` build (signed as in the macOS platform-passkey recipe: `lsregister -f`, launched with `open`) | Test dApp → Connect → Sign → Touch ID | 等待生物识别… shows **only while Touch ID is up**, then 已签名 and it closes by itself [RA9]. | 👆 |
| O-D2 | Mac | Send dust on Gnosis → Touch ID | 提交至网络… → ring → 已确认 with hash. Activity shows `dApp 交易 · 127.0.0.1:8137`, confirmed, and it survives a relaunch. The balance moved. | 👆 |
| O-D3 | Mac | Connect with the owner's accounts; switch account and back | The consent shows the owner's account and network (G11). The dApp logs EIP-55 only (L-D6). | |
| O-E1 | Owner's Chrome | chrome://extensions → Load unpacked from the 082 `extension/dist` (or reload once merged) → reload the dApp tabs → sign in if asked | The same address as the owner's wallet. | 👆 |
| O-E2 | Owner's Chrome | S3 in the side panel, then S4 dust | The 已签名 tick in the panel; submitting → ring → landed tick. The panel's Activity lists the dApp tx (L-D3 live, previously "No activity yet"). | 👆 |
| O-E3 | Owner's Chrome (optional) | Sign → chrome://serviceworker-internals Stop → approve | The answer arrives once (EX8 in the real profile). | 👆 |
| O-I1 | iPhone, relaunch with `"VELA_PARALLEL_SPACE":"0"` (no `--console`) | S3 sign → Face ID; S4 dust → Face ID; then 钱包 | 已签名！ and 已确认. The balance moves within 5 s (G26). **G24**: 钱包 lists USDC on Base ≈ 0.470005 (≈ $0.47), as the desktop does. The balance log names any chain that failed. | 👆 |
| O-T1 | iPhone and Mac, Trusted-Signer route only, **after the owner deploys the signer page and `LAUNCH` moves** | T1 from the test dApp | The page header names `127.0.0.1:8137` / `192.168.50.9:8137` once (L-HOST). The self-reported-site warning is still there. A zero-value plain send reads Send 0, not 盲签 [RC8]. | 👆 |

## 7. Close-out

```sh
chaos mode=pass; pkill -f chaos-proxy.py; pkill -f 'http.server 813'
diff $S/proxy-before.txt <(scutil --proxy) && echo "system proxy untouched"
rg -n '0x[0-9a-fA-F]{130,}|/v3/[0-9a-f]{20,}|#[A-Za-z0-9_-]{40,}|signature=0x|privateKey|mnemonic|seed' $S/logs   # must print nothing (FR-019)
```

- **SC-007.** A table of failure row → log line, one per row in §2–§4 (FR-018).
- **Client matrix** in `results.md`. For every G#, L-… and W#: fixed / already right / no such
  surface, for desktop, extension, iPhone and Android.
- **Regression coverage** — every finding with a device reproduction, and its row:

| Finding | Rows |
|---|---|
| G3 | DX-G3 |
| G13 | DX-G13 (+ S6 optional) |
| G14 | DX-G14, G14-zero/num, E-G14, I-G14, A-G14 |
| G16 | U2 (extension) |
| G17 | EX5 |
| G18 | EX4, EX4b |
| G19 | EX8, O-E3 |
| G20 | EX10 / EX11 |
| G21 | DX-W1, EX-W1, IX-W1, DX6, DX7, IX8 |
| G22 | DX-W1, EX-W1, IX-W1, O-D1 |
| G23 | EX8b |
| G28 | E-G28a/b/c, A-G28 |
| G29 / G30 | DX14 |
| G31 | E-G31 |
| G32 | IX2 |
| G33 | C1 / C2 (desktop, iPhone), EX10 / EX11 |
| G1 | DX-LD7, EX-G1 |
| G2 · G6 · G7 · G11 | DX11 · DX12 · DX13 · U2 (desktop) |
| G8 · G9 · G10 · G12 · G25 · G26 | E-G8 · E-G9/G10 · E-G12 · E-G25 · E-G26, O-I1 |
| G24 | O-I1 |
| L-D3 live | DX-LD3, EX-W1, O-E2 |
| L-D6 live | DX-LD6, O-D3 |
| G4 | not in 082 (research RD15) |

## 8. Round 2: re-verification after Phase 9 (post2-*)

Run after the Phase 9 gates (T226, T235, T243, T251) on builds from a clean, committed tree
(§1.1). Same set-up as §1; evidence goes in `evidence/<client>/post2-<row>.jpg` / `.txt`. These
rows re-run every refuted and unproven row of the post-fix pass and check each new finding
(G34–G73). Where a row here and a §2–§4 row disagree, this row's expectation wins. Pass
`latency=0` with every chaos mode (G73) unless the row sets a latency.

### Desktop (T252)

| id | re-runs | do | expect |
|---|---|---|---|
| post2-D1 | DX9 · **G34** | `mute vela-relay` → slide dust → ⌘W during 提交至网络… → ⌘W again within 5 s → relaunch with the relay still muted | The first ⌘W is held: the bar's notice slot reads 提交至网络… and the column comes forward; `.err` `window: close held`. The second quits. `.err` of the first run shows the write-ahead record (`OpSigned`/persist) **before** `relay: submitting`. After the relaunch Activity lists `dApp 交易 · 处理中 · 127.0.0.1:8137` at once; after `pass` it turns 已确认 (or 失败 if the tracker proves not sent) with no tap. The Safe nonce moved by at most 1 [RJ1]. |
| post2-D1s | DX9 for the wallet's Send · G34 | 钱包 → Send dust with `mute vela-relay`; quit during 提交至网络…; relaunch | The pending send row is there after the relaunch and resolves after `pass` [RJ1]. |
| post2-D2 | S5 | Fee shown → `drop vela-relay` → slide | 失败 · 交易未能提交…; after the verdict no Activity row (a row may flash for < 1 s during the POST); one -32603 `relay unreachable; nothing was sent`; nonce unchanged. |
| post2-D3 | DX-W3 / S6 · **G36 G52** | Gnosis USDC `transfer(0x7687…, 10^30)` from the dApp → slide | The relay rejects it: the dApp gets exactly one `{ok:false, code:-32603, message:"the network refused this transaction; nothing was sent"}` within ~15 s of `status=Rejected`, never ok. The sheet: 失败 + 网络拒绝了这笔交易，什么都没有发出。, no 请重试. Activity detail: 接收方 0x7687…D141 (EIP-55), no explorer button. Nonce unchanged [RJ3, RJ16]. |
| post2-D4 | DX-W1, DX6 · **G37** | `mute vela-relay` → slide dust; keep muted | When the chain check finds the op, the dApp gets the tx hash at that moment (not at ~120 s) and the sheet reads 已确认; it never shows 提交至网络… or the may-have-been-sent caption after that [RJ4]. |
| post2-D5 | CLOSE-TAB · **G40** | Three tabs, A shown; close B and C with ✕; then open a Sign request in A and close a background tab | No `asked host=<closed tab>` line; A's page is not reloaded (the dApp log keeps its results); no 请先完成或取消这个请求 flash for a close. |
| post2-D6 | KEY-FOCUS · **G41** | Uniswap (autofocused) → click the bar → type `a` → Enter; then paste two URLs one after the other with Enter between | Nothing reaches Uniswap's field; Enter gives `browser: navigate asked`; the second paste replaces the first (no `…comhttps://…`) [T201]. |
| post2-D7 | DX14, DX11 · **G42** | Connected test dApp → + → type `app.uniswap.org`; frames at 0.5/1/2/3 s. Then relaunch with a Uniswap tab, click it, press Back | No frame shows the test dApp or its connected dot under `app.uniswap.org`. On Uniswap's first page Back is disabled (never 127.0.0.1:8137); after an in-app route change Back is enabled and stays inside Uniswap [RJ5]. |
| post2-D8 | DX14 · **G43** | `drop uniswap` → new tab → `https://app.uniswap.org` | Exactly three automatic attempts at ~+2/+5/+10 s; no `how=page` after a `retry attempt=`; the panel stays up between attempts [RJ8]. |
| post2-D9 | L2–L4, DX2 · **G44** | `blackhole uniswap` → open; `pass` before the third attempt | The first attempt runs by ~20–22 s (the hung load is replaced), 正在重试… busy; at most one `skipped (engine still loading)` line per load; after `pass` the page loads within ~12 s [RJ9]. |
| post2-D10 | L5 · **G45** | `https://expired.badssl.com/` | The certificate sentence; no automatic retry; `.err` `class=certificate` [RJ10]. |
| post2-D11 | C1 · **G46** | As §2 C1 (fixed wording) | The notice while the call is still pending, ~15–25 s; `chain notice: shown chain=100` before `gave up` [RJ11]. |
| post2-D12 | S7/S8 · **G47** | `drop vela-relay` → Send dust → `pass` (note its chaos-log time) | The fee is back ≤ 15 s after `pass`, by the chaos log and the `.err` stamps; `.err` has `fee: quote failed chain=100 cause=… re-quote #n` and `fee: quote back chain=100 …` [RJ12]. |
| post2-D13 | DX-G14 Ethereum · **G48** | No fault: the DX-G14 request after `wallet_switchEthereumChain 0x1` | If the public node rate-limits the deployment read, the fee row reads 被限流 · 正在自动重试 (unreachable: 暂时连不上 Ethereum…), never 无法连接 Vela 服务 [RJ13]. |
| post2-D14 | G14-num · **G49** | `value:1000` (a JSON number) | The simulation box reads `−0.000000000000001 xDAI` or shows no row; never `−0` [RJ15]. |
| post2-D15 | DX-LD3, T181 · **G50 G51** | A confirmed dApp record's detail; a pending one; switch to 钱包 while the may-have-been-sent column shows, then back to 探索 | The 哈希 fits with its copy button visible; on the pending record 删除记录 is a quiet control under the explorer; the maybe-sent column is back when 探索 returns [RJ18]. |
| post2-D16 | T181 notes, DX3′ · **G53** | `blackhole 'gnosis\|xdai\|1rpc'` for 3 min with the wallet home open; relaunch with the fault on | No `net: offline` / `came back` lines; after the relaunch the home lists the other chains' assets, and any RPC banner names Gnosis only [RJ14]. |
| post2-D17 | DX3′, SC-006b · **G54** | Open 8 tabs | The lit tab and + stay visible (tabs shrink, then the strip scrolls). |
| post2-D18 | **G67 G68 G70** | ⌘W with nothing submitting; `rg -n '^\[vela-wallet\] [^0-9]' .err`; bfcache Back to the test dApp; look at the badge | The window closes; every `.err` line is timestamped; the tab is renamed after the bfcache Back; the badge covers no ✕ or +. |
| post2-D19 | DX4 (unproven) | `blackhole iana` → example.org's own link; then load a fresh entry, fault it, go Back to it | The DX4 expectation, including the Back/Forward-to-a-faulted-entry leg (not from the bfcache). |
| post2-D20 | DX5 (unproven) | Read the Uniswap build's third-party hosts from chaos.log first; blackhole those that it requests | Usable, no panel (the live-progress path is exercised: chaos.log shows `HOLE` lines). |
| post2-D21 | DX-LD7, DX-LD6 (unproven) | Needs a second parallel-space account with no history (the extension's Parallel Two/Three kind). If the desktop fixture has one account, record "not run" with that reason | The DX-LD7 unfiltered wording, and the account-switch leg of DX-LD6 (EIP-55 `accountsChanged`). |

### Chrome extension (T253)

| id | re-runs | do | expect |
|---|---|---|---|
| post2-E1 | P0 probe, T182 · **G35** | No fault: dust send → after the slide, when 提交至网络… or 已提交 shows, `chrome.sidePanel.close`; repeat once with a panel reload during the may-have-been-sent caption (`drop vela-relay`) | The dApp gets exactly one ok = the op hash, never 4900; worker log `req.answered cause=surface_closed maybe_sent=1`. The op lands once (nonce +1). Reopened panel: the row 处理中 → 已确认 (or 失败 for the drop variant after `pass`) [RJ2]. |
| post2-E2 | EX6 | Sign (or a tx before the slide) → close the panel | 4900 "The browser closed before the request finished" once (unchanged). |
| post2-E3 | EX4b · **G55** | Panel on Settings (and once on Contacts) → tab B Connect; then a tx from tab A | The card shows within 1 s with no tap. |
| post2-E4 | EX-S5 · **G36** | Fee shown → `drop vela-relay` → slide → `pass` at +30 s | 失败 at ~90 s; the dApp gets one -32603 `relay unreachable; nothing was sent` (the tracker's NotSent fell inside the window), never ok; nonce unchanged [RJ3]. |
| post2-E5 | EX-W3 probe · **G36 G57 G60** | USDC `transfer(0x1111…, 10^30)` on Gnosis | Before the slide: the danger line 这笔交易预计会失败…; the amount fits the 360 px panel and the fiat is formatted (no `1e+24`). Slide → relay rejects → one -32603 refused; 失败 + 网络拒绝了这笔交易，什么都没有发出。, no 请重试 [RJ3, RJ19]. |
| post2-E6 | EX-W1 · **G37 G39 G56** | `mute vela-relay` → slide dust | The title reads 提交至网络… with the may-have-been-sent caption. When the chain check finds the op, the dApp gets the tx hash at once and the sheet ends 已确认, never 提交至网络… again. If nothing finds it, the op hash arrives ≤ 121 s after the slide [RJ4]. |
| post2-E7 | EX13 · **G38** | `vela.silentReceipt(100)` → dust | 已确认 within ~15 s of the op landing (the relay's `included` + tx hash is confirmed through the chain), not 还没上链 [RJ4]. |
| post2-E8 | EX-LD6 · **G58** | Settings → 切换账户 → Parallel Two; then plant a lower-case grant and boot the panel on Settings | The site gets `accountsChanged` [Two] (EIP-55) and `eth_accounts` = Two; the grant is rewritten without visiting 钱包 [RJ20]. |
| post2-E9 | EX7, SC-007-en · **G59** | Pin 简体中文 → close and reopen the panel | zh sheets; the picker ticks 简体中文. |
| post2-E10 | EX-LOG · **G61** | After E4, E5 and a fee failure: the panel console; Settings → 反馈 preview | Lines `submit verdict=…`, `tracker: …`, `fee: quote failed … / quote back …`; no "ACCEPTED but NOT landed" for E4; the preview lists `panel:submit.not_sent ×1`, `panel:submit.refused ×1` with no hash, address or URL. |
| post2-E11 | EX-G1 · **G62** | Parallel Three, sidebar filter Gnosis | 该网络暂无交易记录 / 此网络暂无交易. |
| post2-E12 | EX8b · **G63** | Leave the panel idle 3 min with DevTools closed, then Connect | At most one `sw.start` after the first idle stop (no 30 s cycle); the card still shows within 1 s. |
| post2-E13 | EX10/EX11 · **G64** | `blackhole 'gnosis\|xdai\|1rpc'` → Block number twice | Call 1 ≤ ~3 × 8 s with `read.fail kind=timeout`; call 2 ≤ ~8 s (one cooled endpoint tried). |
| post2-E14 | EX4 · **G65** | A signs while B's Connect is queued | B's card shows at once; no tick over it. |
| post2-E15 | S7/S8 · **G47** | As post2-D12 in the panel | Fee back ≤ 15 s after `pass`; the cause line hides during 估算中…, sits under 网络费, and the sheet does not jump; `fee:` lines. |
| post2-E16 | S3 · **G66** | The isolated e2e run (with the two new lifecycle cases) while CfT has the extension open | Green on 4174; `extension/dist` is untouched by the run (mtime and hash unchanged); open pages keep working. |
| post2-E17 | EX0 · G72 | Bug-report label | Equals `git rev-parse --short HEAD` of the clean tree the build came from. |

### iPhone and Android parity (T254)

| id | re-runs | do | expect |
|---|---|---|---|
| post2-I1 | **G34** parity | `mute vela-relay` → slide dust → force-quit during 提交至网络… → relaunch (same `-e` JSON) | The pending dApp row is there at once and resolves after `pass`; log `sign: write-ahead …` before the POST [RJ1]. |
| post2-I2 | **G36** parity | The USDC 10^30 transfer from the test dApp | One -32603 refused; 失败 + the refused words, no 请重试 [RJ3]. |
| post2-I3 | **G37** parity | `mute vela-relay` → slide; the chain check finds it | The tx hash reaches the dApp at once; the sheet 已确认 [RJ4]. |
| post2-I4 | IX6 · **G48** parity | A rate-limited or black-holed deployment read on the fee row | The chain's words (被限流 · 正在自动重试 / 暂时连不上 …), never 无法连接 Vela 服务 [RJ13]. |
| post2-I5 | **G53** parity | `blackhole 'gnosis\|xdai\|1rpc'` 3 min | No `net: offline` in the `log collect` archive [RJ14]. |
| post2-A1 | Android smoke | A-LD3; and, if cheap, the 10^30 transfer | The row within 5 s; a refused op answers -32603 refused. The JVM suite (T251) is the gate. |
