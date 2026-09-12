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

## Success criteria

| SC | Claim | Verified | Evidence |
| --- | --- | --- | --- |
| SC-001 | dust leaves the fixture Safe; receipt confirmed | **device (half)**: submitted + on-chain success via the relay's receipt; *confirmed on the receipt screen* awaits phase 4 | phase 3 log, gnosisscan tx `0x5316cb66…7447` |
| SC-002 | the founder's own passkey, one prompt | — | — |
| SC-003 | force-stop / reopen / notification | — | — |
| SC-004 | one prompt per attempt after cancel | — | — |
| SC-005 | every refusal worded on screen | — | — |
| SC-006 | fee token changed, re-quoted, paid | — | — |
| SC-007 | no send fixture in the live route | test + grep | the send states no longer fall to `drawn.base`; `Scan`/`BatchImport` keep theirs by design |
| SC-008 | drift gate exhaustive; tests grow | — | — |
| SC-009 | `live in 042` markers gone | — | — |
| SC-010 | release APK carries no fixture key material | — | — |

## Owed

- **The desktop's twin of the submit spine** (T014): `app-desktop/vela-wallet/src/executor/user_op.rs` still carries `to_multi_send_call`, `envelope`, `classify_rejection`, `usable` and the draft/padding order in its own words; `relay.rs` keeps `parse_bundler_error` / `is_bundler_underfunded`. Re-point them at `vela_core::user_op` in a desktop session (it needs a desktop build to prove nothing moved).
