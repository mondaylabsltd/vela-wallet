# Results (iOS): the 083 findings, checked on iPhone and Simulator

**Pass**: check only, no product code changed. Branch `084-mobile-web-083-parity`, HEAD `112a810e` (rust identical to `c0694047`; `git diff c0694047 HEAD -- rust` is empty). Date 2026-09-29.
**Hand-off**: `specs/083-windows-dapp-browser-stability/handoff-android-ios-web.md` §8 (iOS), §10 (iOS table), §11, §12.

## Where each observation was made

| Where | What | Core that ran |
|---|---|---|
| **Phone**: "ABC" iPhone 11 (`iPhone12,1`), iOS 26.5.2, hardware UDID `00008030-001A75961445802E`, devicectl id `F30282CB-FA41-589E-A3BD-31855EB64414`. The only reachable phone (iPad and 15 Pro were unavailable), so the "which phone" question was not asked. Debug build installed over the top with `devicectl device install app` (Vela Wallet 0.9.5 was there before). | launches with `--console`, Web Inspector JS through ios-webkit-debug-proxy, 3 existing UI tests | xcframework fingerprint `1b72fc93…` (below) |
| **Simulator**: iPhone 17, iOS 26.2 (`1829E1B2-1C84-46F3-A343-FCF8A3905C59`), Xcode 26.3 | the whole hermetic XCTest suite, my probes (real core, real WKWebView, real RpcPool), sheet screenshots | same |

Every simulator result is labelled "sim". No result here comes from the Windows/desktop build.

## Core freshness (proof)

- On arrival `rust/scripts/check-ios-core-fresh.sh` exited **1** (stale: built `011fe473…`, tree `1b72fc93…`).
- Rebuilt with `rust/scripts/build-ios-xcframework.sh` (exit 0, ~14 min, mostly waiting on the shared cargo lock) and `rust/scripts/build-ios-dev-fixtures.sh` (exit 0).
- `check-ios-core-fresh.sh` then exited **0**: "ok — the xcframework is this tree's core (1b72fc9332c9c57eae031761134bf9863b06230fcdf901cce4b10dfd71e3c8dc)". Run again after both rebuilds and before every device run.
- The device binary (`VelaWallet.debug.dylib`, 84 MB) contains 083-only core strings: `but reverted` (1), `not_confirmed` (1), `balance_changes_measured` (1), `would_fail` (2), `spec 083` (1), `dapp_url` (1).
- Core-level tests on this tree (`cargo test -p vela-core --features crux …`): `app_tx_tracker` 26/26, `only_the_operations_own_execution_can_fail_it` ok (R2), `app_fee_policy balance_changes` 3/3, `cable::session` 4/4, `app_sign_request cancelled` 5/5 (D1b safety net).

## Hermetic suite

`xcodebuild … -only-testing:VelaWalletTests` on the iOS 26.2 sim: **931 tests, 120 suites, 928 passed, 0 failed, 3 skipped**. `CoreWireDriftTests` and `BrowserWireDriftTests` both **passed** on the rebuilt core. The doc said they "may flag" a new 083 field; they do not. See "Doc errors" for why (the shell does not decode the new outcome variants at all).

## Results table

Verdict words: **present** = the finding reproduces here; **absent** = it does not; **handled-confirmed**; **HELD-money**; **HELD-owner** (Face ID / second phone / a person's tap on the phone); **HELD-tap** (needs a UI driver: nothing available that can tap without the Mac's shared mouse, see "Blockers"). `tx` = none: no transaction, hash or slide anywhere in this pass.

| ID | Doc predicted | Observed (verbatim where it is on screen) | Verdict | Evidence |
|---|---|---|---|---|
| **I-S3b** | likely P0 | `UserOpSpine.submit` with relay `rpcError -32521 "AA25 invalid account nonce [existingHash:0x1111…1111]"` **returned `0x1111111111111111111111111111111111111111111111111111111111111111`** (the marker). `port.calls = [eth_getCode, eth_estimateUserOperationGas, eth_sendUserOperation]`, so the run reached the send. | **present** | `probe-log-hermetic…txt` (I-S3b); device: HELD-money |
| **I-S2** | likely P0 | `SignExecutor.perform(sign_and_submit)` with receipt `{success:false, receipt:{transactionHash:"0xabc"}}` answered **`{"result":"0xabc","type":"succeeded"}`**. `calls` include `eth_sendUserOperation` and `eth_getUserOperationReceipt`. | **present** | same log (I-S2); device: HELD-money |
| **I-U1** | likely P0 | Shell side: `balance_changes_measured` / `BalanceChangesMeasured` appears **nowhere** in `app-ios/VelaWallet` (grep). Core side: the 3 `balance_changes_*` fee-policy tests pass on this tree, so the missing input is the shell's. The Max sheet itself was not driven (needs USDC ≥ 0.2 and a Uniswap Max). | **present (code + core tests)**; sheet: HELD-money | `cargo-feepolicy.txt` |
| **I-S3** | likely P1 | `SignExecutor(receiptWaitMs:300, receiptPollMs:50)` with receipt `null`: `{"type":"receipt_pending","user_op_hash":"0xop"}` after 315 ms (3 polls). | **present** | log (I-S3) |
| **I-R3** | likely P1 | (a) `aftercareReceipt(.stillConfirming, track 'dropped'/'final')`: stage failed, title «失败», captions `["发送 · −0.001 XDAI", "交易未能提交。您的资金安全无虞——请重试。"]`, hash nil, explorer nil. (b) `receipt(error .submitFailed, detail "The transaction was included but reverted (0xabc)")`: the same generic sentence, no 0xabc. GOOD text exists but is unused: «转账在链上被回滚 —— 你的资金没有送出,网络费可能仍被扣除。可点下方「浏览器」查看失败原因,或返回重试。» | **present** | log (I-R3a, I-R3b) |
| **I-U1b** | likely P1 | `FeeExecutor.simulate` answers `simulation_failed` for a refusal (code, `FeeExecutor.swift:214`). Sheet model for `estimate_failed`: fee row 网络费 / **点击重试**, warning **«无法连接 Vela 服务 — 请检查网络，稍后会自动重试。»**, tappable. For `would_fail` (never sent by this shell) the row is 点击重试 with **no** sentence at all; the doc's GOOD sentence 这笔交易预计会失败 is not drawn. | **present** | log (I-U1b/c) |
| **I-U1c** | likely P1 | With a failed quote and 2 coins the model has `chevron=true` and a refresh «刷新费用»; `feeTapped` (code) re-quotes and never opens the list when `fee.failed != nil`. The tap itself was not driven. | **present (model + code)**; tap: HELD-tap | log (I-U1b/c) |
| **I-H2** | likely P1 | Records written exactly as `SignExecutor.recordRow` does, run through the real activity core and `WalletLive.activityRow`: dust row «已发送» / «至 0x7687…D141» / «−0.001» «xDAI»; swap row «已发送» / «至 0xd614…9c40» / **«−0»** / unit «». Detail: title «已发送 », amount «−0», facts `接收方=0xd614…9c40, 网络=Base, 哈希=0x09c3…4c9f`. Neither names the site. `toWire` keys carry no `dapp_url`, no `intent`. Core half exists: with the record decoded raw the core emits `"dapp":{"intent":null,"intent_term":null,"site":null}` and `FeedItemWire` has no `dapp` field. | **present** (exactly the doc's "BAD with an 083 core") | log (I-H2, I-H2/F1/F3, I-H2core); device swap row: HELD-money |
| **I-REC** | likely P1 | `recordRow` for a `wallet_sendCalls` whose top level says `to 0x…dEaD, value 0xffffffffffffffffffff`: record **to = 0x000000000000000000000000000000000000dEaD, value = 0xffffffffffffffffffff**, `dappOrigin` set, `dappUrl` absent. The sheet is honest (it draws the calls leg: 发送 −0 xDAI 接收方 0x88cC…6894). | **present** | log (I-REC); `I-REC-sim-forged-batch-sheet.png` |
| **I-D1b** | via core | Core: `a_close_then_a_cancelled_prompt_answers_4001_once` and 4 siblings pass on this tree. Shell: `SignExecutor` reports `passkey_cancelled` (code). The Face ID half cannot run. | **core confirmed**; live: HELD-owner | `cargo-d1b.txt` |
| **I-R2** | via core | `only_the_operations_own_execution_can_fail_it` ok; `app_tx_tracker` 26/26. Shell sends `receipt_with_logs` with the bundle's logs (code). | **handled-confirmed (core)**; device swap smoke: HELD-money | `cargo-r2.txt`, `cargo-tracker.txt` |
| **I-W20** | via core | `cableConnectUrl(staticSeed 32×9, qrSecret 16×7, advert routing ABCDEF)` from the Swift binding of this build → `wss://cable.auth.com/cable/connect/ABCDEF/0539BBA5AFC914678133142ACA279E2B` (upper-case). | **core confirmed**; two-iPhone ceremony: HELD-owner | log (I-W20) |
| **I-U8** | likely P2 | The Send dust sheet on the phone offers 速度 超快 (default), 标准, 较慢 all priced «0.01 xDAI · ≈¥0.07» (Gas 出价 17~43 / 16~29 / 15~21 wei). The 14× ratio needs the on-chain cost of a real op. | **not measured**: HELD-money | `I-U8-device-speed-picker-open.png` |
| **I-F1** | likely P2 | Same record path as I-H2: no `assetChanges`, no `calldata` flag stored; approve carries no `balance_changes` (code). Row would read «−0». | **present (builder level)**; device: HELD-money | log (I-H2) |
| **I-F3** | likely P2 | Detail of the router row: **«接收方=0xd614…9c40»**; the dust row also «接收方». | **present** | log (I-H2/F1/F3) |
| **I-H4a** | likely P2 | (1) `receipt(kind personalSign, error submitFailed)`: «失败» + **«交易未能提交。您的资金安全无虞——请重试。»** + «完成». (2) `eth_signTypedData_v4` with `primaryType:"Missing"`: the real core sheet is **slide-enabled** («Missing» headline, 此结构化数据无法通过已知描述符解码…, 签名对象 未验证合约, 无网络费用 — 链下签名, slide 滑动以确认 · 签名); `SignExecutor.perform` then answers `{type:failed, message:"eth_signTypedData_v4 carried nothing this wallet could sign"}` with **0 relay calls**. Free trigger confirmed; the slide itself was not slid. | **present** | log (I-H4a, I-H4a-exec); `I-H4a-sim-unhashable-typed-data-sheet.png` |
| **I-H4b** | likely (low) P2 | needs a non-cancel Face ID error on the owner's account. | **HELD-owner** | |
| **I-B5792** | partially P2 | On the phone page: `wallet_getCapabilities` → **`ERR 4200 Vela does not support wallet_getCapabilities`**, `wallet_getCallsStatus` → **4200**. `wallet_sendCalls` through the executor answers `{type:succeeded, result:"0xdef"}` (a bare tx hash), not `{id}`. Page-level result needs the Bundle send. | **partially confirmed**; Bundle: HELD-money | log (I-B5792); phone inspector |
| **I-W13** | likely P2 | `RpcPool.call` (the dApp path) with the person's own Gnosis RPC = a node that accepts and never answers: **read 0: 8525 ms, read 1: 301 ms, read 2: 305 ms**. (Sim; a local silent https node instead of the chaos proxy.) | **present** | log (I-W13) |
| **I-H1** | likely P2 | `methodCopy` (used by the sign-in sheet and the create chooser): 这台设备 → **«Touch ID 或 Windows Hello»**; 手机或平板 → **«扫码，用附近设备创建»**. | **present** | log (I-H1) |
| **I-H5** | likely P2 | needs the second phone. | **HELD-owner** | |
| **I-H8** | likely P2 | Real WKWebView, tab on the test dApp, then a failed 2nd navigation to expired.badssl.com: `url=http://127.0.0.1:8000/ host=127.0.0.1:8000 title=«Vela test dApp (Android)»`, `failedURL=https://expired.badssl.com/`, `failure=certificate`. `onMeta` was sent `https://expired.badssl.com/` with the OLD title, then re-pointed to the old site. | **present** | log (I-H8) |
| **I-W2** | partially P2 | During a 6 s slow load of another host, at t+0.3 s and t+3.0 s: `url/host = 127.0.0.1:8002` (the NEW host at once), `title = «Vela test dApp (Android)»` (the OLD title). `onMeta(new url, old title)` sent at once. Caret-at-end/append in the address bar not driven. | **partially confirmed**; typing: HELD-tap | log (I-W2) |
| **I-W6** | partially P2 | Tapped `target=_blank` (gesture): `createWebViewWith` runs, the engine loads it **in the same tab** (dApp replaced). `window.open()` with gesture returns **null**, same replacement. No gesture: `window.open` → null and `a[target=_blank].click()` → nothing (both also on the phone). | **partially confirmed** (matches "today") | log (I-W6g, I-W6n); phone inspector |
| **I-W7** | partially P2 | Tapped `mailto:`, `itms-apps:`, `shortcuts:`, `tel:`, `sms:` → shell policy **handToSystem** (leaves the app, no Vela prompt). `location.href='mailto:'` / `'tel:'` → `type other` → cancel (good). Downloads (sim, iOS 26.2): `a.bin` replaced the dApp with a raw-file view (`webView.url` = the file, empty title); `a.zip` (attachment) left a blank document; **no panel and no failure**, tab named 127.0.0.1:8002. Phone fresh tab: no `browser load failed` line for either. | **partially confirmed** | log (I-W7g, I-W7dl); `phone-console-dl*.txt` |
| **I-W10** | handled P2 | Sim and phone Send dust sheet: «发送», «−0.001 xDAI», «接收方 0x7687…D141», slide «滑动以确认 · 确认发送», no 无法解码. | **handled-confirmed** | `I-W10-device-send-dust-sheet.png`, `I-W10-sim-send-dust-sheet.png` |
| **I-W11** | partially P3 | Code: `signingStarted: {}` (SigningController.swift:246). Model: `isSubmitting` → title «提交至网络...» (existing tests). Face ID part not run. | **partially**; Face ID: HELD-owner | |
| **I-EXE** | likely P3 | Sim (Base, Universal Router `execute`): headline **«Execute»** (English, amber), «根据函数签名解析得出 — 非经过验证的描述符，请仔细核对细节。», slide «滑动以确认 · 确认». | **present** | `I-EXE-sim-router-execute-sheet.png` |
| I-F2 | handled P3 | Detail fact «哈希 = 0x7ca7…9269» (short form, mono). Not seen on screen. | **handled-confirmed (model)**; screen: HELD-tap | log |
| I-W9 | handled P3 | Phone: launch with a real page, then relaunch **without** `VELA_URL`: 0 inspectable pages after 15 s (the tab does not load itself). Start-page visuals not seen. | **handled-confirmed (behaviour)** | phone inspector, `phone-console-nourl.txt` |
| I-W3 | handled P3 | Sim, real refused connection: `failure=offline`, retries at +2 s, +5 s, +10 s, then stops; panel words «网络不稳定，页面没能打开。» / «正在重试…». Phone: cert → `-1202 → certificate`, no retries; nohost → `-1003 → not_found`. The `drop`/`blackhole`/recovery cases through the fault proxy were not run. | **handled-confirmed** for the classes and schedule; proxy cases: HELD-owner | log (I-W3); `phone-console-*.txt` |
| I-DNS | handled P3 | Phone, plain Wi-Fi: `vela-083-nohost.invalid` → **`NSURLErrorDomain -1003 → not_found`**. On the Mac sim (behind the Mac's local proxy) the same host never failed in 45 s and later ended `-1200 → certificate`. | **handled-confirmed (phone)** | `phone-console-nohost.txt` |
| I-W4 | handled P3 | Phone and sim: expired / self-signed / wrong.host / untrusted-root → **`-1202 → certificate`**, `autoRetry=false`, no retries (reason «网站证书有问题，Vela 已阻止打开。»). Pill lock not seen. | **handled-confirmed** | logs |
| I-W5 | handled P3 | Sim: killing the WebContent process fires `onRendererGone` once; `reload()` gives a working page (`typeof ethereum = object`). Panel and sheet-closing not seen. | **handled-confirmed (engine)**; panel: HELD-tap | log (I-W5) |
| I-D1 | handled P3 | Phone, existing UI test `testTheDappSheetOffersASpeedAndNeverSignsOneItLeft`: after `swipeDown(.fast)` the page got **no 4001** within 30 s: the test failed at its own line 481 (`dismissing the sheet did not refuse the page`). That test is stale (asserts the pre-079 swipe rule). Hermetic `theCloseBeforeTheApprovalAnswersThePageOnce` passes. | **handled-confirmed** (swipe never answers); ✕ on device and background/foreground: HELD-tap | `phone-ui-tests-sheet-only.txt`, `I-D1-device-send-dust-speed-swipe-test.mp4` |
| I-W14 | handled P3 | Sim consent sheet: «连接到 127.0.0.1:8000», identicon + «Parallel One 0x88cC…6894 切换账户 ›», «网络 Gnosis ›», buttons «拒绝» / «连接». Phone: the UI test connects and returns `["0x88cCA0EeDbF2C4426110bbFc998F048689266894"]`. Account switch not driven. | **handled-confirmed** | `I-W14-sim-connect-consent.png`, `I-W14-device-connected.png` |
| I-W15 | handled P3 | needs taps (Back per tab). | **HELD-tap** | |
| I-W19 | handled P3 | owner's phone key. | **HELD-owner** | |
| I-W1, I-U1d, I-D3, I-W12 | n.a. | not run. | n.a. | |

### §10 "Also noticed" (iOS)

| # | Observed | Verdict |
|---|---|---|
| I-1 | Sheet for `{to: Gnosis USDC, data: approve(0x1111…,0), calls:[{to: founder, value:0x1}]}`: headline **发送 / −0.000000000000000001 xDAI / 接收方 0x7687…D141** (the stray leg) while `callsOf` signs `[0xDDAf…7A83 v=0 d=0x095ea7b3]`. The allowance editor beside it reads the real approval (授权上限, 被授权方 0x1111…1111) and holds the slide shut, which is why the sheet is not a fully calm 发送. | **present** |
| I-2 | `wallet_sendCalls [1 wei to 0x…dEaD, USDC transfer]`: sheet **发送 / −0.000000000000000001 xDAI / 接收方 0x0000…dEaD**, slide live «确认发送»; the second (token) leg is drawn nowhere. `callsOf` signs 2 legs. | **present** |
| I-3 | `input` calldata dropped: sheet **发送 / −0 xDAI / 接收方 0xDDAf…7A83**, slide live; signed `data=0x`. | **present** |
| I-4 | JSON-number `value: 1000000000000000`: sheet **发送 / −0 xDAI / 接收方 0x7687…D141**, slide live; signed value 0, record value 0x0. | **present** |
| I-5 | `SendExecutor` uses the same `spine.submit` (code); the I-S3b probe exercises that function. | **present (shared path)** |
| I-6 | tracker vs sign-path record race: code-read only (both call `TxRecords.patch`); needs a real revert. | **not measured** |
| I-7 | bare `PasskeyExecutor` for dApp signing (code). | **code-read**; live: HELD-owner |
| I-8 | `RelayClient.userOpReceipt` of a reverted op → `confirmed=false, txHash=0xabc`; the `resolveUserOp` closure (RootView.swift:411-415) returns the tx hash whatever `confirmed` says (code-read). | **present** |
| I-9 | Swift bindings export `parseExistingUserOpHash`, `userOpSafeOpHash`, `relayErrorMessage`; **not** `userOpHash`, `existingOp`, `opExecutionFailed`, `revertedTransaction`, `pageWaitCapMs`, `isHedgedRead`, `earlyVerdict`. | **confirmed** |
| I-10 | **Phone (iOS 26.5.2)**: a page script clicking a link 1.5 s after any gesture, with nobody touching the phone, **launched the App Store** (`itms-apps://…`) and then **Mail** (`mailto:` link on the page). Sim: `linkActivated → handToSystem` for `mailto`, `itms-apps`, `sms`, `tel`, and for a freshly created `<a>` appended and clicked. | **present, on device** |
| I-11 | Cross-origin iframe (port +1): a **tapped** `target=_blank` link inside it **navigated the whole tab** (`createWebViewWith`, sourceFrame = sub@8001; tab ended on `…/?opened-by-frame=1`); a tapped `target=_blank itms-apps` inside it → handToSystem. Without a gesture, `_blank` from the frame does nothing (WebKit blocks it). Frame `mailto:` is cancelled. | **present** (one tap) |
| I-12 | pill lock: needs the screen. | HELD-tap |
| I-13 | `BrowserEngine.swift:634-635` says the system still asks; Mail and the App Store opened on the phone with no prompt at all. | **present** |
| I-14 | Sim, real WebKit against a host that accepts and never answers: panel appears after **60.5 s** (`-1001 → timeout`, `explore.loadOffline`, first retry pending 2000 ms). | **present** |

## Extra observation (not in the hand-off)

`approve(spender, 0)` (a revoke) on Gnosis: sheet «授权» / «金额 0 USDC» / «被授权方 0x111111…111111», and the allowance editor reads **«无限额»** in red, slide shut until a chip is chosen (`I-sim-approve-zero-reads-unlimited-sheet.png`). Possibly the guard treating 0 as "no finite cap"; the hand-off does not mention it.

## Test-probe files (all new, all under `app-ios/VelaWallet/VelaWalletTests/`, no production file touched)

- `Probes083ParityTests.swift`: `Probes083Bytes` (S3b, S2, S3, B5792, R3, H4a, I-1…I-4, REC, H2 mapping), `Probes083Feed` (feed rows/detail, raw core feed, I-8, H4a executor), `Probes083Misc` (H1, W20, U1b/U1c), `Probes083Pool` (W13).
- `Probes083BrowserTests.swift`: `Probes083Browser` (W6, W7, I-10, I-11, downloads, H8, W4/DNS, W2, W5, I-14; serialized) and `Probes083Panels` (W3).
- `Probes083SheetTests.swift`: `Probes083Sheet` (the real sheet text for W10, I-1…I-4, EXE, H4a).

They write "PROBE083|…" lines to stdout and to a file in the session scratchpad, and assert only that the scenario reached the step it claims to measure (e.g. `eth_sendUserOperation` was called). They are uncommitted.

## Dirty tracked files this pass left (do not revert; not committed)

- `app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift` (3 insertions, 2 deletions: the doc comment on `browser_load_classify` and its checksum `39421 → 32590`).
- `app-ios/VelaWallet/VelaWallet/Dev/vela_dev_fixtures.swift` is **unchanged** (the hand-off expected a diff).
- Other dirty paths in the tree (`assets/wasm/…`, `rust/pkg-web/…`, Android probe tests) belong to the sibling agents.

## HELD

**HELD-money** (nothing was spent; no slide, no top-up, no Ethereum):

| Item | Exact spend |
|---|---|
| I-S3b device (optional) | Gnosis, 2 × 0.001 xDAI to `0x76875e38…D141` plus 2 fees (each < $0.01). The hermetic result already decides it. |
| I-S3 device (optional) | Gnosis Send dust 0.001 xDAI + fee (< $0.01), Airplane Mode for 125 s. |
| I-S2 device (optional, flaky) | Base router call with a 40 s deadline, slid 3–5 s before it: a few cents if it reverts, nothing if refused. |
| I-B5792, I-REC | Gnosis `wallet_sendCalls`, one call `{to:acct,value:'0x0'}`, fee only: about < $0.01 each. |
| I-H2 / I-F1 / I-F3 / I-U8 / I-R2 smoke | One Base 0.05 USDC → ETH swap (about $0.08 fee, ≈ 0.135 USDC total). MultiTest holds about 0.035 USDC, so first ONE ETH → USDC 0.0001 swap (0.0001 ETH plus about 0.000031 ETH fee) as the top-up. Plus one Gnosis Send dust for the dust row. |
| I-U1 sheet + slid Max | After the top-up, the sheet check costs nothing; the slid Max USDC → ETH is the last step (about $0.08). |

**HELD-owner** (what the owner must do on the phone): Face ID: I-D1b, I-H4b, I-W11 Face ID part; second phone: I-W19, I-W20 live, I-H5; the fault-proxy items I-W13 (device), I-W3 drop/blackhole/recovery need 设置 ▸ 无线局域网 ▸ 配置代理 ▸ `192.168.50.17:8899` (a Settings tap) and the chaos lock (the proxy on 8899 was held by another agent all session; I never took the lock or set the phone's proxy).

**HELD-tap** (no driver): I-W15, I-W2 typing/caret, I-D1 ✕ and background/foreground on the phone, I-U1c tap, I-F2 on screen, I-12 lock, I-W5 panel, I-W14 account switch. A person can do them with the phone in front of them; the launch line and the test page are in the hand-off.

## Doc errors and surprises

1. §8.1's `cargo test -p vela-core op_execution` and `--test app_tx_tracker` need `--features crux`: without it the first fails to compile (`vela_core::app` is gated) and the second reports **0 passed** (a false green). The R2 test is `app::tx_tracker::execution_failure::only_the_operations_own_execution_can_fail_it`, not a name matching `op_execution`.
2. The drift tests do not fail after the rebuild (the doc said they may). The only regenerated Swift file differs by a doc comment and a checksum; `vela_dev_fixtures.swift` did not change at all.
3. I-W7 step 5 predicted a 无法加载此页面 panel over the dApp for a download; on iOS 26.2 (sim) and 26.5.2 (phone) no failure is reported: WebKit either shows the file or leaves a blank document, and the address bar re-points to the file host.
4. I-W6 step 3 (no gesture) is GOOD on both sim and phone; only the tapped case replaces the dApp.
5. The existing UI test `BrowserAcceptanceTests.testTheDappSheetOffersASpeedAndNeverSignsOneItLeft` is stale: it still expects a swipe-down to refuse the page (pre-079). It fails on the phone at line 481, which is the evidence for I-D1. The whole `BrowserAcceptanceTests` class is in the scheme's skip list, so `-only-testing` runs 0 tests; a copy of the generated `.xctestrun` without the skip list was used (kept in the scratchpad, repo untouched). `DappBrowserStabilityProbeTests.testProbeTheBrowserCheckpoints` has a `slide` helper and may send Send dust: it was not run.
6. §8.1's page-devtools recipe needs Safari's Web Inspector; here `ios_webkit_debug_proxy` over USB worked on the phone (the page list needs the Target protocol, wrapped `Target.sendMessageToTarget`).
7. **The iPhone 11 was in use by a third party.** Another session (working tree `/Volumes/data/production/vela-wallet`) ran `DappBrowserStabilityProbeTests` and a WebDriverAgent (`iproxy 8100`), and a `devicectl --console` launch with `VELA_DEV_PROXY`, on the same phone in the parallel space during this pass (10:34–10:40). My first two launches overlapped its start. Every later device step was preceded by a `ps` check for other `devicectl`/`xcodebuild` sessions (0 each time).
8. Port 9222 (the usual ios-webkit-debug-proxy port) is taken by the Android agent's `adb forward`; I used 9260. Port 8137 (the UI tests' fixed server port) is taken on the Mac by the Android agent, so the existing UI tests cannot run on the simulator.

## Blockers

There is no way to tap on the phone or in the Simulator that does not use the Mac's shared mouse (`HIDIdleTime` 0.09 s: a person or another agent is using it), so no `cliclick`; WDA on the phone belongs to the other session and was not driven. Sheets on the simulator were opened by a page that requests them on load, with a grant for the sim-only origin written into the simulator's UserDefaults; each was captured by `simctl io screenshot` and the app terminated (equivalent to a refusal: nothing signed).

## Final state left

- Phone: Debug build of this branch installed over the top; last launched with `VELA_PARALLEL_SPACE=0` (owner's real account). The Mail and App Store processes my I-10 check launched were terminated with SIGTERM, came back as background processes twice, and stayed down after SIGKILL (a compose draft to `test@example.com` was never sent; whether Mail still holds one could not be seen). Web Inspector proxy stopped. No Wi-Fi proxy was ever set; no chaos lock taken.
- My servers on 8000/8001/8002 stopped (own PIDs only). Simulator shut down.
- Git: on `084-mobile-web-083-parity`, nothing committed, pushed, or reset.

## Evidence (`specs/084-mobile-web-083-parity/evidence/ios/`)

Phone: `I-W10-device-send-dust-sheet.png`, `I-U8-device-speed-picker-open.png`, `I-U8-device-speed-picked-slow.png`, `I-device-approve-unlimited-sheet.png`, `I-device-approve-unlimited-capped-revoke.png`, `I-W14-device-connected.png`, `I-W14-device-already-granted.png`, `I-D1-device-send-dust-speed-swipe-test.mp4`, `phone-console-*.txt`, `phone-ui-tests-sheet-only.txt`.
Sim: `I-W14-sim-connect-consent.png`, `I-W10-sim-send-dust-sheet.png`, `I-1-…`, `I-2-…`, `I-3-…`, `I-4-…`, `I-REC-…`, `I-EXE-…` (2), `I-H4a-…`, `I-sim-approve-zero-reads-unlimited-sheet.png`.
Logs: `probe-log-hermetic-XCTest-sim-iOS26.2.txt`, `probe-log-browser-XCTest-sim-iOS26.2.txt`, `xctest-baseline-VelaWalletTests-sim.txt`, `cargo-*.txt`.
