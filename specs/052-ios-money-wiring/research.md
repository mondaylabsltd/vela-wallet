# Research: iOS Money Wiring

Facts read from the code on 2026-09-14, at `6e693b26` (051 with `origin/main`
merged). File references are to that tree.

## D0 — What this cut does NOT have to build, measured first

051's most expensive mistake was asserting from memory what the repository
contained. So the inventory came first, and it changes the shape of the cut:

| | |
|---|---|
| `bridge_object!` exports | **23 of 23** already in `vela-core-uniffi/src/onboarding_bridge.rs:209-372`, `SendCore` :281, `FeePolicyCore` :289, `TxTrackerCore` :296 |
| `user_op_*` free functions | **all eight** already in `lib.rs` (`user_op_floors` :1113 … `user_op_relay_json` :1235) |
| relay classification | `quoted_fee_usable` :1258, `parse_existing_user_op_hash` :1268, `is_bundler_underfunded` :1274, `classify_relay_rejection` :1288, `relay_error_message` :1302 |
| the committed Swift bindings | `app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift`, 353,772 bytes, `open class SendCore` at :4821 |
| Swift references to any of it | **zero** |

**Decision**: this cut adds **no `vela-core-uniffi` export and no
`vela_core_uniffi.swift` regeneration**. That is a first — 050 added three
bridge lines and 051 added Multicall3 and the price ladder — and it means the
five-to-eight-minute `build-ios-xcframework.sh` round trip is not on this
cut's critical path at all. The one Rust edit is a `crate-type` line on
`vela-dev-fixtures-uniffi` (D1), which adds no code and no export.

## D1 — The parallel space: a second STATIC xcframework, linked only in Debug

**Decision**: build `vela-dev-fixtures-uniffi` as a **staticlib** into a second
xcframework at `app-ios/VelaDevFixturesKit/Artifacts/VelaDevFixturesFFI.xcframework`
(gitignored), and link it from the **Debug** build configuration only. Its
generated `vela_dev_fixtures.swift` is committed under
`app-ios/VelaWallet/VelaWallet/Dev/` and excluded from the Release
configuration by `EXCLUDED_SOURCE_FILE_NAMES`.

**No second SPM package.** Xcode links a package product in *every*
configuration; a package target compiling the generated Swift would fail to
build in Release the moment its FFI module is absent, because the file
references `RustBuffer` outside any `#if canImport` guard. `VelaDevFixturesKit/`
is therefore a directory holding `Artifacts/`, not a `Package.swift`.

**Mechanics** (target config `23ED5E5B…` Debug, `23ED5E5C…` Release in
`app-ios/VelaWallet/VelaWallet.xcodeproj/project.pbxproj`):

| Setting | Configuration | Value |
|---|---|---|
| `OTHER_LDFLAGS` | Debug | `("$(inherited)", "-lvela_dev_fixtures")` |
| `LIBRARY_SEARCH_PATHS[sdk=iphoneos*]` | Debug | `…/VelaDevFixturesKit/Artifacts/VelaDevFixturesFFI.xcframework/ios-arm64` |
| `LIBRARY_SEARCH_PATHS[sdk=iphonesimulator*]` | Debug | the `ios-arm64-simulator` slice |
| `SWIFT_INCLUDE_PATHS[sdk=…]` | Debug | each slice's `Headers` |
| `EXCLUDED_SOURCE_FILE_NAMES` | Release | `vela_dev_fixtures.swift ParallelSpaceBinding.swift` |

The `[sdk=…]` idiom is already in this project (`EXCLUDED_ARCHS[sdk=iphonesimulator*]`),
so it is the house style rather than a new one.

**The risk worth naming: duplicate symbols.** Two Rust staticlibs in one
binary could collide. Measured rather than assumed — `nm -gU` on the committed
core archive shows **386 non-uniffi globals**, all `compiler_builtins`
intrinsics and five `__rustc` allocator shims; LTO (`lto = true`,
`codegen-units = 1` in `rust/Cargo.toml`) has internalised std, alloc and
`uniffi_core`. A fixtures archive from the same toolchain partitions the same
way and the linker's lazy member loading resolves each once. **If the first
link reports a duplicate**, the fix stays inside the build script:
`ld -r -exported_symbols_list` keeping only `_uniffi_vela_dev_fixtures_*` and
`_ffi_vela_dev_fixtures_*`. Verified at the first link, not hoped for.

**Alternatives rejected**: (b) a `VelaWallet Dev` target — doubles every build
setting, needs a second bundle id, changes CI's destination logic and puts two
apps on the founder's phone; (c) `#if DEBUG` + `dlopen` — uniffi's Swift binds
`uniffi_*` symbols at link time, so this means a hand-written `dlsym` shim,
which is a second assertion assembly wearing a different name.

## D2 — The door, and the account it opens

**Decision**: the door is a launch environment variable **and** a persisted
flag, matching both predecessors: `VELA_PARALLEL_SPACE=1` enters and persists
`vela.parallelSpace`; `=0` leaves; unset follows what is persisted.
`VELA_PARALLEL_SIGNER=n` persists `vela.parallelSigner` (the web's
`vela.parallel.signWith(n)`, `fixture_assert`'s `preferred`).

It is applied in `RootView.init`, **after** `DevAccountSeed.applyIfRequested`
and **before** `session.boot()`. Writing the record to disk before the session
machine reads its store sidesteps Android's trap entirely: there,
`session.add_account` stores only the active index and `set_wallet` would
overwrite the founder's real wallet, so the record had to be written first
anyway (`project_android_live_wiring`, 043 phase 2).

**The account is upserted, never written over the list.** `AccountStore.saveAccount`
already upserts by id (`AccountStore.swift:44`) and is the function to use.
`DevAccountSeed`'s pattern — replace the list, remove it when the pin is absent
— is correct for a **read-only, key-less** seed on a simulator and is
**forbidden here** (FR-003): this door is opened on a phone holding the
founder's own wallet, and a list replacement there is the ANDROID-8 incident
with a different serial number.

The record is the fixture's own, built from the three exports:

| Field | Source |
|---|---|
| `id` | `fixture_accounts()[0].credential_id_hex` |
| `address` | `fixture_multi_address()` — the golden multi-key Safe `0x88cCA0…6894` |
| `publicKeyHex` | key 0's `public_key_hex` |
| `keys` | every fixture account, `transports: "internal"` |
| `name` | the fixture's own `name` |

Leaving removes exactly that id and resets the active index; sign-out leaves
too.

## D3 — The signer seam

**Decision**: `protocol UserOpSigner { func sign(challenge: Data, credentialIdHex: String?, transports: String, method: KeyMethod) async throws -> Assertion }`
— a literal transcription of Android's `UserOpSigner.kt`, over the signature
iOS already has: `PasskeyExecutor.assert(challenge:credentialIdHex:transports:method:)`
(`PasskeyExecutor.swift:275`), which returns the `Assertion` struct at :50.

Two implementations: `PasskeyUserOpSigner` forwards; `FixtureUserOpSigner`
(Debug-only file) calls `fixtureAssert(challenge:allowCredentialIds:preferred:)`
and fills the two fields the fixture record lacks (`userIdHex: nil`,
`authenticatorAttachment: "platform"`).

**What the executor is handed is the SafeOp hash, never a WebAuthn signing
hash.** The core packs the assertion into the operation's signature and the
Safe verifies the client data it finds there. A shell that pre-hashed would
produce a signature over the wrong bytes and learn about it from a reverted
transaction.

**Exactly one call per attempt is a counted test** (FR-009), not a comment:
the test signer counts invocations, cancels mid-ceremony, and asserts the
count stays one.

## D4 — The spine, ported literally

**Decision**: `Core/UserOpSpine.swift` is a transcription of
`app-android/…/feature/send/core/UserOpSpine.kt` (205 lines), which is itself
the desktop's `executor/user_op.rs` order. The order is the file's content and
must not be improvised:

`keysOf` → `isChainWithoutNativeCoin` → `isDeployed` (fatal if `nil`) →
`nonce` (fatal if `nil`, **before any prompt**) → `user_op_floors` →
`quoted_fee_usable` gate → placeholder fee leg → `user_op_draft` →
`estimateUserOpGas` + `user_op_apply_estimate` (a failed estimate is fatal
**only** when `user_op_has_contract_call`) → `user_op_with_calls` with the
settled leg → `user_op_safe_op_hash` = the challenge → `signer.sign` →
`user_op_sign` → `sendUserOp` → on rejection `relay_error_message` →
`parse_existing_user_op_hash` (idempotent re-submit) → `classify_relay_rejection`.

It also carries `signMessage` — `safe_message_hash` → assertion →
`eip1271_signature` — which **053 needs and this cut does not call**. It is
ported now anyway, because the spine exists so that a person's transfer and a
dApp's transaction cannot become two opinions, and splitting the file across
two cuts is how they would.

## D5 — The relay client

**Decision**: `Core/RelayClient.swift`, ported from `RelayClient.kt` (447
lines). Its two load-bearing properties:

1. **The bundler base is whatever the pool names for that chain**, and the
   pool's best RPC URL rides as the `X-Rpc-Url` header on every REST call. The
   pool already answers both (`RpcPool.bestRpcUrl(chainId:)` at
   `RpcPool.swift:192`, and `kind: "bundler"` routing on `call`).
2. **Two caches with the TTLs the other clients use**: the in-band quote 8 s,
   the account info 30 s. `clearCaches()` is what `network_admin`'s
   `clear_bundler_cache` means — the third `// live in 052` marker.

Methods: `probeTreasury` (404 = uncovered, not an error), `accountInfo` →
`feeRecipient() = settlementRecipient ?? depositAddress`, `inBandQuotes`,
`bundlerQuote`, `estimateUserOpGas`, `sendUserOp` (3 retries, 3 s apart),
`userOpReceipt`, `userOpStatus`, `nonce`, `isDeployed`, `gasSignals`.

**`CoreHTTP` gets a REST GET.** It has `rpc` and `rpcEnvelope`
(`CoreHTTP.swift`) and no plain GET with a header; that is the one addition.

## D6 — `EstimateFee` is answered by the live fee session

**Decision**: `SendExecutor` answers `estimate_fee` by dispatching into the
resident `FeePolicyCore` and awaiting its view — the web's
`send-executor.ts:237 → ports.feeQuote` and the desktop's `money.rs:458`. Both
other shells bridge the two machines in the shell because the core keeps them
apart on purpose.

This needs the second `async`-over-`CoreStore` shape in the client (the first
is `RpcPool`). It is narrower: one quote in flight per surface, keyed by a
generation token, so a stale answer cannot land on a newer attempt.

**The bundler is the gas-price authority and the wallet never vetoes its
quote** — a standing rule from the wallet↔bundler parity work, restated here
because `fee_policy` is where a shell would be tempted to clamp.

## D7 — The pending row is the feed's own shape

**Decision**: `persist_tx_records` writes through `Core/TxRecords.swift` in the
camelCase shape `TxRecords.toWire` already reads
(`id, userOpHash, txHash, from, to, toName, value, symbol, decimals, logoUrls,
chainId, timestamp, status, type, usd`), adding a `writeRecords` beside the
existing `merge`. One store, one shape; `TxRecords` is `@MainActor` and holds
no `await` between read and write, so the web's `withTxLock` is satisfied
structurally (the file says so at :20).

`timestamp` is **seconds**, as every client stores it (`TxRecords.timestamp`
reads it as such and `dayStartMs` multiplies). `type` is `"send"`.

After the tracker's `update_tx_records`, the shell dispatches
`activity_feed::ReconcileCompleted` — the web's `ports.feedReconciled`.

## D8 — Background tracking: what iOS actually grants

**Decision**, three layers, and the spec says which are guaranteed:

1. **Foreground**: a 3-second tick while anything is pending, as web
   (`tracker-resident.ts:61`) and desktop (`tracker.rs:53`).
2. **Backgrounding with pending**: `beginBackgroundTask` grace — read
   `backgroundTimeRemaining` rather than assuming; it is ~30 s on modern iOS —
   keep ticking, end the task; then submit a
   `BGAppRefreshTaskRequest("app.getvela.VelaWallet.tracker")` with
   `earliestBeginDate = now + 15 min` as best effort.
3. **Return**: `scenePhase == .active` → `AppResumed`; launch → `LoadPendingTxs`.

**What is NOT claimed**: a cadence. iOS decides whether the refresh ever runs,
and a force-quit cancels it. Android's worker gives 120 s; iOS gives ~30 s and
a maybe. The honest statement is FR-007's: **no verdict is lost**, because the
core abandons at 24 h and every launch resumes from storage.

**There is no `vela.tracker` key on any client.** Android's string of that name
is its WorkManager task name. Pending state is rebuilt from
`vela.transactionHistory` rows with `status == "pending"`, a `userOpHash` and
an empty `txHash` — which is exactly what `load_pending_txs` documents.

`Info.plist` gains `BGTaskSchedulerPermittedIdentifiers` and
`UIBackgroundModes = [fetch]`. Registration is
`.backgroundTask(.appRefresh(…))` on the `Scene`, which registers before
launch finishes.

## D9 — The notification

**Decision**: one `UNMutableNotificationContent` per confirmation, posted only
when `UIApplication.shared.applicationState != .active` (the shell decides
*whether*, the core decides *that it happened*). `threadIdentifier`
`"transactions"`; request identifier = the user-op hash, which dedupes a
re-delivery; `userInfo["vela.receipt"]` carries the hash;
`willPresent` returns `[]` so a foreground app never gets a banner over its own
receipt.

**Permission is requested at the first submit**, never at launch (Android 043
D6), and a refusal degrades to the in-app receipt with no second ask.

The tap opens the transaction: the delegate stores the hash, `RootView`
observes it and enters `.txDetail`. `FlowNav.enter(_:)` carries no payload
(`FlowNav.swift:68`), so a `pendingReceiptHash` is resolved against the feed
once the row exists; a cold-start tap waits for `allowedRoute == .wallet`.

## D10 — The three inherited markers

| Marker | Answer |
|---|---|
| `ContactsExecutor.swift:108` `load_send_history` | `TxRecords.load` filtered to sends, projected to the core's history shape. It currently answers `history_failed` **deliberately** — an empty list would tell the core nobody has ever been paid and make every address wear the poisoning warning forever. |
| `ContactsLive.swift:163` `activity: []` | the same records, as the contact detail's 最近往来 rows. |
| `NetworkAdminExecutor.swift:263` `clear_bundler_cache` | `RelayClient.clearCaches()` (D5). |

## D11 — What the fee-token sheet and the contact picker read

The fee sheet's rows are the chain's accepted fee assets with the person's
balances — `fee_policy`'s `in_band_quotes` crossed with the balance machine's
holdings, which is what the shared `FeeTokenSelector` does on every client.

The contact picker reads the **contacts machine**, and Android found the trap:
its `contacts` machine only opened on the contacts page, so the picker was
empty (`project_android_live_wiring`, 043). On iOS `ContactsStore` is already
resident and booted from the contacts tab; this cut boots it from the picker
too, which is idempotent (`CoreStore.boot` returns `false` the second time).

## D12 — What is deliberately not decided here

Split, sweep and batch (054); the scanner and `add_network`'s only entry (055);
`simulate_calls` (055); dApp signing (053). `sd1b`, `sd2b`, `sd2c`, `sd2d`,
`sd3b`, `sd3c`, `s1` keep their fixtures and their gallery routes.

## D13 — The device loop

`rust/scripts/build-ios-xcframework.sh` only when `vela-core-uniffi` changes
(it does not, D0); `rust/scripts/build-ios-dev-fixtures.sh` once for the
keyset. Then, from `app-ios/VelaWallet`:

```
xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet \
  -destination 'id=84146B7B-C679-46AC-8426-41AA42E6F403' test
xcodebuild … -destination 'platform=iOS,id=00008130-001C68C804E1401C' \
  -only-testing:VelaWalletUITests/LiveWiringAcceptanceTests \
  -resultBundlePath /tmp/accept.xcresult test
xcrun xcresulttool export attachments --path /tmp/accept.xcresult \
  --output-path /tmp/accept-images
```

The device door: `xcrun devicectl device process launch --device 00008130-… \
-e '{"VELA_PARALLEL_SPACE":"1"}' app.getvela.VelaWallet`. The simulator door:
`SIMCTL_CHILD_VELA_PARALLEL_SPACE=1 xcrun simctl launch …`.

**One automation session at a time** — a `devicectl … --console` left running
holds the device's usage assertion and the next run reports
`Authentication canceled` (050 phase 7).
