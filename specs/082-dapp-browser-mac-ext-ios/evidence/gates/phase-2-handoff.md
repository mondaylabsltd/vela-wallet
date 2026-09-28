# 082 — what the core, exports and bindings phases hand to the clients

Raw reports of the merge, T044 (UniFFI), T045 (wasm) and T046/T047 agents, in that order. The client phases act on every 'passed on' / 'needs' item for their client.

The four core branches are merged into `082-dapp-browser-mac-ext-ios` with no conflicts, and every gate passes. I also finished the three core items the groups left for merge, and made three commits. Nothing was pushed, and I only touched the files listed below.

## Merge commits (in order)
- `9b635184` merge(082): core group A
- `4584999d` merge(082): core group C
- `9c5a65aa` merge(082): core group EF
- `3f4fefa5` merge(082): core group G

**Conflicts: none.** The groups' files never overlapped, and git merged the `tasks.md` checkbox lines cleanly. T009–T043 and T180 are all `[x]`; T044 onward stay `[ ]`. Right after the merges, the core suite passed 2030 tests with 0 failures. That equals a 1845-test base plus the groups' own additions (A 79, C 11, EF 46, G 49).

## Left-for-merge items done in core
1. **`f961c782` — one character rule instead of two (G item 1).**
   - `name_verify::is_never_in_a_name` is now `pub(crate)`, and the copy in `sim_outcome.rs` is deleted.
   - Covered by `app_sim_outcome.rs`, which checks every character range (31 pass).
2. **`49b312aa` — text passed into a sentence can no longer pull in another sentence (G review item 6).**
   - Before this, a revert reason, token symbol or page title containing `$t(key)` was expanded into a wallet sentence. Group G had only closed the revert-reason path.
   - `i18n/mod.rs` `translate` now copies i18next's `skipOnVariables` rule: if filling the variables added a `$t()` call, nothing in that sentence is expanded.
   - No sentence in the corpus and no client passes `$t(` through a variable, so nothing the app shows today changes.
   - Proof: five new cases in `scripts/dump-vectors/i18n.dump.mjs`, with expected values produced by i18next itself. `i18n-behaviour.json` goes from 210 to 215 cases. With the guard removed, 3 of the 5 fail in `tests/conformance.rs`. Re-running the dumper reproduces the file exactly.
3. **`6e11743c` — tests that drive the groups' modules together.**
   - New file `tests/core_082_wiring.rs` (6 tests):
     - A lost reply is one pending dApp row in Activity, under the local hash and with its site. After two `not_found` answers past 60 s and a chain read to the head, that same record turns failed, and the sheet's ending goes from "may have been sent" to "not sent".
     - The same op found on chain by its event becomes one confirmed row with the event's tx hash.
     - The record words the clients copy through (record kind and status, tracker patch status) mean the same in the Activity feed.
     - The browser bar (`address_bar`) and the Activity row (`dapp_site`) name a site the same way.
     - The plain-send card (`plain_send_of`) and the Activity row (`native_amount`) show the same amount.
     - Every text key the 082 core hands a client (`explore.loadProxy` included) exists in all 15 locale files.
   - One new test in `tests/app_rpc_pool.rs`: when a node crashes on `eth_simulateV1`, there is no chain notice and the sheet shows the "couldn't check" caution.

## Passed on to the next phases
**Exports T044 (UniFFI)**
- From A: the §12 functions, plus `EXTENSION_REQUEST_TTL_MS`, `RPC_READ_TIMEOUT_MS` and `cooldown_ms`. `cooldown_ms` takes `u32` in the core but the contract says `u64`, so that needs settling. `send::receipt_outcome_of` is not in §12; decide whether to export it.
- From G: `simOutcome(user, replyJson)` returning `SimOutcomeRecord`.
- From EF: `browserLoadGiveUpMs`, `browserLoadShouldGiveUp`, `browserLoadStalled`, `browserLoadRetryWhenNetworkReturns`, `browserAddressBar`, `browserSiteLabel`, `netHealthStep`, `markMissTtlMs`, `balanceReadPlan`. `browserLoadClassify` can now return `"proxy"` with no export change.
- From C: nothing.

**Exports T045 (wasm):** the list in the task text. EF names `browserSiteLabel`, `markMissTtlMs` and `balanceReadPlan`.

**Bindings T046**
- Add export lines to `generate_wallet_state_bindings.rs` for `SignEnding`, `SignEndingState` and `TrackStatusAnswer`. Optionally add `StableRef`, `TokenRef`, `ReadKind` and `ReadSlot` if the web wants them.
- Regenerate for the new fields in `FeedItem`, `FeedTxRecord`, `FeedView`, `ClearSurface`, `ClearSigningView` and the new `ClearPlainSend`.
- Rebuild wasm, the xcframework and the Kotlin bindings before `verify:wasm` and the Swift/Kotlin harnesses. They now replay the 215 behaviour cases, and `i18n/mod.rs` changed, so the wasm fingerprint moves.

**Desktop (T049, T056–T058, T069, T072/T073)**
- Update these files for the new fields and variants:
  - `executor/pool.rs`
  - `executor/tracker.rs`
  - `executor/sign_request.rs`
  - `signing/status.rs`
  - `wallet/money.rs`
  - `flows/live.rs`
- When the pool's verdict is a range limit, pass the held error body as `error_json`. `pool.rs:690` currently throws it away, and if it stays that way the chain read retries the same too-wide window forever.
- Add a `PlainSend` arm at `signing/live.rs:211-231`, and fix the value reader at `signing_host.rs:1025-1029`.
- Fill `dapp_origin` in the `FeedTxRecord`/`FeedView` literals the G report lists.
- T057: call `sim_outcome` instead of the desktop's own delta code.
- T058: switch to the core `LoadWatch`. On `Deferred`, keep the 20 s give-up armed.
- T069: count an unthrottled sweep with no answer as a miss, timeouts included, and skip the network-back retry for a load the page started.
- Dedupe the tracker hand-off by record or request id, not by op hash.

**Web and extension (T090 and later)**
- Draw `PlainSend` in `live.ts`, and fix the first-call value reader in `sheet.svelte.ts`.
- Add the new fields to `sign-types.ts`.
- Dedupe `#lastHandoffHash` by record or request id.
- Update tests that expect `-32603` after a submit; that answer is now the op hash.
- Run `normalizeGrantSpelling` at extension boot.
- Fix the `wallet_sendCalls` value in `dapp-history.ts:190-200`.
- Signer page (RC8): use the same empty-calldata test as the core, including the trim.

**iOS**
- `ClearWire.swift` must decode `plain_send`, but it is missing from T109's file list.
- Add `"proxy"` to the failure-class mirror and show `explore.loadProxy`.
- T113: count an unthrottled `RpcPool` failure as a miss, timeouts included.
- Adopt `read_plan`, and delete the `SimDeltas` parser.
- Dedupe `handedOff` by record or request id.
- Fix the record builder in `SignExecutor.swift`.

**Android**
- Update `ClearWire.kt`. `CoreWireDriftTest.kt:935` fails until then.
- The value reader needs an `isNull` check: `optString` returns `"null"` for a JSON null.
- Add `"proxy"` to the failure-class mirror.
- Delete `core/net/NetHealth.kt` and its test, and the balance read lists.
- Dedupe `handedOff` by record or request id.
- Fix the record builder in `SignExecutor.kt`.

**All clients (from A)**
- Answer `FindOpEvent` with `OpEvent`.
- A confirmation that came from a found event has no receipt behind it: fetch the logs or skip token auto-add.
- Persist and restore `maybe_sent` and `submit_block` (T181–T184).
- Dedupe `ReceiptUpdate` on the tracker outcome too, not just the status.
- Send `CeremonyStarted` and `CeremonyDone` around the passkey, and word the sheet from `phase`.
- Drive the submit loop with `submit_step`, and bump the nonce only on Accepted.
- Show the chain notice for `unreached_chains` as well as `failed_chains`.

**Decisions for you**
- A range limit on `eth_getLogs` still counts as a success for the endpoint; the contract says "no score change".
- A found event with success is not checked for a Safe `ExecutionFailure`.
- A restored maybe-sent record with no `submit_block` scans only the last 5,000 blocks.
- EURC (Base) and EURC.e (Tempo testnet) are valued at $1.
- A download or 204 may show the "Other" failure panel.

## Gates (in `/Volumes/data/production/vela-wallet-082/rust`, on the final tree)
| Gate | Result |
|---|---|
| `cargo test -p vela-core --features i18n-all,crux` | 2037 passed, 0 failed, 0 ignored, 54 result lines; residency 134,713 bytes against the 138,800 budget |
| `cargo clippy -p vela-core --all-targets --features i18n-all,crux -- -D warnings` | exit 0 |
| `cargo fmt --all --check` | exit 0 (only the new `core_082_wiring.rs` needed rustfmt) |
| `cargo test --workspace --features vela-core/i18n-all,vela-core/crux` | 2059 passed, 0 failed, 60 result lines |
| Extra: `cargo check -p vela-core --features bindings,i18n-all,crux --bins` | builds |

Files changed in my three commits:
- `/Volumes/data/production/vela-wallet-082/rust/crates/vela-core/src/app/name_verify.rs`
- `/Volumes/data/production/vela-wallet-082/rust/crates/vela-core/src/app/sim_outcome.rs`
- `/Volumes/data/production/vela-wallet-082/rust/crates/vela-core/src/i18n/mod.rs`
- `/Volumes/data/production/vela-wallet-082/scripts/dump-vectors/i18n.dump.mjs`
- `/Volumes/data/production/vela-wallet-082/rust/crates/vela-core/tests/vectors/i18n-behaviour.json`
- `/Volumes/data/production/vela-wallet-082/rust/crates/vela-core/tests/core_082_wiring.rs`
- `/Volumes/data/production/vela-wallet-082/rust/crates/vela-core/tests/app_rpc_pool.rs`

---

T045 is done: 16 wasm exports are in `rust/crates/vela-core-wasm/src/lib.rs` and committed as `09a9c7d5`. That covers the 13 in the task list plus 3 more (`signEndingOf`, `userOpEventTopic`, `parseUserOpStatus` is in the list). Only that one file changed and nothing was pushed. The uniffi file the other agent is editing was not touched or staged.

## Gates (run in `/Volumes/data/production/vela-wallet-082/rust`, final tree)
| Gate | Result |
|---|---|
| `cargo build -p vela-core-wasm --target wasm32-unknown-unknown --release` | exit 0; raw wasm 7,021,812 bytes (build-web's ceiling is 8,000,000) |
| `cargo test -p vela-core-wasm` | exit 0; 9 passed, 0 failed (new module `core_082_exports`, which checks each export against the core function it wraps) |
| `cargo clippy -p vela-core-wasm --all-targets -- -D warnings` | exit 0 |
| `cargo clippy -p vela-core-wasm --target wasm32-unknown-unknown -- -D warnings` | exit 0 |
| `cargo fmt -p vela-core-wasm --check` | exit 0 |
| `cargo fmt --all --check` | exit 1, only from the unfinished `vela-core-uniffi/src/lib.rs` |
| Scratch check: wasm-bindgen 0.2.126 `--target web` into the scratchpad, then node `initSync` | all 16 exports are in the `.d.ts` and returned the expected values |

`build-web.mjs --check` was not run. It would fail because `rust/pkg-web` is out of date until T046 rebuilds it.

## Exports (Rust signature → TS signature)
Structured answers are JSON strings in the core's own serde shape, the same shape the files in `W/core/generated/` already use.

1. `user_op_hash(op_json: &str, chain_id: u64) -> JsResult<String>` → `userOpHash(op_json: string, chain_id: bigint): string`
   - `op_json` uses the same shape as `attestSafeOpHash`. Any signature in it is ignored.
   - Returns the 0x-lowercase EntryPoint v0.7 hash.
   - The step that turns that JSON into a `UserOperation` is now one helper, `attest_operation`, shared with `attestSafeOpHash`. Its behaviour is unchanged.
2. `user_op_submit_step(reply_json: &str, attempt: u32, maybe_delivered: bool, local_hash: &str) -> JsResult<String>` → `userOpSubmitStep(reply_json: string, attempt: number, maybe_delivered: boolean, local_hash: string): string`
   - Accepted replies:
     - the core's `SubmitReply` JSON: `{"hash":"0x…"}`, `{"error":"<error member as JSON text>"}` or `"no_answer"`;
     - an error member passed as an object, `{"error":{…}}`.
   - A `null` error is rejected.
   - Output is the core's `SubmitStep`:
     - `{"retry_after":{"delay_ms":3000}}`, or
     - `{"done":{"type":"accepted"|"maybe_sent","user_op_hash"}}`, or
     - `{"done":{"type":"not_sent","rejection":null|"relayer_unavailable"|"bundler_underfunded"|{"other":"…"}}}`.
3. `user_op_not_sent_detail() -> String` → `userOpNotSentDetail(): string` (`"relay unreachable; nothing was sent"`)
4. `user_op_event_topic() -> String` → `userOpEventTopic(): string`
   - Not in the task list. The web tracker (T086) needs this topic to build the `eth_getLogs` filter for a `FindOpEvent`.
5. `user_op_status_method() -> String` → `userOpStatusMethod(): string` (`"pimlico_getUserOperationStatus"`)
6. `parse_user_op_status(json: &str) -> Option<String>` → `parseUserOpStatus(json: string): string | undefined`
   - Returns `TrackStatusAnswer` JSON: `{"status","stage","tx_hash"}`.
7. `sign_ending_of(method: &str, payload_json: &str, submitted_user_op: Option<String>) -> JsResult<Option<String>>` → `signEndingOf(method: string, payload_json: string, submitted_user_op?: string | null): string | undefined`
   - Not in the wasm list, but covered by "signEnding*" in the task text. Without it the web would have to build `SignEnding` itself, which is the local copy RA8 removes.
   - Input is `SignResponsePayload` JSON; output is `SignEnding` JSON.
8. `sign_ending_state(ending_json: &str, entry_json: Option<String>) -> JsResult<String>` → `signEndingState(ending_json: string, entry_json?: string | null): string`
   - Takes a `TrackEntryView`. `null`, `undefined` and `""` all mean "the tracker has not taken it yet".
   - Returns `SignEndingState` JSON.
9. `dapp_receipt_wait_ms(elapsed_ms: f64) -> f64` → `dappReceiptWaitMs(elapsed_ms: number): number`
10. `sign_request_ttl_ms() -> f64` → `signRequestTtlMs(): number` (300000)
11. `rpc_read_timeout_ms() -> u32` → `rpcReadTimeoutMs(): number` (8000)
12. `rpc_cooldown_ms(consecutive_failures: u32) -> f64` → `rpcCooldownMs(consecutive_failures: number): number`
    - Takes `u32` because the core does, so TS gets a plain number rather than a `bigint`.
    - Values: 0 → 0; 1..5 → 30000, 60000, 120000, 240000, 300000.
13. `browser_site_label(title: &str, host: &str) -> String` → `browserSiteLabel(title: string, host: string): string`
    - Returns `{"name","host_line"}`; `host_line` is `null` when the name is the host.
14. `mark_miss_ttl_ms(kind: &str, status: Option<u16>) -> Option<u32>` → `markMissTtlMs(kind: string, status?: number | null): number | undefined`
    - When a status is passed, the class comes from `mark_miss_of_status(status)`.
    - Otherwise `kind` is read as a `MarkMiss` wire name. A name the core doesn't know counts as `unknown`.
    - `undefined` means the miss lasts for the session.
15. `balance_read_plan(chain_id: u32, stables_json: &str, wrapped_native: Option<String>, custom_json: &str) -> JsResult<String>` → `balanceReadPlan(chain_id: number, stables_json: string, wrapped_native: string | null | undefined, custom_json: string): string`
    - Inputs: `stables_json` is `[{symbol, contract}]` (extra fields are ignored); `custom_json` is `[{contract, symbol, name?, decimals}]`. `""` counts as an empty list.
    - Output is a JSON array of `ReadSlot`.

Errors are thrown as the crate's usual `{code, message}` object, for example `{"code":"Internal","message":"…userOpSubmitStep: reply: …"}`.

## For T046 and the web
- **Types:** `SubmitStep` and `SiteLabel` have no ts-rs type in the core, so the web has to write those two types by hand in `kernels.ts`. `SignEnding`, `SignEndingState`, `TrackStatusAnswer`, `ReadSlot`, `StableRef` and `TokenRef` get generated types once T046 adds their export lines.
- **uniffi alignment:** I couldn't agree shapes with the T044 agent. Their uniffi work in progress defines a `uniffi::Enum UserOpSubmitStep`, while the wasm side answers the core's `SubmitStep` JSON, so the two surfaces use different result types. The `markMissTtlMs` rule (the status wins when there is one) should also be checked against T044.
- **tasks.md:** T045's checkbox in `tasks.md` is still `[ ]`. Ticking it was outside "lib.rs only", and the other agent is editing the same checkout at the same time.

---

T044 is committed as `bb953540` on `082-dapp-browser-mac-ext-ios`. It changes only `/Volumes/data/production/vela-wallet-082/rust/crates/vela-core-uniffi/src/lib.rs`, it was not pushed, and every gate passed.

## Gates (run in `/Volumes/data/production/vela-wallet-082/rust`, final tree)
| Gate | Result |
|---|---|
| `cargo build -p vela-core-uniffi --release` | Finished (2m 26s) |
| `cargo test -p vela-core-uniffi` | 14 passed, 0 failed (10 new tests in `tests_082`, 4 existing multicall tests) |
| `cargo clippy -p vela-core-uniffi --all-targets -- -D warnings` | clean |
| `cargo fmt --all --check` | exit 0 |
| `bash scripts/smoke-swift.sh` | "48753 conformance cases green through the Swift bindings (244 skipped …)"; the generated Swift compiles with every new type |
| `bash scripts/smoke-kotlin.sh` | "48753 conformance cases green through the Kotlin bindings (244 skipped …)" |

One hash test runs both Gnosis vectors through the `UserOpDraft` record, with the gas fields as decimal strings. It gets back the EntryPoint's own `topics[1]` hash for each.

## Exports (Swift signatures as generated)
**Submit and follow**
- `userOpHash(draft: UserOpDraft, chainId: UInt32) throws -> String`
- `userOpSubmitStep(reply: UserOpSubmitReply, attempt: UInt32, maybeDelivered: Bool, localHash: String) -> UserOpSubmitStep`
  - `UserOpSubmitReply` is one of: `hash(hash)`, `rpcError(errorJson)`, `noAnswer`.
  - `UserOpSubmitStep` is one of: `accepted(userOpHash)`, `maybeSent(userOpHash)`, `notSent(rejection: RelayRejection?)`, `retryAfter(delayMs: UInt32)`.
- `userOpNotSentDetail() -> String`
- `userOpStatusMethod() -> String`
- `parseUserOpStatus(json: String) -> TrackStatusAnswer?`, where `TrackStatusAnswer` is `{status: String, stage: String?, txHash: String?}`.

**The ending of a request**
- `signEndingOf(method: String, payloadJson: String, submittedUserOp: String?) throws -> String?`. It returns the `SignEnding` as JSON.
- `signEndingState(endingJson: String, trackEntryJson: String?) throws -> String`. It returns the `SignEndingState` as JSON; passing nil or `"null"` for the entry gives `following`/`landing`.
- `dappReceiptWaitMs(elapsedMs: Double) -> Double`
- `sendReceiptOutcomeOf(trackEntryJson: String) throws -> String?`. It returns the `SendReceiptOutcome` as JSON.

**Simulation**
- `simOutcome(user: String, replyJson: String) -> SimOutcomeRecord`, where the record is `{kind, deltasJson, revertReason?, noticeRisk?, noticeKey?}`.

**Browser**
- `browserLoadGiveUpMs() -> UInt32`
- `browserLoadShouldGiveUp(elapsedMs: UInt32, committed: Bool, progress: Double) -> Bool`
- `browserLoadStalled() -> BrowserLoadFailure`
- `browserLoadRetryWhenNetworkReturns(class: String) -> Bool`
- `browserAddressBar(shown: String?, pending: String?, failed: String?) -> BrowserAddressBar`, where the record is `{url, host, lock: "closed"|"open"|"none"}`.
- `browserSiteLabel(title: String, host: String) -> BrowserSiteLabel`, where the record is `{name, hostLine: String?}`.
- `browserLoadClassify` has the same signature and now returns class `"proxy"` for probe code 6, Apple CFNetwork 306–310 and Android -5. Its doc comment is updated.

**Network, logos, balances**
- `netHealthStep(misses: UInt32, online: Bool, reached: Bool) -> NetHealthStep`, where the record is `{misses, online, edge: "went_offline"|"came_back"?}`.
- `markMissTtlMs(kind: String, status: UInt16?) -> UInt32?`. When a status is given, the status decides. Without one, `kind` is read as a `MarkMiss` name, and a name the core does not know counts as `unknown`. This matches the rule the wasm export uses.
- `balanceReadPlan(chainId: UInt32, stablesJson: String, wrappedNative: String?, customJson: String) throws -> [BalanceReadSlot]`, where the record is `{kind, contract?, symbol, name, knownDecimals: UInt32?, pegUsd: Double?}`. Blank JSON text counts as an empty list.

## Decisions for you
- **`sendReceiptOutcomeOf` is an addition.** It is not in §12. It wraps `send::receipt_outcome_of`, because both phones still work out the tracker-to-`ReceiptUpdate` mapping in the shell. Say if you would rather drop it.
- **Left out on purpose:**
  - `EXTENSION_REQUEST_TTL_MS`, `RPC_READ_TIMEOUT_MS` and `cooldown_ms`: only the extension pins these, and they are in the wasm exports.
  - `userOpEventTopic`: `FindOpEvent` already carries the topic in its `topic0` field.

## Left for later steps
- Tick T044 in `tasks.md`; I did not touch that file.
- T046 still has to rebuild the xcframework, regenerate the committed `app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift`, and build the Android `.so`. The smoke scripts already regenerated the gitignored `rust/bindings/kotlin` and `rust/bindings/swift` from this tree.

---

T046 and T047 are finished, with four commits on `082-dapp-browser-mac-ext-ios`, none pushed. `check-ios-core-fresh.sh` prints ok, and 14 of the 16 gates pass; the two that fail are the two reds tasks.md expected.

## T046: what was regenerated

| Commit | What |
|---|---|
| `92bab021` | **TypeScript types** (`gen:core-types`): 369 wallet-state types, 2 of them new (`SignPhase`, `ClearPlainSend`). I also added six top-level types to `rust/crates/vela-core/src/bin/generate_wallet_state_bindings.rs`. These are the JSON shapes the new wasm functions return or take: `SignEnding`, `SignEndingState`, `TrackStatusAnswer`, `StableRef`, `TokenRef` and `ReadSlot` (plus `ReadKind`). The core already marks each for export, but nothing reached them. Without this the web would write those types by hand. Say if you would rather drop it. |
| `0d96a18a` | **wasm**: `rust/pkg-web` and `assets/wasm/vela_core_bg.db3646bd0ab2.wasm`, replacing `55d58f879149`. It is 4,140,758 bytes and carries all 16 new functions. `verify:wasm` passes 48,798 cases. `pnpm sync:wasm` wrote the gitignored `static/` copy. |
| `579807b8` | **iOS**: rebuilt the xcframework (gitignored) and the committed `app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift`, which has all 20 new functions. `check-ios-core-fresh.sh` prints ok; `smoke-swift.sh` passes 48,753 cases. The iOS dev fixtures were rebuilt and their Swift file did not change. |
| (not committed, gitignored) | **Android**: `build-android.sh` built the `.so` for 3 ABIs, plus the debug dev-fixtures libraries. `smoke-kotlin.sh` regenerated `rust/bindings/kotlin` (all 20 new functions present) and passes 48,753 cases. `build-dev-fixtures.sh --host` also ran. |

## T047: gates

The raw output is in `/Volumes/data/production/vela-wallet-082/specs/082-dapp-browser-mac-ext-ios/evidence/gates/phase-0-1.txt`, committed as `0817e4f3`.

**Passed (14):**
- Core tests: 2,037 passed, 0 failed.
- Language-data size: 134,713 bytes, or 138,729 on the route the web build uses, under the 138,800 budget.
- The two Gnosis operations: our computed hash equals the EntryPoint's own for both.
- `clippy -D warnings` and `fmt --check`.
- The four i18n steps, with no change afterwards to `paths.rs`, `i18n_catalogs` or `assets/i18n`.
- iOS/Android copy parity.
- `gen:core-types --check`, `build-web --check` and `verify:wasm`.
- `check-ios-core-fresh.sh`, `smoke-swift.sh` and `smoke-kotlin.sh`.

**Expected reds (2):**
- **`check-event-payloads.mjs`**: 5 mismatches, all on iOS; before T046 it found 0.
  - `SendExecutor.swift:459`, `TrackerStore.swift:74` and `SigningController.swift:230` don't send `maybe_sent` / `submit_block` yet.
  - `TrackerExecutor.swift:70` doesn't send `tx_hash`.
  - The `SendStore.swift:254` row is labelled `NetHealthBody`, but the real gap is the missing `not_sent` on the "failed" receipt answer. The checker picked the wrong type because two candidates scored equally.
  - Every one of these fields defaults in the core, so the core still accepts what the shells send now.
- **`pnpm check`**: 29 errors in 15 files. They are hand-built objects missing the new required fields, plus switches with no case for the new tracker operations. This stays red until T075.

## What breaks in each client until it mirrors the new variants

**Android** (the decoder refuses values it doesn't know, so the whole view or operation fails to decode):
- `TrackerWire.kt`:
  - `TrackStatus` has no `not_sent` and `TrackOutcome` has no `maybe_sent`. The whole tracker view fails once any entry is in either state.
  - `TrackOperation` has no `holdings_moved` or `find_op_event`, so those operations can't be decoded or answered.
  - `TrackShellResult` has no `op_event`.
- `ClearWire.kt`: `ClearSurface` has no `plain_send`, so the clear-signing view fails for a dApp call with no data (a plain send).
- `SendWire.kt`:
  - `SendReceiptStatus` has no `maybe_sent` or `not_sent`, so the send view fails once a submit reply is lost.
  - `SendReceiptOutcome` has no `acknowledged`, and `Failed` has no `not_sent`.
- `RpcWire.kt`: `RpcTransportOutcome` has no `not_connected`.
- Predicted from reading, not run: `CoreWireDriftTest` will go red on the `RpcTransportOutcome` and `SendReceiptOutcome` completeness checks.

**iOS:**
- `ClearWire.swift`: `ClearSurface` is a strict Swift enum with no `plain_send`, so the clear-signing view fails to decode for a plain send.
- `TrackerExecutor.swift`: `holdings_moved` and `find_op_event` are logged as unhandled and answered with a `clock` result. So the tracker's search for the operation's event on chain never gets a real answer.
- `TrackerWire.swift` and `SendWire.swift` (`SendReceiptWire.status`) read status and outcome as plain strings. They still decode, but `not_sent` and `maybe_sent` get no words of their own.
- New fields such as `phase`, `pending_op_maybe_sent`, `relay_tx_hash`, `unreached_chains` and the feed's `kind`/`status`/`site` are ignored until they are mirrored.

**Web:**
- `tracker-executor.ts:152,254`, `tracker-resident.ts:95` and `dapp-receipt.ts:159` have no case for the new variants and return `undefined` for them at runtime.
- The other 25 errors are missing required fields: `maybe_sent`/`submit_block`, `not_sent`, `maybe_delivered`, `tx_hash`, `plain_send`, `phase`, the two empty-state keys and `dapp_origin`.

**Desktop:** it uses the Rust crate directly. I did not build it; any `match` that doesn't cover the new variants will fail to compile there in phase 3.

## Loose ends
- T046 and T047 are not ticked in `tasks.md`; T044 and T045 aren't either.
- `extension/dist` was not rebuilt. It is gitignored and the web phase's test setup builds it, so a device pass on the extension needs a fresh build.
- `device-pass-plan.md:6` still names the old wasm file (`vela_core_bg.55d58f879149.wasm`), as a record of the 079 build. I left that file alone.