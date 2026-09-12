# 043 — Results: the wallet moves money on the phone

**Branch**: `043-android-money-wiring` (on 042) · **Started**: 2026-09-12 ·
**Device**: Xiaomi `alioth`, serial `9d5f42fb`, Android 13.

The founder's rule for this program: every feature is verified on the
connected device before it is called done. Each phase below ends with what
the device showed, quoted from `uiautomator`, with the screenshot's name in
the scratchpad. A success criterion is marked **device** or **test-only**;
nothing else.

## Baselines (T001)

| Artefact | At 042's tip (`062949ee`) |
| --- | --- |
| `libvela_core_uniffi.so` arm64-v8a (stripped, release profile) | 14,003,760 bytes |
| armeabi-v7a / x86_64 | 9,782,332 / 13,095,864 bytes |
| Machines driven by Android | 13 of 26 (+ `ManageTokensCore` declared, not instantiated) |
| Android unit tests | 369, 0 failures |
| Ceiling for +3 machines (041's per-machine figure ~357 KB) | ≤ 15.1 MB arm64 |

## Phase log

### Phase 0 — the bridge grows three machines (T001–T009)

`SendCore`, `FeePolicyCore`, `TxTrackerCore` join the thirteen bridge objects;
four wire files (`SendWire` 842 lines — all 40 events mirrored so 045 adds a
screen, not a wire; `FeeWire`, `TrackerWire`, `MtokWire`); `CoreWireDriftTest`
grows seven tests over the four families (53 drift tests, 0 failures). The
gate earned its keep on the first run: Kotlin had given `SendToken` an `id`
field the core derives rather than carries — caught, removed.

**Bridge size**: arm64-v8a `.so` 14,003,760 → **16,070,456 bytes (+2,066,696
for three machines, ~689 KB each)**. That is nearly double 041's per-machine
figure (~357 KB) and over the plan's ceiling (15.1 MB); `send` alone is a
40-event machine with the whole controller in it. Recorded, not hidden — the
AAB ships one ABI per device, and the founder decides whether 2 MB buys a
send path. `work-runtime-ktx` and `POST_NOTIFICATIONS` added (T009).

**Device** (`043-p0-home.png`, 2026-09-12 08:52): the new APK opens cold on
the Xiaomi; home reads `觉得九点半 | 0x7687…D141 | 总余额 · GBP | £3.73 | 收款 |
转账 | 扫码 | 活动 | 暂无交易记录 | 资产 | ETH Arbitrum 0.002 ETH £3.71 | POL
Polygon 0.152784 POL £0.01 | 钱包 | 通讯录 | 探索 | 设置`. Same figure as before
the bridge grew; no exception in logcat.

### Phase 1 — foundational: transport, store, assembly, seams (T010–T017)

- **`RelayClient`** (T010): bundler JSON-RPC through the pool with
  `RpcKind.Bundler`, REST against the base THE POOL names (`RpcPool.bundlerBase`
  and `bestRpcUrl` added — the core already answered `BundlerBaseRequested`;
  Android had never asked), `X-Rpc-Url` riding every REST call, the web's two
  caches (quotes 8 s, account info 30 s), the submit retry loop. Ten tests
  (T011) on a scripted port: 404 = uncovered, busy = retried, refusal = not,
  a hashless receipt = pending, the caches' clocks.
- **The feed's writes** (T012): `writeRecords` (replace-by-id, same lock, same
  cap), `patchRecords`, `pendingRecords` (submitted kinds without a verdict).
  Five tests (T013), including the refused-store answer.
- **The submit spine's pure half into the core** (T014): `to_multi_send_call`,
  `quoted_fee_usable`, `GasFloors` (in-band / Tempo), `in_band_batch`,
  `tempo_batch`, `draft_operation`, `apply_estimate`, `replace_calls`,
  `envelope_signature`, `classify_relay_rejection`, `relay_error_message`,
  `is_bundler_underfunded` — 1,090 lines of `user_op.rs` become ~1,430, six
  new tests including the fixture-signed envelope. **The desktop was NOT
  re-pointed**: its `executor/user_op.rs` keeps a twin of these functions
  (~250 lines); re-pointing it means a desktop build this session did not
  run. Owed, named below.
- **uniffi** (T015): `UserOpDraft` / `UserOpCall` / `WalletKeyRecord` /
  `GasFloorsRecord` records, `UserOpFeeMode` and `RelayRejection` enums, and
  `user_op_floors`, `user_op_draft`, `user_op_apply_estimate`,
  `user_op_with_calls`, `user_op_safe_op_hash`, `user_op_sign`,
  `user_op_relay_json`, `user_op_has_contract_call`, `quoted_fee_usable`,
  `parse_existing_user_op_hash`, `is_bundler_underfunded`,
  `classify_relay_rejection`, `relay_error_message`, `entry_point_address`.
  No Kotlin computes a hash, a leg, a limit or a signature.
- **Seams** (T016, T017): `UserOpSigner` (default = onboarding's
  `PasskeyExecutor.assert`, rpId `getvela.app`); `ParallelSpaceHook` in main
  with a `ParallelSpaceBinding` per build type — release installs nothing.

Gates at the checkpoint: `cargo test` (core `user_op`: 27 ok), clippy clean
for both crates, drift 53 / feed 17 / relay 10 tests green.

### Phase 2 — the parallel space (T018–T024)

`vela-dev-fixtures-uniffi` (857 KB host, three ABIs into `src/debug/jniLibs`),
`ParallelSpaceBinding` per build type, the door (`--ez vela.parallelSpace
true|false`), the badge, `ParallelSpaceTest` (2). **Which wallet**: the
MULTI-key golden Safe `0x88cCA0…6894` every client's parallel space sends
from — a single-key fixture Safe would be a wallet nobody funded.

**Device** (`043-p2-parallel-home.png`, `-receive.png`, `-relaunch.png`,
`-left.png`): entering shows `Parallel space | 0x88cC…6894 | £0.53 | XDAI
Gnosis 0.71697` with the badge `平行空间 · #0 · 0x88cC…6894`; the receive
screen lists `0x88cC…6894` on all 13 networks; a relaunch WITHOUT the extra
keeps the space (badge and account); `--ez vela.parallelSpace false` returns
to `觉得九点半 | 0x7687…D141 | £3.73`, no badge, no fixture record left.

Two device-found defects on the way: (1) after a relaunch the badge stayed
but the ACCOUNT reverted — the session core's `add_account` persists only
the active index (the record is the onboarding machines' write), so the
provider now writes the record first and, once the boot has answered,
switches to it if the boot already read it or appends it otherwise; (2) a
second `Boot` is ignored by the core, so leaving re-establishes the store's
list through `set_wallet` instead.

### Phase 3 — the send (T025–T034)

`FeeExecutor` (6 arms), `SendExecutor` (18 arms, `sendUserOpInBand`'s
order), `SendController` (send + fee hosts, the D7 bridge both ways),
`SendLive` (pick / form / confirm / receipt / fee sheet / contact sheet,
every figure the view's), `SendCallbacks` through `FlowHost`, editable amount
and recipient inputs (drawn read-only until now), the alert dialog, the
container's `RelayClient` and `SendController`, the passkey signer bound
from the onboarding ceremony. Tests: `FeeMachineTest` (3),
`SendMachineTest` (2, debug source set, the fixture keyset signs),
`SendLiveTest` (6); suite 394 → **405, 0 failures**.

**Device — money moved.** In the parallel space on the Xiaomi: 转账 → XDAI /
Gnosis → `0.001` to `0x76875e38…D141` (the founder's own wallet) → Continue
(`网络费 0.01 xDAI`) → confirm (`0.001 XDAI · 发送方 Parallel space · 收款人
0x7687…D141 · 网络 Gnosis · 预估手续费 ~0.01 xDAI · 确认并发送`) → receipt
`交易已提交至网络 | 等待区块链确认... | Gnosis 通常在约 15 秒内确认`. The
fixture keyset signed (`parallel signed credential=76656c612d66`), the relay
accepted (`sender=0x88cCA0… nonce=0x8`), userOp
`0x2658de219bc212de74f61b811fa88949d89b033b3b5a1ce958913202e0252684`, and
the relay's receipt says **`success: true`, tx
`0x5316cb6628e16a97e7731a16fb9a66cd4029565404ec66c887c31b40de567447`, block
`0x2df8cad`** — https://gnosisscan.io/tx/0x5316cb6628e16a97e7731a16fb9a66cd4029565404ec66c887c31b40de567447.
The pending row was on disk before `TrackSubmitted` (`feed.write stored
rows=1` precedes `TrackHandedOff` in the log), and after closing the receipt
the home reads `今天 | 已发送 | 至 觉得九点半 | −0.001 XDAI` with the balance
down to `0.70597 XDAI`. A second dust send repeated it. Screenshots
`043-p3-pick/form/form-filled/confirm/receipt/home-after-send.png`.

**Device-found defects, all fixed in this phase** (none visible to a test
that drives the machine directly):
1. The amount and recipient inputs drove their text straight from the
   machine's round trip; fast typing dropped characters (`0.001` → `.01`, a
   42-character address lost six). The fields keep a local echo and ignore
   views that merely echo what they sent.
2. `load_account_credential` was answered by credential id only; the screens
   open the send with the ADDRESS → `AccountUnavailable` on the first
   Continue. Both spellings are accepted.
3. `vela_getInBandGasQuote` rows carry the balance as HEX and `feeToken` as
   JSON null; the parser expected decimals and dropped every row →
   `EstimateFailed`. Re-transcribed from the desktop's `parse_quote_row`.
4. The feed re-read only on focus, so the pending row appeared after a
   relaunch, not at submit. `records_persisted` now dispatches the feed's
   `ReconcileCompleted`.

Not yet (phase 4): the receipt stays *submitted* until the tracker exists;
`TrackSubmitted` is acknowledged and logged (`no tracker bound`).

### Phase 4 — the receipt outlives the screen (T035–T040)

`TrackerExecutor` (6 arms, receipt logs cached per hash for
`notify_confirmed` → `token_trust::ReceiptLogsConfirmed`), the tracker hosted
in `WalletController` (3-second foreground tick only while something is
pending; `AppResumed` on open and focus; the verdict collector → the send
machine's `ReceiptUpdate`, once per change), `TrackerWorker` (WorkManager,
15 s × 8 while the app is away), `TrackerNotifier` (channel `transactions`,
deep link `vela.receipt=<hash>`), the permission asked at the first receipt,
`FeedExecutor` patched by the verdict and re-read. `TrackerMachineTest` (3);
suite 405 → **408, 0 failures**.

**Device**:
- *Receipt flips on screen* (`043-p4-receipt-confirmed.png`): within 6 s of
  confirming, `已发送 0.001 XDAI | 至 0x7687…D141 · Gnosis | 交易哈希
  0xcf735a61…81b03c | 在区块浏览器中查看 | 完成`.
- *Resume from the store*: after the seeds fix (below), a cold start found the
  two older pending sends, polled, and confirmed both
  (`tracker.patch confirmed tx=0x5316cb66…`, `tx=0xb0316de1…`).
- *Force-stop* (`043-p4-after-forcestop.png`): confirm → `am force-stop` four
  seconds later → reopen: the row is on the home, `LoadPendingTxs →
  PollReceipt → ReceiptWithLogs → tracker.patch confirmed tx=0xebadf95f…`.
- *Notification* (`043-p4-notification.png`): confirm → HOME within 1.5 s →
  the worker ticks (`tracker.worker tick round=0,1`), the receipt lands while
  the app is away, `tracker.notify posted hash=0xea070ceb09 tx=0x9880164a13`,
  and `dumpsys notification` holds
  `NotificationRecord pkg=app.getvela.wallet … channel=transactions`.
- *Deep link*: launching with `--es vela.receipt <hash>` opened the detail
  sheet with the fixture's title, counterparty, network, date and hash around
  a live amount (`已收到 USDT | … | 0x9F3c…21aE | Ethereum | 0xdAC1…1ec7`) —
  NOT the row's detail. Fixed in phase 5 (`FlowLive.txDetail` now builds every
  line from the item); re-driven there.

**Device-found defects, fixed in this phase**:
5. The tracker's first poll at `open()` asked the pool for chain 100 before
   the settings machine had its rows; the endpoint source answered "no row",
   the pool kept that empty config for the whole process, and Gnosis was dead
   — no balance, no send, no receipt — until the next launch. The source now
   waits (≤15 s) for a loaded, non-empty list before answering. A regression
   only the device could show: every JVM test seeds its rows up front.
6. Confirm, then leave before the relay answers: `backgrounded()` saw nothing
   pending, the submit landed afterwards, and no clock ticked until the next
   resume. `trackSubmitted` now hands the worker the clock when nobody is in
   front.

### Phase 5 — the screen says what the core refuses (T041–T044)

**What changed**: `SendLive.alertText` maps every `SendAlertKind` to the
corpus's title and body; `formWarning` renders `amount_warning` and the
same-asset ceiling (`SendFeeIssueView`, base units → `fromBase`) under the
amount; `confirmNotice` puts the treasury shortfall (`treasury_bootstrap`,
with the amount hint in the chain's coin), the relay's refusal (`tx_error`)
and the passkey wait (`tx_status == Signing`) in one slot above the slider,
each with its one action (`retryAfterBootstrap` / `retryAfterError` /
`cancelSigning`). The slider is off whenever a notice is up. The recipient's
identicon is drawn only for a well-formed address (the anti-poisoning rule);
half-typed text gets the placeholder. `FlowLive.txDetail` builds title,
status, counterparty (alias first), network, date and hash from the tapped
item.

**Tests** (`SendRefusalsTest`, debug set, real machines + scripted relay):
malformed address → `InvalidAddress` at Continue, in the corpus's words;
over-balance → the ceiling sentence on the form AND the same body as the
`InsufficientBalance` alert at Continue; a parked signer + `cancelSigning` →
back on Confirm, no error, one signature, nothing at the relay, and the second
slide is a second prompt (2 signs, 1 relay call); a relay rejection → the
confirm notice with `txRetryBtn`, `retryAfterError` clears it. `SendLiveTest`
covers each notice, the Cancel action, every alert title, the units, the
identicon gate; `FlowLiveTest` the live detail lines. Suite: 419, 0 failures.

**Device** (Xiaomi, parallel space, Gnosis, balance 0.62897 XDAI):
- *Malformed address* (`p5-alert-invalid.png`): `0xabc` + 0.001 → Continue →
  `地址无效 | 请输入有效的以太坊地址（0x...）。 | 完成`.
- *Over balance* (`p5-form-ceiling.png`, `p5-alert-overbalance.png`): amount
  5 → the form reads `发送 5 加网络费 0.01，共需 5.01 XDAI；当前余额为
  0.62897。 最多可发送 0.61897 XDAI。`; Continue with a real address →
  `余额不足 | 总额超过你的余额。 | 完成`.
- *Identicon* (`p5-form-ceiling.png` vs `p5-form-ceiling-valid.png`): `0xabc`
  shows the placeholder mark; the founder's address its own.
- *Deep link re-driven* (`p5-deeplink-detail.png`): `--es vela.receipt
  0x3623cd94…` → `已发送 XDAI | 已确认 | −0.001 XDAI | 接收方 | 觉得九点半 |
  网络 | Gnosis | 日期 | 今天 10:42 | 哈希 | 0xebadf95f…420219 |
  在区块浏览器中查看` — the row's own lines, alias from the contacts machine.
- *Cancel during signing*: not drivable here — the fixture keyset signs in a
  millisecond, so there is no window to cancel in. Proven in the test with a
  parked signer; on the device it needs the founder's passkey (SC-002/004).
- *Treasury low / relay refusal*: not stageable against the real relay;
  test-covered.

**Device-found defects, fixed in this phase**:
7. The first cut printed the ceiling in base units (`发送 5000000000000000000
   加网络费 10000000000000000 …`) — the core's doc says "the shell formats"
   and the JVM test had asserted on the key, not the figure.
8. `0xabc` earned an address-specific identicon.
9. The keyboard: `uiautomator` lists the Continue button under an open IME,
   so a scripted tap lands on the keyboard — close it with BACK
   (`dumpsys input_method` → `mInputShown=false`) before tapping. Not a
   product defect; recorded for the next device loop.
10. The stage stays `Confirm` while the passkey prompt is up (`tx_status =
    Signing`); the first cut put Cancel on the receipt page, which nobody is
    looking at then. The test caught it (`SD3`, not `SD4A`).


### Phase 6 — the sheets, and the arms 042 left (T045–T051)

**What changed**: `MtokExecutor` (five arms: one Multicall3 `aggregate3`
per network for `name`/`symbol`/`decimals`, the SAME `vela.customTokens`
rows the balance walk and `token_trust` read, invalidate → a forced balance
refresh); `ManageTokensCore` hosted in `WalletController` (`manageTokens`,
`openAddToken` / `addTokenInput` / `addTokenSave` / `deleteCustomToken`), the
sheet live through `FlowLive.addToken` + `AddTokenCallbacks` (the field
types with local echo; a well-formed address is looked up on every network
at once). `IdentityResolver` — the desktop's `identity.rs` in Kotlin: own
accounts → 24 h positive cache (`vela.recipientIdentity`, one document) →
the passkey index → `.bnb`/`.arb`/`.g`/Basename/ENS asked together, answered
in priority order, namehash on the core's `keccak256`; the contacts machine
and the send machine ask the same one (`ContactsExecutor.identity`,
`SendExecutor.identity`). `ClearBundlerCache` → `RelayClient.clearCaches()`
through `SettingsController(clearBundlerCache)`; the websocket endpoint
probe is one `eth_chainId` frame over OkHttp's socket. `grep 'live in 042'`
→ nothing.

**Tests**: `MtokMachineTest` (real machine, fake multicall: found on the one
answering network, saved as a camelCase row, priced once; nothing answers →
`not_found`; half an address looks nothing up; an already-stored token is
recognised at save time and not written twice — the web's verbatim rule:
the card reads "added" only after a save attempt). `IdentityWaterfallTest`
(own account without a call; index before names; a reverse record decoded
and labelled ENS; misses not cached; 24 h expiry; zero address never asked;
EIP-137 vectors; over-long names refused). Suite: 431, 0 failures.

**Device** (Xiaomi, parallel space):
- *Add a token* (`p6-addtoken-found.png`, `p6-addtoken-added.png`): 资产 →
  添加 → paste Gnosis USDC (`0xDDAf…7A83`) → the card `USD//C on xDai |
  USDC · 精度 6 · Gnosis` from a real multicall → 添加到钱包 → chip
  `已添加`; the store holds
  `{"contractAddress":"0xddaf…7a83","symbol":"USDC","name":"USD//C on xDai","decimals":6,"networkName":"Gnosis"}`.
  The row does not appear in 资产 or the pick list: the Safe holds none,
  and the balance walk drops zero rows (`BalanceExecutor`, 041's rule) — it
  appears with its first balance.
- *Contact pick* (`p6-contact-sheet.png`, `p6-contact-filled.png`): the
  person icon → `选择联系人 | 扫码填写地址 | 通讯录 | 0x7687…d141 | Bob |
  Carol | Alice` → tap → the field fills with the address and the identity
  line reads `觉得九点半` — the passkey index, through the waterfall — and
  the confirm page says `收款人 觉得九点半 · 0x7687…d141`.
- *Fee-token sheet* (`p6-fee-sheet.png`): 网络费 → `手续费币种 | XDAI |
  余额 0.62897 | ~0.01 XDAI | 预估费用` — live from `FeeView`. One option:
  the relay's Gnosis quote lists only the native coin for this Safe, so a
  change of fee token (SC-006) cannot be shown here; `FeeMachineTest`
  covers the re-quote.
- *System back*: confirm → BACK → the form (amount, recipient, identity
  line intact) → BACK → the pick → BACK → home.
- *SC-002* (the founder's own passkey): not done — needs a finger.

**Device-found defects, fixed in this phase**:
11. The picker was empty: the contacts machine was only opened by the
    contacts screen. It opens with the send flow now.
12. The history-derived contact was named `null` — `org.json.optString`
    renders a JSON null as the word; `ContactsExecutor` now reads record
    names null-safely (the feed executor already did).
13. The system back key closed the whole send flow from any page (the drawn
    stack had one entry); it is the header's back now.


## Success criteria

| SC | Claim | Verified | Evidence |
| --- | --- | --- | --- |
| SC-001 | dust leaves the fixture Safe; receipt confirmed | **device** | phase 3 + 4 logs; tx `0x5316cb66…7447`, receipt screen `已发送 0.001 XDAI … 0xcf735a61…81b03c` |
| SC-002 | the founder's own passkey, one prompt | — | — |
| SC-003 | force-stop / reopen / notification | **device** | phase 4 log: force-stop → `tracker.patch confirmed tx=0xebadf95f…`; HOME → `tracker.notify posted`, `NotificationRecord … channel=transactions` |
| SC-004 | one prompt per attempt after cancel | test | `SendRefusalsTest`: parked signer, cancel → Confirm, 1 sign, 0 relay calls; second slide → 2 signs, 1 relay call. Device needs a real passkey (no cancel window with the fixture keyset) |
| SC-005 | every refusal worded on screen | **device** | phase 5: `地址无效…`, `余额不足…`, the ceiling sentence in human units; treasury/relay notices test-covered |
| SC-006 | fee token changed, re-quoted, paid | test | `FeeMachineTest` re-quote; the sheet is live on the device but the Safe has one fee asset on Gnosis |
| SC-007 | no send fixture in the live route | test + grep | the send states no longer fall to `drawn.base`; `Scan`/`BatchImport` keep theirs by design |
| SC-008 | drift gate exhaustive; tests grow | — | — |
| SC-009 | `live in 042` markers gone | grep | phase 6: `grep -rn 'live in 042' app-android` → nothing |
| SC-010 | release APK carries no fixture key material | — | — |

## Owed

- **The desktop's twin of the submit spine** (T014): `app-desktop/vela-wallet/src/executor/user_op.rs` still carries `to_multi_send_call`, `envelope`, `classify_rejection`, `usable` and the draft/padding order in its own words; `relay.rs` keeps `parse_bundler_error` / `is_bundler_underfunded`. Re-point them at `vela_core::user_op` in a desktop session (it needs a desktop build to prove nothing moved).
