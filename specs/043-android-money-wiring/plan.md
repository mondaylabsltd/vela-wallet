# Implementation Plan: Android Money Wiring

**Branch**: `043-android-money-wiring` | **Date**: 2026-09-12 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/043-android-money-wiring/spec.md`

## Summary

Wire the send path on Android: the `send`, `fee_policy` and `tx_tracker`
machines through uniffi, `manage_tokens` instantiated, the drawn send screens
and their three sheets reading the core's view, a relay transport through the
person's own pool, the passkey assertion reused from onboarding, the pending
row written before tracking, and a notification when a confirmation lands
while the app is away. Before any of it: the parallel space — the core's fixed
keyset signing behind a debug-only door — so every phase is verified on the
connected Xiaomi over `adb` and only the last criterion needs a finger.

## Technical Context

**Language/Version**: Kotlin 2.2 / Compose, AGP 9.3, minSdk 29; Rust 1.97
(`vela-core`, `vela-core-uniffi`, uniffi bindings by `uniffi-bindgen`).

**Primary Dependencies**: existing — uniffi, kotlinx.serialization, DataStore,
okhttp (the pool's transport). New — `androidx.work` for the background
receipt poll (research D5), `POST_NOTIFICATIONS` (D6). No xlsx, no camera,
no WebView in this spec.

**Storage**: `VelaStore` keys the other clients use — `vela.transactionHistory`
(the feed's records, camelCase shape, research D4), `vela.customTokens`; a
debug-only `vela.parallelSpace` flag.

**Testing**: JVM unit tests over the real machines through JNA (040's pattern)
with a fake relay transport; `CoreWireDriftTest` entries for four families;
a device pass per phase (FR-013), driven by `adb` in the parallel space;
SC-002 by the founder.

**Target Platform**: Android 10+ (the Xiaomi test device runs 13).

**Constraints**: no Kotlin decides what the core decides (FR-014); every chain
and relay request goes through the pool; the fee shown is the fee signed; the
pending row is written before tracking; release builds carry no fixture door
(FR-001, research D2). Bridge size measured before and after (041's method).

**Scale/Scope**: 3 machines exported + 1 instantiated (Android goes from 13
to 17 of 26); ~900 lines of desktop UserOp assembly to split into core and
shell (D3); 4 send screens + 3 sheets from fixture to live; 2 backfills.

## Constitution Check

No constitution is ratified (`.specify/memory/constitution.md` is the blank
template). The program's standing rules stand in, and each is a gate this
plan re-checks after design:

| Rule | Where it bites here | Status |
| --- | --- | --- |
| One implementation: money rules live in the core | UserOp assembly moves to `vela-core`, not into Kotlin (D3) | pass |
| A machine costs an executor + a display-model builder | `SendExecutor`, `FeeExecutor`, `TrackerExecutor`, `MtokExecutor`; `FlowLive.send*` | pass |
| Views subsets, ops/results exhaustive | drift-test entries for four families | pass |
| Any Rust edit moves the web fingerprint | rebuild last (042's lesson) | pass |
| Device verification per phase | a device task closes every phase | pass |

## Project Structure

### Documentation (this feature)

```text
specs/043-android-money-wiring/
├── plan.md              # this file
├── research.md          # D1–D12
├── data-model.md        # send attempt, fee quote, pending record, parallel space
├── contracts/
│   └── shell-operations.md   # what the shell answers, per machine, per op
├── quickstart.md        # build, install, open the door, drive a send over adb
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
rust/crates/vela-core-uniffi/src/onboarding_bridge.rs   # + SendCore, FeePolicyCore, TxTrackerCore
rust/crates/vela-core-uniffi/src/lib.rs                 # + UserOp assembly exports (D3)
rust/crates/vela-core/src/user_op.rs                    # + the pure half of the desktop's assembly
rust/crates/vela-dev-fixtures-uniffi/                   # NEW cdylib: the fixed keyset for debug builds (D2)
rust/scripts/build-android.sh                           # + the fixtures lib when VELA_DEV_FIXTURES=1
rust/bindings/kotlin-dev/                               # generated, gitignored, debug source set only

app-android/vela-wallet/app/
├── build.gradle.kts                                    # debug source set: kotlin-dev bindings + jniLibs; work-runtime
├── src/main/AndroidManifest.xml                        # POST_NOTIFICATIONS
├── src/debug/java/app/getvela/wallet/dev/ParallelSpace.kt        # the door, the badge, the signer (D2)
├── src/main/java/app/getvela/wallet/dev/ParallelSpaceHook.kt     # the seam main code calls; no-op in release
├── src/main/java/app/getvela/wallet/feature/send/
│   ├── core/SendWire.kt        FeeWire.kt        TrackerWire.kt        MtokWire.kt
│   ├── core/SendExecutor.kt    FeeExecutor.kt    TrackerExecutor.kt    MtokExecutor.kt
│   ├── core/RelayClient.kt     # bundler JSON-RPC + REST through the pool's bundler endpoints (D8)
│   ├── core/UserOpSigner.kt    # passkey or fixture assertion → core packs the signature (D3)
│   ├── core/SendController.kt  # hosts send + fee + manage_tokens; the tracker lives in WalletController
│   └── SendLive.kt             # FlowLive for SendPick/Form/Confirm/Receipt + FeeToken/ContactPick/AddToken
├── src/main/java/app/getvela/wallet/feature/wallet/core/TrackerWork.kt   # WorkManager worker + notification (D5, D6)
├── src/main/java/app/getvela/wallet/feature/wallet/core/FeedExecutor.kt  # + writeRecords (D4)
└── src/test/java/app/getvela/wallet/   # SendMachineTest, FeeExecutorTest, TrackerExecutorTest, SendLiveTest, FakeRelay, drift entries
```

**Structure Decision**: a new `feature/send/` package beside `wallet/` and
`flows/`, because the send path has its own controller and four wire
families; the tracker stays in `wallet/` because it serves the feed and, in
044, the dApp path. The parallel space is a debug source set, not a flag.

## Phase plan

Each phase ends on the device. The order is 032's, because 032's reasons
hold: nothing can be verified without the signer, nothing can be signed
without a quote, nothing can be tracked without a submit.

- **Phase 0 — the bridge grows three machines.** `bridge_object!` for
  `SendCore`, `FeePolicyCore`, `TxTrackerCore`; the wire files and drift
  entries; bindings regenerated; `.so` size recorded before/after.
  *Device*: the app still opens and shows the same £ figure.
- **Phase 1 — the parallel space.** The fixtures cdylib (D2), the debug
  source set, the door (`--ez vela.parallelSpace true` and a persisted flag),
  the badge, session sign-in as fixture account 0.
  *Device*: the receive screen shows the fixture Safe the web derives.
- **Phase 2 — the relay and the quote.** `RelayClient` through the pool's
  bundler endpoints; `FeeExecutor` (gas signals, bundler quote, in-band
  quotes, fee recipient, gas estimate, TTL); the UserOp assembly split (D3).
  *Device*: pick → form → Continue shows a real fee on Gnosis.
- **Phase 3 — the send.** `SendExecutor`, `SendController`, `SendLive` for
  the four screens and the fee-token / contact-pick sheets, `UserOpSigner`.
  *Device*: dust leaves the fixture Safe; receipt says *submitted* with a
  hash (SC-001 half).
- **Phase 4 — the receipt outlives the screen.** `TrackerExecutor`,
  `PersistTxRecords` into the feed store before `TrackSubmitted`, the pending
  row on the home, the 3-second tick in foreground, WorkManager + notification
  in background.
  *Device*: SC-001 (confirmed), SC-003 (force-stop / reopen / notification).
- **Phase 5 — the refusals, the cancel, the sheets, the backfills.**
  Every `SendAlertKind` and `SendSubmitFailure` on screen; the cancel
  checkpoints; `manage_tokens` + AddToken; the two `// live in 042` arms.
  *Device*: SC-004, SC-005, SC-006; SC-002 with the founder's finger.
- **Phase 6 — the record.** Gates (app, rust, android), the fingerprint
  rebuilt last, `.so` delta, results.md with device evidence per SC.

## Complexity Tracking

| Addition | Why it is needed | Simpler alternative rejected because |
| --- | --- | --- |
| A second cdylib for the fixed keyset (D2) | FR-001: release carries no key material; uniffi bindings must match the `.so` they load | a cargo feature on the main `.so` would need per-build-type bindings; a runtime flag would ship the keys |
| Kotlin bridging `send` → `fee_policy` (D7) | the core asks the shell to estimate; the estimate is another machine's answer | routing inside the core would couple two machines the desktop and web keep apart |
| WorkManager for the background poll (D5) | a receipt landing while the app is away must still reach the person | a foreground service is heavier than a 2-minute wait window justifies |
