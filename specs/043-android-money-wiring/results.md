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

## Success criteria

| SC | Claim | Verified | Evidence |
| --- | --- | --- | --- |
| SC-001 | dust leaves the fixture Safe; receipt confirmed | — | — |
| SC-002 | the founder's own passkey, one prompt | — | — |
| SC-003 | force-stop / reopen / notification | — | — |
| SC-004 | one prompt per attempt after cancel | — | — |
| SC-005 | every refusal worded on screen | — | — |
| SC-006 | fee token changed, re-quoted, paid | — | — |
| SC-007 | no send fixture in the live route | — | — |
| SC-008 | drift gate exhaustive; tests grow | — | — |
| SC-009 | `live in 042` markers gone | — | — |
| SC-010 | release APK carries no fixture key material | — | — |

## Owed

- **The desktop's twin of the submit spine** (T014): `app-desktop/vela-wallet/src/executor/user_op.rs` still carries `to_multi_send_call`, `envelope`, `classify_rejection`, `usable` and the draft/padding order in its own words; `relay.rs` keeps `parse_bundler_error` / `is_bundler_underfunded`. Re-point them at `vela_core::user_op` in a desktop session (it needs a desktop build to prove nothing moved).
