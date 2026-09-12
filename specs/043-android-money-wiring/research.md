# Research: Android Money Wiring

Facts were read from the code on 2026-09-12; file references are to the
merged tree at the start of 043.

## D1 — Export `send`, `fee_policy`, `tx_tracker` through the existing bridge macro

**Decision**: three `bridge_object!` invocations in
`rust/crates/vela-core-uniffi/src/onboarding_bridge.rs` (the macro at :159,
the thirteen existing objects at :209–306), same `dispatch` / `resolve_effect`
/ `view` shape, same `{view, effects[{id, operation}], cancelled_effect_ids}`
envelope. `ManageTokensCore` already exists (:281) and is instantiated by this
feature. **Rationale**: the road is proven thirteen times; the wasm crate
already exports all four (`vela-core-wasm/src/wallet_state.rs`). **Cost**: 041
measured ~357 KB per machine on arm64; expect ≈ +1.1 MB/ABI, recorded in
results. **Alternative**: a multiplexed bridge object — 041 measured it saves
1.7 %, rejected then and now.

## D2 — The fixed keyset ships as a separate debug-only cdylib

**Decision**: a new crate `rust/crates/vela-dev-fixtures-uniffi` (cdylib,
depends on `vela-core` with `dev-fixtures`), exporting `fixture_accounts()`,
`fixture_assert(challenge, allow_credential_ids, preferred) -> assertion`,
`fixture_registration(index)`; built by `build-android.sh` only when
`VELA_DEV_FIXTURES=1` (Gradle sets it for the debug build type); its Kotlin
bindings under `rust/bindings/kotlin-dev/` and its `.so` under
`src/debug/jniLibs/`, both in the **debug source set only**. Main code reaches
it through a seam (`dev/ParallelSpaceHook.kt`) whose release implementation
is a no-op. **Rationale**: FR-001 says a release build carries neither door
nor keys; the desktop's compile-time gate (`parallel_space.rs:47`) is the
precedent. A cargo feature on the main `.so` cannot work: uniffi's Kotlin
bindings verify every exported function's checksum at load, so one bindings
file cannot serve two `.so` variants, and two bindings files collide in the
debug classpath. **Alternatives**: (a) compile the fixtures into the main
`.so` always and gate only the door — the keyset is public test data (the web
serves `/parallel` in production), so the risk is nil, but it contradicts the
spec as written and the desktop; rejected. (b) port the signer to Kotlin —
`dev_fixtures.rs` exists precisely so no shell writes a second WebAuthn
assembly; rejected.

## D3 — The UserOp assembly moves into the core; the shell keeps transport

**Decision**: the pure half of the desktop's
`app-desktop/vela-wallet/src/executor/user_op.rs` (896 lines: `simulate_gas`,
`submit_in_band`, `submit_tempo`, `sign_and_submit`, `envelope`,
`classify_rejection`, `fallback_fee`, `key_set_of`, `to_multi_send_call`) is
split: assembly (call data, init code, fee leg, nonce/gas fields, SafeOp hash,
signature packing, rejection classification) into `vela-core::user_op`, next
to the primitives it already holds (`calculate_safe_op_hash` :357,
`build_user_op_signature` :489, `build_dummy_signature` :525,
`user_op_to_json` :572, `parse_existing_user_op_hash` :639); transport (nonce
read, `eth_getCode`, `eth_estimateUserOperationGas`, `eth_sendUserOperation`)
stays in the shell. Exported via uniffi as a small `UserOpDraft` API: draft →
`safe_op_hash` → assertion → `finalize_signature` → JSON for the relay. The
desktop is re-pointed at the moved code in the same change so there is one
implementation, or — if its crate cannot take the churn in this spec — the
move is recorded as owed with the two copies named. **Rationale**: the
challenge signed is the EIP-712 SafeOp hash, the packing is
`build_user_op_signature` with `validAfter‖validUntil` zeros, and 032 wrote
all of it once already; Kotlin re-deriving it would be the second copy 041
refused for pricing. **Alternative**: port to Kotlin — rejected (FR-014).

## D4 — The pending record is the feed's own row shape, written under the feed's lock

**Decision**: `PersistTxRecords` writes to `vela.transactionHistory` in the
camelCase shape `FeedExecutor.kt:135–172` reads (`id, userOpHash, txHash,
from, to, toName, value, symbol, decimals, logoUrls, chainId, timestamp,
status, type, usd`), through a new `FeedExecutor.writeRecords` that takes the
existing `writeLock`, de-dupes by id and keeps `TX_CAP`. After the tracker's
`UpdateTxRecords`, the shell dispatches `activity_feed::ReconcileCompleted`
(the web's `ports.feedReconciled`, desktop's `PATCHED` counter). **Rationale**:
one store, one shape, one lock; the web writes the same key
(`records.ts:76`). Today Android has no pending-write path at all
(`mergeRecords` is the incoming scan's).

## D5 — Background receipt polling through WorkManager

**Decision**: while foregrounded, a 3-second tick answers `Now`/`Tick` as the
web (`tracker-resident.ts:61`) and desktop (`tracker.rs:53`) do. On
background with pending records, an expedited one-time `WorkRequest` polls
through the same `TrackerExecutor` every ~30 s for the core's
`WAIT_WINDOW_MS` (120 s), then a periodic request (15-minute floor) until the
core abandons at 24 h; `AppResumed` on resume. **Rationale**: the spec's
"money in flight outlives the screen" without a foreground service; the core
owns the cadence, the worker only supplies the clock. **Alternative**: a
foreground service with a persistent notification — heavier, and the wait
window is two minutes.

## D6 — The confirmation notification

**Decision**: one `NotificationChannel` ("transactions"), posted on
`NotifyConfirmed` when no activity is foregrounded, deep-linking to the
receipt (`vela.startDestination` is not a flow door — a new intent extra
opens the wallet with the flow stack seeded to the receipt). Permission
(`POST_NOTIFICATIONS`, API 33+) requested at the first submit, never at
launch; refusal degrades to the in-app receipt. `NotifyConfirmed` also feeds
the cached receipt logs to `token_trust::ReceiptLogsConfirmed` as both other
shells do.

## D7 — `EstimateFee` is answered by the live fee session

**Decision**: `SendExecutor` answers `EstimateFee` by dispatching
`fee_policy::QuoteRequested` into the `FeePolicyCore` host and awaiting the
estimate on its view (web `send-executor.ts:237` → `ports.feeQuote`; desktop
`money.rs:458`). `ChooseFeeToken` → `SelectFeeAsset`; `StartTtl` → a
coroutine timer. **Rationale**: both other shells bridge the two machines in
the shell; the core keeps them apart on purpose.

## D8 — Relay transport

**Decision**: `RelayClient` issues bundler JSON-RPC
(`eth_estimateUserOperationGas`, `eth_sendUserOperation`,
`eth_getUserOperationReceipt`, `eth_getUserOperationStatus`,
`pimlico_getUserOperationGasPrice`, `vela_getInBandGasQuote`) and REST
(`GET /v1/treasury/{chain}`, `GET /v1/account/{chain}/{safe}`) against the
chain's bundler endpoint from `NetworkEndpointSource.kt:54`, through the pool
so bans and cooldowns apply (`RpcPool.kt:136` already knows a bundler base).
The 8-second in-band quote cache lives here and is what `ClearBundlerCache`
clears (backfill).

## D9 — The signer seam

**Decision**: `UserOpSigner.sign(challenge, credentialIds, method)` returns
the onboarding `Assertion`; its default implementation is
`PasskeyExecutor.assert` (`PasskeyExecutor.kt:365`, rpId `getvela.app`); the
parallel space substitutes the fixtures cdylib. `CancelPasskeySign` cancels
the coroutine; the core's `SigningStarted`/`CancelSigning` are the
checkpoints. Exactly one prompt per attempt is a test, not a hope.

## D10 — Backfills

- `ResolveIdentity` (`ContactsExecutor.kt:119`, send's `ResolveIdentity`): the
  web's waterfall (`recipient-identity.ts:191`) — index lookup by wallet ref,
  then reverse resolution across `NAME_SERVICES`, cached. Android implements
  the same order; the name-service registry calls go through the pool.
- `ClearBundlerCache` (`NetworkAdminExecutor.kt:209`): clears `RelayClient`'s
  quote cache (D8). `NetworkProbes.kt:59` follows if it is the same cache.

## D11 — The device loop

`./gradlew :app:assembleDebug` (with the NDK build; ~4 min cold),
`adb install -r`, `am start -n app.getvela.wallet/.MainActivity` with
`--ez vela.parallelSpace true` for the door, `uiautomator dump` for bounds
and text, `input tap`, `screencap`. Evidence: one screenshot per phase in the
scratchpad and its UI text quoted in results. Gradle under the JBR.

## D12 — What is not decided here

Split/sweep/batch UI (045), the scanner (046), dApp signing (044). The
`Scan` and `BatchImport` fixtures stay; `OpenScanner` / `OpenBatchImport`
events are not dispatched from live screens.
