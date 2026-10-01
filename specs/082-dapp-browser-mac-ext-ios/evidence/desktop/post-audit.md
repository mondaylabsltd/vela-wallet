# 082 post-fix device pass, desktop: adversarial audit

- Audited: 2026-09-29 04:34–05:10 CST, against build fb8c7026 (branch 082-dapp-browser-mac-ext-ios)
- Inputs: `quickstart.md` §2, `spec.md`, `research.md` (RD1, RD3, RD14, RA2, RA8, RA12, RC4, RC6, RF1, RG3, RG4), `post-results.md`, all 114 `post-*` files in this folder (every image was opened and read), `scratchpad/logs/desk-post*.err`, `chaos-post.log`, `desktop-oslog.txt`, and the source at the tip of the branch.
- Method: each row counts as passing only when the evidence shows its expectation. **confirmed** means the evidence shows the expectation. **refuted** means the evidence shows it is not met. **unproven** means nothing shows the key claim.
- I edited no code and made no commits. The only file I wrote is this one.

## 1. Verdict

| | rows |
|---|---|
| confirmed | 25: DX0, DX-G3, DX13, DX12, DX11, L1, DX1, DX2, DX3′, C2, DX8, U1/U2, U5·S1·S2, DX-W1, T181, DX-LD3, DX6, DX7, S5, DX-G13, DX-G14, DX-LD5, SC-006a, SC-006b, SC-007-en |
| refuted | 9: DX14, L2–L4, L5, C1, CLOSE-TAB (new), KEY-FOCUS (new), S7/S8, DX9, G14-zero/num (num leg) |
| unproven | 5: DX4, DX5, DX-LD7, DX-W3, DX-LD6 |

The testers' verdicts hold up. No "pass" was refuted outright. The only corrections are small: miscounted log lines, harness-relative frame labels, and one inference that does not follow (see §5).

**Release blocker: DX9 (P0).** A payment that landed leaves no trace. There is no Activity row, no tracker entry and no dApp answer, and it is still missing 16 minutes later. That is the double-payment path ruling 8 was meant to close.

**P1:**
- A relay-rejected op is answered to the dApp as a success (DX-W3).
- The tab ✕ also navigates to the closed tab (CLOSE-TAB).
- Address-bar keystrokes go to the page (KEY-FOCUS).
- The single shared webview shows another tab's live, connected page under a new tab's host (a G28-class trust break).

## 2. Money (recomputed from chain)

Query: Gnosis `eth_getLogs` on EntryPoint `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, topic0 `0x49628fd1…e1419f` (UserOperationEvent), topic2 = the Safe `0x88cCA0EeDbF2C4426110bbFc998F048689266894`. Blocks 48485057–48488057 (the last 3000, back to about 00:24 CST, before the pass began), in chunks of 500. Script: `scratchpad/audit/uoe.py`. Result: **7 events, one per nonce 44–50, all success=1, no duplicates.** EntryPoint `getNonce(safe, 0)` = **51**, and the xDAI balance = **0.24967**.

| block | landed (CST) | nonce | userOpHash | tx | row | what the UI and the dApp said |
|---|---|---|---|---|---|---|
| 48487271 | 03:27:55 | 44 | 0x9da32d52…c7859b71 | 0x111ce047…463702d1 | DX-W1 | may have been sent, then 已确认 (found through the chain log while the relay was muted). The dApp got the op hash once. ✓ |
| 48487329 | 03:33:05 | 45 | 0xaddcdc94…c7fb2de3 | 0xd8eee45b…c7a850fe | W1b | may have been sent. The dApp got the tx hash. ✓ |
| 48487385 | 03:37:45 | 46 | 0x9f915781…c5f50850 | 0xbe17d3b9…79808173 | T181 / DX-LD3 | 处理中 across the relaunch, then 已确认. ✓ |
| 48487465 | 03:44:35 | 47 | 0x5b30adaa…2b91de0b | 0x372e387e…ee23e8b0 | DX6 run 1 | may have been sent. The dApp got the tx hash. ✓ |
| 48487487 | 03:46:25 | 48 | 0xcbde0dc7…57070859 | 0x8c8a5542…b104ab86 | DX6 run 2 | 已确认. The dApp got the tx hash. ✓ |
| 48487511 | 03:48:25 | 49 | 0x5574bf1c…aa04ade6 | 0xcd4666b8…80f09a1e | DX7 | 已确认 while the relay was black-holed. The dApp got the tx hash. ✓ |
| 48487627 | 03:58:05 | 50 | 0x7df211ed…249c36e4 | 0xe557f5f0…5c5ed570 | **DX9 run 2** | **Nothing.** The app was quit 2 s before it landed. After the relaunch there was no row and no tracker line (desk-post-DX9.err has none for 0x7df211eddc), the dApp got no answer, and it is still absent at 04:14 (post-DX-LD5-wallet-no-banner.jpg). **P0.** |

- Balance: 0.32667 → 0.24967 xDAI = 0.077 = 7 × (0.001 dust + 0.01 in-band fee). This matches the 7 events exactly. `actualGasCost` is 0 in every event because the fee travels in-band.
- **Not sent, and truly not delivered:**
  - S5 (03:51:16) and DX9 run 1 (03:55:10) were not delivered. S5's CONNECT was dropped before any byte. In DX9 run 1, `chaos-proxy.py:185-186` sleeps 20 s before the upstream connect and before answering `200`, and the client gave up at 13.5 s. The chaos log's late `LATENCY … 1ms` lines are the proxy connecting after the client had already left, so no TLS or HTTP byte was forwarded.
  - The DX-W3 op 0xa974c5dd… was accepted and then Rejected by the relay. No event exists, and the nonce stayed at 51.
- **Nothing was sent twice.** No op the UI called failed/not sent landed *from that attempt*.
- Subtlety: S5, DX9 run 1 and DX9 run 2 all carry the **same userOpHash, 0x7df211ed…** (the same nonce, defaulted gas and the same fee give a deterministic hash). So the hash the UI twice labelled 失败 / "nothing was sent" did land 7 minutes later, from the third attempt. The desktop writes no record for NotSent, so there is no conflict here. Any client that records a NotSent or failed op under its hash would later see that same hash land.
- Other chains: the Base USDC (0.0849 → 0.0349) and Base ETH drops are not this client's. The desktop sent only on Gnosis. That is out of scope.

## 3. Rows

| id | tester | audit | why |
|---|---|---|---|
| DX0 | pass | confirmed | `.err` line 1 is the dev-proxy line. `02:24:34.982 asked … gen=1 how=navigation`, then committed (+2.06 s) and finished. chaos `02:24:34 PASS example.com:443`. The image shows lock + example.com. |
| DX-G3 · G3 | pass | confirmed | The 6-frame strip shows each edit drawn (/abc → / → no slash → placeholder → pasted → lock + example.org). Log: asked/committed gen=2, and the process stayed alive. |
| DX13 · G7 | pass | confirmed | Frames 1, 2 and 4 show lock + host with no magnifier, and frame 3 shows edit mode with the URL all selected. The frames carry no timestamps, so "+0.3/+2.3 s" is not evidenced. The state is. |
| DX14 · G29 G30 | fail | refuted | Log: 7 automatic attempts, with `retry attempt=2 → asked how=page + how=navigation → attempt=1` twice (02:28:16, 02:28:30). The os_log has only one `loadRequest` at 02:28:16.586, so the page-started verdict is the wallet's own retry. The panel frames at 12 s show the other tab's test dApp. The G29/G30 parts (no abort, host, no lock, title, bar edit) are confirmed in the images. |
| DX12 · G6 | pass | confirmed | In the three crops the address field and hairline sit at identical offsets, and post-DX12.txt gives 36.0/47.5–79.5/91.0 pt for both page types. |
| DX11 · G2 | pass (UX defects) | confirmed | relaunch.jpg: start page, all tabs unlit, Back/Forward/Reload grey. `02:36:23.480 navigate asked … view=pending`, and one Enter opened the 4th tab. Clicking the Uniswap tab: `asked … gen=2`, commit +1.0 s. The defects are real: click-uniswap-tab 0.5 s shows the test dApp under `app.uniswap.org`, and back-crosses-tabs shows the Uniswap tab lit over 127.0.0.1:8137. |
| L1 | pass | confirmed | The frames show the hairline from l1-a (+0.51 s by mtime) with the bar kept at 127.0.0.1:8137 through l1-d. Commit came at +7.19 s (`02:38:52.744`), and l1-e shows lock + example.com. |
| DX1 · W7 | pass | confirmed | desktop-oslog.txt has one `WebPageProxy::loadRequest` at 02:41:46.095 and the next main-frame event is the commit at 02:41:56.169. `retry … skipped (engine still loading)` at 02:41:55.705. The frames show the panel, then Uniswap. |
| L2–L4 | fail | refuted | 35 skip lines (not the 32 the report gives). WebKit `didFailProvisionalLoad` came at 02:47:07.103 (60.4 s), and attempt 1 ran only at 02:47:07.992. `pass` 02:47:23.8 → loaded 02:48:13.4 (49.6 s). The frames show a static 重试 from 3 s to 30 s. |
| L5 | fail | refuted | chaos shows PASS (no fault). `probe … verdict=other`, `class=other`, then `retry attempt=1` 3 s later. The panel has no reason line. Cause confirmed in code (§4 D8). |
| DX2 | pass | confirmed | It loaded 2.2 s after `pass` (02:52:01.5). The wallet.json Recents before/after in post-DX2.txt show that only the Uniswap timestamp moved, after the load. The load came from WebKit's own 60 s timeout (see D7). |
| DX4 · W18 | partial | unproven | `location.href` from the console was not run (no console). The substitute, a page link (`asked host=iana.org how=page`), shows the hairline while the bar keeps example.org, then the panel at +7.6 s with "retried by hand only", and no retry for 30 s. The Back/Forward-to-faulted-entry leg was not exercised (bfcache). |
| DX5 | partial | unproven | The chaos log has 0 HOLE lines in the window, so the live-progress threshold path was never exercised. |
| DX3′ · W6 W8 | pass (notes) | confirmed | `probe … verdict=proxy route=127.0.0.1:9` and the panel 代理没有响应。 at +3.17 s. There are 896 `proxy: 127.0.0.1:9 unreachable` lines and every read ended `not connected`. The tester's "nothing reaches chaos-post.log, so nothing went direct" does not follow: a direct bypass would not touch 8899 either. The app log is the real evidence. |
| C1 · G33 | fail | refuted | The notice came at 03:03:18.107, when the first call gave up after 43 716 ms. The first pass over the pool's two live endpoints ended at about 03:02:48.8 (+14.4 s), because rpc.gnosischain.com is banned in this state. The log says `not connected`, not the quickstart's `outcome=timeout` (a spec wording gap, D25). Retry is busy with spinner + 重试 (c1r frames), and `retry … → failed` ✓. |
| C2 | pass | confirmed | `pass` 03:04:46.774, click 03:04:47.0, `chain notice: cleared` 03:04:49.391 (2.4 s). The page shows `eth_blockNumber ok 0x2e3da56`. |
| DX8 · W14 | pass (notes) | confirmed | held.jpg shows 请先完成或取消这个请求 and the column. Strip: the hint is gone by 2.6 s. dapp-log: `4001`, `events: []`, and the earlier `eth_blockNumber` kept. Two `navigation held (tab)` lines, the second from the ✕ click (D3). |
| CLOSE-TAB (new) | fail | refuted | std2.err 03:00:34–45: five closes, five `asked host=<closed tab>` lines. The frames show a page no remaining tab owns. The L1 lines gen=3/4 are the same thing. RD1 says "closing a background tab no longer reloads". |
| KEY-FOCUS (new) | fail | refuted | The frames show the bar in edit mode while `a` lands in Uniswap's token search. There is no `navigate asked` for the Enter. SC-006b reproduces it (`navigate asked host=app.aave.comhttps:`). |
| U1/U2 · G11 | pass (notes) | confirmed | zh and en consent images: the title names the host once, an account row (avatar, MultiTest, 0x88cC…6894, 切换账户), and a network row with logo. The dApp log shows the EIP-55 account and `eth_chainId 0x64`. No 安全站点/不安全/已加密 and no Secure/Not secure/Encrypted. The network picker blanks the page (D18). |
| U5 · S1 · S2 | pass | confirmed | The column stayed after Esc ×5 and clicks ×5, and `personal_sign 4001` shows. Caveats: the harness keeps only the last result per method, so "once" rests on the page promise resolving once. Esc was sent with the column focused. With the webview focused, Esc would go to the page (D4). |
| DX-LD7 · G1 | partial | unproven | The G1 legs (unfiltered 暂无交易记录 / 收款将实时显示在这里。, History 暂无交易) were not shown because there was no empty account. The filtered legs 该网络暂无交易记录 and 此网络暂无交易 are confirmed. |
| S7/S8 | fail | refuted | The fee-row words and shut slide are ✓. Recovery: the first re-quote CONNECT was at 03:24:23 (+13.7 s after `pass` 03:24:09.3), 估算中… at 03:24:25.1 and the fee at ≤03:24:28.3 (15.8–19.0 s). The frame labels p12/p15 in post-S8-fee-back-after-pass.jpg count from the harness start (~4 s after pass), not from pass. The txt mtimes and the chaos log are authoritative. There is no `fee:` failure or recovery line anywhere (grep; the only `fee:` emitter is user_op.rs:377). |
| DX-W1 · G21 G22 | pass (notes) | confirmed | The frames show 正在准备交易… (a01/a38), then 提交至网络…, then the may-have-been-sent caption + UserOp 哈希 with no 重试, then 已确认 with tx 0x111ce047…. `verdict=maybe_sent`. `find-event … → logs` at 03:28:12 while muted. The dApp got the op hash once (one `dapp:` line) at +120.5 s. On chain: nonce 44, one event. The 处理中 Activity row for this op was not captured here (it confirmed first). T181 covers it. |
| T181 | pass | confirmed | The after-relaunch image shows `处理中 · 127.0.0.1:8137`. The op landed at 03:37:45 while the wallet kept saying 处理中. `pass` 03:41:27.3 → `find-event → logs` 03:41:34.8 → `status=Included` → 已确认 in the frames. |
| DX-LD3 · L-D3 | pass (visual) | confirmed | The row appears within 5 s of the verdict and survives the relaunch, and the detail has dApp 交易 + chip + 发起方. Two caveats. "Within 5 s" is measured from the maybe-sent verdict, not from the slide: for about 74 s after the slide nothing is recorded, and that is the window DX9 falls into. Visual defects D13 and D14 are confirmed in the images. |
| DX6 · W1 | pass (notes) | confirmed | Both runs show `outcome=network` twice → `maybe_sent` and never 失败, and each landed once (47, 48). In run 1 the 已确认 ending was not captured between frames, but the dApp got the tx hash. |
| DX7 | pass | confirmed | Blackhole after `submitting`, then `maybe_sent`, then `find-event → logs` at 03:48:48 with the relay still HOLE. The frames show 已确认 from 03:48:54 through 03:49:10. One answer (tx hash) at 03:50:05.96. Nonce 49 landed once. |
| S5 · NotSent | pass | confirmed | `verdict=not_sent … in=621 ms`, then 失败 · 交易未能提交。您的资金安全无虞——请重试。 at +5 s. The page shows `-32603 "relay unreachable; nothing was sent"`. No row, nonce unchanged. |
| DX9 · W17 | fail (P0) | refuted | `window: close held` 03:57:59.006, then the process exits with no verdict. The op landed at 03:58:05 (nonce 50). After the relaunch there are six rows, no tracker line, and the balance dropped. ⌘W is unbound (main.rs:443-448). |
| DX-G13 · G13 | pass | confirmed | The live test prints `status: Some(… NotFound …)`, `1 passed`. Every status poll used `pimlico_getUserOperationStatus`. `status=Included` (T181) and `status=Rejected` (W3). No -32601 anywhere. |
| DX-W3 · W3 | skipped | unproven | The on-chain revert path (失败 + 转账在链上被回滚…) was not producible: the relay rejects reverting ops. The S6 relay-reject leg shows 失败 1 s after `status=Rejected` ✓, but the dApp got `ok:true` + op hash at 04:13:05 (D2). |
| DX-G14 · G14 | pass | confirmed | Gnosis: 发送 / −0.001 xDAI / 接收方 0x7687…D141 / 确认发送, with no contract card. Ethereum: −0.001 ETH with an amber "could not check". One 4001 each (the scratch-page results list keeps each request). The Ethereum fee-row wording is D12. |
| G14-zero / G14-num | partial | refuted | Zero and omitted: 发送 / 0 xDAI / 确认 ✓. Numeric: the contract card is right, but its simulation box reads `余额变化 xDAI −0` (post-G14-num-minus-zero.jpg). RC4/RC6 say never show a zero figure there. |
| DX-LD5 · L-D5 | pass (notes) | confirmed | Arbitrum shows the amber caution, `chainChanged 0xa4b1`, no chain=42161 log line and no banner. The revert shows the red "这笔交易预计会失败 — 但仍会扣除 gas。". |
| DX-LD6 · L-D6 | partial | unproven | Every address received (eth_requestAccounts, eth_accounts, accountsChanged on 8137 and 8141) is EIP-55. The account-switch leg, which is the leg that was broken before 082, was not run (one account). |
| SC-006a | pass (words) | confirmed | Ten panels, each `asked` then `failed class=offline` about 5.5 s later. The frames show no WebKit error page. Because the class is offline, the auto-retry rule for not_found does not apply. It also reproduced D6 (`vela-t5 … how=page + how=navigation` at 04:18:48) and D7 (62 skip lines). |
| SC-006b | pass | confirmed | The Recents image holds exactly the 10 committed dApps with their own titles and icons, and none of the .invalid or concatenated loads. |
| SC-007-en | pass | confirmed | The bar, menu, connection panel and consent in en contain none of "Secure site", "Not secure" or "Encrypted". |

## 4. Defects (most severe first)

### P0

**D1: A payment that lands after a close during the submit leaves no record, no tracker entry and no dApp answer** (DX9, nonce 50)
- Repro: `mute vela-relay` (latency 0) → Send dust → slide → during 提交至网络… click the window's close button (held, silently) → click it again within 5 s (the app quits) → relaunch. Activity has no row for op 0x7df211ed…, even 16 min later. The chain has its UserOperationEvent. xDAI is down 0.011.
- Code:
  - `app-desktop/vela-wallet/src/executor/sign_request.rs:380-410`: `Event::OpSubmitted`, the only thing that persists a record and hands the op to the tracker, is sent only after `user_op::submit` returns.
  - `app-desktop/vela-wallet/src/executor/relay.rs:588-628`: `close_verdict` lets the second close through while `SUBMITS_IN_FLIGHT > 0`.
  - research RD14's rationale ("RA3/RG3 cover what happens after a quit anyway") is false for a quit mid-POST. RG4 rejected a write-ahead record.
- Fix direction: persist the locally computed hash as a pending record before the POST, or do not honour a quit while the POST is in flight.

### P1

**D2: A relay-rejected op is answered to the dApp as success** (DX-W3 / S6)
- Repro: Gnosis USDC `transfer(x, 10^30)` from a dApp → slide. `verdict=accepted`, `status=Rejected` 15 s later, and the sheet shows 失败. 100 s later the page gets `{ok:true, result:<op hash>}`.
- Code: `app-desktop/vela-wallet/src/executor/sign_request.rs:412-424`, `:512-517` and `:601-610`. `await_receipt` polls only the relay's receipt, and `after_receipt_wait` maps "no receipt" to `ReceiptPending`, so the page gets the op hash. The tracker's Rejected/NotSent is never consulted.

**D3: The ✕ on a tab also fires that tab's select handler**
- Result: the single webview navigates to the closed tab's URL, the shown tab is set to a closed id, no tab is lit, and the shown page is replaced (RD1 violated). During a request, the same click flashes 请先完成或取消这个请求 although the close went through.
- Rows: CLOSE-TAB, L1, DX8.
- Repro: on the start page or any page, click ✕ on a background tab, then read the log for `asked host=<closed tab>`.
- Code: `app-desktop/vela-wallet/src/explore/components.rs:286-292`. The close `on_click` never calls `cx.stop_propagation()`, although the comment above says a close "must never merely select", and it is nested inside the tab's `on_click` at `:309-313`. `app-desktop/vela-wallet/src/wallet/page.rs:12353-12366` then runs `Go::Tab` → `show_tab` + `navigate_to`.

**D4: The address bar never takes keyboard focus from the WKWebView** (KEY-FOCUS)
- Result: typed keys and Enter go to the page. A URL can be typed into a dApp's field, the page can see what was typed, and in SC-006b two pastes concatenated into `https://app.aave.comhttps://curve.fi`.
- Repro: open app.uniswap.org (it autofocuses) → click the bar → type `a` → Enter.
- Code: `app-desktop/vela-wallet/src/wallet/page.rs:13352-13373` (`edit_address` calls only `window.focus(&self.address_focus)`). `app-desktop/vela-wallet/src/webview.rs` has no first-responder or focus hand-back (grep shows no match).

**D5: One shared webview for all tabs**
- Result: until commit, a new or restored tab shows the previous tab's live, clickable page with its green connected dot, under a bar naming the new host. Back/Forward walk another tab's history. Titles and the lit state then disagree with the page. This is the G28 class ("the bar names a site the page is not").
- Rows: DX14, DX11, L1, L2–L4, DX4.
- Repro: on the connected test dApp → + → type `app.uniswap.org` → the 0–3 s frames show the test dApp under `app.uniswap.org`. Then load Uniswap in a restored tab and press Back to see 127.0.0.1:8137 under the Uniswap tab.
- Code: `app-desktop/vela-wallet/src/webview.rs:256-274` (one `BROWSER` view: `navigate`, `back`, `forward`). `app-desktop/vela-wallet/src/wallet/page.rs:13726-13733` (`nav_enabled` from the shared `can_go_back`). `page.rs:12392-12399` (`Go::Back` → `webview::back`). RD1 deferred "one WKWebView per tab".

### P2

**D6: The auto-retry schedule restarts** (DX14; also L2 at 02:48:12 and SC-006a at 04:18:48)
- Result: the engine poll sees the wallet's own retry navigation before `Load::Requested` arrives and classes it `how=page`, and `requested()` resets `attempt` and `failure`. The panel vanishes for about 3 s and there are 7 attempts in 42 s.
- Repro: `drop uniswap` → new tab → `https://app.uniswap.org`, then read the log for `retry attempt=2` → `asked how=page`.
- Code: `rust/crates/vela-core/src/app/browser_load.rs:623-644` (`engine_started` overwrites `next_asked = AutoRetry` with `Page`, and `self.url == Some(url)` does not hold because the watched URL is as typed, without the engine's trailing `/`). `app-desktop/vela-wallet/src/wallet/page.rs:13606-13610` (`LoadStep::Navigate` calls `webview::navigate`; the watch hears about it asynchronously at `:13569`).

**D7: Retries are starved while WebKit's provisional load hangs**
- Result: `EngineStillLoading` hands the attempt back forever. There is no visible attempt for about 60 s, recovery after the network returns waits for WebKit's own 60 s timeout (49.6 s in L2), and a skip line is logged every 2–5 s (35 in L2, 25 in DX2, 62 in SC-006a).
- Repro: `blackhole uniswap` → open → watch 60 s.
- Code: `rust/crates/vela-core/src/app/browser_load.rs:767-776`; `app-desktop/vela-wallet/src/wallet/browser_host.rs:795-806`.

**D8: An expired certificate is classed `other`** (L5)
- Result: no certificate sentence, and an automatic retry, against FR-003.
- Repro: open `https://expired.badssl.com/`.
- Code: `app-desktop/vela-wallet/src/explore/probe.rs:32-44` and `:55-83`. ureq/rustls surfaces the handshake error as `ureq::Error::Io` wrapping `rustls::Error`, which `io_code` maps to 0. `pool.rs:973-976` already does the `downcast_ref::<rustls::Error>()` that the probe lacks.

**D9: The chain notice shows only when the whole call gives up** (C1 / G33 not fixed on the desktop)
- Result: the notice comes at 43.7 s, not after the first pass (about 14 s). The pool sets `unreached_chains` after the first pass, but the host reads pool health only after a page read settles.
- Repro: `blackhole 'gnosis|xdai|1rpc'` → Block number.
- Code: `app-desktop/vela-wallet/src/wallet/browser_host.rs:229-244` (`refresh_health` only after `Work` resolves) and `:340-375` (the recheck loop starts only once something is already down). `rust/crates/vela-core/src/app/rpc_pool.rs:1749`.

**D10: The fee comes back more than 15 s after the relay returns, and there are no fee log lines** (S7/S8)
- Result: after the 3/6/12 s steps the cadence is 15 s, plus the quote's own time. SC-003 requires 15 s or less. There is no `fee: quote failed … re-quote #n` or `fee: quote back` line (FR-018).
- Code: `rust/crates/vela-core/src/app/fee_policy.rs:701-711`; `app-desktop/vela-wallet/src/wallet/signing_host.rs:596-640` (no vlog).

**D11: A numeric `value` shows `余额变化 xDAI −0`** (G14-num)
- Result: a 1000-wei delta is rounded to 0 and keeps its minus, against RC4/RC6. The rule lives in the shell, not the core (FR-020).
- Code: `app-desktop/vela-wallet/src/signing/live.rs:387-404` (`signed_amount` → `format_token_amount`).

**D12: The Ethereum fee row blames "Vela 服务" for a public node's rate limit** (DX-G14)
- Result: with no fault set, `eth_getCode … (rate limited)` shows 无法连接 Vela 服务 — 请检查网络, and the slide is shut, so the person cannot send on Ethereum and is told the wrong cause.
- Code: `app-desktop/vela-wallet/src/signing/live.rs:1421-1440` (`fee_row_state`: an unanswered deployment read shows `fee_unreachable`); `app-desktop/vela-wallet/src/wallet/signing_host.rs:75-78`.

**D13: The transaction detail's 哈希 overflows the column** (DX-LD3)
- Result: the 66-character value is clipped by the window edge and its copy button is off-screen (FR-010).
- Code: `app-desktop/vela-wallet/src/flows/components.rs:319-330` (the value div has no truncate or overflow rule); `app-desktop/vela-wallet/src/flows/live.rs:390-398` (the full hash is passed).

**D14: The "don't send it again" trace is easy to lose** (DX-LD3, T181)
- A full-width red 删除记录 is the most prominent control on a 处理中 (may-have-been-sent) record.
- Switching to 钱包 while the column shows the maybe-sent caption drops the column, and it does not come back.
- Code: `app-desktop/vela-wallet/src/wallet/page.rs:14102-14130` (the ending is kept only while `self.panel == PanelId::Signing`).

**D15: A relay-rejected record is misleading** (DX-W3)
- The detail's amount block is empty.
- 接收方 shows the token contract 0xDDAf…7A83, not the transfer's recipient.
- 在区块浏览器中查看 is offered for an op that never reached the chain.
- The caption says 请重试 for a deterministic revert.
- Code: not located; the record fields are built for `eth_sendTransaction` from `to` (see `flows/live.rs` detail builder).

**D16: Home balances disagree with themselves**
- The total reads $8.79 while the listed assets add up to $1.62, with 部分余额仍在更新。 for minutes.
- After relaunching with only Gnosis/1rpc faulted, the home read 24 个网络 RPC 不可用 with an empty asset list and 所有网络 0, while more than 20 other chains answered (chaos-post PASS lines).
- `net: offline` / `net: came back` flapped every ~20 s (T181.err).
- Code: `app-desktop/vela-wallet/src/executor/pool.rs:872-908` (net health is fed by any chain's consecutive failures, and each "came back" invalidates the dashboard). The total/rows mismatch source was not located.

**D17: The tab strip overflows**
- Result: at 6+ tabs the lit new tab and + go past the right edge (DX3′, DX5, SC-006b, SC-007).
- Code: `app-desktop/vela-wallet/src/explore/components.rs:248-250` (fixed `w(px(TAB_W))`, no shrink or scroll).

### P3

**D18: Any popup blanks the page.** The network picker and the ⋯ site menu hide the webview, so the whole dApp page goes blank (U1, SC-007). Code: `webview.rs` visibility toggle (`browser.visible = false`, near `:248-252`).

**D19: ⌘W is not bound, and the close hold is silent.** There is no Close Window menu item. The RD14 hold refuses the first close with no words. Code: `app-desktop/vela-wallet/src/main.rs:443-448`.

**D20: Log hygiene**
- `probe … route=system` is printed while `VELA_DEV_PROXY` is in force (`browser_host.rs:716-724`).
- `fee:`, `in-band:`, `signer page:` and `core: … booting` lines carry no timestamp (`user_op.rs:377`).
- `relay: submitting` is logged twice.
- With a dead proxy, `signer page: … publishes no version … — update the wallet` misreads a network failure (`signer_integrity.rs:81-88` returns an empty list on any fetch failure, then `:160-175`).

**D21: Answers and endings lag the wallet's own knowledge**
- DX-W1: the dApp got the op hash 34 s after the wallet already had the tx hash (`sign_request.rs:601-610` asks only the relay).
- DX6: the sheet still said "may have been sent" after the dApp had its tx hash, and 已确认 shows only about 2 s before the column closes itself.
- Under mute, 正在准备交易… lasts about 40 s (the estimate times out 2 × 15 s).

**D22: Copy and consistency**
- The consent title 连接 sits above 连接到 …, so the verb appears twice.
- The consent and the connected panel say the same thing in two different sentences (G10-like).
- The contract line says 未验证 twice.
- The decoded recipient is lower-case 6+6 (`0x76875e…c0d141`) where the send card shows EIP-55 4+4.
- The caption says 请重试 while the only control is 完成.
- 关闭此页交易会在后台继续 needs a comma.
- The revert sheet's slide stays live after the relay's estimate already said `simulation failed`.

**D23: Small browser defects**
- A tab whose first load failed is restored as 新标签页, so the failed address is lost.
- After a bfcache Back the tab keeps the old title ("Example Domain" over the test dApp).
- Recents 清空 clears with no confirmation.
- The bar accepts `https://app.aave.comhttps://curve.fi` as host `app.aave.comhttps:`, then classes it offline.

**D24: The PARALLEL SPACE badge covers controls (dev only).** It hides a tab's title and ✕ and the + button.

**D25: Spec and harness gaps**
- quickstart C1 expects `outcome=timeout`, but the desktop logs `not connected` for a CONNECT that never answers.
- The quickstart's nonce `eth_call` template has a 24-byte key and reverts.
- `chaos-proxy.py:185-186` applies a leftover `latency` to every fault mode, including mute (the W1b artifact).

## 5. Evidence-quality notes (the testers' claims that needed correcting)

1. The L2–L4 report says 32 skip lines. The log and post-L2-L4.txt have 35. DX2 says 26, and the log has 25.
2. In post-S8-fee-back-after-pass.jpg the labels (p12 = 估算中, p15 = fee) read as though the fee was back within 15 s. They count from the capture script's start, about 4 s after `pass`. The absolute mtimes in post-S7-S8.txt (03:24:25.1 / 03:24:28.3) and the chaos log (re-quote CONNECT at 03:24:23) are what show more than 15 s.
3. DX3′'s "no line reaches chaos-post.log: nothing went direct" does not follow, because a direct bypass would not touch 8899 either. The app's 896 `proxy: 127.0.0.1:9 unreachable` lines and the all-`not connected` reads are the evidence.
4. DX13's "+0.3 s / +2.3 s" and DX-LD3's "row within 4.2 s" have no timestamp in the frames. LD3 relies on post-DX-W1b-T181-LD3.txt and is measured from the verdict, not from the submission.
5. The "✕ → 4001 once" in U5 and DX8 rests on a harness that keeps the last result per method. The events list and the single `dapp:` log lines (for sends) are the only multiplicity evidence.
6. DX6 run 1's 已确认 ending was never captured. The tx-hash answer and the chain are what prove it.
7. The DX-W1 row did not capture the 处理中 Activity state for its own op. T181 did, for nonce 46.

## 6. Environment

- `scutil --proxy` is unchanged against `proxy-before-desktop-stageA.txt` (diffed at audit time).
- The chaos proxy is at pass, and only this app used 8899.
- FR-019 scan over desk-post*.err, chaos-post.log, post-*.txt and post-results.md is clean. All 45 `seed` hits are `bsc-dataseed.binance.org`.
