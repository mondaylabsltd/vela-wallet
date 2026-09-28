# Device pass: dApp browser on bad networks (desktop, Chrome extension, iPhone 11)

**Pull status.** I ran `git fetch origin main`. Local `main` and `origin/main` are both `de93634f` (the PR #327 merge) and the tree is clean, so there was nothing to pull.

**Build freshness has changed since the readers looked.** Both "blockers" in the reports were true when read, but other sessions have since rebuilt:
- **Chrome extension:** `app-web/vela-wallet/extension/dist` is now built from 079. `dist/manifest.json` says 0.9.5, `dist/vela_core_bg.55d58f879149.wasm` matches `rust/pkg-web/vela_core_wasm_url.js`, and `feeRequoteDelayMs` is in `dist/app/immutable/chunks/Cd2I35EH.js` (dated 09-28 14:13). The founder's Chrome still has to reload the unpacked extension.
- **iOS:** `rust/scripts/check-ios-core-fresh.sh` now says "ok … 21eb9628…" (it said exit 1 when the reader ran it).
- **Desktop:** the installed `/Applications/Vela Wallet.app` contains `de93634` (desktop reader).

Run both checks again right before the pass. Claims marked ✔ below I re-read in code myself; the rest are the readers' citations.

---

## 1. Log capture recipe

### Shared setup (read this first)

```sh
S=/private/tmp/claude-501/-Volumes-data-production-vela-wallet/d4a496f4-ab96-4481-916a-65326d48067e/scratchpad
mkdir -p ~/vela-logs; T=$(date +%m%d-%H%M)
chaos(){ curl -s "http://127.0.0.1:8899/__chaos?$1"; echo; }   # e.g. chaos 'mode=blackhole&match=gnosis|xdai'
tail -F $S/logs/chaos.log | tee ~/vela-logs/chaos-$T.log        # one line per connection: PASS/DROP/HOLE/FAIL/RESET
```

- **Chaos proxy.** It is already listening on `*:8899` with upstream `127.0.0.1:1088`, so phones on the LAN can use it. Mac LAN IP: `192.168.50.9`. The test dApp is served on `:8137` and `:8138`.
  - The port is fixed at 8899 (`scripts/device/chaos-proxy.py:154`). One fault therefore hits every client pointed at it. Test one platform at a time and run `chaos mode=pass` between rows.
- **Shorthand in the matrix:**
  - `blackhole <re>` means `chaos 'mode=blackhole&match=<re>'`. The same pattern applies to `drop` and `reset_mid`.
  - `latency N <re>` means `chaos 'mode=latency&latency=N&match=<re>'`.
  - `match=''` means every host. Only use it when nothing else you care about is pointed at 8899.
- **Loopback is never proxied** (WebKit, Chrome, and the desktop wallet's `proxy.rs` `is_local`). The test dApp at 127.0.0.1 is never faulted. Use real https hosts for page-load faults.

### macOS desktop (Rust/gpui, wry WKWebView)

```sh
# 0. Quit every copy (⌘Q). Check nothing is left:
pgrep -fl vela-wallet        # a dev binary from another session (target/release/vela-wallet) would also catch velawallet:// callbacks; close it first
# 1. App stderr: the only [vela-wallet] lines. Launch Finder-style (no shell proxy env).
open -n -F "/Applications/Vela Wallet.app" --stderr ~/vela-logs/desktop-$T.err --stdout ~/vela-logs/desktop-$T.out \
  --env VELA_LANG=zh --env VELA_SECTION=explore        # optional: --env VELA_BROWSER_URL=http://127.0.0.1:8137/
tail -F ~/vela-logs/desktop-$T.err
# 2. WebKit's own load/fail lines: the ONLY place a real WebKit failure code appears (wry hides didFail)
log stream --style compact --predicate 'process == "vela-wallet" AND subsystem == "com.apple.WebKit" AND category IN {"Loading","Process"}' | tee ~/vela-logs/webkit-$T.log
#    afterwards: log show --start "2026-09-28 15:00:00" --predicate '<same>' --style compact
# 3. Screenshots with no input (Claude): python3 $S/gui.py shot <name>   (gui.py idle = seconds since last key)
# 4. Point the Mac at chaos (restore afterwards). Service name from `networksetup -listallnetworkservices`.
networksetup -setwebproxy Wi-Fi 127.0.0.1 8899; networksetup -setsecurewebproxy Wi-Fi 127.0.0.1 8899
scutil --proxy | egrep 'HTTPS?(Proxy|Port)'
networksetup -setwebproxy Wi-Fi 127.0.0.1 1088; networksetup -setsecurewebproxy Wi-Fi 127.0.0.1 1088   # restore
```

Caveats:
- **Stderr is sparse.** It carries only boot, locale, `relay: submitting…`, `relay: busy…`, `previous op pending`, `submit failed`, `fee: relay estimation unavailable`, `trusted signer:`, `browser:` (wry build error) and `panic:` lines.
- **Silent paths.** Page loads, probe verdicts, auto-retries, proxy route flips (System→Direct), pool timeouts, the chain notice, fee re-quotes and receipt polls log nothing. For those, rely on the WebKit log, chaos.log and timestamped screenshots.
- **Don't launch from the terminal as-is.** The shell exports `all_proxy`/`http(s)_proxy`, which adds an Env route candidate. If you must, use `env -u all_proxy -u http_proxy -u https_proxy -u ALL_PROXY -u HTTP_PROXY -u HTTPS_PROXY … "/Applications/Vela Wallet.app/Contents/MacOS/vela-wallet" 2>&1 | tee …`.
- **Faults may be bypassed.** WebKit follows the system proxy only. The wallet's own traffic (RPC, relay, the load probe) falls back to **Direct** after any proxy-side failure (`proxy.rs:309-333` ✔). A chaos fault on relay or RPC hosts can therefore be bypassed. chaos.log shows it: the host stops appearing.
- **The proxy client may rewrite settings.** If the founder's proxy client manages the system proxy, it may put 1088 back. Re-check with `scutil --proxy` before each fault row (unverified).
- **Dev builds need a separate state dir.** Any dev build must use `VELA_STATE_DIR=<dir>`, or it rewrites `~/Library/Application Support/VelaWallet/wallet.json`.
- **Don't send input during the pass.** Claude must not drive input while the founder is at the Mac (the 90 s HID-idle gate applies).

### Chrome MV3 extension

**Recommended: the real Chrome 154 binary in a throwaway profile.** This gives CDP, a fault proxy for this browser only, and leaves the founder's Default profile untouched.

```sh
# 0. Freshness (re-check): must print 0.9.5 and the same wasm hash as rust/pkg-web
grep '"version"' /Volumes/data/production/vela-wallet/app-web/vela-wallet/extension/dist/manifest.json
ls /Volumes/data/production/vela-wallet/app-web/vela-wallet/extension/dist/*.wasm; grep WASM_URL /Volumes/data/production/vela-wallet/rust/pkg-web/vela_core_wasm_url.js
# 1. Launch (runs alongside the founder's main Chrome)
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --user-data-dir=$S/chrome-vela-real \
  --remote-debugging-port=9334 --proxy-server=http://127.0.0.1:8899 --no-first-run --no-default-browser-check
#    by hand, once: chrome://extensions → Developer mode → Load unpacked → app-web/vela-wallet/extension/dist
#    (branded Chrome ignores --load-extension; the id is still bjbdmnmpgcfkocfcfdocopkioacojkhl via manifest `key`)
# 2. NetLog (passive, does not keep the SW alive): chrome://net-export → Start Logging to Disk → ~/vela-logs/netlog-$T.json
# 3. CDP targets: curl -s http://127.0.0.1:9334/json/list   (service_worker …/background.js; page …/wallet.html?panel)
```

Caveats:
- **CDP logger.** Claude attaches a small logger to `:9334` (not written yet). It sets `Target.setAutoAttach` flatten, then enables Runtime, Log and Network, writing to JSONL. **Attaching to the SW, or opening SW DevTools, keeps the worker alive and hides eviction.** Leave the SW unattached for rows EX4, EX8 and EX9, and watch chrome://extensions ("service worker (Inactive)") or chrome://serviceworker-internals instead.
- **Ground truth is the dApp itself.** The SW logs one line (`background.js:107`) and content.js logs none. The test dApp prints `{ok, code, message}` for every call. Keep ⌥⌘I open on the dApp tab with Preserve log on.
- **Panel logs.** Right-click in the side panel → Inspect. It logs `[UserOp]`, `[sign_request]`, `[tx_tracker]`, `[rpc-pool]`, `[fee-policy]` and similar. For panel-only faults, run `localStorage.setItem('vela.dev.console','1')`, reload, then use `vela.failRelay(100)`, `vela.silentReceipt(100)` or `vela.clearFaults()`. These are per document and never reach the SW.
- **Default profile instead.** Use NetLog plus `open -a "Google Chrome" --args --enable-logging --v=0` and `tail -F "$HOME/Library/Application Support/Google/Chrome/chrome_debug.log"`. That file is overwritten at each launch, so copy it out. Whether SW console lines reach it is unverified. CDP is refused on the default data dir (Chrome 136+).
- **After a reload, reload every dApp tab.** Until then they answer `-32603 "Extension context invalidated."`.
- **No parallel space in the Default profile.** It swaps the real wallet out of extension storage.

### iPhone 11 ("ABC")

```sh
# stdout is the ONLY place print("[vela-wallet] …") goes. The app must be started this way, not from its icon.
xcrun devicectl device process launch --device F30282CB-FA41-589E-A3BD-31855EB64414 --terminate-existing --console \
  -e '{"VELA_PARALLEL_SPACE":"0","VELA_LANG":"zh","NSUnbufferedIO":"YES"}' app.getvela.VelaWallet 2>&1 | tee ~/vela-logs/ios-$T.console
# WebKit networking / CFNetwork errors / SafariViewService (no [vela-wallet] lines here)
idevicesyslog -u 00008030-001A75961445802E -p 'VelaWallet|com.apple.WebKit.Networking|com.apple.WebKit.WebContent|SafariViewService' | tee ~/vela-logs/ios-$T.syslog
# after the pass
/usr/bin/log collect --device-udid 00008030-001A75961445802E --last 30m --output ~/vela-logs/ios-$T.logarchive
/usr/bin/log show ~/vela-logs/ios-$T.logarchive --predicate 'process == "VelaWallet" OR process BEGINSWITH "com.apple.WebKit"' --info
```

Caveats:
- **Launch traps.**
  - `-e` replaces all `DEVICECTL_CHILD_*` variables, so put every knob in the JSON (`VELA_URL`, `VELA_PAGE=explore-live`, `VELA_SKIP_LAUNCH_ANIMATION`).
  - Ctrl-C kills the app.
  - Whether `NSUnbufferedIO` is needed is unverified.
- **Face ID trap (050/052).** A running `--console` session has made passkey or Face ID sheets cancel themselves ("Authentication canceled"). If that happens on a 👆 row, stop the console session and redo the row. You lose stdout for that row.
- **The only page-load log line** is `browser load failed: <url> — <domain> <code> → <class>` (`BrowserEngine.swift:624`). It is the ground truth for the -1200 question (W9). Submit failures, re-quotes, HEAD probes, retries and the chain notice log nothing.
- **Build and install.** The build must be **Debug**: VELA_* knobs, the parallel space and Web Inspector are DEBUG-only.
  - xcodebuild needs the hardware UDID: `-destination 'platform=iOS,id=00008030-001A75961445802E'`.
  - devicectl install uses the CoreDevice id.
- **Point the phone at chaos.** Wi-Fi ▸ Configure Proxy ▸ Manual `192.168.50.9:8899`, with **Shadowrocket OFF** for chaos rows. Use Shadowrocket (REJECT and REJECT-DROP rules) only for IX2. Turn the Wi-Fi proxy Off at the end. The Mac's own system proxy should be back on 1088 during iPhone rows.
- **Page console.** Safari ▸ Develop ▸ ABC ▸ page (Settings ▸ Safari ▸ Advanced ▸ Web Inspector on the phone).
- **Screenshots** only work through XCUITest. The scheme skip is not overridden by `-only-testing` on Xcode 26.3, so the copied-`.xctestrun` recipe is needed (Claude-only, IX8).
- **Parallel space persists.** After any parallel-space launch, relaunch with `"VELA_PARALLEL_SPACE":"0"` before the founder's own-wallet rows.

---

## 2. Device-pass matrix

👆 means the founder must press Touch ID or Face ID (or the passkey). Reused 079 ids: L1–L6, C1–C2, S1–S8, U1–U7, T1–T5. New ids are DX (desktop), EX (extension) and IX (iPhone). "W#" points to §3.

### Desktop (macOS app)

| id | do | expect | 👆 |
|---|---|---|---|
| DX0 | Start the logs (§1) and launch. Open `https://example.com`. | `.err` has `core: … booting` and the locale line. The WebKit log shows `loadRequest` → `didCommitLoadForFrame … isMainFrame=1`. | |
| U2 | Open `http://127.0.0.1:8137` → Connect | Title "连接到 127.0.0.1:8137". Connect is the filled button. | |
| U1 | Lock on the test dApp (http) and on example.com (https). Open the connection panel and the site menu. | http: open lock in warning colour. https: closed grey lock. No visible 安全站点, 不安全 or 已加密. | |
| U3 / U4 | Network picker; account picker | Logo and balance per network, no invented zeros. An identicon per account; 切换账户 works. | |
| U5 | Sign → look at the column header | The host once. | |
| S1 | Sign → Esc ×5, click the webview beside the column ×5 | The column stays. The page has no answer. | |
| S2 | ✕ | Closes. The page gets 4001 once. | |
| S3 | Sign → approve | 签名中… → 已签名 → closes itself. The page gets the signature. | 👆 |
| S4 | Send dust (Gnosis) → approve | 提交至网络… → ring "Gnosis 通常在约 15 秒内确认" → 已确认 with hash and explorer link → closes itself. The page gets the tx hash. `.err` has `relay: submitting sender=…`. | 👆 |
| T1–T3 | Only if the account signs through the Trusted Signer: Sign → 去签名页确认 → slide on the page → passkey. Then repeat and cancel the prompt (T3). | The app shows a button, not a slide; exactly one slide, on the page. The host is named, with no 未知站点. T3: "你取消了…" and the slide works again. | 👆 |
| — | Switch the Mac to chaos (§1 step 4). Confirm with `scutil --proxy`. | | |
| L1 | `latency 6000 example` → type example.com, Enter | Hairline within 0.5 s. The bar keeps the old host until commit. The page arrives at about 6–8 s. | |
| DX1 | `latency 9000 app\.uniswap\.org` → Go. `gui.py shot` every 2 s for 40 s. | **Spec:** no panel; commit at about 10 s. **W7 to catch:** a 网络不稳定 panel at about 8 s (3 s watchdog + 5 s probe), then a second `loadRequest` in the WebKit log at about 10 s (the auto-retry restarting a working load). **W8:** later probe CONNECTs to app.uniswap.org vanish from chaos.log, meaning the wallet flipped to Direct. Record the first commit time. | |
| L2 | `drop uniswap` → open app.uniswap.org | A Vela panel with the host and "网络不稳定，页面没能打开。", never WebKit's page. **Record time to panel:** about 3–8 s means the probe failed through the proxy; about 20 s means it fell back Direct and reached the host (W6). | |
| L3 | Leave L2's panel up | "正在重试…" at about +2, +5 and +10 s, then it stops. Run `pass` before the third attempt → the page loads by itself. | |
| L4 | Fault on → tap Retry | The panel stays with 正在重试… for the whole attempt. No WebKit page in any shot. | |
| L5 | `https://expired.badssl.com/` | The certificate sentence. No auto-retry and no "continue anyway". | |
| DX2 | `blackhole app\.uniswap\.org` → Go. Wait 60 s, then `pass`. | A panel with the offline or timeout reason (note about 8 s vs about 20 s). Retries at +2, +5 and +10 s. Recents gains nothing. After `pass` the page loads (tap 重试 if the schedule has ended). | |
| DX3 | Leftover dead proxy: `networksetup -setwebproxy/-setsecurewebproxy … 127.0.0.1 9` → open a dApp you have not visited → restore 8899 | **Ideal:** a prompt failure that points at the proxy. **Likely today (W6):** about 20 s of hairline, then the generic 网络不稳定 panel and retries, while wallet balances still load (the wallet went Direct). The WebKit log shows `didFailProvisionalLoad` early. Record the time and the words. | |
| DX4 | With a page loaded, `blackhole <second host>` → click an in-page link to it. Then toolbar Back/Forward to a black-holed entry. | Known gap (W18): no hairline, no panel, the old page stays. The founder decides whether that is acceptable. | |
| DX5 | `blackhole googleapis\|gstatic\|googletagmanager` → load app.uniswap.org | The page is usable with no panel. Record how long the hairline stays (W22). | |
| C1 | Test dApp on Gnosis, `blackhole gnosis\|xdai` → Block number. Time until the notice appears. Tap its Retry twice. | One line naming Gnosis; the chip is unchanged. **Record the delay** (expect a long one, W11). Retry shows no busy state. If the notice never appears, chaos.log shows the RPC went Direct. | |
| C2 | `pass` → Block number | The notice clears within about 5 s of a good read, with no tap. | |
| S7 | `drop vela-relay` → Send dust, do not approve. Time the fee row. `pass` without touching. (Repeat once with `blackhole`.) | "Service unreachable" sentence (not "—"), slide disabled, refresh visible. Record the seconds; route doubling may push it past 30 s. After `pass` the fee appears within 15 s with no tap (SC-005's first real run on desktop). A fee that appears while the relay is still faulted means relay traffic went Direct. | |
| S8 | Tap the fee refresh | Spinner → new fee. | |
| S5 | Fee shown → approve → immediately `drop vela-relay` | A cross with a plain reason; the page gets one error; no pending record. **Before any retry**, check the Safe nonce or explorer to see whether it landed (W1). | 👆 |
| DX6 | `reset_mid vela-relay` → approve a dust send. Watch RESET lines. | **Money rule:** never "failed" for an op that may be in the mempool. **W1 to catch:** "The gas relayer could not be reached. Please try again." while the explorer later shows it landed. Do not resend from the dApp. | 👆 |
| DX7 | Approve dust. As soon as 提交至网络 shows, `blackhole vela-relay`. Wait 100 s, then `pass`. | At about 90 s the page gets the userOpHash. The column says it was handed to the network, has not landed, and Vela keeps checking; never "failed". After `pass`, the tracker flips it to landed without a tap. | 👆 |
| S6 | Optional (needs Arbitrum funds): Send dust on Arbitrum | After the window, the still-confirming sentence. ✕ closes without rejecting; the page is answered once. W4 means it stays "still confirming" even if the relay rejected it. | 👆 |
| DX8 | Tab 1 test dApp, tab 2 another site. In tab 1 Send dust, unapproved → click tab 2 → back to tab 1. | Current code (W14): the request is cancelled (the page gets 4900), the column closes, and tab 1 reloads from its last full-load URL. The founder decides. | |
| DX9 | Optional, dust only: `latency 20000 vela-relay` → approve → during 提交至网络 press ⌘W → relaunch | Current (W17): quits without asking. After relaunch, check the explorer and Activity: did it land, and is anything tracking it? | 👆 |
| T5 | Only on the Trusted Signer route: `blackhole sign\.getvela\.app` → Sign → 去签名页确认 | Phone parity is "签名页没能打开" with Retry and the request kept. **Desktop today (W16):** the browser's own error page, and the wallet waits up to 5 min. Record what the column shows. | 👆 if the page opens from cache |
| DX10 | Type `uniswap` in the address bar with the proxy on 1088, then with the system proxy Off | Proxy on: DuckDuckGo results. Off: the network panel and retries (W21). Note the words. Restore 1088. | |
| L6 / SC-006a | Type 10 addresses `https://vela-t1.invalid` … `vela-t10.invalid` | Each ends on 找不到这个网站 with no retry. | |
| SC-006b | Open 10 real sites (the founder's usual dApps), then open Recents | Recents holds exactly the pages that loaded, each with its own title and icon. None of the `.invalid` ones, and nothing from L2, DX2 or DX3. | |
| U7 | Recents and favourites for app.uniswap.org | Its icon; letter fallback "U". | |
| SC-007-en | Relaunch with `--env VELA_LANG=en` → bar, consent, connection panel, site menu | No **visible** "Secure site", "Insecure site — not encrypted" or "Encrypted". | |
| — | Restore the proxy to 1088 and run `chaos mode=pass` | | |

### Chrome extension

| id | do | expect | 👆 |
|---|---|---|---|
| EX0 | Freshness check (§1). Launch the throwaway profile, Load unpacked, reload the dApp tabs. | The card shows 0.9.5 and the wasm hash equals `WASM_URL`. | |
| EX1 | Toolbar icon → wallet → sign in with the passkey | Same address as the founder's wallet (rpId getvela.app). | 👆 |
| U2 | Tab A `http://127.0.0.1:8137` → Connect | The side panel opens with "连接到 127.0.0.1:8137"; Connect is filled. No 安全站点 text. | |
| U5 | Sign → header | The host once. | |
| S1 | Sign → Esc ×5, click the panel scrim ×5 | The sheet stays. The dApp has no answer. | |
| S2 | ✕ | The dApp gets 4001 once. | |
| S3 | Sign → approve **in the side panel** | The 已签名 tick shows in the panel before it returns to the wallet. The dApp gets the signature once. This is the row 079 left untested (SIDEPANEL-TEST). | 👆 |
| S4 | Send dust → approve | submitting → ring → landed tick in the panel. The dApp gets the tx hash. | 👆 |
| EX2 | With the panel open, run in the dApp console: `setTimeout(()=>ethereum.request({method:'personal_sign',params:['0x48656c6c6f',ethereum.selectedAddress]}),6000)` | Record the surface. Expected: a popup window opens (no user gesture) while the panel shows the wallet. It is answered once and closes on the answer. | 👆 |
| EX3 | Close all DevTools, idle 60 s (chrome://extensions shows "Inactive") → Chain, Block number | Both answer normally after a cold start. | |
| EX4 | Tab B `http://127.0.0.1:8138` → Connect (the panel already opened for tab A) | **Want:** the :8138 card within 1 s. **Suspected (W15):** the panel stays on the wallet. No card within 30 s confirms it; close tab B (otherwise 4900 at 300 s). | |
| EX5 | Tab A: Sign (the sheet shows) → reload tab A without deciding → Sign again → look at the panel. Approve what shows first. | **Want:** the stale sheet goes away and only the new request can be signed. **Suspected (W2):** the old sheet is first in line and approving it "succeeds" with nobody listening; the new request waits behind it. Message signing only. | 👆 |
| EX6 | Sign → close the side panel with Chrome's own ✕ | **Want:** 4900 at once. **Suspected (W15):** still pending after 60 s. | |
| EX7 ⏱ 5 min | Sign → leave it 5 min 10 s with DevTools closed (do other rows in another window) → approve | At about 300 s the dApp shows 4900. The sheet must then be gone or expired and must not sign. **Suspected (W2):** it still signs and shows the tick. | 👆 |
| EX8 | Sign → chrome://serviceworker-internals → Stop the Vela worker (DevTools closed) → watch the dApp → approve in the panel | **Want:** 4900. **Suspected (W2):** -32603 with raw "message channel closed" text, and the panel still signs an answer that is never delivered. | 👆 |
| EX9 | `blackhole ''` (everything) → on a fresh origin: Connect → Switch to Gnosis → Chain | The consent card is instant, connect succeeds, the switch returns null plus `chainChanged`, and Chain answers. All local. This stands in for "proxy dead". | |
| EX10 | `blackhole gnosischain` → Block number and Chain on Gnosis, timed | **Want:** under 3 s from the next endpoint. **Suspected (W11):** about 20 s for every call, every time. Cross-check with NetLog. | |
| EX11 | `blackhole gnosis\|1rpc` → Block number | The dApp shows `-32603 "Vela could not reach a node for chain 100: …"`. Record the time (about 60 s suspected). Vela itself shows nothing. | |
| S7 | `drop vela-relay` → Send dust → `pass` without touching | The service sentence and a shut slide. The fee appears within 15 s by itself. | |
| S8 | Fee refresh | Spinner → new fee. | |
| S5 | Send dust → approve → immediately `drop vela-relay` | A cross with a plain reason; one error to the dApp; no pending record. Check the explorer before retrying (W1). | 👆 |
| EX12 | `latency 6000 vela-relay` → Send dust → approve | submitting → ring → landed tick. The dApp's `eth_getTransactionReceipt(hash)` returns a receipt. | 👆 |
| EX13 ⏱ ~2.5 min | Panel DevTools: `vela.silentReceipt(100)` → Send dust → approve → wait more than 120 s → in the dApp console, call `eth_getTransactionReceipt` on the returned hash every 10 s | The panel says still confirming, then later landed. **Suspected (W12):** the dApp got the userOpHash and its receipt stays null even after the op lands. | 👆 |
| SC-007-en | Wallet language → English → repeat U2, the connection list and the signing header | No visible "Secure site", "Not secure" or "Encrypted". | |
| — | `chaos mode=pass`; close the throwaway Chrome | | |

### iPhone 11

| id | do | expect | 👆 |
|---|---|---|---|
| IX0 | (Claude) Freshness gate → exit 0; Debug build; `devicectl … install app` | Installed. Note the core fingerprint. | |
| IX1 | Launch with logs (§1), `VELA_PARALLEL_SPACE=0` | The founder's own wallet, no parallel badge. | |
| U3 / U4 | Explore → connection panel → network picker; account picker | Logo and balance, no zeros; identicons. | |
| U2 | Open `http://192.168.50.9:8137` → Connect | "连接到 192.168.50.9:8137"; Connect is filled. | |
| U1 | Lock on http and https pages; connection panel; site menu | A lock and 已连接 only; no 安全站点, 不安全 or 已加密. | |
| U5 | Sign → header | The host once. | |
| S1 / S2 | Sign → swipe down ×5, scrim ×5; then ✕ | The sheet stays; then 4001 once. | |
| S3 | Sign → approve | 已签名！ tick → closes itself. | 👆 |
| S4 | Send dust → approve | 提交至网络… → ring "Gnosis 通常在约 15 秒内确认" → 已确认 with short hash → closes itself. The page gets the tx hash. | 👆 |
| U6 | Two tabs → switcher | Two different snapshots. | |
| T1–T3 | Trusted-signer route only: Sign → 去签名页确认 → one slide on the page → passkey. T3: cancel the prompt. | The app shows a button, not a slide. The origin names the host, with no 未知站点. Back in the app: 已签名！ T3: "你取消了…" and the slide works again. | 👆 |
| — | Shadowrocket OFF; Wi-Fi proxy Manual `192.168.50.9:8899` | | |
| L1 (SC-003) | `latency 6000 example` → type example.com → Go, with screen recording on | Hairline within 0.5 s of Go (the first device timing). The old host stays until commit. The page arrives at about 6–7 s. | |
| L2 | `drop uniswap` → app.uniswap.org | Panel with host and 网络不稳定…. Console: `browser load failed: … → offline` (or refused). | |
| L3 / L4 | Leave the panel; then tap Retry with the fault on | 正在重试… at about 2, 5 and 10 s, then it stops (`pass` before the third → loads). L4: the panel stays for the whole attempt. | |
| L5 | `https://expired.badssl.com/` | The certificate line, no retry. | |
| IX2 | `blackhole example` → new tab → example.com. Stopwatch; stop at 2 min. | **Ideal:** words within seconds. **Code (W5):** white page with a 10% hairline and no words until WebKit gives up (about 60 s? unmeasured), then 网络不稳定. Note whether Retry is ever tappable during an attempt (disabled today). | |
| IX3 | Wi-Fi proxy Off, Shadowrocket ON with rule `DOMAIN-SUFFIX,uniswap.org,REJECT` → app.uniswap.org. Then REJECT-DROP. Then remove the rule → Retry. | The console line gives domain and code. **If -1200** → "网站证书有问题…" with no retry = the false certificate alarm (W9). REJECT-DROP should look like IX2. After the rule is removed, Retry loads. | |
| IX4 | Airplane mode ON → open an https dApp → wait 25 s → Airplane OFF → wait 30 s without touching | **Code (W5):** the panel stays until Retry is tapped. **Ideal:** it reloads by itself. | |
| IX5 | Force-quit. Launch with Shadowrocket off and turn it on within 5 s. Open the network picker, Recents and favourites. Then force-quit and relaunch with a steady network. | **Code (W20):** logos and favicons that failed stay as letters or dots all session and come back after the relaunch. | |
| — | Back to the Wi-Fi proxy `192.168.50.9:8899`, Shadowrocket OFF | | |
| C1 | Test dApp on Gnosis, `blackhole gnosis\|xdai` → Block number → tap the notice's Retry | The notice names Gnosis and the chip stays green. Retry gives no busy feedback (W11). | |
| C2 | `pass` → Block number | The notice clears without a tap, once a call reaches Gnosis. | |
| IX6 | Relaunch (so isDeployed is not cached) with `blackhole gnosis\|xdai` → Send dust. Watch the fee row for 60 s → `pass`. | **Code (W10):** 估算中… with no reason line and no chevron, slide dark. After `pass` the fee appears within about 15 s. | |
| S7 | `drop vela-relay` → Send dust → `pass` without touching | 估算失败 with the reason line, slide dark. Fee within 15 s with no tap (SC-005 on the device). | |
| S8 | Fee refresh | Spinner → new fee. | |
| S5 | Send dust → approve → immediately `drop vela-relay` | A cross with a reason; one error; no pending record. Check the explorer before retrying (W1). | 👆 |
| IX7 | Opportunistic: a dApp tx that reverts on-chain after estimation (e.g. minimum slippage on a volatile pair) | **Code (W3):** "已确认 ✓", closing after 2.6 s, while the dApp and the explorer say it failed. | 👆 |
| S6 | Optional (Arbitrum funds): Send dust on Arbitrum | After 120 s, still confirming; ✕ closes without rejecting; answered once. | 👆 |
| T5 | Trusted route, cold cache (clear Safari website data; whether that clears the SFSafariViewController cache is unverified): `drop sign\.getvela` → Sign. Then `blackhole sign\.getvela`, and close the Safari tab by hand after 10 s. | drop: the tab closes itself; "签名页没能打开…" with Retry; the request stays. blackhole: the card within about 1.2 + 5 s of closing. Note any case where Safari's own error page stays. | |
| T4 | After one successful visit: `drop sign\.getvela` → Sign | The page opens from the device cache and signs. | 👆 |
| SC-006a/b | 10 × `https://vela-tN.invalid`, then 10 real sites → Recents | Only the 10 that loaded, each with its own title and icon. | |
| U7 | Recents and favourites for app.uniswap.org | Its icon; "U" fallback. | |
| SC-007-en | Relaunch with `"VELA_LANG":"en"` → bar, consent, connection panel, site menu | No **visible** "Secure site" or "Insecure site — not encrypted". The http lock's VoiceOver label carries `connect.browser.a11yInsecure` by design (`ExploreLive.swift:376-390`), so check visible text only. | |
| IX8 | Parallel space (`VELA_PARALLEL_SPACE=1`): Send dust → fee shows → `reset_mid vela-relay` → slide. Watch the fixture Safe on the explorer for 2 min. Do it 3 times. | If the sheet says failed but the dust lands, the lost-reply gap (W1) is confirmed. No 👆: fixture keys. | |
| — | Wi-Fi proxy Off; relaunch with `VELA_PARALLEL_SPACE=0` | Back on the founder's wallet. | |
| (Claude) | Probe run: build-for-testing → copy the `.xctestrun` → remove `DappBrowserStabilityProbeTests` from SkipTestIdentifiers → `test-without-building -only-testing:…` → `xcresulttool export attachments` | Both probe tests pass, with PNG and .txt pairs. Needs UI Automation ON and a trusted developer certificate on the phone. | |

---

## 3. Weak spots to watch

Ranked by user impact: money safety first, then frozen or misleading loads for users behind proxies, then signing blockers, then polish.

| # | Weak spot | Platforms | Where | Impact | Confidence | Rows |
|---|---|---|---|---|---|---|
| W1 | **Lost submit reply is shown as "failed — try again".** The pool re-POSTs `eth_sendUserOperation` on timeout. Any pool failure becomes Unreachable → "The gas relayer could not be reached. Please try again." There is no pending record and no poll by the (locally computable) userOpHash. Only an `[existingHash:]` reply is recovered. | Desktop, iOS (Extension not examined) | desktop `executor/relay.rs:597-599` ✔, `executor/user_op.rs:726-729` ✔, `executor/chain.rs:37` (10 s nonce TTL); iOS `Core/UserOpSpine.swift:429-430`, `Features/Signing/Core/SignExecutor.swift:256-257`; core `app/rpc_pool.rs:150-163, 1745-1749` | A payment that lands is reported as failed; retrying can execute it twice. Breaks "a timeout is not a failure" at the submit step. | likely (code path verified; relay acceptance not observed) | D S5/DX6, I S5/IX8, E S5 |
| W2 | **Extension: a request outlives its asker.** A reload or navigation settles nothing (only `windows.onRemoved` and `tabs.onRemoved` exist). The 300 s timeout is enforced only in content.js. A stopped SW loses `pending`, and the restart sweeps every `vela.req.*`. The panel sheet stays signable and `delivered:false` is ignored. | Extension | `extension/background.js:137, 253-277 ✔, 289-298 ✔`; `extension/content.js:50-63` ✔ (4900 = `UNKNOWN_PENDING`; thrown → -32603 raw text); `src/lib/dapp/DappRequestHost.svelte:212-223` | A person who reloads a stuck dApp approves both the old and new request: a double swap or send. The dApp never hears about the first. | verified-in-code (SW-stop timing likely) | EX5, EX7, EX8 |
| W3 | **iOS shows a reverted dApp tx as "已确认 ✓".** `awaitReceipt` matches `.resolved(_, txHash, _, _)` and drops `confirmed:false`. Aftercare turns any non-op hash into `.landed`. | iOS (desktop shows a revert sentence per results.md; Android `SigningAftercare.kt:41` has the same shape, unverified) | `Features/Signing/Core/SignExecutor.swift:327-328` ✔; `Core/RelayClient.swift:551-556` ✔; `Features/Signing/SigningAftercare.swift:54-58` ✔; `SigningLive.swift:476-480, 505-506` | A failed swap (slippage is likelier on slow networks) reads as success, and because of D3 nothing ever corrects it. | verified-in-code | IX7 |
| W4 | **Relay status is never read (D2).** Every client calls `eth_getUserOperationStatus`; vela-relay serves only `pimlico_getUserOperationStatus`. A relay-rejected op shows "still confirming, don't send it again" for 24 h, and the relay's broadcast tx hash is never shown. | All | desktop `executor/relay.rs:725-741` ✔; iOS `Core/RelayClient.swift:571-578` ✔; Android `feature/send/core/RelayClient.kt:466-470` ✔; web `services/tx-reconciler.ts:97`, `rpc-adapter.ts:28` ✔; relay `vela-relay-core/src/wire.rs:205,219` ✔ | A dishonest "still on its way" for a dead op; the person waits a day. | verified (code + reader's live probe) | S6, DX7 |
| W5 | **iOS: no load watchdog and no recovery.** `URLRequest(url:)` has no timeout (default about 60 s, unmeasured). There is no Stop, Retry is disabled while each attempt hangs, and there is no NWPathMonitor, so a page failed after 3 tries stays failed when Wi-Fi or the VPN returns. | iOS | `Features/Explore/Core/BrowserEngine.swift:179, 208, 229, 714` ✔; `Components/Explore/BrowserWebView.swift:82-83` ✔ (`enabled: !retrying`, also breaks busy≠disabled); compare desktop `explore/load_watch.rs:35-39` | The commonest China failure (a hanging proxy node) looks like a frozen browser for about a minute with no words. | verified-in-code; timing unverified | IX2, L4, IX4 |
| W6 | **Desktop judges page loads by a ureq HEAD over the wallet's route, not WebKit's.** The wallet falls back to Direct, ignores PAC (`mode auto` unhandled) and trusts only webpki roots; WebKit follows system proxy, PAC and keychain. A dead leftover proxy gives about 20 s of hairline and then a generic "network" reason while balances load. PAC users get false panels and self-inflicted reloads. | Desktop | `explore/load_watch.rs:336-358` ✔ (Ok → wait until `GIVE_UP` 20 s, :177-190 ✔); `executor/proxy.rs:77-80` ✔, `309-333` ✔, `472-513`; `webview.rs:442-476` | Slow and misleading failures exactly for users with broken or PAC proxies. | likely | DX3, L2, DX2 |
| W7 | **Desktop auto-retry restarts loads WebKit is still making.** The 5 s probe budget → Timeout → panel at about 8 s → `navigate()` 2 s later cancels the provisional load. Windows of about 10, 13 and 18 s. wry 0.56 exposes the WKWebView (reader: `WebViewExtMacOS::webview()`), so `isLoading` or `estimatedProgress` could tell the watchdog WebKit is still working; that is unused. | Desktop | `explore/load_watch.rs:35-39 ✔, 225-260 ✔, 357 ✔`; `wallet/page.rs:13228-13270`; `webview.rs:256-265` | On a slow but working proxy (6–10 s to first byte), heavy dApps flash "check your network" and reload themselves. | verified-in-code | DX1 |
| W8 | **Desktop flips the route process-wide on any transport error, timeouts included.** A global index moves System→Direct for every caller for up to 60 s. A second concurrent failure drops the list without trying Direct. Each failing POST pays its timeout once per candidate (RPC 8→16 s, bundler 15→30 s). One slow probe can push all RPC and relay traffic onto a GFW-blocked Direct route. | Desktop | `executor/proxy.rs:219-233` ✔ (`advance`), `262-271` ✔ (`is_transport`), `309-333` ✔; callers `pool.rs:733`, `relay.rs`, `load_watch.rs:342` | Fee quotes, dApp reads and receipt polls fail or double in time for reasons unrelated to the chain or relay. It also makes chaos faults unreliable on desktop. | verified-in-code | DX1 (chaos.log), C1, S7 |
| W9 | **A TLS handshake reset (-1200) is classed as Certificate.** It gets "网站证书有问题，Vela 已阻止打开。" and is never retried, which is exactly what a failing Shadowrocket node in TUN mode produces. Android's `-11 ERR_FAILED_SSL_HANDSHAKE` → Certificate (`:107`) has the same shape. On desktop, a probe TLS error → Certificate (a TLS-intercepting proxy or antivirus). | iOS (and Android, Desktop) | `rust/crates/vela-core/src/app/browser_load.rs:129` ✔, `:185` ✔ (no retry); `i18n/locales/zh/explore.json:64` | A false security alarm on a legitimate dApp, with no automatic recovery. | likely (the -1200 on Shadowrocket is unverified) | IX3 |
| W10 | **iOS fee row stays on 估算中… with no reason when the chain cannot say if the account is deployed.** It does re-ask on the core schedule, but the reason line exists only for `fee.failed`. A refresh tap starts a second loop without cancelling the first. | iOS | `Features/Signing/Core/SigningController.swift:386-401` ✔, `432-438` ✔; `Core/RelayClient.swift:677-688`; `SigningLive.swift:1014-1018, 1047-1054` | Reads as a hung wallet whenever public RPCs are blocked. | verified-in-code | IX6 |
| W11 | **A chain outage surfaces slowly and its Retry looks dead.** Desktop: the notice appears only after the pool concludes (3 passes × endpoints × 8 s, doubled by candidates), and Retry has no busy state. iOS: Retry fires one `eth_blockNumber` with no feedback, and the notice clears only on a later good call. Extension: no notice at all; reads walk a fixed list at 20 s per endpoint with no ban memory. | All | desktop `wallet/browser_host.rs:215-226, 340-358, 368-382`; iOS `App/RootView.swift:1553-1556`, `ExploreScreen.swift:203-211`, `Core/RpcPool.swift:122-125`; ext `extension/background.js:62 ✔, 425-460 ✔`, `src/lib/dapp/core/ext-chains.ts:12-14, 48-54` | The dApp spins 20–60 s or more per read before anything is said. | verified-in-code | C1/C2, EX10, EX11 |
| W12 | **Extension: the userOpHash is handed to the dApp after 120 s, but the dApp's receipt polls go to public nodes that don't know it.** The translation in `handleReadOnlyRPC` has no caller, and window mode closes on answer so nothing tracks the op. (The in-app browsers do resolve userOp hashes: desktop `executor/dapp_browser.rs` ResolveUserOp, iOS `DbrExecutor.swift`.) | Extension | `src/lib/services/safe-transaction.ts:3391-3395`; `src/lib/signing/core/sign-executor.ts:253-262`; core `app/sign_request.rs:2191-2199`; `extension/lib/protocol.js:122-143`; `src/lib/services/dapp-submit.ts:987-1008`; `extension/background.js:166-168` | On a slow relay the dApp says failed while the tx lands; resubmission risk. | verified-in-code | EX13 |
| W13 | **dApp transactions never appear in Activity (D3).** A pending, still-confirming or failed dApp tx is visible nowhere once the sheet closes. | All | core `app/activity_feed.rs:633-640, 811-832`; `tests/app_activity_feed.rs:265-298` | Makes W1, W3 and W4 invisible after the fact. | verified-in-code | — |
| W14 | **Desktop tab switch is a full reload of the single webview** and retires the old document, so an open sign or consent request gets 4900 and closes. | Desktop | `wallet/page.rs:12213-12231` ✔; core `app/dapp_browser.rs:926-970` | Lost dApp state on every switch; a glance at another tab cancels a pending signature. | verified-in-code | DX8 |
| W15 | **Extension side panel is tied to its first tab and to its own life.** The panel caches its first tab (`tabId ??=`), so a second tab's request never shows. Chrome's panel ✕ settles only through async work during pagehide. | Extension | `src/lib/dapp/DappRequestHost.svelte:130` ✔, `188-201, 262-277`; `extension/background.js:234-243` | The second dApp tab spins for 300 s; closing the panel leaves the dApp hanging. | likely | EX4, EX6 |
| W16 | **Desktop Trusted Signer has no "page could not open" state.** The browser shows its own error and the wallet waits up to 5 min. | Desktop | `executor/trusted_signer.rs:80`; no `signerDown` in app-desktop ✔ (phones: Android `SigningLive.kt:101`, iOS `TrustedSignerSheets.swift:57`) | A 5-minute silent wait when getvela.app is blocked. | verified-in-code | T5 (desktop) |
| W17 | **Closing the desktop window during a submit quits the app** (`QuitMode::LastWindowClosed`, no close guard). | Desktop | `src/main.rs:344` ✔ | Money can move with no record or tracking. | verified-in-code | DX9 |
| W18 | **Desktop loads the wallet did not start are invisible:** in-page links, redirects, and Back/Forward (which are `history.back()`/`forward()` JS). They get no hairline and no panel. 079 accepted this, but it is the most common navigation inside a dApp. | Desktop | `webview.rs:256-285` | A click on a bad network looks like a dead button. | verified-in-code | DX4 |
| W19 | **Simulation wording (D5).** "Could not simulate … reject it" appears as a danger for unsupported and unreachable alike, while a real revert reads "no change". Arbitrum's `-32603` on `eth_simulateV1` is treated as transient and can add 42161 to `failed_chains`. | Desktop, iOS, Android | desktop `executor/sim.rs:40-47, 91-94`; iOS `SigningController.swift:347-373`, `SimDeltas.swift:76-97`, `SigningLive.swift:609-614`; core `rpc_pool.rs:627-645, 1638-1645` | Trains people to ignore the warning; a healthy chain may be flagged down. | verified-in-code (+ live probe) | — |
| W20 | **iOS logo and favicon failures are remembered for the whole session.** | iOS | `Components/Wallet/RemoteLogoView.swift:27, 46, 52` ✔ | 079's F9 and F16 visuals vanish after a flaky start. | verified-in-code | IX5 |
| W21 | **Address-bar search goes to duckduckgo.com**, which is blocked in mainland China without a proxy. | All | core `app/dapp_rpc.rs:486` ✔ | "搜索 uniswap" produces "check your network". | likely | DX10 |
| W22 | **Desktop hairline stays until `didFinish`** while blocked Google fonts or analytics hang. | Desktop | `explore/load_watch.rs:271-273`; `wallet/page.rs:12755` | Cosmetic; reads as a slow wallet. | likely | DX5 |
| W23 | **The paths under test log nothing.** Desktop has no log crate; stderr only. iOS uses `print()` only, invisible to syslog and the bug report; one browser line at `BrowserEngine.swift:624`. The extension SW has one `console.error` at `background.js:107`. | All | as cited | Field reports from China carry no trace. | verified-in-code | — |
| W24 | **Test coverage misses the proxy shapes.** The iPhone probe only uses a loopback refusal (`DappBrowserStabilityProbeTests.swift:188, 202-203`); chaos-proxy has no "CONNECT 200 then close" mode (`:101-143`); the extension panel helper is broken (SIDEPANEL-TEST). | iOS, Extension | as cited | The failure shapes that matter most for proxy users have the least evidence. | verified-in-code | IX2, IX3, S3 (ext) |
| W25 | **Speculative iOS tail.** (a) The about:blank fix depends on `webView.url` at provisional start; `load()` never records the requested URL (`BrowserEngine.swift:449-462, 497`). (b) A committed page whose JS bundle was cut renders white and is still recorded as a visit (`:494-520, 563-570`). (c) Safari's own error page can remain in the trusted-signer tab when the HEAD probe succeeds (`TrustedSigner.swift:397-404`). | iOS | as cited | Possible white pages; possible false "签名页没能打开". | speculative | IX2, T5 |

**Where the readers disagree or drifted:**
- **Build freshness.** The extension reader's "dist is pre-079" and the iOS reader's "gate exit 1" no longer hold: other sessions rebuilt both (see the header). Re-check right before the pass.
- **Desktop failure timing.** The iOS reader says desktop "answers in 3–8 s". The desktop reader shows that holds only when the probe fails through the proxy. With Direct fallback it is about 20 s and a wrong reason (W6). Both are in DX3 and L2.
- **The iOS "about 60 s".** This is the `URLRequest` default idle timeout. 079 found that behind a proxy WebKit reports no failure at all, only its own about:blank, and when that blank arrives has never been measured. IX2 measures it.
- **D4.** The D4 reader says the memory note ("fix belongs in vela-bundler") is out of date: the $0.01 floor is in core `fee_policy.rs:811-845, 866-868`.

---

## 4. 079 leftovers into 082

**i18n headroom:** ja+en residency is at 138,750 of 138,800 bytes (results.md).

| Leftover | Root cause | Fix location | Proposed fix | i18n impact | Doable here? |
|---|---|---|---|---|---|
| **HOST-TWICE** | Core always sets `context.dapp.name = host` (`trusted_signer.rs:343-353`). The page's `baseView` takes name and origin from the same host, and `render.js:489-490` draws both with no equality check. The page never got the F14 rule. | `app-web/trusted-signer/src/lib/resolve.js` (baseView), `render.js:490`; then `rust/crates/vela-core/src/trusted_signer/integrity.rs` (`BUILD_ALLOWED`, `LAUNCH`) | Add `originShown: !!host && host !== name` in resolve.js (render stays free of judgements). Keep `view.dapp.origin` for the claimed-origin warning (`resolve.js:1364`). Add two sample tests. Release in order: `bun samples/build-single.mjs` → new hash first in `BUILD_ALLOWED` → owner deploys from main (every published version) → `curl -I …/b/<new>/sign` gives 200 immutable → move `LAUNCH` → new app builds. Re-run the hostile, takeover, fee-leg and unlimited suites. | none (the page has its own locales) | Code yes; the deploy is the owner's. Old builds keep `ec038e11`. |
| **D3** | `activity_feed::accept()` keeps only Send/Receive (`:633-640`) and `build_items` likewise (`:811-832`). A test pins it (`tests/app_activity_feed.rs:265-298`). The rule came from the Expo era, whose Connections list was retired in 039. Records are written by all four shells and tracked. | core `app/activity_feed.rs` (+ `FeedItem` `:217-246`); thin row-kind edits in web `lib/wallet/live.ts:430`, Android `WalletLive.kt:165`, iOS `WalletLive.swift:399` | Accept `DappTx` where `from == me`. Add `dapp_item()`: hex wei → decimal via `fee_policy::from_base_units`; zero value → no amount; empty `to` → no counterparty. Add an additive `#[serde(default)] kind` (and ideally `status`) to `FeedItem`. Keep signatures and connects out. Update the test. Regenerate ts-rs bindings and wasm. | none (`history.txLabelDappTx` exists) | Yes, after a founder ruling (Q1). |
| **D5** | Each native shell parses `eth_simulateV1` itself and reduces unsupported, unreachable and refused to "unavailable" (Danger on iOS and desktop, Caution on Android). A revert (status 0x0) is skipped and reads "no change". Arbitrum answers `-32603 "method handler crashed"`, which the pool treats as transient and may use to add 42161 to `failed_chains`. | New core `app/sim_outcome.rs` (or `token_trust.rs`) + `app/rpc_pool.rs`; three shells' sim parsers and `SigningLive`/`live.rs`; corpus `componentsUi.signing.simUnavailableWarning` | A core classifier: Deltas / Reverts{reason} / NotOffered / Unreachable. Reverts → Danger with `simWillFail(Reason)`; the other two → Caution with neutral wording. Scope `eth_simulateV1` errors in rpc_pool so they never mark a chain failed. Update the pinned tests (`SimulationSheetTests.swift:230`, `SigningLiveTest.kt:301`). | Reword 1 key × 15 locales, shorter (en 151 → about 85 B). This **frees** residency. Reuse `simWillFail`/`simWillFailReason`. | Yes (medium risk; device check on Arbitrum and Gnosis) |
| **D6** | The shared `provider/inpage.js` `applyAccounts` lower-cases every address (`:162-170`) while the request promise resolves the raw, checksummed result (`:245-247`). | `rust/crates/vela-core/provider/inpage.js`; `app/dapp_browser.rs`, `app/dapp_permissions.rs`; `app-web/vela-wallet/e2e/extension-live-provider.e2e.ts:305-311` | Keep the wallet's spelling and compare case-insensitively for the change check. Apply `primitives::checksum_address` (`primitives.rs:96`) at every grant write and address emit. Update the e2e. | none | Yes (low risk) |
| **D7** | The web, iOS and Android live builders copy the fixture's `history.emptyFilter` and ignore the chain filter. Desktop switches correctly (`flows/live.rs:208-226`, test `:4925-4947`). | web `lib/flows/live.ts:166-180` + `messages.ts:58`; iOS `Features/Flows/FlowsLive.swift:183`; Android `feature/flows/FlowLive.kt:186-190` + `I18nKeys.kt` ~488 | All networks → `history.emptyTitle`; one network → `history.emptyFilter`. One unit test per shell, modelled on desktop's. Optionally add desktop's "first read landed" guard on iOS and Android. | 0 bytes (reuse `history.emptyTitle`). `home.emptyNoActivityNetwork` has no consumer and could free about 90 B. | Yes (low risk) |
| **D4** | Core in-band fee = `max(gas × price × 3, native_minimum)`, where `native_minimum` = $0.01 of the native coin. The relay prices xDAI at $1, so the floor is always 0.01 xDAI. The relay's own native admission floor is 0.00001 coin. | core `app/fee_policy.rs:97, 811-845, 866-868` (+ vectors); the relay needs no change for native coins | A founder pricing decision (Q2): keep the floor and explain it, or for **native** fee coins drop to the relay floor (e.g. `max(3 × real gas, 0.00001)`). Stablecoins stay at 0.01 token (relay `admission.rs:409-416`). Never frame the fee as a multiple of the amount. | none to change the floor; an explanation string (about 40–60 B) exceeds the 50 B headroom | Yes, after Q2 |
| **D2** | (a) Arbitrum ops accepted but never mined: relay executor, other repo. (b) The wallet polls `eth_getUserOperationStatus`, which the relay does not serve (W4). The e2e stub (`e2e/stub-chain.ts:301`) and `RelayClientTest.kt:217` answer the wrong name. | (b) desktop `executor/relay.rs:725-741`, iOS `Core/RelayClient.swift:571-578`, Android `RelayClient.kt:466-470`, web `tx-reconciler.ts:97`/`rpc-adapter.ts:28` → one core constant in `tx_tracker.rs`; relay `vela-relay-core/src/wire.rs:196-208` for an alias | Rename to `pimlico_getUserOperationStatus` via a core constant; fix the stub and the test. Ask the relay owner for an `eth_` alias (fixes shipped builds). Optionally carry the relay's `transactionHash` into still-confirming as an explorer link. Device-check the new "rejected" ending. | none | (b) yes; (a) no (relay repo) |
| **SIDEPANEL-TEST** | Since 077 the panel is `<locale>/wallet.html?panel` (`extension/panel.js:17-20`). The helper still looks for `/request.html` with no search string and reads `h1` (`e2e/extension-helpers.ts:106-145`). The test expects the panel to close (`extension-live-provider.e2e.ts:339-341`). | `e2e/extension-helpers.ts`; `e2e/extension-live-provider.e2e.ts:316-341` | Filter on `pathname.endsWith('/wallet.html') && search.has('panel')` and read the dialog's `aria-label`/`h2`. The panel stays after answering. Add a signed-tick test with a MutationObserver on `.landing-over` (the tick lasts 1.4 s). Manual stand-in this pass: Extension S3. | none | Yes (test-only). Whether fixture keys sign inside the panel without a CDP authenticator is unverified. |
| **SC-003 iOS timing** | Unit-tested only (`progressShowsFromTheRequest`); never timed on the device. | This pass | iPhone L1 with a screen recording; count frames from Go to the hairline. Desktop DX1 covers the desktop side. | none | Yes (founder pass) |
| **SC-006 10+10** | Android ran fewer loads; iOS and desktop have none. | This pass | SC-006a/b rows on all three (10 × `.invalid` + 10 real sites, then read Recents). | none | Yes |
| **SC-007 en sweep** | Not run. | This pass | SC-007-en rows. Note: `explore.secureSite` and `connect.browser.a11yInsecure` are still in the corpus, and iOS uses the latter as the http lock's screen-reader label by design (`ExploreLive.swift:376-390`), so sweep **visible** text only. | none | Yes |
| **T004 iOS before-probe** | Never ran (UI Automation was off). It cannot be a "before" any more because main has the fixes. | — | Retire it. Replace with the after-probe (Claude row in the iPhone table) plus the proxy-shaped rows the probe never covered (IX2, IX3). Optionally add a "CONNECT 200 then close" mode to `scripts/device/chaos-proxy.py` so the Shadowrocket TUN shape can be automated. | none | Partly (retire = doc; probe = Claude with UI Automation ON) |

---

## 5. Open questions for the founder

1. **D3.** Should on-chain dApp transactions (only those, not signatures or connections) appear in Activity, with a pending or failed status on the row? This reverses the Expo-era rule. Without it, W1, W3 and W4 are invisible once the sheet closes.
2. **D4.** Keep the $0.01 fee floor for native fee coins (revenue and anti-spam), or drop it to the relay's admission floor (0.00001 native)? That would change the quotes on Gnosis, Arc, Stable and every other cheap-native chain.
3. **Lost submit reply (W1).** When the POST went out but no reply came back, should the sheet say "may have been sent — Vela keeps checking, don't send again" and track the locally computed userOpHash, instead of "failed, try again"? That needs a new sentence, and there is 50 B of i18n headroom. Choose between raising the 138,800 cap or trimming first (D5 rewording, the unused `home.emptyNoActivityNetwork`).
4. **Desktop proxy policy (W6, W8).** When the system proxy fails, should the desktop wallet keep silently falling back to Direct? It is more resilient, but it disagrees with what WebKit does, bypasses the proxy the user chose, and moves every caller on one timeout. And should PAC be supported? The answer decides how desktop page-load failures get judged.
5. **Desktop tabs (W14).** With one shared webview, switching tabs reloads the page and cancels an open signature. Accept that, block tab switching while a request is open, or move to one webview per tab (more memory)?