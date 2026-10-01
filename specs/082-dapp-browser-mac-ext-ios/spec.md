# Feature Specification: The dApp browser holds up on the Mac, in Chrome and on the iPhone — and 079's leftovers close

**Feature Branch**: `082-dapp-browser-mac-ext-ios` (worktree `vela-wallet-082`, off `main` de93634f)

**Created**: 2026-09-28

**Status**: Draft

**Input**: Owner, 2026-09-28:

> 在当前mac / mac chrome （装chrome 扩展钱包 vela wallet） 以及连接的iphone 设备 上安装最新的vela wallet , 然后 来测试 dapp browser ，遇到需要我按指纹的地方，提醒我，我来帮助你， 你要看界面视觉，文案，UI/UX, 以及mac日志 chrome 扩展日志 ，ios日志，等等，来判断是否需要优化， 目标是要达到很好的体验，在一个不稳定的环境里构建稳定的用户体验
>
> 以及 079的几个遗留问题也放到 082 中去做

079 ran this pass on the Android phone and fixed what it found on every client. 082 runs the
same pass by hand on the three clients 079 covered mostly by code audit, unit tests and a
scripted probe: the **Mac desktop app**, the **Chrome extension in the owner's own Chrome**, and
the **iPhone 11 ("ABC")**. All three run the latest `main`. The owner is at the keyboard and
confirms every passkey prompt (Touch ID on the Mac, Face ID or passcode on the iPhone). Each
client's own log is recorded while the owner uses it. Faults are made on purpose with the
fault proxy 079 built (latency, refused connection, black hole, cut tunnels, per host) and by
turning the system proxy on and off. Screenshots and log excerpts go in `evidence/`.

## Rulings the owner gave during the pass (2026-09-28)

1. **A submit whose reply was lost** (the operation went to the relay, the answer never came
   back, so it may be on chain): the sheet no longer says "failed, try again". It says the
   operation may have been sent, Vela keeps checking, do not send it again — and tracks the
   locally computed operation hash to its end. (Weak spot W1 in `device-pass-plan.md`.)
2. **L-D4 fee floor**: no change in 082. The owner, verbatim:
   > gnosis arc 的 native coin 就是稳定币啊，那是因为中继没更新，没人出来吧

   On Gnosis and Arc the native coin is a stablecoin, so 0.01 xDAI is the stablecoin floor;
   the rest is the relay's to update. Recorded as a hand-off.
3. **Desktop proxy policy**: the wallet's own traffic follows the system proxy (including
   PAC), as WebKit does. No silent fall-back to a direct connection, no process-wide route
   switch on one timeout. When the proxy fails, say so. (W6, W8.)
4. **Desktop tabs while a request is open**: switching away is held while a connect or
   signing request is open, so the request is never cancelled by a glance at another tab.
   (W14.)
6. **Fault injection touches only the app under test**:
   > 不能影响电脑 手机 其他应用的流量，只能影响这个要测的app 的流量

   No system proxy is changed on the Mac or the iPhone. The extension's rows run in a separate
   browser instance with its own proxy; the desktop (dev-fixtures builds) and iPhone (Debug
   builds) get a dev-only `VELA_DEV_PROXY=<host>:<port>` launch switch that routes that app's web
   and wallet traffic — and nothing else — through the fault proxy.
7. **L-D3 is in scope** (the owner's instruction to carry 079's leftovers into 082): an
   on-chain dApp transaction appears in Activity with its status.

8. **A relay-independent landing check is in 082** (plan Q1): while an operation may have been
   sent and the relay is silent, the tracker also looks for the operation's own event on chain
   (EntryPoint `UserOperationEvent` by the locally computed operation hash), so "may have been
   sent" resolves without the relay and the last double-payment path closes.
9. **A dApp transaction that reverts inside the wait answers the page with its tx hash** on all
   four clients (plan Q2); the sheet and Activity say it failed, with the explorer link.
10. **The desktop says why a tab switch is held**: "请先完成或取消这个请求" in the address bar
    while a connect or signing request is open (plan Q3; `explore.requestOpen`).

**Superseded on 2026-10-01** (the owner, for the merge with `main`, "用 main 的规则"):
ruling 1's answer to the page and ruling 9 both follow 083 now. A transaction is never answered
with its op hash: the page waits for the chain, and is told -32603 "not confirmed yet" when the
shell's wait ends first. A revert is answered -32603 "The transaction was included but reverted
(tx)". Ruling 1's sheet words ("可能已经发出，请不要重复发送"), the write-ahead and the tracker
stand.

## What 082 starts from: 079's leftovers

These are recorded in `specs/079-android-dapp-browser-stability/results.md` ("Not done" and
"Hand-off to the owner"). They are in scope here.

| Id | Leftover | Where it shows |
|---|---|---|
| L-HOST | The signing page shows the host twice when the site's name is its host ("127.0.0.1:8137" over "127.0.0.1:8137"). The apps fixed this in 079 (F14); the page did not | trusted signing page |
| L-D3 | A transaction a dApp asked for never appears in Activity, on any client | all clients |
| L-D5 | "无法模拟这笔交易…" is shown as a danger when the chain's public node simply cannot simulate — the same look as "this transaction will fail" | all clients |
| L-D6 | The account-changed event gives the dApp a lower-case address, while the connect answer gives the checksummed one — the same account looks like two to a strict dApp | all clients |
| L-D7 | History on "全部网络" with no transactions reads "此网络暂无交易" ("no transactions on this network") | all clients |
| L-D4 | A plain Gnosis transfer is quoted ~0.01 xDAI in fees. The fee service is another repo; the wallet-side part (what the person is shown, and when) is in scope | all clients |
| L-D2 | Arbitrum in-band operations are accepted by the relay and never mined. The relay is another repo; the wallet-side part (telling the person honestly, never marking a failure on time alone) is in scope | all clients |
| L-PANEL | In the extension's side panel, the "signed" tick is not covered by an automated check (the check's helper looks for a page the panel no longer uses) | extension |
| L-SC | 079 rows that did not run: progress within 0.5 s timed on the iPhone (SC-003); Recents after 10 failed + 10 good loads (SC-006); the no-"safe site"-words sweep in English (SC-007); the iPhone before-probe (T004) | iPhone, all clients |

## What the pass found

Filled in as the pass runs, in the order a person meets each finding. Each finding gets an id
(G1, G2, …), a one-line description, the client(s), the evidence file, and the owner's ruling
when one is given. The client matrix at the end of `results.md` records, per finding, whether
each client is fixed, already right, or has no such surface.

Evidence names refer to `evidence/<client>/<name>.jpg` (screenshots, resized) and the log excerpts beside them; `evidence/chaos-proxy.log` holds every connection the fault proxy saw.

| Id | Sev | Client | Finding | Evidence |
|---|---|---|---|---|
| G3 | **P0** | Desktop | **⌘V, Backspace, Delete or ⌘X in the Explore address bar aborts the app.** The bar's key handler runs the shared editor inside the page's own update (`cx.listener`); the editor reports the edit through `address_change`, which updates the page again → gpui "cannot update WalletPage while it is already being updated" → panic inside AppKit's key dispatch, which cannot unwind → SIGABRT. Introduced 2026-09-27 by 92ef667d (078 shared editor); on `main`, in no release yet. Reproduced 2 of 2 on the owner's own app and the parallel copy; fixed on this branch and re-run (paste, Backspace, Enter: no crash, the page loads) | `evidence/desktop/crash-paste-address.txt`, `p07-address-focused.png`, `p14-backspace.png`, `p16-testdapp.png` |
| G13 | **P1** | All four | **Operation status is never read.** Every client asks the relay `eth_getUserOperationStatus`; the relay answers `-32601 method not found` and serves `pimlico_getUserOperationStatus` (probed live 2026-09-28). So `tx_tracker` always gets "status unavailable": a relay-rejected operation never ends and reads "still confirming — don't send it again" for 24 h, and the relay's broadcast hash for a submitted op (079 D2) is never shown | live probe in `evidence/relay-status-probe.txt` |
| G17 | **P0** | Extension | **A request outlives the page that asked.** Sign → reload the dApp without deciding → Sign again: the side panel still shows the first (orphaned) request; approving it signs for nobody, and only a second approval answers the page. With a send or a swap that is two payments for one intent. (Weak spot W2; row EX5, parallel space, Chrome for Testing 151.) | `ext-e09-after-reload-sign.png`, `ext-e10-second-sheet.png`, `logs/ext-cdp.jsonl` |
| G21 | **P0** | Extension, Desktop, iPhone (all three measured) | **A payment that landed is reported as failed, with "try again".** With the relay's replies lost (new chaos mode `mute`: the request arrives, the answer never comes back), a dust send landed on Gnosis at 15:22:50 (nonce 22, `0xc6f3544f…`), 48 s after the slide; 42 s *after* it landed the panel said "Failed — The transaction couldn't be submitted. Your funds are safe — please try again." and the dApp got `-32603 All bundler endpoints failed`. Following the advice pays twice. Owner ruling 1 covers the fix. **Desktop** (same fault, 15:56): landed at block 48479132 (nonce 41, `0xa6180e26…`) while the sheet said "提交至网络…", then "失败 — 交易未能提交。您的资金安全无虞——请重试。"; stderr `submit failed: The gas relayer could not be reached. Please try again.` **iPhone** (16:03): landed at block 48479213 (nonce 42, `0x31ff9297…`) while the sheet said "提交至网络…" for 50 s, then "失败 · 交易未能提交。您的资金安全无虞——请重试。" | `evidence/extension/w1-lost-reply-landed.txt`, `ext-e18-mute-{15,40,90}s.png`, `strip-w1-desktop.png`, `d35-crop.png`, `ios-i32-w1-{10,50,95}s.png` |
| G29 | **P0** | Desktop | **The first failed page of a new tab aborts the app.** A fresh (or restored, G2) tab whose first navigation fails before commit — here a refused CONNECT through the proxy — leaves WKWebView's `URL` nil and no `about:blank`; the toolbar's `host()` / `current_url()` fall back to `wry::WebView::url()`, which unwraps that nil inside a draw → `panic … wry-0.56.1/src/wkwebview/mod.rs:1370` → "failed to initiate panic" → abort. Fixed on this branch (`webview.rs` asks WebKit for nil first) and re-run: no crash, the failure panel with the host and Retry at 5 s, retries at +3/+6/+9 s | `logs/par3.err`, `strip-G29.png` |
| G30 | P2 | Desktop | After a failed load in a fresh tab the address bar is empty (placeholder), so the page no longer says which address failed; the tab keeps the previous site's title (G2) over the failure panel | `strip-G29.png`, `d23-crop.png` |
| G31 | P3 (low confidence) | iPhone | Through the per-app dev proxy, a refused CONNECT surfaces as `NSURLErrorDomain -1000` → class `not_found` → "找不到这个网站，请检查网址。" with no automatic retry, although the site exists. The phone's own network gave `-1004 → refused` / `-1005 → offline` for the same shape (SC-006), so this may be the dev-proxy path only; worth a look at how `-1000` is classed | `ios-i30-L2-*.png`, console `browser load failed … -1000 → not_found` |
| G22 | P2 | Extension, Desktop | While the relay is not answering, the sheet says "Waiting for biometric..." for 40 s — in the parallel space no biometric is asked at all; it is the network wait. The status names the wrong cause | `ext-e18-mute-15s.png` |
| G19 | **P0** | Extension | **A signature is produced after the page was already told it failed.** Sign → Chrome stops the extension's worker (MV3 does this on its own; forced here with CDP `Target.closeTarget`) → the dApp at once gets `-32603` with Chrome's raw text "A listener indicated an asynchronous response by returning true, but the message channel closed before a response was received", while the side panel keeps the sheet, lets the person approve, and shows "Signed!". For a send the panel would submit the operation while the dApp believes it failed and offers to try again. Verified with a message; the send path is the same code (row EX8) | `ext-e12-signed-for-nobody.png` |
| G23 | P1 | Extension | **After the worker restarts, new requests never show.** Following G19 (worker stopped mid-request), reload the dApp → Connect → Sign: the side panel shows nothing, reloading the panel shows nothing, the dApp waits for its 300 s timeout. The worker's storage still holds three `vela.req.*` entries from page instances that no longer exist, and none for the new one. Reached here by stopping the worker through CDP; whether Chrome's own idle stop reaches it is to be confirmed in the fix | `ext-e19-panel-after-reload.png`, worker storage dump in `logs/ext-cdp.jsonl` session |
| G18 | P1 | Extension | A second tab's request never reaches the side panel: with the panel open for tab A, tab B (`:8138`) → Connect shows nothing in the panel and opens no window; tab B spins until its 300 s timeout. The panel keeps the first tab it served (`tabId ??=`, `DappRequestHost.svelte:130`) (W15, row EX4) | `logs/ext-cdp.jsonl` |
| G16 | **P1** | Extension (+ web) | **The consent card in the side panel is unstyled**: Cancel and Connect are 17 px tall with no padding, Cancel has no button look, Connect's text is dark on orange, and the raw method name `eth_requestAccounts` shows under the body. `DappRequestHost.svelte` uses 15 CSS tokens that do not exist (`--space-4`, `--color-accent-on`, `--color-bg-elevated`, `--font-size-body`, …); `SigningBody.svelte` uses 3 more (`--color-text-primary`, `--font-size-body`, `--font-weight-semibold`). No test catches an undefined token | `ext-e03-panel-consent.png`, `ext-e04-consent-zoom.png` |
| G14 | **P1** | Desktop, Extension (iPhone right) | A dApp's plain value transfer (`eth_sendTransaction {to, value: 0.001, no data}`) to an address that has code (here another Safe) is drawn as an undecodable contract call. Desktop: "Contract interaction" + caution "Unable to decode — no ERC-7730 descriptor (0 bytes)" + "Interacting with: Unverified 0x7687…"; the amount appears only as a simulated balance change, and not at all on Ethereum where no simulation ran. Extension: red **"Blind signature"** + red danger box, **no amount and no recipient**. The iPhone draws the same request as "发送 −0.001 xDAI → 接收方 0x7687…D141". Empty calldata is a plain send whatever the recipient is | `strip-send.png`, `strip-send-gnosis.png`, `strip-ext-w1.png` (left), `ios-i13-send-3.5s.png` |
| L-D3 live | P1 | Extension | Confirmed on the device: after two dApp sends landed, Activity still reads "No activity yet" | `strip-ext-w1.png` (right) |
| L-D6 live | P2 | Desktop | Confirmed on the device: `eth_requestAccounts` → `0x88cCA0EeDb…`; `accountsChanged` → `0x88cca0eedb…` | `p30-dapp-log.png` |
| G28 | **P1** | iPhone | **The address bar names a site the page is not.** Type `https://app.uniswap.org` while the tab still shows jumper.exchange: the bar at once reads "🔒 app.uniswap.org" while Jumper's page stays on screen, live and tappable, for 30 s and more; the progress hairline sits at ~7 %; no failure, no words (W5: no load watchdog on iOS). The rule from 079 (L1) is the reverse: the old host stays until the new page commits. The same hang made 7 of 8 "working" dApps in the SC-006 run never commit, so Recents held only example.com | `strip-uni.png` (`ios-i26-uni-{5,12,30}s.png`), `strip-sc006.png` |
| G32 | **P1** | iPhone | **A site that never answers gets 60–75 s of silence.** `blackhole example` → type example.org: nothing but the host in the bar until WebKit's own `-1001` at ~62 s; the panel "无法加载此页面 / 网络不稳定，页面没能打开。/ 正在重试…" appears at ~75 s (desktop: panel at 5–8 s through its watchdog). Worse, for that whole minute the bar reads "🔒 example.org" over the previous site — here the test dApp, still **connected** (green dot) and clickable (G28). Row IX2 | `ios-i33-ix2-{10,45,90}s.jpg`, console `browser load failed: https://example.org/ — -1001 → timeout` |
| G33 | P2 | iPhone, Extension | **An unreachable chain says nothing for a minute.** iPhone: every Gnosis node black-holed (chaos log: 3 × HOLE each for rpc.gnosischain.com, gnosis-rpc.publicnode.com, 1rpc.io) → Block number → no chain notice within 60 s (079 C1 expects a one-line notice naming Gnosis); after `pass` the next read answered. Extension: each dApp read hung 45–57 s, then `-32603 "Vela could not reach a node for chain 100: Failed to fetch"` (raw fetch text), no notice, no memory of the dead nodes between calls (W11; G20 below) | `ios-i35-c1-60s.jpg`, `evidence/chaos-proxy.log` 16:17–16:18 |
| G20 | P1 | Extension | Every dApp read on a chain whose nodes are unreachable takes 45–57 s and ends in `-32603 … Failed to fetch`; the next call pays the same again; the first call after recovery took 6.3 s | `logs/ext-cdp.jsonl`, rows EX10/EX11 |
| S7 results | — | Extension, iPhone | Relay refused at the quote: the fee row names the cause ("Couldn't reach Vela — check your connection. We'll retry…" / "无法连接 Vela 服务 — 请检查网络，稍后会自动重试。") with a retry control and a shut slide; after `pass` the fee came back with no tap in 8 s (extension) and 14 s (iPhone) — pass (SC-003) | `ext-e14-fee-relay-down.jpg`, `ext-e15-fee-back.jpg`, `ios-i34-s7-relay-down.jpg` |
| Owner Chrome | — | Extension (owner's Chrome 154, profile "用户1") | Reloaded to 0.9.5 by the agent (chrome://extensions ↻). With the owner's real wallet (no signature, no biometric): G16 reproduced (the unstyled consent card, raw `eth_requestAccounts`); connect → checksummed address; Sign → ✕ → exactly one `4001`; `accountsChanged` lower-case (L-D6). Observation: six wallets announce on the page (WalletPair, Vela, Remote Inject ×2, Nightly, …) and WalletPair owns `window.ethereum`, so a legacy dApp that reads only `window.ethereum` reaches another wallet; EIP-6963 dApps list Vela. Not a Vela defect; noted for support answers | `evidence/extension/owner-chrome-*.jpg` |
| SC-006 iPhone | — | iPhone | 10 × `https://vela-tN.invalid`: each "找不到这个网站，请检查网址。", no retry, none in Recents (pass). 10 real dApps: curve.fi `-1004 → refused` and sushi.com `-1005 → offline` panels (pass); the other 8 did not commit within 9 s (see G28), so only example.com reached Recents — the Recents rule held, the loads did not | console `browser load failed` lines, `strip-sc006.png` |
| G26 | P2 | iPhone | The home balance does not move after the app's own send: xDAI read 0.38167 thirty minutes after this phone sent 0.011 (chain: 0.35967); only a pull-to-refresh corrected it. A person who just paid sees the old balance | `ios-i20-home-assets.png`, `ios-i21-home-after-refresh.png` |
| G24 | P2 | iPhone | The same wallet lists USDC on Base (0.470005) on the desktop but not on the iPhone; Base ETH appears on the iPhone only some time after a cold start (absent at 14:17, present at 15:28) | `p05-home-loaded.png`, `ios-i01-launch.png`, `ios-i20-home-assets.png` |
| G25 | P2 | iPhone | Tab switcher: the two cards differ in height and sit ~50 pt apart vertically; the new-tab tile is a small square whose label is cut to "新…"; a tab restored at launch and not yet shown still wears the placeholder drawing instead of a snapshot | `ios-i19-tab-switcher.png` |
| G1 | P2 | Desktop, Extension/web (wide layout) | Home "Activity" with no rows shows its heading and nothing else — no empty line, no loading state (iOS shows "暂无交易记录 / 收款将实时显示在这里。") | `d02-home-after-10s.png`, `p05-home-loaded.png`, `ios-i01-launch.png` |
| G2 | P2 | Desktop | A tab restored at launch shows its old title ("Uniswap Interf…") and an enabled Back arrow over the start page and an empty address bar | `d03-explore.png`, `d04-explore-5s.png` |
| G6 | P3 | Desktop | The tab strip moves up ~5 pt and the toolbar ~7 pt when a web page is showing, compared with the start page — the chrome jumps on every switch | `cmp-toolbar.png` |
| G7 | P3 | Desktop | After Enter the address bar stays in edit mode (magnifier, whole URL selected) until something else is clicked, instead of showing the lock and host | `p16-testdapp.png`, `zoom-address.png` |
| G8 | P3 | iPhone | Recents: a site whose title is its host shows the host twice (title line and host line) | `ios-i02-explore.png` |
| G9 | P2 | iPhone | Connect consent names the host twice: the title "连接到 192.168.50.9:8137" and the site row right under it | `ios-i06-consent.png` |
| G10 | P2 | iPhone | Connect consent says the same thing twice in two wordings — "该网站可以看到你的地址，并向你发起签名请求。任何资金转移都需要你确认。" above the buttons and "该网站想查看你的地址并请求你签名。未经你批准，它无法转移任何资金。" below them | `ios-i06-consent.png` |
| G11 | P3 | Desktop | Connect consent does not show which account or network will be connected (iPhone and Android do); they appear only after connecting | `p18-connect-consent.png` |
| G12 | P2 | iPhone | Signing sheet header truncates the host to "192.168.…" and the chain to "Ethe…" at 375 pt, with room to spare — the host is the one trust fact on the sheet | `ios-i08-sign-sheet.png` |
| G4 | P3 | Desktop | Sign-in sheet (onboarding, outside the browser): "Phone or tablet — Scan a code and **create** it on a nearby device" while signing in; "This device — Touch ID **or Windows Hello**" on a Mac | `p03-signin.png` |

Checked and by design: the closed lock on `http://127.0.0.1` / `http://192.168.x` — core counts
loopback and private-network http as secure (`DbrTabView.secure`); the open lock needs a public
http host.

## Post-fix device pass (2026-09-29)

The desktop (dev-fixtures, parallel space) and the extension (Chrome for Testing 151) were run
again at `fb8c7026` against quickstart §2–§3, then audited row by row against the evidence and the
chain. Results: `evidence/desktop/post-results.md`, `evidence/extension/post-results.md`; audits:
`evidence/desktop/post-audit.md` (25 confirmed, 9 refuted, 5 unproven) and
`evidence/extension/post-audit.md` (19 confirmed, 7 refuted, 2 unproven). The money was recomputed
from Gnosis logs: every op landed at most once, and no op the UI called "not sent" landed from
that attempt — but two landed ops were reported wrongly (G34, G35). Regressions confirmed fixed
on the device: G3, G7, G6, G2, G11, G16, G17, G18, G19, G21 (on the relay-mute path), G22, G23,
G29/G30, W6, W7, W8, W14 (inside a request), G13, G14 (plain send), L-D3, L-D5.

The same root cause on two clients is one finding. "(code)" marks a client with the same design
that the pass did not run. Evidence paths are under `evidence/`.

| Id | Sev | Client | Finding | Evidence |
|---|---|---|---|---|
| G34 | **P0** | Desktop; web, iPhone, Android (code) | **A payment that lands after the app quits mid-submit leaves no record, no tracker entry and no dApp answer.** DX9 run 2: close held, second close quit, the op landed 6 s later (nonce 50, block 48487627); after relaunch no Activity row, 16 min later still none. The record is written only after the POST returns (RJ1) | `desktop/post-DX9.txt`, `post-DX9-relaunch-activity-missing-row.jpg`, `post-DX9-relaunch-60s-balance-moved-no-row.jpg` |
| G35 | **P0** | Extension | **Closing or reloading the side panel after the submit claim answers the dApp 4900 while the op lands** (nonce 26, block 48487286, 13 s after the 4900). The worker settles every owed record on surface loss, claimed or not (RJ2) | `extension/post-EX-W1-T182.txt`, `post-EX-W1-T182-04-P0-sent-then-panel-closed.jpg` |
| G36 | **P1** | Desktop, Extension; iPhone, Android (code) | **A relay-rejected or proven-not-sent op is answered `ok` + op hash** while the sheet says 失败 · 请重试 (DX-W3/S6: 100 s after `status=Rejected`; EX-S5: 35 s after 失败; EX-W3 probe: 1 ms after the rejection). 请重试 is also wrong for a deterministic reject (RJ3) | `desktop/post-DX-W3-dapp-answer-ok-ophash.jpg`, `extension/post-EX-S5.txt`, `extension/post-EX-W3.txt` |
| G37 | **P1** | Desktop, Extension | **The answer and the sheet lag what the tracker already knows**: the op hash was answered 34 s (desktop) / 51 s (extension) after the chain check had the tx hash; the extension sheet fell back to 提交至网络… for 49 s after 已确认; the desktop sheet said "may have been sent" after the page had its tx hash (RJ4) | `extension/post-EX-W1.txt`, `post-EX-W1-03-submitting-again-after-confirmed.jpg`, `desktop/post-DX-W1.txt`, `desktop/post-DX6.txt` |
| G38 | P2 | Extension; all clients (core) | A landed op reads 还没上链 for 5 min 49 s while the relay's status said `included` + tx hash: the tracker records the relay's tx hash and never uses it (RJ4) | `extension/post-EX13.txt`, `post-EX13-02-still-confirming-5min.jpg` |
| G39 | P2 | Extension | The dApp answer after a lost reply overshoots the 120 s window (136.9 s): the deadline is checked only between 15 s polls (RJ4) | `extension/post-EX-W1.txt` |
| G40 | **P1** | Desktop | **The tab ✕ also selects the closed tab**: the one webview loads a page no remaining tab owns, no tab is lit, and during a request the hold hint flashes although the close went through (CLOSE-TAB, L1, DX8) | `desktop/post-DX8-close-background-loads-closed-tab.jpg`, `post-DX8.txt` |
| G41 | **P1** | Desktop | **Address-bar keys go to the page** when the page holds focus: a typed URL lands in a dApp's field; two pastes concatenated into `https://app.aave.comhttps://curve.fi` (KEY-FOCUS, SC-006b) | `desktop/post-KEYFOCUS-bar-keys-go-to-page.jpg`, `post-SC-006b-first5-and-concatenated-url.jpg` |
| G42 | **P1** | Desktop | **One shared webview**: until commit a new or restored tab shows the previous tab's live, connected page under the new host; Back walks another tab's history (G28 class) (RJ5) | `desktop/post-DX14-newtab-shows-other-tab.jpg`, `post-DX11-click-uniswap-tab.jpg`, `post-DX11-back-crosses-tabs.jpg` |
| G43 | P2 | Desktop | The auto-retry schedule restarts: the wallet's own retry is taken for a page-started load, so 7 attempts ran in 42 s and the panel vanished twice (RJ8) | `desktop/post-DX14.txt` |
| G44 | P2 | Desktop | Retries are starved while WebKit's provisional load hangs: no attempt for ~60 s, recovery waits for WebKit's own 60 s timeout (49.6 s after `pass`), a skip line every 2–5 s (RJ9) | `desktop/post-L2-L4.txt`, `post-DX2.txt`, `post-SC-006.txt` |
| G45 | P2 | Desktop | An expired certificate is classed `other`: no certificate sentence and an automatic retry, against FR-003 (L5) (RJ10) | `desktop/post-L5.txt`, `post-L5.jpg` |
| G46 | P2 | Desktop | G33 is not fixed on the desktop: the chain notice shows only when the call gives up (43.7 s), not after the first pass (~14 s) (C1) (RJ11) | `desktop/post-C1.txt`, `post-C1-notice.jpg` |
| G47 | P2 | Desktop, Extension | The fee comes back 15.8–19 s after the relay returns (extension: can exceed 15 s by phase), and no client logs `fee:` failure or recovery lines; the extension sheet keeps a stale cause line under 估算中…, under 速度 instead of 网络费, and jumps ~32 px (S7/S8) (RJ12) | `desktop/post-S7-S8.txt`, `extension/post-S7.txt` |
| G48 | P2 | Desktop; iPhone (code, RF5) | With no fault, the Ethereum fee row says 无法连接 Vela 服务 — 请检查网络 and shuts the slide for a public node's `eth_getCode` rate limit (RJ13) | `desktop/post-DX-G14.txt`, `post-DX-G14-ethereum.jpg` |
| G49 | P2 | Desktop; iPhone, Android (code) | A numeric `value` shows `余额变化 xDAI −0`: a 1000-wei delta rounds to 0 and keeps its minus (G14-num) (RJ15) | `desktop/post-G14-num-minus-zero.jpg` |
| G50 | P2 | Desktop | The transaction detail's 66-character 哈希 overflows the column; its copy button is off-screen (DX-LD3) | `desktop/post-DX-LD3-detail-confirmed.jpg` |
| G51 | P2 | Desktop | The "don't send it again" trace is easy to lose: a full-width red 删除记录 is the most prominent control on a 处理中 record, and switching to 钱包 drops the maybe-sent column for good (DX-LD3, T181) (RJ18) | `desktop/post-DX-LD3-detail-pending.jpg`, `post-T181-column-gone-before-quit.jpg` |
| G52 | P2 | Desktop; all clients (core feed) | A relay-rejected dApp record misleads: 接收方 is the token contract, not the transfer's recipient; 在区块浏览器中查看 is offered for an op that never reached the chain; the caption says 请重试 (RJ16, RJ3) | `desktop/post-DX-W3-activity-failed-detail.jpg` |
| G53 | P2 | Desktop; iPhone, Android (core) | Network health flaps (`net: offline` / `came back` every ~20 s with one chain faulted); after a relaunch with only Gnosis faulted the home read 24 个网络 RPC 不可用 over an empty asset list (RJ14) | `desktop/post-results.md` (T181 row), the desktop log `desk-post-T181.err` in the pass's scratchpad |
| G54 | P2 | Desktop | The tab strip overflows at 6+ tabs: the lit new tab and + are off the right edge | `desktop/post-DX3p-panel.jpg`, `post-SC-006b-recents.jpg` |
| G55 | **P1** | Extension | RB9 is missing: a request arriving while the panel shows Settings, Contacts or Feedback is never shown (58 s and 33 s until a manual tap); the dApp would wait 5 min. The layout effect reads a non-reactive field (RJ20) | `extension/post-EX4b-settings-no-card.jpg`, `post-EX4b.txt`, `post-EX-W3.txt` |
| G56 | P2 | Extension | The may-have-been-sent title reads 已提交 ("Submitted") instead of RA10's 提交至网络… | `extension/post-EX-S5-02-maybe-sent.jpg` |
| G57 | P2 | Extension | An ERC-20 transfer the relay's estimate says will revert is shown with a normal fee and no warning, signed and submitted with default gas (RJ19) | `extension/post-EX-W3.txt`, `post-EX-W3-01-sheet-amount-cut.jpg` |
| G58 | P2 | Extension | An account switch made in Settings never reaches connected sites (`eth_accounts` stayed One); lower-case grants are rewritten only when the wallet route mounts (RJ20) | `extension/post-EX-LD6.txt`, `post-EX-LD6-settings-switched-to-two.jpg` |
| G59 | P2 | Extension | The side panel ignores the pinned wallet language (English sheets with 简体中文 pinned) (RJ20) | `extension/post-EX7.txt`, `post-SC-007-en-language-picker.jpg` |
| G60 | P2 | Extension | The signing amount is clipped at 360 px and the fiat reads `≈ $1e+24` (SC-008) | `extension/post-EX-W3-01-sheet-amount-cut.jpg` |
| G61 | P2 | Extension | Panel-side failures leave no usable log line (maybe-sent and 失败 verdicts, fee failures, the tracker), never reach the bug report, and one log line says "ACCEPTED but NOT landed" for an op the relay never accepted | `extension/post-EX-LOG.txt`, `post-EX-S5.txt` |
| G62 | P3 | Extension | Filtered empty states are never shown (the feed's chain filter is never dispatched) | `extension/post-EX-G1-filtered-gnosis.jpg` |
| G63 | P3 | Extension | An idle open panel restarts the worker every ~30 s; the 200-line log ring floods (RJ20) | `extension/post-EX8b.txt` |
| G64 | P3 | Extension | The worker logs 8 s aborts as `kind=network` and re-pays every cooled endpoint on each call (RJ20) | `extension/post-EX10.txt` |
| G65 | P3 | Extension | The full-panel 已签名！ tick hides the next queued card for ≥ 1.4 s (RJ20) | `extension/post-results.md` (EX4) |
| G66 | P3 | Extension | No e2e covers a panel close during a claimed submit or a request arriving on Settings; the isolated e2e rebuilt the live `extension/dist` under the running browser | `extension/post-S3-e2e.txt` |
| G67 | P3 | Desktop | Log hygiene: `probe … route=system` while the dev proxy is in force; untimestamped `fee:`, `in-band:`, `signer page:` and `core: … booting` lines; `relay: submitting` twice; a dead proxy logged as "update the wallet" | `desktop/post-DX3p.txt`, `post-DX7.txt` |
| G68 | P3 | Desktop | ⌘W is unbound, and the RD14 close hold refuses the first close with no words (RJ17) | `desktop/post-DX9-cmdW-does-nothing.jpg`, `post-DX9-close-held.jpg` |
| G69 | P3 | Desktop | Any popup (network picker, ⋯ menu) blanks the whole dApp page | `desktop/post-U1-network-picker-zh.jpg`, `post-SC-007-en-site-menu.jpg` |
| G70 | P3 | Desktop | Small browser defects: a tab whose first load failed is restored as 新标签页; a stale title after a bfcache Back; Recents 清空 with no confirmation; the PARALLEL SPACE badge covers a tab's ✕ and + | `desktop/post-DX11-failed-tab-restored-as-newtab.jpg`, `post-DX4-forward-bfcache.jpg` |
| G71 | P3 | Desktop, Extension | Copy and consistency nits (list in `desktop/post-audit.md` D22 and `extension/post-audit.md` D19) | as listed there |
| G72 | P3 | Extension | Version label drift: `dist` was built from an uncommitted tree 38 s before the commit that carries the same wasm | `extension/post-EX0.txt` |
| G73 | P3 | Harness | The chaos proxy applies a leftover `latency` to every mode; quickstart C1 expected `outcome=timeout` (the desktop logs `not connected`); the nonce template's key was 24 bytes | `desktop/post-results.md` (harness notes) |

**Not defects** (with evidence; the auditors or testers raised them):

- The hero total above the listed assets while 部分余额仍在更新 shows: `balance_dashboard::display_total`'s `max(live, cached)` rule (#188, `balance_dashboard.rs:1419-1440`). What made it last for minutes is G53.
- An empty amount block on a token-transfer dApp record: RG2 draws an amount only for a native value > 0.
- The revert sheet's slide stays live after the estimate said it will fail: L-D5 (a warning informs, never blocks); the relay's rejection is answered by RJ3.
- The watchdog panel shown for ~2.5 s while a 9 s-latency page still loads (DX1): RD3's fast panel after the probe timed out; it cleared by itself at commit and there was exactly one `loadRequest` (W7 fixed).
- 正在准备交易… for ~40 s under `mute`: the truthful stage (RA9) for the relay's estimate timeouts.
- The 已确认 ending on the desktop closes after ~2 s: the 079 beat (~2.6 s), and the tx hash stays in Activity.
- The failed dApp row still showing −0.001 xDAI: the record keeps what was asked; the 失败 prefix says it did not move (RG2).
- EX-S5 showing may-have-been-sent for a refused CONNECT: the expected web difference (RA1, RA4).

**P3 deferred from 082** (RJ7; each goes into results.md with this reason):

- G69, popups blank the page: the native WKWebView composites above everything gpui draws (`webview.rs` module note); a popup over a live page needs a snapshot layer, which belongs with per-tab views (RJ5 follow-up).
- G70, a tab whose first load failed is restored as 新标签页: the core records a tab's address only at commit (`explore_sites`); keeping an uncommitted address across a restart is a model change for a P3.
- G70, Recents 清空 without confirmation: the desktop has no inline-confirm pattern yet (no bottom sheets); the words exist (`connect.browser.clearAllBody`) for the design pass.
- G71, the consent title's verb twice, the consent vs connected-panel sentences, 未验证 twice, the decoded recipient in 6+6 lower case, the extension's consent card without account/network rows (G11 parity), the IP avatar "1", 滑动以确认 · 确认, "Raw call data" on `personal_sign`, the device-storage count, the account-sheet total, the dev badge over the account name, a mid-animation arc: copy and layout polish on surfaces that already read correctly; one design pass after 082.
- G71, 请重试 beside a lone 完成 on S5: for a proven-not-sent op the advice is true (send it again); no control is implied.
- The chain notice pushes the page down ~37 pt when it appears (C1): a layout shift, not a wrong state; reserved space comes with the design pass.
- G72: a process fix, not code — quickstart §1.1 now requires a clean committed tree before any build.

Fixed in 082 although P3 (cheap, in files already touched): G62–G68, the bfcache title and the badge of G70, the zh comma and the U+2212 minus of G71, G73.

**Refuted and unproven rows re-run after the fixes** are quickstart §8 (`post2-*` evidence).

### Claude's decisions, pending the owner's review

These round-2 decisions were taken without the owner and are recorded as research RJ1–RJ7
(the planner's further choices are RJ8–RJ21). The owner may reverse any of them.

1. **Write-ahead (RJ1).** The dApp-tx record and the wallet Send records are persisted under the
   locally computed op hash, "may have been sent", before the first byte goes to the relay; a
   proven "not sent" withdraws it. Supersedes RD14's and RG4's reasoning.
2. **A claimed submit is never answered 4900 (RJ2).** Once the panel's submit claim carries the op
   hash, closing or reloading the panel answers the dApp `ok(op hash)`; before that, 4900 stays.
   Supersedes RB10 for claimed-submit requests.
3. **A refusal is an error (RJ3).** A relay-rejected op, or a not-sent op proven inside the answer
   window, answers `-32603` with a fixed "nothing was sent" sentence; an on-chain revert still
   answers the tx hash (ruling 9). The sheet never says 请重试 for a deterministic refusal.
4. **The tracker drives the answer (RJ4).** The page is answered as soon as the tracker has the
   outcome, the relay's tx hash is confirmed through the chain, and the window is enforced inside
   each poll.
5. **Desktop tabs (RJ5).** 082 takes the minimum: a tab's page is hidden until that tab's own load
   commits, and Back stops at the tab's first page. One WKWebView per tab is a follow-up.
6. **i18n (RJ6).** One new sentence, `componentsUi.signing.refused`, paid for by shortening
   `componentsTx.receipt.failedHint` (its "try again" tail); net ≈ −58 B, no cap raise.
7. **P3 deferrals (RJ7).** P3 items may be deferred with a one-line reason; P0–P2 are fixed in 082.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — On a bad network, a dApp in any Vela client never looks frozen or broken (Priority: P1)

A person opens a dApp in the Mac app's browser, in Chrome with the extension, or in the
iPhone's browser. The network is slow, a site or its chain node is unreachable, the signing
relay is down, or the system proxy drops a tunnel mid-request. Whatever fails, the person sees
what is happening in plain words, knows what they can do next, and the client recovers by
itself when the network comes back. The dApp gets exactly one answer per request.

**Why this priority**: This is the owner's goal in their own words, and most of Vela's users
sit behind proxies that fail in exactly these ways.

**Independent Test**: With the fault proxy in front of one client, run the page-load rows
(slow first byte, refused, black hole, bad certificate), the chain-down row and the relay-down
row. Watch the screen and the client's log.

**Acceptance Scenarios**:

1. **Given** a site that answers only after 6 s, **When** the person opens it, **Then** progress shows within 0.5 s and the previous page's name stays until the new page arrives.
2. **Given** a site whose connection is refused or never answered, **When** the person opens it, **Then** the client shows its own panel with the host, a plain reason and Retry. It never shows the engine's error page or a blank white page, and it retries a few times by itself, then stops.
3. **Given** the site's chain node is unreachable, **When** the dApp reads the chain, **Then** a one-line notice names the chain, and it clears by itself when the node answers again.
4. **Given** the relay is down when the person opens a send, **When** the fee cannot be quoted, **Then** the fee row says why, the slide is disabled, and the fee appears within 15 s of the relay returning, with no tap.
5. **Given** a pending request, **When** the connection drops after the signature, **Then** the person sees a named outcome (sent, still confirming, or failed with a reason), and the dApp gets exactly one answer.
6. **Given** the Chrome extension's background worker is stopped by the browser while a request waits for the person, **When** the person then approves or rejects, **Then** the request completes and the dApp gets its answer.

---

### User Story 2 — Connecting and signing read the same, and feel as polished, on the Mac, in Chrome and on the iPhone (Priority: P1)

A person connects a dApp, signs a message and sends a small amount, first with a good network.
The consent, the signing sheet, the passkey prompt, the "signed" and "sent" endings and the
connection panel look and read like one product on every client, in Chinese and in English.

**Why this priority**: 079 checked these surfaces on the Android phone by hand and on the
other clients mostly by code; the owner asked to see them on the real Mac, Chrome and iPhone.

**Independent Test**: Walk connect → sign message → send dust on each client with the owner
confirming each passkey prompt; screenshot every state; compare the three side by side.

**Acceptance Scenarios**:

1. **Given** a dApp asks to connect, **When** the consent appears, **Then** its title names the site once, "Connect" is the filled button, and nothing calls an https site "safe".
2. **Given** a signing request, **When** the person swipes, taps outside or presses Escape, **Then** the request stays open. Only the explicit ✕ closes it, and then the dApp is told "rejected" once.
3. **Given** the person approved, **When** the signature or transaction completes, **Then** the sheet shows a named ending (signed / sent / confirmed with a short hash) and closes by itself.
4. **Given** the person closes the sheet after approving, **When** the operation continues, **Then** the dApp still gets its answer and the ending does not come back.
5. **Given** any of these screens, **When** it is read at the phone's width and the desktop's width, in Chinese and English, **Then** no text is cut off, overlaps, or shows a raw key, error code or engine message.

---

### User Story 3 — A transaction a dApp asked for shows up in Activity (Priority: P1) — L-D3

After a person approves a transaction a dApp asked for, it appears in Activity like a send
made in the wallet: pending first, then confirmed or failed, with the site it came from.

**Why this priority**: Today the transaction is invisible after the sheet closes. A person who
looks for it in Activity finds nothing and may send again.

**Independent Test**: Send dust from the test dApp on each client; open Activity.

**Acceptance Scenarios**:

1. **Given** a dApp transaction was just submitted, **When** the person opens Activity, **Then** it is listed as pending, and it updates to confirmed or failed without a refresh.
2. **Given** the app is closed right after submitting, **When** it opens again, **Then** the transaction is still listed.

---

### User Story 4 — Risk signals are honest (Priority: P2) — L-D5, L-HOST

A warning in red means something is wrong with this transaction. When the wallet simply could
not check (the chain's node cannot simulate), it says so in a calm tone, apart from "this
will fail". The signing page names the site once.

**Why this priority**: A danger colour for "we could not check" teaches people to ignore
danger colours. The doubled host reads as a glitch on the one page that must look trustworthy.

**Independent Test**: Sign on a chain whose public node has no simulation, and on a chain where
the simulation reverts; compare. Open the signing page from a site whose name is its host.

**Acceptance Scenarios**:

1. **Given** the node cannot simulate, **When** the signing sheet shows, **Then** the line reads as "could not check" in a neutral or caution style, never the danger style.
2. **Given** the simulation says the transaction reverts, **When** the signing sheet shows, **Then** the danger style is kept.
3. **Given** a site whose name equals its host, **When** the signing page opens, **Then** the host is shown once.

---

### User Story 5 — The dApp always sees the same address (Priority: P2) — L-D6

Every address the wallet hands a dApp — on connect, on account change, on request — is in one
spelling (the checksummed form).

**Why this priority**: Strict dApps compare strings; one account in two spellings looks like an
account switch or a second account.

**Independent Test**: Connect, switch accounts in the wallet, switch back; log every address the
test dApp receives on each client.

**Acceptance Scenarios**:

1. **Given** a connected dApp, **When** the person switches accounts, **Then** the account-changed event carries the same checksummed spelling that connect returned.

---

### User Story 6 — Empty history says the right thing (Priority: P3) — L-D7

**Why this priority**: A small wording error, but seen by every new user.

**Independent Test**: A new account, History set to all networks.

**Acceptance Scenarios**:

1. **Given** no transactions on any network, **When** History shows all networks, **Then** the empty line does not say "this network".

---

### User Story 7 — Fees and slow confirmations are explained, not guessed (Priority: P2) — L-D4, L-D2 (wallet side)

**Why this priority**: A ~0.01 xDAI fee for a plain transfer, or a transaction that never
lands, erodes trust faster than any visual flaw. The fixes that belong to the relay are
another repo's; what the person is shown is the wallet's.

**Independent Test**: Quote a plain Gnosis transfer; submit one on a chain whose relay does not
mine, and watch past the expected window.

**Acceptance Scenarios**:

1. **Given** a quoted fee, **When** the person reads it, **Then** it is shown before signing in the fee coin and in fiat, and it is never presented as a multiple of another number.
2. **Given** an operation that has not landed past its expected window, **When** the person looks, **Then** the wording says it is still confirming, with the explorer link; it is never marked failed on elapsed time alone.

---

### User Story 8 — Every failure the person sees can be diagnosed from the client's log (Priority: P2)

When something goes wrong on a client, that client's log names what failed (host, kind of
failure, the request involved) without secrets, so a bug report can be traced.

**Why this priority**: The owner asked to judge the pass from the Mac, extension and iPhone
logs. Where a log is silent, the pass cannot tell a network fault from a product bug.

**Independent Test**: For each failure row in US1, find its line in that client's log.

**Acceptance Scenarios**:

1. **Given** any failure panel or error line shown to the person, **When** the log for that moment is read, **Then** a line names the failing host or service and the failure kind.
2. **Given** any log line, **When** it is read, **Then** it contains no private key material, passkey secret, full signature payload or seed.

---

### User Story 9 — 079's missing evidence is collected (Priority: P3) — L-SC, L-PANEL

**Why this priority**: They are gaps in the evidence, not known bugs.

**Independent Test**: Run each missing row once on the named client.

**Acceptance Scenarios**:

1. **Given** the iPhone, **When** a slow page is opened by a tap, **Then** progress is visible within 0.5 s (timed from a screen recording or the log).
2. **Given** 10 failed and 10 successful loads on one client, **When** Recents is read, **Then** it holds only the 10 pages that loaded, each with its own title and icon.
3. **Given** the English UI, **When** every browser surface is swept, **Then** no "secure site", "not secure" or "encrypted" text appears — only the lock.
4. **Given** the extension's side panel, **When** a signature completes, **Then** an automated check sees the "signed" tick.

### Edge Cases

- The system proxy is turned off (or on) in the middle of a page load or a signature.
- A request is pending when the person locks the Mac, puts the iPhone to sleep, or Chrome stops the extension's worker.
- Two requests arrive back to back from the same dApp; two dApps in two Chrome tabs ask at the same time.
- The passkey prompt is cancelled, times out, or the person is asked on another device.
- The dApp navigates away or closes its tab while the person is signing.
- The signing page's host is unreachable on first use and on a later use (cached).
- The account has no balance on the dApp's chain; the dApp's chain is not one Vela supports.
- The iPhone is on cellular with the proxy's tunnel cut; the Mac wakes from sleep with stale connections.

## Requirements *(mandatory)*

### Functional Requirements

**Page loads and the chain (US1)**

- **FR-001**: Every client MUST show progress within 0.5 s of a load the person started, and keep the previous page's name until the new page arrives.
- **FR-002**: Every client MUST replace any engine error page and any blank page from a failed load with its own panel: the host, a plain reason (offline / certificate / not found / network unstable) and Retry.
- **FR-003**: Automatic retries MUST be few, spaced, and stop by themselves; a certificate failure MUST NOT retry automatically nor offer "continue anyway".
- **FR-004**: When the site's chain cannot be reached, the client MUST show a one-line notice naming the chain and clear it without a tap when the chain answers.

**Requests and signing (US1, US2)**

- **FR-005**: Every dApp request MUST end in exactly one answer to the page (result, or a rejection/unavailable error), including when the page navigates, the tab closes, the extension worker restarts, or the network drops after the signature.
- **FR-006**: A signing request MUST close only on the explicit ✕ (swipe, outside tap and Escape leave it open), as 079 ruled.
- **FR-007**: After approval, the sheet MUST show a named ending — signed, submitting, waiting (with the chain's expected time), confirmed (short hash), still confirming, or failed with a plain reason — and MUST NOT mark a failure on elapsed time alone.
- **FR-008**: Fee quoting MUST explain a failure, offer a refresh control, and re-quote without a tap within 15 s of the service returning.
- **FR-009**: Consent and signing surfaces MUST name the site once, and no surface may call a site "safe", "not secure" or "encrypted" in words; a lock icon alone marks https vs http.
- **FR-010**: Every surface in this spec MUST read correctly in Chinese and English at the client's real widths — no cut-off text, no raw keys, error codes or engine messages.

**079 leftovers (US3–US7, US9)**

- **FR-011**: A transaction a dApp asked for MUST appear in Activity on every client from the moment it is submitted, update to its final state, and survive an app restart. (L-D3)
- **FR-012**: "The node could not simulate" MUST be shown distinctly from "the simulation says it fails", and never in the danger style. (L-D5)
- **FR-013**: The signing page MUST show the site's host once when its name equals its host, using the same rule as the apps. (L-HOST)
- **FR-014**: Every address given to a dApp (connect, account-changed, accounts request) MUST use one spelling, the checksummed form, on every client. (L-D6)
- **FR-015**: History's empty state for all networks MUST NOT refer to "this network". (L-D7)
- **FR-016**: The wallet-side fee and confirmation wording for L-D4 and L-D2 MUST follow FR-007 and FR-008; the relay-side causes are recorded as hand-offs with evidence. (L-D4, L-D2)
- **FR-017**: The extension side panel's "signed" ending MUST be covered by an automated check. (L-PANEL)

**Observability (US8)**

- **FR-018**: Every failure shown to the person MUST have a matching log line on that client naming the host or service and the failure kind.
- **FR-019**: Logs MUST NOT contain key material, passkey secrets, seeds or full signature payloads.

**Consistency**

- **FR-020**: Any rule a fix needs (retry timing, status choice, address spelling, empty-state choice, simulation severity) MUST be decided once in the shared core and drawn by each client, as 079 established; a fix found on one client MUST be checked on the other three (Android included) and fixed there too, or recorded as not applicable.

### Key Entities

- **Finding (G#)**: something the pass saw that falls short — description, client(s), evidence, severity, the owner's ruling if any, and its status per client.
- **Leftover (L-…)**: a 079 item carried into 082, with its root cause, the fix's location, and its status.
- **Pass row**: one executable check (id, what to do, what to expect, whether the owner's biometric is needed), grouped by client; reuses 079's row ids where they apply.
- **Evidence**: a screenshot or log excerpt tied to a row or finding, stored under `evidence/`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the Mac, in Chrome and on the iPhone, every page-load fault row (slow, refused, black hole, certificate, proxy cut) shows the client's own panel or progress in 100% of attempts, with 0 frames of an engine error page or blank page.
- **SC-002**: In every signing row on the three clients, the dApp receives exactly one answer per request (0 hung requests, 0 double answers), including across a worker restart in Chrome and a dropped connection after the signature.
- **SC-003**: The fee reappears within 15 s of the relay returning, without a tap, on the three clients.
- **SC-004**: A dApp transaction appears in Activity within 5 s of submission on every client, and is still there after a restart. (L-D3)
- **SC-005**: 0 danger-styled "could not simulate" lines; the signing page's header shows the host once for a name-equals-host site. (L-D5, L-HOST)
- **SC-006**: Across connect, account switch and requests, 100% of addresses the test dApp receives are checksummed. (L-D6)
- **SC-007**: For every failure row, the client's log has a line naming the host or service and the kind of failure (100% of rows), and a secret scan of the collected logs finds nothing.
- **SC-008**: 0 findings of cut-off text, raw keys or engine messages remain open on the surfaces in this spec, in Chinese and English.
- **SC-009**: The 079 evidence gaps are closed: iPhone progress ≤ 0.5 s timed; Recents after 10 + 10 loads holds only the 10 successful pages; the English sweep finds 0 "secure/not secure/encrypted" words; the side-panel "signed" check passes.
- **SC-010**: No regression: 079's device rows still pass on the clients they ran on, and every client's automated suite stays green.

## Decisions left to the owner

- **i18n budget**: the ja + en residency is at 138,750 of 138,800 bytes. Any fix that needs new words (US3's "from <site>" line, US4's "could not check" line, US6's empty line) needs the cap raised or an existing phrase reused. The plan will reuse existing phrases first and list every new string with its byte cost.
- **Real-funds rows**: sends and approvals move the owner's money; they run only with dust amounts on cheap chains, each confirmed by the owner at the passkey prompt.

## Assumptions

- "Latest" means the tip of `origin/main` at the start of the pass (de93634f); the Mac app is a locally signed bundle installed in /Applications, the extension is rebuilt in place where the owner's Chrome loads it unpacked, and the iPhone gets a build installed from this Mac.
- The owner's own wallet is used on all three clients, as in 079; the parallel-space fixed keys may stand in for rows that need no real passkey (for example, automated fault rows).
- Logs are the clients' own: the Mac app's process log, the extension's worker and page consoles, and the iPhone's device log, captured on this Mac during the pass.
- The Mac desktop window cannot go below 1280×800, so phone-width checks apply to the iPhone and the extension's side panel only.
- Relay and fee-service fixes (the causes of L-D2 and L-D4) are outside this repo; 082 records evidence and the wallet-side behaviour only.
- Android is not re-tested by hand in this pass, but any shared-rule or copy change is also applied and unit-tested there (FR-020).
