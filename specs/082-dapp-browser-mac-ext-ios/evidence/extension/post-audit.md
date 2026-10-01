# 082 post-fix device pass, Chrome extension: adversarial audit

- Audited 2026-09-29 (after 03:51 +0800). Inputs: `quickstart.md` §3, `spec.md`, `research.md` (RA/RB/RC/RF/RG), `tasks.md` T152–T159 + T182, `post-results.md`, every `post-*` file in this folder (all 75 images opened), `logs/chaos-ext.log`, `logs/ext-cdp.jsonl`, `logs/e2e-exb.log`, `logs/testdapp-8137.log`, and the code in `/Volumes/data/production/vela-wallet-082` at `fb8c7026`.
- Rule used: a row is **confirmed** only when a file or log shows its expectation, **refuted** when evidence shows the expectation failing, **unproven** when the key claim has no evidence.
- No code was changed and nothing was committed.

## Verdict

28 rows: **19 confirmed, 7 refuted, 2 unproven.**
Defects: **1 × P0, 2 × P1, 9 × P2, 10 × P3.**

The money chain is clean except for one thing. Four ops landed, one per intent, with no duplicate. One of them (nonce 26) landed while the dApp had been told 4900 "The browser closed before the request finished". That is the P0 below: the G21 double-payment harm, reached by closing the side panel.

## Money, recomputed from chain

Query: EntryPoint `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, topic0 `UserOperationEvent`, topic2 = Safe `0xD400866e00B055B20752a826CD5C89b811de130b`. Gnosis, blocks 48484585–48487585 (≈15:44Z–19:54Z, the whole pass from 18:17Z), in chunks of 500 via `rpc.gnosischain.com`. `getNonce(safe,0)` was 23 at blocks 48486300 and 48487038 and is **27** at latest. The xDAI balance went 0.8029998688636814 → 0.7589998688636814 (−0.044 = 4 × (0.001 dust + 0.01 in-band fee)). actualGasCost is 0 for all four (in-band).

| block (UTC) | op hash | tx | nonce | row | what the UI / dApp said | verdict |
|---|---|---|---|---|---|---|
| 48487039 (19:08:15) | 0xb23551ac…2d53b6 | 0xfc30da06…8d7208 | 23 | EX-W1 | maybe-sent → 已确认 (chain check 19:08:47); dApp ok = op hash at 19:09:38.93 | ok, but the sheet fell back to 提交至网络… after 已确认 (D3) |
| 48487160 (19:18:30) | 0x8a313908…9d0690 | 0xe88f61b8…6e16a9 | 24 | EX12 | ring → 已确认; dApp ok = tx hash 19:18:34.88 | ok |
| 48487180 (19:20:10) | 0x7429e548…d88e7b | 0x0eb2ef03…fec910 | 25 | EX13 | 还没上链 for 5 min 49 s while landed; dApp ok = op hash | wrong state, not "failed" (D9) |
| 48487286 (19:29:15) | 0x839be8dc…14cc27 | 0xa37e65b4…41d1d1 | 26 | P0 probe | **dApp got 4900 at 19:29:02.230, 13 s before it landed**; the reopened panel later lists it as confirmed | **P0 (D1)** |

Not landed (0 events): EX-S5 `0x86d36d30…84adabdc` (nonce 24, superseded by EX12's op, can never land); T182 `0x668276cf…608cbbca` (nonce 26, superseded by the P0 op, can never land); EX-W3 `0x4d558afa…449439d7` (nonce 27, relay `rejected`). Residual: the EX-W3 op is still signature-valid until nonce 27 is used. Its inner call reverts and maxFeePerGas is 0, so if anyone ever submitted it, it would only burn nonce 27. No duplicate. The desktop pass ran at the same time on a different Safe (nonces 44–50), so there was no interference.

Wrong in the other direction (told "sent", never landed): EX-S5 and EX-W3 dApps got `ok` + op hash (D6).

## Rows

| id | tester | audit | why |
|---|---|---|---|
| EX0 | pass | confirmed | `post-EX0.txt`: dist wasm `80945236660e` = `rust/pkg-web` WASM_URL; manifest 0.9.5. Nuance: dist was built 02:16:19 from the 80db06a tree, 38 s before `fb8c7026` committed that same wasm (fb8c7026 touches only `rust/pkg-web`). So content = fb8c7026, but the in-app label says 80db06a (D22). |
| U2 · G16 | pass | confirmed | `post-U2-{en,zh}{,-zoom}.jpg`: exact en/zh title, body and buttons; bordered Cancel, filled accent Connect with white label, ≈52 px; no `eth_requestAccounts`. sw log: arrived 18:27:33.117 → shown .253; ✕ → one error answer, 连接 → one ok. Parity gap: no account/network rows (P3, D19). |
| EX2 | pass | confirmed | sw log `req.surface surface=panel`, shown +1 ms, one `req.answered … ok`. `post-EX2-sheet.jpg` shows the sheet in the panel. The "cleared within ~3 s" timing has no timestamps; code auto-closes at 1.4 s (`dapp-receipt.ts` SIGNED_TICK_MS). |
| EX4 · G18 | pass | confirmed | sw log: B arrived 18:31:03.550; A answered 18:31:13.852; B shown .853; B answered once 18:31:37. Direct case +2 ms. **`post-EX4-tick-over-B.jpg` is byte-identical to `post-EX2-signed.jpg` and `post-S3.jpg`**, so it cannot show B's card under the tick. That claim rests on the log only. |
| EX4b | partial | **refuted** | The literal Wallet→Settings→Wallet sequence works (card +1 ms). But RB9, the row's contract ("switches to the wallet screen when a request is owed while another screen shows"), fails. With the panel on Settings, the request got `req.surface=panel` 18:32:47.400 and no `req.shown` until 18:33:45.188 (manual tap). It reproduced again in EX-W3 with a transaction: arrived 19:43:17.091, shown 19:43:50.226. `post-EX4b-settings-no-card.jpg` shows no card, badge or hint. (D2) |
| EX5 · G17 | pass | confirmed | `post-EX5.txt`: sheet removed +88 ms after `Page.reload`; `req.settled cause=page_left` 18:34:30.861; one record, one answer; no `vela.req.*` after. testdapp log shows the reload GET at 02:34:30. The screenshot pair is identical and proves nothing by itself; the text does. |
| EX6 | pass | confirmed | `panel.down` 18:35:36.203 → `surface_closed` .207 → dApp 4900 +358 ms, once. Closed with `chrome.sidePanel.close` (headless has no ✕); the port close is the same path. This behaviour is also the root of D1 when a submit is claimed. |
| EX7 | pass | confirmed | Page sent 18:36:15.642; `req.settled cause=expired` 18:41:21.090; dApp 4900 at +305.45 s, once; sheet gone +3 ms (`post-EX7-after-5m10s.jpg`). The panel was English with zh pinned (D10). |
| EX8 · G19 | pass | confirmed | `req.resumed state=shown`, `sw.start records=1 recovered=1` at 18:43:05.431; no error to page; one ok 18:43:25.989. Used CDP `ServiceWorker.stopWorker` from a page session, not serviceworker-internals. Nit: `sw.start` came +0.86 s after the stop, not "within 0.3 s". |
| EX8b · G23 | pass | confirmed | Connect/Sign +4–7 ms after idle stops; the `sw.start` cadence (18:45:48, 18:46:18, 18:46:48, 18:47:40) proves Chrome's own stops; nothing left in storage. The 30 s restart cycle is D15. |
| EX3-idle | pass | confirmed | shown 18:48:36.135 → claim 18:49:44.665 (68.5 s), no `sw.start` in between, answered ok. `ext-cdp.jsonl` has **no lines between 18:17 and 18:56:28**, so nothing was attached to the worker in Stage A. The screenshot is byte-identical to EX8's and carries no timing. |
| EX9 | pass | confirmed | chaos `blackhole ''` 02:50:13–02:50:35 (HOLE on every RPC host); page timeline in `post-EX9.txt` falls inside it: card +6 ms, switch null +2 ms + `chainChanged 0x64`, chainId +1 ms; sw log arrived 18:50:23.121. |
| EX10 / EX11 · G20 G33 | pass | confirmed | Four faulted calls: 24.0 / 22.0 / ~24 / 16.7 s, plain -32603 text; recovery 0.62 s; sw log `read.exhausted tried=3` each. Defects: calls #2 and #4 log `kind=network` for what are 8-s aborts (FR-018 kind wrong). Every call re-pays 3 × 8 s because cooled endpoints are only reordered, not skipped (RF2 overstates). (D16) |
| S7 / S8 | pass | confirmed | UI: cause line + shut slide (`post-S7-relay-down.jpg`); chaos drop 03:01:20 → pass 03:01:58; re-quote CONNECT 03:02:03; fee back 19:02:11Z (+13 s) per `post-S7.txt`. **`post-S7-fee-back.jpg` does not show the fee back**: it shows 估算中…, the red line and a shut slide. The fee-back state has text evidence only. Refresh spinner/fee in the S8 images. **Tester claim corrected:** the panel does write an app-level line, `[InBand] quote failed for chain=100: PoolFailedError: All bundler endpoints failed for chain 100` (19:01:25/28/34/48). It names no host or kind, and there is no "quote back" line (D12). Worst-case re-quote can exceed 15 s (D18). |
| E-G14 · G14 | pass | confirmed | `post-E-G14-*.jpg`: 发送 / -0.001 xDAI / 接收方, no red, slide 确认发送; zero/omitted 0 xDAI + 确认; numeric → 盲签 with no amount (as RC4/RC6 design); Ethereum -0.001 ETH, no simulation. sw log: exactly 5 requests 19:04:45–19:05:37, each one error answer. Nits: ASCII `-` (hard-coded `live.ts:431`) where the row writes `−`; short address 6+6; zero and omitted images are byte-identical (D19). |
| EX-W1 · G21 G22 | fail | **refuted** | G22 and G21 (money) hold: 正在准备交易… 41.5 s, no 重试, one ok, landed once. Expectation fails on three points. (1) Title 已提交, not 提交至网络… (RA10, D4). (2) The answer came at 136.9 s, not ~120 s (D5). (3) After 已确认 the sheet showed 提交至网络… again for 49 s (`post-EX-W1-03-…jpg`: a confirmed row sits under the "submitting" sheet) (D3). No screenshot exists of this run's maybe-sent or 已确认 overlay (text record only). The "Activity shows 处理中" part was not observed in this run. |
| T182 (reload during maybe-sent) | pass | **unproven** | Persistence half confirmed: `post-EX-W1-T182-02` shows 处理中 · 127.0.0.1:8137 after the reload, and `-03` shows 失败 after pass. The task's other half (tasks.md:1025, "turns 已确认 after pass") was never observed: EX-W1's op resolved before the reload, and the drop variant cannot land. The reload also answered the page **4900** for a may-have-been-sent op (19:27:13.167), against RA2 (D1). |
| P0 · panel closed after 已发送！ | fail | **refuted** (defect proven) | Expectation (RA2: "Ok with a hash, never 4900"; US2 AS4: "closes the sheet after approving → the dApp still gets its answer"). Evidence: `submit verdict=accepted` 19:29:01.192 → `panel.down` 19:29:02.226 → `req.settled cause=surface_closed` .230 → dApp 4900; chain: nonce 26 landed at 19:29:15 (block 48487286). (D1) |
| EX-S5 · NotSent (web) | pass | confirmed | Drop 03:13:40 → pass 03:14:10; 失败 at +87.2 s (`post-EX-S5-03-failed.jpg`, exact zh text); Activity row 失败; nonce 24 unchanged on chain. "not_found twice ≥ 60 s" is inferred: the panel logs no tracker step. **`post-EX-S5-01-preparing.jpg` shows the maybe-sent overlay, not 正在准备交易….** Defects: page answered ok + op hash 35 s after 失败 (D6); no log line at the verdict (D12); "ACCEPTED but NOT landed" log (D13). |
| EX12 | pass | confirmed (weak) | Ring → 已确认 → auto close; dApp ok = tx hash; receipt 0.3 s; chain nonce 24. But chaos shows `LATENCY` only at 03:18:09 (the quote's CONNECT); the submit at 19:18:23 reused the tunnel. The fault never touched the submit, so this row equals a no-fault S4. |
| EX13 · W12 | partial | **refuted** | The dApp half is confirmed: 22 polls, each the real receipt. The panel half fails: 已提交 · 还没上链 for 5 min 49 s about an op on chain since 19:20:10, while the relay said `included` + tx hash. 已确认 came only after `clearFaults` (D9). |
| SC-007-en | pass | confirmed | `post-SC-007-en{,-connections,-sign-header,-sign-details}.jpg`: no secure/not secure/encrypted words; sw log connect ok 19:31:47, sign error 19:32:59. `post-SC-007-en.jpg` and `-U2.jpg` are the same file. The language picker ticks English while zh was pinned (D10). |
| EX-LD6 · L-D6 | partial | **refuted** | EIP-55 spelling is confirmed for every address the page logged. Two sub-claims fail. (a) "Old grants rewritten at the wallet's first boot": a panel boot on Settings left the planted lower-case grant as it was; only the wallet route rewrites (D20). (b) A switch made in Settings → 切换账户 never reached the site; the panel showed Two while `eth_accounts` stayed One (`post-EX-LD6-settings-switched-to-two.jpg`) (D8). |
| EX-G1 · G1 L-D7 | partial | **refuted** | Unfiltered is confirmed (`post-EX-G1.jpg`, `-history-all.jpg`). Filtered to Gnosis, it still reads 暂无交易记录 / 暂无交易, not 该网络暂无交易记录 / 此网络暂无交易 (`post-EX-G1-filtered-gnosis.jpg`) (D14). No Activity skeleton was seen. |
| EX-LOG · W23 | pass | confirmed | `post-EX-LOG.txt`: host-only lines, counts; preview image lists the six `sw:` counters, no URL/address/hash. Secret scan of `ext-cdp.jsonl` and `chaos-ext.log` with the quickstart §7 patterns: 0 hits (the 6 "seed" hits are `bsc-dataseed*` hosts). Ring coverage and panel-side gaps are D12 and D15. |
| EX-W3 · W3 | skipped | unproven | There is no on-chain revert, so "失败 + failedHint + tx hash" was never exercised. |
| EX-W3 probe | fail | **refuted** (defect proven) | Panel console `[UserOp] Estimation RPC error … Safe execution failed … reverted` at **19:43:56, during the sheet's fee quote, 22 s before the slide**. The sheet still showed a normal fee and an open slide (`post-EX-W3-01-sheet-amount-cut.jpg`). `[InBand] … using defaults`, submitted, relay rejected, page answered ok 1 ms later. Amount clipped, `≈ $1e+24` (D7, D6, D11). |
| S3 · L-PANEL | pass | confirmed | `e2e-exb.log`: 11 passed (39.7 s), exit 0, including the L-PANEL tick test; sw log claim 19:41:04.919 → answered .939. `post-S3.jpg` is byte-identical to the EX2 tick. The e2e rebuilt `extension/dist` under the live CfT (D21). |

## Evidence integrity notes

- Byte-identical images (deterministic headless renders, not fraud, but they carry no timing or state of their own): `post-EX2-signed = post-EX4-tick-over-B = post-S3`; `post-EX3-idle-66s = post-EX8-sheet-after-worker-stop`; `post-E-G14-zero = post-E-G14-omitted`; `post-SC-007-en = post-SC-007-en-U2`.
- Mislabelled: `post-S7-fee-back.jpg` (shows the re-quote with the red line, not the fee); `post-EX-S5-01-preparing.jpg` (shows the maybe-sent overlay).
- Missing: EX-W1 has no image of the maybe-sent or 已确认 overlay; E-G14 and EX2 have no page-side log file (the worker log covers them).
- Stage B ran with `cdplog.mjs` attached to the worker from 18:56:28. D1 does not depend on eviction, but the eviction variant of D1 (`recoveryPlan` settling a claimed record) is untested.
- File mtimes are copy times, not capture times (e.g. `post-S3-after.jpg` 03:47 shows the list before EX-W3's row).

## Defects

### D1 · P0 · Closing or reloading the side panel after the submit claim answers the dApp 4900 while the op lands
- Rows: P0 probe, T182 (reload variant), EX6 (the same code path).
- Repro: parallel space, tab A on Gnosis, no fault. `eth_sendTransaction` 0.001 xDAI → slide → when 提交至网络… (whose caption says 关闭此页交易会在后台继续) or 已提交 · 已发送！ shows, close the side panel (Chrome ✕ / `chrome.sidePanel.close`) or reload it → page gets `{code:4900,"The browser closed before the request finished"}`, and the op lands (nonce 26, block 48487286). A dApp that retries pays twice: the accepted op already advanced the local nonce.
- Suspected: `app-web/vela-wallet/extension/background.js:541-553` settles every record the panel owed on port disconnect. `extension/lib/request-life.js:189-193` (`surface_closed` / `window_removed`) and `:142-153` (`recoveryPlan`) ignore `state === 'claimed'` at phase submit. RB10 in research.md prescribes this and conflicts with RA2 and US2 AS4. `src/lib/signing/live.ts:881` shows `send.txBackgroundHint` in the side panel, where closing kills the pipeline. `e2e/extension-lifecycle.e2e.ts:220` pins the 4900.

### D2 · P1 · RB9 missing: a request that arrives while the panel shows Settings/Contacts/Feedback is never shown
- Rows: EX4b, EX-W3 probe.
- Repro: panel on Settings → tab B Connect (or any sign/tx) → no card, badge or hint. The worker logs `req.surface=panel` but no `req.shown` until the person taps 钱包 (58 s and 33 s here). Otherwise the dApp waits out 5 min.
- Suspected: `src/routes/+layout.svelte:89-92`. The `$effect` first reads `panelSurface.caller`, a getter over the plain private field `#caller` (`src/lib/dapp/panel-surface.svelte.ts:130`, not `$state`). On the first run it is null, so the effect returns having tracked nothing and never re-runs.

### D3 · P1 · After the chain check confirms a may-have-been-sent op, the sheet falls back to 提交至网络… for ~49 s
- Rows: EX-W1.
- Repro: `mute vela-relay` → slide dust → maybe-sent overlay → the op lands and is found by the chain check → 已确认 auto-closes after 2.6 s → the underlying sheet shows 提交至网络… with a spinner until the page answer (19:08:49.9–19:09:38.9).
- Suspected: `src/lib/signing/SigningHost.svelte:245-255` (landing raised from the tracker handoff before the answer) + `:269-278` (auto-close on confirmed). `src/lib/services/safe-transaction.ts:3505-3586` (`waitForReceipt` never consults the tracker).

### D4 · P2 · The maybe-sent title reads 已提交 ("Submitted") instead of RA10's 提交至网络…
- Rows: EX-W1, EX-S5, T182.
- Suspected: `src/lib/signing/dapp-receipt.ts:146-153` (`title: copy.submitted`).

### D5 · P2 · The dApp answer after a lost reply overshoots the 120 s window (136.9 s)
- Rows: EX-W1.
- Repro: as D3. The page is answered 136.9 s after the slide, and gets the op hash 51 s after the tx hash was already known on chain.
- Suspected: `safe-transaction.ts:3525` checks the deadline only at the loop top, and each `requestUserOpReceipt` to a muted relay blocks ~15 s. `dapp-submit.ts:558` passes no abort at the deadline.

### D6 · P2 · Relay-rejected and NotSent ops are answered `ok` + op hash to the dApp while the wallet says 失败 · 请重试
- Rows: EX-S5, EX-W3 probe.
- Repro: EX-S5: `drop vela-relay` → slide → `pass` at +30 s → 失败 at +87 s → page ok op hash at +122 s. EX-W3: relay `rejected` → page ok op hash 1 ms later. In both cases the receipt is null forever.
- Suspected: `rust/crates/vela-core/src/app/sign_request.rs:2472-2484` (every failure after `OpSubmitted` → `ReceiptPending`, including a rejection that proves nothing was sent). `safe-transaction.ts:3567-3583` (`not_found` ignored; only `rejected` throws). `dapp-submit.ts:559-562`.

### D7 · P2 · A transfer the relay's estimate says will revert is shown as normal, signed and submitted
- Rows: EX-W3 probe.
- Repro: Gnosis USDC `transfer(0x1111…, 10^30)` from a Safe without it → the relay estimate answers -32500 "target call … reverted" during the quote → the sheet shows the fee and an open slide, with no warning → slide → defaults used → submitted → the relay rejects.
- Suspected: `src/lib/services/safe-transaction.ts:2266` + `2284-2294`. `isPlainTransferCall` (`:1938`) counts an ERC-20 `transfer` as plain, so a revert verdict is treated like a transport failure. The web sheet surfaces no revert (RG6: no web simulation).

### D8 · P2 · An account switch made in Settings never reaches connected sites
- Rows: EX-LD6.
- Repro: panel Settings → 切换账户 → Parallel Two → the site gets no `accountsChanged` and `eth_accounts` stays One. The panel shows Two, the wallet tab One. (Signing later silently switches back via the verified switch.)
- Suspected: `src/routes/[locale]/wallet/+page.svelte:1311-1321`. `followActiveAccount` runs only in the wallet page's `$effect`, and `followedAddress` resets on remount, so the change is read as a boot.

### D9 · P2 · A landed op reads "还没上链" while the relay's status says `included` + tx hash
- Rows: EX13.
- Repro: panel `vela.silentReceipt(100)` → dust → it lands in ~12 s → the overlay says 已提交 · 还没上链 until the fault is cleared (5 min 49 s here; up to 24 h).
- Suspected: `rust/crates/vela-core/src/app/tx_tracker.rs:1110-1116` (non-rejected statuses only set `acknowledged`; `relay_tx_hash` is stored and unused; see the doc at `:355-358`). The chain find is gated to `in_doubt()` entries (`:630-632`, `:1524`).

### D10 · P2 · The side panel ignores the pinned wallet language
- Rows: EX7, SC-007-en.
- Repro: pin 简体中文 in the panel's Settings → close and reopen the panel → English sheets. Settings reads "Language 简体中文" on an English page and the picker ticks English.
- Suspected: `app-web/vela-wallet/extension/panel.js:19` (`negotiate(chrome.i18n.getUILanguage())` only).

### D11 · P2 · The signing amount is clipped and the fiat reads `≈ $1e+24` (SC-008)
- Rows: EX-W3 probe.
- Repro: any amount wider than the 360-px panel (here 10^30 USDC). The number runs off the right edge. The Activity row for that op shows no amount at all.
- Suspected: `src/lib/signing/ui/AmountHero.svelte:60-73` (flex row, no wrap/shrink). `src/lib/signing/live.ts:152` (`toFixed(2)` gives exponent notation ≥ 1e21, and a hard-coded `$`).

### D12 · P2 · Panel-side failures leave no usable log line (FR-018, SC-007)
- Rows: EX-S5, S7/S8, EX-LOG.
- Repro: NotSent/失败 verdict → no console line. Fee failure → only `[InBand] quote failed … All bundler endpoints failed` (no host, no kind, no recovery line). The 反馈 preview lists worker counters only, so maybe_sent and 失败 never reach a report.
- Suspected: `src/lib/wallet/core/tracker-executor.ts` (no logging at all). `src/lib/signing/fee-requote.ts` (no log). Bug-report counters come from the worker only.

### D13 · P3 · Misleading log "ACCEPTED but NOT landed" for an op the relay never accepted
- Rows: EX-S5.
- Suspected: `safe-transaction.ts:3627-3638`.

### D14 · P3 · Filtered empty-state copy is never shown on web/extension
- Rows: EX-G1.
- Suspected: `src/lib/wallet/core/feed.svelte.ts:90-93` (`chainFilter()` has no caller). `src/routes/[locale]/wallet/+page.svelte:1866, 1973` call only `chainFilter.select`. RG5's "web already dispatches it" is false.

### D15 · P3 · An idle open panel restarts the worker every ~30 s
- Rows: EX8b, EX-LOG.
- Effect: each restart adds `panel.up` + `sw.start` + `storage.local.get(null)`. The 200-line ring filled after ~75 min; Stage A's first 17 min were gone.
- Suspected: `src/lib/dapp/panel-surface.svelte.ts:353-358` (reconnects on every disconnect, even when nothing is owed). `extension/lib/swlog.js:27`.

### D16 · P3 · Worker read log mislabels timeouts and re-pays every dead endpoint
- Rows: EX10/EX11.
- Repro: 8-s aborts in calls #2 and #4 were logged `kind=network`. Each faulted call re-pays 3 × 8 s.
- Suspected: `extension/lib/protocol.js:206-210` (`readFailureKind`), `:155-165` (`orderEndpoints`, cooled endpoints still tried).

### D17 · P3 · The full-panel 已签名！ tick hides the next queued card for ≥ 1.4 s
- Rows: EX4.
- Suspected: SigningHost landing overlay with `SIGNED_TICK_MS` (`dapp-receipt.ts:262`).

### D18 · P3 · S7 sheet: stale cause line, wrong placement, layout jump, and a re-quote that can exceed 15 s
- Rows: S7/S8.
- Repro: the red cause line stays under 估算中… for ~7 s. It sits under 速度, not 网络费. The sheet jumps ~32 px. The cadence 3/6/12 s then every 15 s, plus a ~7 s quote, can exceed 15 s after the relay returns. This run hit 13 s by luck of phase.
- Suspected: `src/lib/signing/fee-requote.ts` + `fee_policy::requote_delay_ms`.

### D19 · P3 · Copy and visual nits
- Rows: E-G14, S7, EX2, EX5, U2, EX-S5, SC-007-en, EX-LD6, EX12.
- ASCII `-` instead of `−` (`live.ts:431`).
- Short address 6+6.
- The full recipient address overflows the sheet padding at 360 px.
- The numeric-value blind card says "无 ERC-7730 描述符（0 字节）" and names no recipient.
- 滑动以确认 · 确认 says confirm twice.
- The IP-host avatar letter "1" reads like a counter.
- The dev badge covers the account name.
- Failed rows still show "-0.001 xDAI".
- "Raw call data" labels personal_sign params.
- Device storage shows "14 records in total" against 27 (tester-reported; not visible in the image).
- The account sheet total is $5.16 against home's $14.97.
- The consent card has no account or network rows (parity with iOS/desktop G11).
- A mid-animation brown arc shows on the 已确认 tick.

### D20 · P3 · Lower-case grants are rewritten only when the wallet route mounts
- Rows: EX-LD6.
- Effect: a panel that boots on Settings leaves them lower-case. `eth_accounts` could return lower-case until then; this was not observed.
- Suspected: `src/routes/[locale]/wallet/+page.svelte:1307-1309`.

### D21 · P3 · Test and harness gaps
- Rows: S3.
- No e2e case for a panel close during a claimed submit (the EX6 test pins 4900). The EX4b test does not cover a request arriving while Settings shows.
- The isolated e2e rebuilt `extension/dist` in place under the running CfT, which broke open pages.
- Suspected: `e2e/extension-lifecycle.e2e.ts:220, :291`; the `playwright.isolated.config.ts` webServer build.

### D22 · P3 · Version label drift
- Rows: EX0, EX-LOG.
- The bug report says `v0.9.5 (80db06a)` while the running wasm is fb8c7026's: dist was built from an uncommitted tree 38 s before the commit.
