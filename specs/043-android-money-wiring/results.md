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

(filled at close)
