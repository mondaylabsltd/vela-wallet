# 082 post-fix device pass, iPhone (post2): adversarial audit

- Audited: 2026-09-29, after 10:45 CST. Build under test: Debug `fb8c7026` (Swift) + core `69db6e4d` (round 1) for every row except E-probe, which ran on a `74192155` (round-2) test-host build.
- Inputs: `quickstart.md` §4 and §8, `spec.md` (FR-001…FR-020, SC-001…SC-010), `research.md` (RA2, RA4, RA10, RA12, RE1–RE13, RF1, RF4, RF5, RG3, RG4, RJ1–RJ4, RJ12–RJ16), `post-results.md` (stage A and stage B), and every `post2-*` file in this folder. I opened all 42 images and read all 22 text files. Logs: `scratchpad/logs/ios-post2*.log` (10 files plus their `.stamped` copies), `chaos-ios.log`, and `scratchpad/shots/ios-b-stale-detail.png`. No `ios-post2.logarchive` exists, because `log collect` needs root. I also read the desktop and extension `post-audit.md`, and the source at `fb8c7026` (by `git show`) and at HEAD `62346811`.
- Method: I used the same rule as the desktop and extension audits.
  - **confirmed**: the evidence shows the row's expectation is met.
  - **refuted**: the evidence shows it is not met, including rows the testers themselves marked fail or partial.
  - **unproven**: nothing shows the key claim.
- I edited no code and made no commits, and I did not touch the phone. This file is the only thing I wrote, apart from a read-only chain script (`scratchpad/audit-ios-money.py`).

## 1. Verdict

| | rows |
|---|---|
| confirmed | 23: E-G28b, E-G28c, IX2, E-Stop, E-W5-back, E-G31, L5, C2, S7/S8 (automatic leg), IX-W1, T183 (post-verdict quit only), IX8, S5, I-G14, E-G9/G10, E-G25, E-G8, E-G26, E-L1, E-LD3, E-probe (on `74192155`), SC-006b, SC-007-en |
| refuted | 10: E-G28a (hairline leg), C1, IX6, E-G12, IX7-probe, E-W23, SC-006a, X-STALE, X-HISTORY, X-DEADPROXY |
| unproven | 9: IX0/IX1, IX7, E-W20, E-LD6, E-LD7, X-FIRST-TAP, post2-I1, post2-I4, post2-I5 (the last three were not run) |

post2-I2 and post2-I3 map onto IX7-probe and IX-W1 (round-1 behaviour, as expected).

The testers' verdicts mostly hold. Four claims do not survive:

1. **E-G28a's "the retry schedule restarts by itself (same class as desktop G43)" is refuted.** Only two paths log `retry #0`: `reload()` (the Retry button or ⋯ 刷新) and `networkCameBack()`. The second always logs `[net] net came back`, and that line is absent. At 07:22:35–40 the app went to the background: 32 sockets reported "Software caused connection abort", and a keyboard snapshot was taken "not in a visible window". Someone tapped 重试 on return. A manual Retry restarting the count is documented behaviour (`BrowserEngine.swift:274-285`), not the desktop's G43 race.
2. **X-STALE's "RA4 says this should be 失败" is wrong.** RA4's `not_found` rule covers may-have-been-sent entries only. This op's admission is unknown, and whether the record even carries a hash is unknown (the store was not read). Ethereum `getNonce(Safe,0)` is still **1**, so an op built at nonce 1 stays signature-valid. The honest ending is the core's 24 h *Unknown*, not "failed / nothing sent". The defect is real: 8 days of 处理中.
3. **X-DEADPROXY's "保存并重试 would put Mantle's node on Polygon" overstates the harm.** The network editor refuses a save whose RPC reports another chain id (the Polygon page says 仅当该 RPC 报告的链 ID 与本网络不符时才拒绝保存). The wrong prefill and the error tone before any input are real. The mechanism the tester named (`openRescue` reading the draft "before expandNetwork") is not shown: that code looks the URL up by the same chain id.
4. **The end state "app relaunched with the standard JSON and left on 钱包" is contradicted.** `ios-post2-B-final.log` (launch 10:39:49) ends with `App terminated due to signal 15`. The app is probably not running now. The installed binary is the `74192155` probe host, which is the likely trigger of the 「无法验证App」 prompt the owner now sees.

**Release blockers on the tested build:**

- **P0 (latent, code-verified, not exercised on the phone).** `fb8c7026` has no write-ahead: the dApp record is written only after the relay's verdict. A force-quit during the 32 s 提交至网络… POST loses the record while the op can land. This is desktop D1 on the phone. T183 quit *after* the verdict, so it does not cover this. post2-I1 was not run. HEAD `8cd8adc4` adds the write-ahead, which is not yet verified on the device.
- **P1.** A relay-rejected op is answered `ok` + op hash (IX7-probe). The same defect is desktop D2 and extension D6.
- **P1.** Closing a transaction detail kills History's rows and its back arrow (X-HISTORY). The code is unchanged at HEAD.

## 2. Money (recomputed from chain)

Query: Gnosis `eth_getLogs`, EntryPoint `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, topic0 `0x49628fd1…e1419f`, topic2 = Safe `0x88cCA0EeDbF2C4426110bbFc998F048689266894`, left-padded. Blocks 48489919–48492419 (07:11:50–10:43:30 CST, the whole pass) in five chunks of 500 via `rpc.gnosischain.com`. Script: `scratchpad/audit-ios-money.py`.

**Result: 11 events, nonces 51–61, all `success=1`, none with nonce ≤ 50, no duplicate.** `getNonce(Safe,0)` = **62**. xDAI balance = **0.12867** = 0.24967 − 11 × (0.001 + 0.01 in-band fee). `actualGasCost` is 0 everywhere, because the fee is paid in-band.

| nonce | block | landed | op | tx | row | where the op is in the phone's log | UI / dApp |
|---|---|---|---|---|---|---|---|
| 51 | 48491208 | 09:00:55 | 0xdd6ad0a27e…4ae8091c | 0x9ce6994d6d…cd81047f | IX-W1 | B-main | maybe-sent → 已确认 (chain check); dApp ok = op hash ✓ |
| 52 | 48491275 | 09:06:35 | 0x7621ab25a7…faa7a07d | 0xae648e0ffa…1da78d34 | T183 run A | B-main, T183 | maybe-sent → quit → confirmed after relaunch ✓ |
| 53 | 48491312 | 09:09:45 | 0x03033a6400…b43d9d0a | 0x856d368b58…4f7ce48c | T183 run B | T183, T183b | 处理中 after relaunch → confirmed ✓ |
| 54 | 48491347 | 09:12:45 | 0xbfc819e3b7…6e505583 | 0x5bc5fcc475…0274fc50 | IX8 #1 | T183b | maybe-sent → 已确认; dApp tx hash ✓ |
| 55 | 48491389 | 09:16:20 | 0xf2ccdd39f2…91d4fb2c | 0xa49288e1e9…32299a12 | IX8 #2 | T183b | maybe-sent → 已确认; dApp tx hash ✓ |
| 56 | 48491398 | 09:17:05 | 0xfab9921774…21faf531 | 0xc3115c330f…418ab826 | IX8 #3 | T183b | accepted → 已确认; dApp tx hash ✓ |
| 57 | 48491555 | 09:30:20 | 0x343d54504f…0f17ada4 | 0x2b8dfc0bac…8ebbc8fa | E-G26 #1 | T183b | 已确认 ✓ |
| 58 | 48491593 | 09:33:35 | 0x425853bff4…ce23db5d | 0xc2ba2e5ab3…2aa69580 | E-G26 #2 | T183b | 已确认 ✓ |
| 59 | 48491676 | 09:40:30 | 0x08e3a7957b…53ce7079 | 0xce1adfffb2…0863a9b6 | E-G26 #3 | T183b | 已确认 ✓ |
| 60 | 48491703 | 09:42:45 | 0xcc307e63cd…8d4d5b90 | 0x2709196554…4321953f | E-LD3 | T183b | 处理中 → confirmed ✓ |
| 61 | 48492348 | 10:37:20 | 0x15ed679452…80eeacae | 0xf7ffc22b47…c56d675f | E-probe checkpoints | not in any app log; `post2-E-probe.txt` (the XCUITest window, 10:34–10:38) | 已确认 (round-2 build) ✓ |

- **Never landed, and can never land now.** Each of these nonces was consumed by a later op.
  - S5 `0x8163c4b5…` (nonce 57, dropped CONNECT) and IX7-probe `0xd9f29715…` (nonce 57, relay-rejected) were both superseded by E-G26 #1.
  - The E-W23 relay drop `0x7abc8a96…` (nonce 61) was superseded by the E-probe op.
  - All three were called 失败 / not sent by the UI.
- **No op the UI called failed or not sent landed, and nothing was sent twice.** There is no P0 in the money chain.
- **One "told ok, never landed".** The IX7-probe dApp got `{ok:true, result:0xd9f29715…}` at 09:23:28, 105 s after the sheet said 失败. That is P1, below.
- Stage A (07:12–08:50) produced no event, so the nonce stayed 51, as the tester said.
- Ethereum: `getNonce(Safe,0)` = 1 (re-read via publicnode). The 2026-09-21 "处理中" op (X-STALE) never landed. **Residual risk:** if it was built at nonce 1 it is still signature-valid until Ethereum nonce 1 is used, so it must not be called "failed / nothing was sent" without the relay's `not_found`.
- The desktop fixture uses the **same Safe**. Its nonces are ≤ 50 and none falls in this window, so no desktop op is mixed into the table above.

## 3. Rows

| id | tester | audit | why |
|---|---|---|---|
| IX0 / IX1 | partial | unproven | CONNECT leg ✓: `chaos-ios.log` 07:18:58 `PASS ethereum-data.getvela.app` 1 s after the 07:18:57 launch. The syslog leg as written never ran: `ios-post2-syslog.log` is connect/exit only, and `log collect` needs root. The `OS_ACTIVITY_DT_MODE` console substitute is sound. Freshness holds only against round 1: the build is `fb8c7026`, STALE against HEAD, so every row below measures round-1 code. |
| E-G28a · G28 | partial | **refuted** (hairline leg) | ✓ Bar keeps 🔒 jumper.xyz over the live page (strip 0.6 / 20.1 s). ✓ Panel at 20.1 s, `after=20106ms`. ✓ Words exact and no lock at 21.6 s. ✓ 正在重试… busy and full colour at 23.1 s. ✓ Retries at stall +2.1 / +5.2 s. ✗ "A moving hairline": the frames at 0.6 s, 20.1 s and 23.1 s all show the same ~10 % sliver. The tester's second defect (the schedule ran twice) is refuted as a product defect; see §1.1. |
| E-G28b | pass | confirmed | Page-initiated load via the page's own script (same path as Web Inspector). The bar keeps 🔒 192.168.50.17:8140 at 0.8 s and 20.4 s. Panel at 20.6 s (`after=20607ms`). The bar reads app.uniswap.org with no lock at 21.9 s. |
| E-G28c | pass | confirmed | Fresh tab: `app.uniswap.org`, no lock, hairline at 0.6 s and 20.1 s over white. Panel at 20.5 s. Busy 正在重试… at 23.1 s. MUTE lines in chaos. |
| IX2 · G32 | pass | confirmed | 🔒 192.168.50.17:8137 at 0.6 s and 20.1 s, never example.org. Panel at 20.8 s. Retry #1 at +2.1 s. The green connected dot sits under the example.org panel (D18). |
| E-Stop | pass | confirmed | Strip: 停止 row → hairline gone at +0.6 s → bar unchanged → no panel at +24 s → menu shows 刷新. Log `stopped` 07:33:09.8 with no later stall. Note: `…-first-try-menu-flipped.jpg` shows 停止, not the flip. The flip rests on the log (stall 07:32:01.77, tap → `retry #0` 0.83 s later) and on the code, and both support it (D17). |
| E-W5-back · W5 | pass | confirmed | Log: `-1000 → offline`, retries at +2.0 / +5.1 / +10.5 s, stop, `[net] offline misses=3`, `pass` 07:37:36 → `net came back (rpc)` 07:37:37.2, then on return `retry #1` 07:37:45.0 → committed .9. Frames: panel, then Uniswap loaded with no tap. |
| E-G31 · G31 | pass | confirmed | `-1000 → offline` at 0.08 s. 网络不稳定，页面没能打开。 in every frame, never 找不到这个网站. Retries at 2.1 / 7.2 / 17.6 s. The busy frame is at 7.6 s. |
| L5 | pass | confirmed | `post2-L5.jpg` shows 无法加载此页面 / 网站证书有问题，Vela 已阻止打开。/ expired.badssl.com / 重试, no lock. `-1202 → certificate`, and no retry line in 16 s. |
| C1 · G33 | fail | **refuted** | With the quickstart regex, Gnosis stayed up (answered via `rpc.swiftnodes.io` in 8 s). The regex covers 3 of 23 endpoints, a harness gap. With all 19 Gnosis hosts black-holed, chaos shows one new HOLE host every 8 s, and the notice appears at T0 + 155 s while the call is still pending. That is the RF1 moment (end of pass 1), but ~6× the ~25 s expected. Retry is busy (spinner only). There is no `chain notice: shown/cleared` line (D10). |
| C2 | (in C1) | confirmed | `pass` 07:52:07 → answer `0x2e3e799` at 07:52:14 → notice gone in the c2-010.8 s frame. No tap. |
| IX6 · W10 / RF5 | fail | **refuted** | 估算中... with a refresh button at 5 s, 61 s and "late" (4 min 15 s). No reason and no `[fee]`, `[rpc]` or `[sign]` line from 07:55 to 08:01:57, because the first `isDeployed` read is awaited with no bound (D4). After `pass`, the fee is shown at +20.2 s, so ≤ 15 s is not shown. One 4001. |
| S7 / S8 | partial | confirmed (automatic leg) | s7-013.2 s: 点击重试 plus the red cause line, slide shut. `pass` 08:02:59.9 → 估算中 at +4.2 s → fee and live slide at +6.2 s, no tap. The refresh variant was tried with the relay dropped again: no busy state and the old fee kept (D7). There are no `fee:` lines (D10). The cause line sits under 速度 (D20). |
| IX-W1 · G21 G22 | pass (notes) | confirmed | Frames: 正在准备交易… → 提交至网络… → caption 可能已经发出。Vela 会继续查看，请不要重复发送。 + UserOp 哈希, with no 重试 → 已确认 at 09:01:32 while muted. The dApp log has one entry, `result = 0xdd6ad0a2…` (the op hash), done 09:02:14 (121 s after the slide, per RA12). Log `[sign] submit verdict=maybe_sent`. Nonce 51, one event. Not shown in this run: "Activity shows the pending row" (T183 and E-LD3 show it for other ops). |
| T183 | pass (notes) | confirmed, with a limit | Run B: `maybe_sent` 09:10:10.07 → force-quit 09:10:14.5 (signal 9) → first home frame lists 处理中 · ….17:8140 → `tracker confirmed` in the new pid at 09:10:25.8, relay still muted. The quit came **after** the verdict, so this does not test the G34 window (quit during the POST, before `op_submitted`). See D1. |
| IX8 · W1 | pass (notes) | confirmed | Nonces 54–56, three events. The strip has no 失败 / 请重试. The dApp log shows three tx hashes, once each, all while the fault was on. |
| S5 · NotSent | pass (notes) | confirmed | DROP at CONNECT → `[relay] not_sent rejection=none` → 失败 frame at +4.2 s → the dApp's one `-32603 "relay unreachable; nothing was sent"`. History has 6 rows, all landed. Nonce 57 → 57. |
| IX7 · W3 | skipped | unproven | The relay rejects reverting ops, so an on-chain revert cannot be produced. The same limit applied to desktop DX-W3 and extension EX-W3. |
| IX7-probe · G36 parity | fail (P1) | **refuted** (defect proven) | Pre-slide danger line and wrapped amount ✓. Relay `accepted` 09:21:30 → sheet 失败 · 请重试 09:21:43 → the dApp log shows `ok:true, result 0xd9f29715…` done 09:23:28. No event, and the nonce was reused. No log line for the rejection. The record says 接收方 0xDDAf…7A83 and offers an explorer button with no tx (D2, D9). |
| I-G14 · G14 | pass (notes) | confirmed | Frames: 发送 / −0.001 xDAI / 0x7687…D141 + full EIP-55 (checksum verified with `cast`) / 确认发送. 0x0 and omitted: 0 xDAI, 确认. Numeric: contract card, no amount, no −0. Ethereum: −0.001 ETH. Four 4001s in the page log. The numeric request is **submitted as value 0** (D13). |
| E-G9/G10 · G9 G10 | pass | confirmed | zh and en images: one header 连接到 / Connect to + host, lock, account row, network row, one sentence, 拒绝·连接 / Reject·Connect, nothing below. No safe/secure words. The host wraps by character and leaves one digit alone (D24). |
| E-G12 · G12 | fail | **refuted** | Images: `192.16` / `8.50.…`, `192.168.5` / `0.17:81…`, and at the largest size `192.` / `16…`. The chip is whole. The host is never whole (D6). |
| E-G25 · G25 | pass | confirmed | Image: four equal, top-aligned cells, avatar + host on the dormant tabs, no fake bars, 新建标签页 whole. |
| E-G8 · G8 | pass | confirmed | Image: `192.168.50.17:8142` once, title line only; titled rows keep title + host. |
| E-G26 · G26 | pass | confirmed | `tracker confirmed` and `balance refresh (holdings_moved)` in the same ms (09:40:35.807). The first 钱包 frame reads ¥56.82 at +4.6 s (¥56.89 before). xDAI 0.15067 = chain. No pull. |
| E-W20 · W20 | blocked | unproven | Every logo came from the app's cache, so there was nothing to recover. 307 drops of `chains/eip155-*.json` in 4.5 min (I counted them in chaos: 28 per chain file). See D22 and D23. |
| E-L1 · SC-003 | pass | confirmed | MJPEG frames: hairline at +0.22 s. 🔒 jumper.xyz until +7.16 s, then 🔒 example.com. Log `requested` 08:13:06.17 → `committed` 08:13:13.05. Nit: at +7.16 s, one frame shows example.com over Jumper's last pixels (commit before first paint). |
| E-LD3 · L-D3 | pass (notes) | confirmed | The first 钱包 frame lists 处理中 · ….17:8140 at +6.7 s after accept. The record is written before the answer (`waitForRecord`, `SignExecutor.swift:268-270`), so the ≤ 5 s bound is inferred from code, not observed. Confirmed = tracker line at 09:42:48.7. The row survives a relaunch, and its detail is correct (hash = chain). |
| E-LD6 · L-D6 | partial | unproven | Every address the pages logged is `0x88cCA0EeDbF2C4426110bbFc998F048689266894`, and that is EIP-55 (checked with `cast`). The account-switch leg did not run: there is one parallel fixture, and the other six accounts are the owner's. |
| E-LD7 · L-D7 | skipped | unproven | No empty account. The picker lists only chains that have records. |
| E-W23 · W23 | fail | **refuted** | The unified-log leg was blocked (root). The console lines exist for browser, net, relay, sign and tracker, but not for: the chain notice shown/cleared, IX6's blocked fee, the IX7 rejection, the dead slide in IX-W1's first try, and the fee failure/recovery (only `[rpc] gave_up … kind=bundler`). The preview image lists `scope: kind` with no host ✓. The list is kept in memory only (D10). |
| E-probe · T004 | pass | confirmed (on `74192155`) | `post2-E-probe.txt`: three tests passed, `panel after 21.4 s`, `正在重试…` asserted `existsNoRetry == 1`. The strip shows the panel and the busy button. Its dust landed once (nonce 61). The run replaced the installed app. |
| SC-006a | fail | **refuted** | Three `.invalid` hosts under a 40 s cap each ended `-1200 → certificate` at +19.6 to +20.4 s (log), and the image shows 网站证书有问题，Vela 已阻止打开。 with no retry. Never 找不到这个网站 (D8). The first ten under an 8 s cap never reached a panel. |
| SC-006b | pass | confirmed | By the device log, each of the ten committed within 0.9–7.0 s of its `requested`. The Recents images show the ten with their titles and icons and no `.invalid`. |
| SC-007-en | pass (notes) | confirmed | Images of the consent, connection panel, menu and neverssl bar: no "Secure site", "Not secure" or "Encrypted". |
| X-STALE (found) | fail | **refuted** (defect proven) | Images: 处理中, −0.001 ETH, 发起方 app.uniswap.org, 2026/09/21, no hash row, and the explorer tap does nothing. Chain: Ethereum nonce 1, so it never landed. The tester's "should be 失败 by RA4" is wrong (§1.2); D9 records the defect. |
| X-HISTORY (found) | fail | **refuted** (defect proven) | Six-frame strip: detail → ✕ → row tap does nothing → ‹ does nothing → ‹ goes home. Code confirms it (D3). |
| X-DEADPROXY (found) | fail | **refuted** (defect proven) | Home ⚠ + › with no words. The 修复 RPC sheet shows Polygon with `https://rpc.mantle.xyz` in a red field, while the Polygon settings page shows `polygon-bor-rpc.publicnode.com`. The panels blame Gnosis or the network. Nothing names the proxy (D11, D12, D28). |
| X-FIRST-TAP (found) | fail (once) | unproven | One frame (`ios-b-stale-detail.png`) shows a detail of the 09-28 record. No frame shows which row was tapped. A plausible mechanism is in code: see D14. |
| post2-I1 · G34 | — | unproven (not run) | The tested build has no write-ahead (D1). |
| post2-I2 · G36 | — | = IX7-probe | Fails on round 1, as expected. HEAD `c07e1949` and `865a62e0` target it. |
| post2-I3 · G37 | — | = IX-W1 note | The dApp got the op hash 43 s after 已确认 (D26). |
| post2-I4 · G48 | — | unproven (not run) | IX6 shows the round-1 behaviour: no reason at all. |
| post2-I5 · G53 | — | unproven (not run) | Data point: during S7/S8, `[net] offline misses=3` at 08:02:18.797 and `came back` 35 ms later (D21). |

## 4. Defects (most severe first)

Code references are to `fb8c7026` unless marked HEAD. "HEAD:" says whether the round-2 tree (`62346811`) still contains the cause. Nothing marked "addressed at HEAD" has been verified on a device.

### P0

**D1: No write-ahead on the tested build. A force-quit during the POST loses the record while the op can land** (G34 parity; not exercised)
- Rows: T183 (it quit after the verdict, so this window was not covered), post2-I1 (not run).
- Repro: `mute vela-relay` → slide dust → force-quit while 提交至网络… shows (the POST takes ~32 s before `maybe_sent`) → relaunch → no Activity row for that op, and nothing tracks it, while the relay may still land it.
- Code: `app-ios/VelaWallet/VelaWallet/Features/Signing/Core/SignExecutor.swift:245-270`. The record comes only from `ports.opSubmitted(…)` after `spine.submit` returns, and `waitForRecord` runs after that.
- HEAD: `8cd8adc4` adds OpSigned → ClearToPost. Verify on the device with post2-I1.
- Shared with: desktop **D1** ("A payment that lands after a close during the submit leaves no record, no tracker entry and no dApp answer").

### P1

**D2: A relay-rejected op is answered to the dApp as success** (IX7-probe)
- Repro: Gnosis USDC `transfer(0x7687…, 10^30)` from the scratch dApp → slide → relay `accepted` → the tracker rules Rejected → the sheet says 失败 · 请重试 at +13 s → 105 s later the page gets `{ok:true, result:<op hash>}`. The op never lands.
- Code: `SignExecutor.swift:272-281`. `awaitReceipt` polls only the relay's receipt, then `afterReceiptWait(userOpHash:receipt:nil)` answers the op hash whatever the tracker knows. Nothing logs the rejection: `Features/Send/TrackerExecutor.swift:127-136` patches `failed` silently. (The tester cited :118-128; the patch is at :127-136.)
- HEAD: `865a62e0` / `c07e1949` (RJ3, RJ4).
- Shared with: desktop **D2** ("A relay-rejected op is answered to the dApp as success") and extension **D6**.

**D3: After a transaction detail is closed, History's rows and its back arrow go dead** (X-HISTORY; 3 + 5 reproductions)
- Repro: 钱包 → 活动 全部 → any row → detail → ✕ → tap any row (nothing) → ‹ (nothing) → ‹ (home).
- Code: `app-ios/VelaWallet/VelaWallet/Features/Flows/FlowHost.swift:517` (the ✕ is `dismiss()` only) and `:221-225` (`set: { if !$0 { sheetDismissed = model.state } }`). The flow machine is never told, so it stays in the detail state. `:177-181` then suppresses every sheet for that same state id, and the first ‹ only leaves the invisible detail.
- HEAD: unchanged.
- Shared with: none (iOS only). Same class as the dead-controls sweep.

### P2

**D4: The fee row never states a reason while the chain cannot be read** (IX6 / RF5 / W10)
- Repro: relaunch with every Gnosis endpoint black-holed → Send from a dApp → 估算中... for 4 min 35 s, with no reason and no `fee:` line.
- Code: `Features/Signing/Core/SigningController.swift:469`. `await relay.isDeployed(…)` has no bound. It goes through `Core/RelayClient.swift:825-835` → the pool, which is 3 passes × ~20 endpoints × 8 s (`rpc_pool.rs:163,175`).
- HEAD: **still present.** `SigningController.swift:498-505` bounds only `attempt > 1`, and the first read (`:504`) is unbounded.
- Shared with: desktop **D12** (same RJ13 surface, different symptom: the desktop names the wrong cause).

**D5: The chain notice takes endpoints × 8 s. With a real firewall that is 155 s of silence** (C1)
- Repro: black-hole all 19 hosts that serve Gnosis → Block number → notice at T0 + 155 s.
- Code: `rust/crates/vela-core/src/app/rpc_pool.rs:1737-1749` (RF1 sets `unreached_chains` only at the end of pass 1). RF1's rationale assumed "Gnosis ≈ 24 s"; the chain data now lists 23 endpoints.
- HEAD: unchanged.
- Harness: the quickstart's `gnosis|xdai|1rpc` regex does not take Gnosis down on this client.
- Shared with: related to desktop **D9** ("The chain notice shows only when the whole call gives up"), but not the same cause. The desktop waits for give-up because of host polling. The iPhone fires at the RF1 moment, and the RF1 moment itself is too late.

**D6: The signing header still cuts the host** (E-G12; also IX6, S7, IX-W1, I-G14 sheets, the connection panel and the ⋯ menu headers)
- Repro: Sign on Ethereum from `192.168.50.17:8140` at text stop 4 → `192.16` / `8.50.…`; at stop 6 → `192.` / `16…`.
- Code: `app-ios/VelaWallet/VelaWallet/Components/Signing/SigningAtoms.swift:75-104`. The name/host column has `lineLimit(2)` and `layoutPriority(1)`, but it is squeezed to 48–90 pt by `Spacer(minLength:)`, the `fixedSize` chip and the close button, while ~30 pt stays empty.
- HEAD: unchanged. (The E-probe strip on `74192155` shows `127.0.0.1` / `:8137`, which happens to fit.)
- Shared with: none.

**D7: A re-quote in flight draws the old fee with no busy state, and the slide is shut with no reason** (IX-W1 first try, S8 refresh)
- Repro 1: muting the relay while the sheet is open → the slide dims for 2 min 12 s, while `~0.01 xDAI · ≈¥0.07` stays shown with no spinner and no reason.
- Repro 2: refresh with the relay dropped → a request goes out 0.2 s later, but there is no spinner and the old fee stays for 7 s.
- Code: `Features/Signing/SigningLive.swift:1170` (`refreshing: fee?.busy`, false during a requote of a held quote); `SigningController.swift:519-527`; `Features/Send/FeeStore.swift:356-363`.
- Shared with: none exactly. The extension's D18 is the S7 cause line.

**D8: A mistyped or dead domain behind a proxy reads as a certificate block, with no automatic retry** (SC-006a)
- Repro: no fault, dev proxy on → `https://vela-t11.invalid` → WebKit re-CONNECTs for ~20 s → `-1200 → certificate` → 网站证书有问题，Vela 已阻止打开。
- Code: `rust/crates/vela-core/src/app/browser_load.rs:174` (`-1206..=-1200 => Certificate` groups -1200 SecureConnectionFailed with the real certificate codes). The same line is at HEAD.
- Scope: a system proxy or VPN (common for this user base) produces the same shape.
- Shared with: none (desktop D8 is the opposite mistake: a real certificate error classed `other`).

**D9: A never-landed dApp op reads 处理中 for 8 days, with an inert explorer button and a dominant red 删除记录** (X-STALE; also IX7-probe's 失败 record)
- Repro: open the 2026/09/21 22:20 row (Ethereum, app.uniswap.org) → 处理中, no hash row. 在区块浏览器中查看 does nothing.
- Code:
  - `Features/Send/TrackerExecutor.swift:198-201`: `pendingWire` drops records without `userOpHash`, so a pre-082 record is never tracked.
  - RG4 leaves an abandoned (24 h) record "untouched", and the feed draws the record's `pending`, not the tracker's *Unknown* outcome.
  - `App/RootView.swift:2985-2990`: the explorer URL is built from `txHash ?? ""`, while the button is still drawn.
  - IX7-probe's record names the token contract as 接收方.
- HEAD: `aa6de6ed` removes the explorer button without a tx and names the called contract (RJ16). The endless 处理中 remains a design gap.
- Shared with: desktop **D14** (red 删除记录 prominence) and **D15** ("A relay-rejected record is misleading": explorer for an op never on chain, 接收方 = token contract).

**D10: FR-018 log gaps**
- Rows: E-W23, C1, IX6, IX7-probe, S5, S7/S8.
- Missing lines:
  - The chain notice shown/cleared: `App/RootView.swift:1313-1317` computes the notice inline and never logs it. Still absent at HEAD (`RootView.swift:1597-1610` has only the Retry lines).
  - Fee failed/back: none in this build. HEAD `bc90dd87` adds `VelaLog.feeQuoteFailed` / `feeQuoteBack`.
  - The tracker's rejection: `TrackerExecutor.swift:127-136`.
  - `[sign] submit verdict=not_sent`: `SignExecutor.swift:262-266` logs only accepted and maybe_sent.
- The 反馈 preview ring lives in memory only: empty after a relaunch, and it lists repeats one by one.
- Shared with: desktop **D10** ("…there are no fee log lines") and extension **D12** ("Panel-side failures leave no usable log line").

**D11: The home warning line has no words** (X-DEADPROXY)
- Repro: every chain unreachable → under ¥57.97 there is only ⚠ ›, a 40×15 pt button with no label.
- Code: `Features/Wallet/WalletLive.swift:166-172` (`text: fallback.status?.text ?? ""`).
- HEAD: unchanged.
- Shared with: none.

**D12: The 修复 RPC sheet opens prefilled with another chain's URL, in the error tone** (X-DEADPROXY)
- Repro: dead proxy → tap the home ⚠ line → Polygon · 离线, with RPC URL `https://rpc.mantle.xyz` in a red field before any input. Polygon's stored RPC is `polygon-bor-rpc.publicnode.com`.
- Code: `App/RootView.swift:2104-2116` (the draft is read from `settings.networkAdmin?.networks`), `:2125-2137`, and `Features/Settings/SettingsLive.swift:1183-1189` (the tone is copied from the gallery fixture's error state).
- Not proven: how Mantle's URL got in. The chain-id guard on save would likely refuse it.
- Shared with: none.

**D13: A numeric `value` is silently submitted as 0** (I-G14 numeric leg)
- Repro: `eth_sendTransaction {value: 1000}` (a JSON number) → the sheet says 没有资产离开你的钱包, and a slide would send value 0.
- Code: `Features/Signing/Core/SignExecutor.swift:471` (`(raw["value"] as? String) … ?? "0x0"`; HEAD `:578`). A decimal string would also be read as hex by `padded`.
- The sheet and the signature agree, so nothing is hidden. But the dApp's request is changed instead of refused, and the rule lives in the shell (FR-020).
- Shared with: none (desktop D11 is the −0 display).

**D14: The open transaction is chosen by list position, not by id** (X-FIRST-TAP; not proven on device)
- Code: `App/RootView.swift:3022-3031` resolves `(group,row)` against the live feed. `:2165-2168` (删除记录) and `:2985-2990` (explorer) act on that resolved item.
- Effect: a feed change between the tap and the render (a launch reconcile, a confirm that inserts a row) opens, links, or **deletes** another record. This fits the one frame seen at 08:52.
- Shared with: none.

### P3

**D15: The progress hairline sits at 10 % for the whole 20 s of a hung load and through its retries** (E-G28a/b/c). The quickstart expects it to move. Code: `Features/Explore/Core/BrowserEngine.swift:176, :306` (the `requestedProgress` floor). WebKit reports no progress under a black-holed CONNECT.

**D16: One Enter loads twice in a tab that has no engine yet** (E-G28a jumper.exchange, E-G28c, X-DEADPROXY; 1–5 ms apart). Code: `Features/Explore/Core/BrowserController.swift:236-243`. The `tab_navigated` dispatch commits synchronously, and `reconcile` builds the engine and calls `load(url)` (`:536-539`). Line 242 then calls `load(url)` again.

**D17: The ⋯ row changes meaning while the sheet is open** (停止 → 刷新 when the watchdog fires). A tap meant for Stop runs a Retry (E-Stop first try). The row is derived from `engine.loading`, and the site-sheet header truncates the host with room to spare.

**D18: The connected dot stays over another host's failure panel** (IX2, E-W5-back, E-probe). Shared with: desktop **D5** in trust class (RJ5 hides the chip while the page is not the bar's host).

**D19: Two stacked messages for one fault.** The chain notice (暂时连不上 Ethereum / Gnosis) sits above the failure panel of a page that never loaded (E-W5-back, X-DEADPROXY).

**D20: Layout jumps**
- The chain notice pushes the page 52 pt when it appears and when it leaves, and a tap landed on `go uniswap` (C2).
- The fee cause line sits under 速度 and pushes the slide ~40 pt.
- The submitted state's ring and countdown vanish 2–4 s before 已确认, and the button jumps ~25 pt (IX8, E-G26).
- Shared with: extension **D18** (S7 cause line placement and jump).

**D21: Network health flaps on a single faulted service.** Bundler-only misses give `offline misses=3`, then `came back` 35 ms later (S7/S8, 08:02:18), which fires `networkCameBack` (browser reset, logo clear, balance read). Code: `Core/NetWatch.swift:57-78` feeding every pool outcome to one counter. HEAD: `48e77ce3` (RJ14). Shared with: desktop **D16** (net flapping, G53).

**D22: The `chains/eip155-*.json` files are re-requested every ~30 s with no back-off while unreachable** (E-W20: 307 drops, 28 per file). Suspected: `Core/RpcEndpoints.swift:174` / `Core/ChainTokens.swift:61`; the caller was not traced.

**D23: Every transaction detail draws its chain as a grey 3-letter glyph** (ETH, XDA), never the logo, even after a clean relaunch (E-W20, IX7-probe, E-LD3 images). Code not located.

**D24: Host rendering**
- The consent title wraps the host by character and leaves the port's last digit alone on a line (E-G9, SC-007-en).
- List rows cut the host (`…wap.org`, `192.168….17:8140`) with free space.
- The favourite tile reads `127.0.0.1:…`.

**D25: Copy**
- 请重试 on a deterministic reject (IX7-probe; RJ3 wants 网络拒绝了这笔交易，什么都没有发出。).
- `−0.001xDAI` with no space.
- 全部网络 on the chip vs 所有网络 in the picker.
- The consent and the connection panel word one promise two ways.
- 未验证合约 appears as both heading and badge, and the address wraps to 3 lines.
- The decoded recipient reads `0x76875e...c0d141`: lower-case, 6+6, ASCII dots.
- The fee value 点击重试 contradicts "稍后会自动重试".
- The chain-notice Retry is busy with a spinner only; the desktop shows a spinner and 重试.
- Shared with: desktop **D22** (copy and consistency) and extension **D19** (copy and visual nits).

**D26: Answers and endings lag the wallet's own knowledge** (IX-W1)
- The dApp got the op hash 43 s after the sheet showed 已确认 with the tx hash.
- The 已确认 sheet stays up 44 s, then closes itself on the answer.
- HEAD: `865a62e0` (RJ4).
- Shared with: desktop **D21** ("Answers and endings lag…") and extension **D5**.

**D27: The home total reads `总余额 · USD $8.58` for ~0.7 s on every launch, then `CNY ¥57.54`** (T183).

**D28: A dead proxy is presented as a chain or RPC outage.** The iPhone never says 代理没有响应。, and no log line names the proxy (X-DEADPROXY). This is a parity gap with desktop DX3′, where RD9 is desktop-only. The dev knob is Debug-only, but a dead system proxy or VPN looks the same.

**D29: Page-initiated navigations write no `requested` line** (E-G28b), so the log cannot say when the load started.

**D30: Recents keep an origin's older title for a different, untitled page on the same origin** (E-G8 nit: `/notitle.html` on :8140 is listed as `082 iPhone scratch`).

## 5. Evidence-quality notes

1. `retry #0` (E-G28a 07:22:41, E-Stop 07:32:02) comes from a Retry tap or a menu refresh, not from the engine. The E-G28a instance followed an app background/foreground cycle with a person at the phone.
2. `post2-E-Stop-first-try-menu-flipped.jpg` shows 停止, not the flip.
3. TrackerExecutor's patch is at `:127-136`, not `:118-128`.
4. The E-W23 "S7 has only `gave_up` (no host)" is partly unfair: `kind=bundler` names the service. It still names no cause and has no recovery line.
5. The IX6 fee-back time (+20.2 s) is bounded only by the poll interval, so ≤ 15 s is unproven, not refuted.
6. `ios-post2-B-final.log` ends `App terminated due to signal 15`, so the claimed end state (app open on 钱包) is not shown.
7. The parallel space's 账户 sheet lists the owner's six real accounts, each with a remove ✕ (`post2-E-LD6-account-list.jpg`). One mis-tap by a harness could remove a real account from the phone, so the drive scripts must not tap in that sheet.
8. FR-019 scan (the quickstart §7 patterns) over `ios-post2*.log`, `chaos-ios.log`, every `post2-*.txt` and `post-results.md`: no hits other than `bsc-dataseed`. No VelaLog line carries a full address or URL. The only full address is the Debug `print` "parallel space: entered as 0x88cC…6894".

## 6. Environment

- Phone: the installed binary is the `74192155` probe test host. The last launch was terminated at or before 10:42. The owner reports 「无法验证App」 (the developer "Apple Development: Qin Xie" needs verifying with internet: 设置 → 通用 → VPN与设备管理 → 验证App). This audit did not touch the phone.
- Chaos 8897: the last CONFIG (10:41:29) is `pass` / `.` / latency 0. The app's last CONNECT was at 10:41:24, which fits a termination when the console session ended. Ports 8898/8899 are untouched.
