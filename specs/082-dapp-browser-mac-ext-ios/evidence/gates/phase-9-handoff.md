# 082 Phase 9: what the round-2 core and its bindings give the four clients

The merge + bindings step (T197's rebuilds), 2026-09-29, on `082-dapp-browser-mac-ext-ios` in
`/Volumes/data/production/vela-wallet-082`. Nothing was pushed. This is the single hand-off for
DESK_A, DESK_B, WEB_B, IOS and AND. The authority is `contracts/core-rules.md` (§1, §3, §4, §5, §7,
§10, §11, §12 and §16, "Round 2"), and `data-model.md` §1, §2, §4 and §11.

## 1. Merge and commits

| Commit | What |
|---|---|
| `7a86cebc` | `merge(082): the round-2 core`. It merges `082-core-h` at `dab496f6`. The CORE commits T185–T198 are listed in the CORE report. The review's four follow-ups are `f5a1628a`, `09309fe3`, `b0ea4056` and `2762df54`, and `dab496f6` updates the docs and the gates file. There were **no conflicts**: since the merge base `91a12b0b`, the core branch and DESK_A/WEB_A touched no file in common. |
| `9f876e48` | **wasm**: `rust/pkg-web` and `assets/wasm/vela_core_bg.3cf42392a06f.wasm` (4,184,297 bytes). It replaces `80945236660e`. `sync:wasm` wrote the gitignored `static/` copy. |
| `b27819d0` | **iOS**: the committed `app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift`, plus the xcframework (gitignored, stamped `4dd8ce5c…`). The dev-fixtures xcframework was rebuilt too, and `vela_dev_fixtures.swift` did not change. |
| (gitignored, not committed) | **Android**: `libvela_core_uniffi.so` for arm64-v8a, armeabi-v7a and x86_64 in `app/src/main/jniLibs`; the debug dev-fixtures `.so` for the same three ABIs; `rust/bindings/kotlin` (from `smoke-kotlin.sh`); `rust/bindings/kotlin-dev` (from `build-dev-fixtures.sh --host`). |
| (this file) | The hand-off, and T197 ticked in `tasks.md`. |

The ts-rs types were already regenerated on the core branch (`141b2d8c`), and `gen-core-types --check`
is clean on the merged tree.

## 2. Gates on the merged tree

| Gate | Result |
|---|---|
| `cargo test -p vela-core --features i18n-all,crux` | **2099 passed, 0 failed, 0 ignored (53 result lines); equal to the review re-run at `dab496f6`** |
| `--test i18n_residency` | **runtime JSON ja + en 138,671 of 138,800 (129 B headroom); resident ja + en 134,651; 6 passed** |
| `cargo test -p vela-core-uniffi -p vela-core-wasm` | 25 passed, 0 failed (uniffi 15, wasm 10) |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy -p vela-core --features crux -- -D warnings`; `-p vela-core-uniffi -p vela-core-wasm --all-targets` | clean (both) |
| i18n corpus (`gen:i18n` → `lint:i18n` → `verify:i18n`) | `gen:i18n` rewrote nothing (no diff; 1785 paths / 1696 leaves); `lint:i18n` no new defects; `verify:i18n` 75,440 comparisons, zero divergences |
| `node rust/scripts/build-web.mjs` and `--check` | written; `rust/pkg-web is current (wasm 4184297 bytes)` |
| `verify:wasm` (`rust/scripts/verify-web.mjs`) | 48,813 conformance cases green (199 skipped: core-only functions), plus 5 boundary regressions green |
| `app-web/vela-wallet: node scripts/sync-wasm.mjs --check` | `static/vela_core_bg.3cf42392a06f.wasm matches pkg-web` |
| `node rust/scripts/gen-core-types.mjs --check` | onboarding 27, session 13, wallet-state 371: current |
| `bash rust/scripts/check-ios-core-fresh.sh` | **ok**: the xcframework is this tree's core (`4dd8ce5c396c…`) |
| `smoke-swift.sh` / `smoke-kotlin.sh` | 48,768 cases green through each (244 skipped); the onboarding bridge is green |
| `build-android.sh` | 3 ABIs (main) + 3 ABIs (debug dev fixtures) |
| `node scripts/check-event-payloads.mjs` | **11 mismatches, expected** until T236 (iOS). `TrackerStore.swift:92` sends `submitted` without `admitted`. Ten `SignExecutor.swift` sites (lines 205, 227, 293, 297, 299, 302, 311, 335, 337, 339) send `failed` without `refused`, and the ruler then mislabels them `NetHealthBody`. Both fields have serde defaults, so the core still reads these events. 522 literal sites checked, 2 skipped. |

**Expected reds until each client adopts:**
- Desktop does not compile: 16 errors in the binary and 29 with tests (CORE's count), until T217.
- `pnpm check` stays red until T227.
- The iOS build breaks on `netHealthStep` (`NetWatch.swift:67`) and on any exhaustive switch over the new variants, until T236/T241.
- The Android build breaks on `netHealthStep` (`core/net/NetHealth.kt:35`, `NetHealthTest.kt:29`) and on the strict wire decoders, until T244/T249.

## 3. Open items this step did not close

1. **Uncommitted core work in `/Volumes/data/production/vela-wallet-082-core-h`.** It is not merged and not mine. It changes `src/app/sign_request.rs` (+9 lines in `on_submit`) and adds a test to `tests/app_sign_request.rs`: `a_withdrawn_write_ahead_s_late_ack_never_clears_a_newer_post`.
   - The defect: `RecordPersisted` names no record. Say a shell's `WRITE_AHEAD_WAIT_MS` wait runs out on a stalled disk, and the write-ahead is withdrawn. That record's late ack can then clear a NEWER request's `ClearToPost` before the newer record is on disk. That breaks RJ1 for the newer op.
   - The fix bumps the attempt so the late ack is dropped.
   - Once it is committed on `082-core-h`, it needs one more merge plus a wasm, xcframework and `.so` rebuild, because the fingerprint moves. No client code changes.
   - Until then, the edge needs a stalled disk (> 5 s) and a second dApp request, so it is unlikely on a device pass.
2. `specs/082-dapp-browser-mac-ext-ios/evidence/ios/post-results.md` is untracked in the integration worktree. It appeared at 07:19 from another agent, and I left it alone.
3. `extension/dist` is gitignored and was not rebuilt. It still loads `80945236660e`, so rebuild it (WEB gate T235) before any extension device row.
4. Every device build needs a fresh bindings step after the last core commit. Run `check-ios-core-fresh.sh` before any iPhone run.

## 4. What changed in the core (exact names)

### `user_op` (§1)
- `REFUSED_DAPP_DETAIL = "the network refused this transaction; nothing was sent"`. It is the -32603 detail for a relay refusal.
- `WRITE_AHEAD_WAIT_MS = 5000`.
- `EstimateFailure { Reverts{reason: Option<String>}, Unavailable }` (serde tag `type`), and `estimate_failure(error_json)`. Both need the `crux` feature.
  - **Reverts** when the message or a string `data` holds "reverted" or "AA23", or the code is -32521. The reason comes only from ABI `Error(string)` bytes, and is sanitised.
  - **Unavailable** for -32500 alone (AA21), a timeout, an exhausted pool, a rate limit, or no answer.

### `fee_policy` (§1)
- `FeeFailure::ChainRead{rate_limited: bool}`. Its wire form is `{"chain_read":{"rate_limited":…}}`; every other failure is still a plain string.
  - The shells produce it: the deployment read is theirs, and the fee machine never emits it.
- `requote_delay_ms` is 3000, then 6000, then 8000 ms. `REQUOTE_TIMEOUT_MS = 6000` bounds each automatic re-quote.
- `failure_reason_key(failure) -> Option<&str>`:
  - relay failures → `componentsUi.funding.denialNetworkError`
  - `ChainRead{true}` → `home.balanceDetailStatusRetrying`
  - `ChainRead{false}` → `explore.chainDown`, with `{{chain}}` = the chain's name
  - `MissingPublicKey` / `CalculationFailed` → `None` (no reason line, show the dash)

### `tx_tracker` (§3)
- `Event::Submitted{.., admitted}`. `admitted` sets `acknowledged`, so the entry never reads MaybeSent, and a `not_found` no longer counts against it.
  - `admitted: true` on a **NotSent** entry starts a fresh entry. This is the review's fix for a NotSent that arrived while the POST was still out. A `Rejected` entry stays terminal.
- `Event::Withdrawn{user_op_hash, record_ids}` drops those ids. An entry left with no ids is removed, with no patch and no `HoldingsMoved`.
- `TrackOperation::TxReceipt{chain_id, tx_hash, user_op_hash}` is issued for a relay-named tx hash at the receipt cadence, one in flight per hash.
  - Answer with `TrackShellResult::TxReceipt{user_op_hash, now_ms, receipt_json}`, where `receipt_json` is the RPC `result` as it came (`"null"` = not mined) and `None` = no answer.
  - The core checks only the op's own logs for `ExecutionFailure`.

### `sign_request` (§4)
- **Events**
  - `OpSigned{id, user_op_hash, submit_block?, now_ms}`: after the passkey, the local hash and the head read, before any POST.
  - `OpTracked{user_op_hash, status: TrackStatus, tx_hash?, now_ms}`.
- **Operations**
  - `ClearToPost{id, user_op_hash}`: answer `Responded`.
  - `DeleteRecord{record_id}`: answer `RecordUpdated`.
- **Record and hand-off**
  - `SignRecordClose::Admitted` sets the record's `maybeSent` to false; it stays pending.
  - `SignTrackerHandoff.admitted`.
  - New struct `SignTrackerWithdraw{user_op_hash, record_ids}`.
- **View**
  - `SignView.tracker_withdraw: Option<SignTrackerWithdraw>`.
  - `SignView.failure_refused: bool`, set and cleared with `error`.
- **Outcome and ending**
  - `SignSubmitOutcome::Failed{message, refused}`: a `refused: true` answers `REFUSED_DAPP_DETAIL` whatever `message` says.
  - `SignEndingState::Refused`, drawn as a cross, `statusFailed` + `componentsUi.signing.refused`, with no Retry words.
- **Answer rules (the core's; clients only forward).** Past `OpSubmitted`, unanswered, same op:

  | Tracker status | Answer |
  |---|---|
  | `Confirmed` / `Dropped` with a tx hash | `Ok(tx hash)` |
  | `Rejected` | -32603 `REFUSED_DAPP_DETAIL` |
  | `NotSent` | -32603 `NOT_SENT_DAPP_DETAIL`, **only** if the op was may-have-been-sent. For an Accepted op it waits (review). |
  | anything else | wait |

  - Once answered, the attempt moves on, so the shell's own late `Submit` result is dropped.
  - `AccountSwitched` is accepted whatever the attempt.
  - A `Landed` ending (it holds a tx hash) is never drawn NotSent or Refused; it stays `Following{Landing}`.

### `send` (§5)
- `Event::OpSigned{user_op_hash, submit_block?, now_ms}`.
- `SendOperation::ClearToPost{user_op_hash}`: answer with the new `SendShellResult::PostCleared`.
- `SendOperation::MarkAdmitted{record_ids}`: one write, answer `RecordsPersisted`.
- `SendOperation::DeleteTxRecords{ids}`: one write, answer `RecordsPersisted`.
- `SendOperation::TrackWithdrawn{user_op_hash, record_ids}`: send it to the tracker's `Withdrawn`, answer `TrackHandedOff`.
- `TrackSubmitted.admitted`.
- A `ReceiptUpdate{Failed{not_sent: true}}` stamps the receipt only while the receipt reads MaybeSent (review).

### `activity_feed` (§7)
- `FeedTxRecord.call_data: Option<String>` (serde default).
- `FeedItem.counterparty_role: FeedCounterpartyRole{Recipient (default), Contract}`.
  - Call data that is exactly `transfer(address,uint256)` → the decoded recipient, `Recipient`.
  - Other call data → `to`, `Contract`, labelled `componentsUi.signing.interactingLabel`.
- `tx_hash` is `None` when it equals `user_op_hash`. An op hash never becomes an explorer link, so no explorer control is drawn without a URL.

### `browser_load` (§10, desktop only, no FFI)
- `same_address(a, b)`.
- `LoadWatch.own_request` and `LoadWatch.clock_ms`.
- `retry_fired_at(generation, in_front, now_ms)` and `take_due_at(now_ms)`. The old two keep their signatures.
- The wallet's own retry never resets `attempt`. A hung provisional load older than `GIVE_UP_MS` and not live is replaced with `RetryAction::Load(url)`.

### `net_health` (§11)
- `NetHealth{misses, online, sources, unsourced, last_reach_ms, run_started_ms}`. It is **no longer `Copy`/`Eq`**.
- `net_health_step(state, reached, source: Option<u32>, now_ms)`.
- `SOURCES_BEFORE_OFFLINE = 2`, `OFFLINE_QUIET_MS = 10000`. Offline needs 3 misses in a row from 2 or more sources (a miss with no source counts as its own), and nothing reached for 10 s.

### `l10n::number`
- `format_signed_token_amount(delta_base_units, decimals, preset) -> Option<String>`.
  - `None` for zero or unreadable text.
  - U+2212 for a minus, `+` for a plus.
  - Dust is written exactly, never `−0`.

### Corpus (§16)
- New: `componentsUi.signing.refused` ("The network refused it — nothing was sent." / 网络拒绝了这笔交易，什么都没有发出。).
- `componentsTx.receipt.failedHint` is trimmed.
- zh and zh-TW `send.txBackgroundHint` gained the comma.
- 1785 paths / 1696 leaves. ja + en 138,671 of 138,800 (129 B left).

### Exports
- **UniFFI (Swift/Kotlin)**
  - `userOpRefusedDappDetail()`
  - `userOpWriteAheadWaitMs()`
  - `userOpEstimateFailure(errorJson) -> UserOpEstimateFailure{kind: "reverts"|"unavailable", reason?}`
  - `feeRequoteTimeoutMs()`
  - `feeFailureReasonKey(failure) -> String?`
  - `formatSignedTokenAmount(deltaBaseUnits, decimals, preset) -> String?`, where `preset` is `comma_dot`, `dot_comma`, `space_comma` or `indian`
  - `netHealthFresh() -> NetHealthState`
  - **changed:** `netHealthStep(state: NetHealthState, reached, source: UInt32?, nowMs: Double) -> NetHealthStep{state, edge?}`. The old `(misses, online, reached)` form is gone.
  - `feeRequoteDelayMs(failure, attempt)` and `feeFailureReasonKey(failure)` take the wire name **or** the `ChainRead` JSON.
- **wasm**
  - `userOpRefusedDappDetail()`
  - `userOpWriteAheadWaitMs()`
  - `userOpEstimateFailure(errorJson)`, returning JSON `{"type":"reverts","reason":…}` or `{"type":"unavailable"}`
  - `formatSignedTokenAmount(delta, decimals, preset)`
  - `feeRequoteTimeoutMs()`
  - `feeFailureReasonKey(failure)`
  - `feeRequoteDelayMs` accepts the `ChainRead` JSON
  - There is no wasm `netHealthStep`; the web has no net-health counter.
- **ts-rs (`W/core/generated/`)** regenerated:
  - new types: `FeedCounterpartyRole`, `SignTrackerWithdraw`
  - `FeeFailure` is now `string | {chain_read: {rate_limited}}`
  - also: `FeedItem`, `FeedTxRecord`, `SendEvent`, `SendOperation`, `SendShellResult`, `SignEndingState`, `SignEvent`, `SignOperation`, `SignRecordClose`, `SignSubmitOutcome`, `SignTrackerHandoff`, `SignView`, `TrackEvent`, `TrackOperation`, `TrackShellResult`

## 5. The protocol every client must adopt

1. **Write-ahead (RJ1).** The dApp sign path and the wallet's Send (including a split Send) follow the same order:
   1. Sign.
   2. Compute the local hash, and read the head (`submit_block`).
   3. Dispatch `OpSigned`.
   4. **Wait for `ClearToPost`, at most `userOpWriteAheadWaitMs()`.** The core sends it only once the record is on disk and the tracker holds it with `maybe_sent: true`.
   5. Run the asker check (RB2).
   6. POST.

   If `ClearToPost` does not arrive in time: do not POST. Report `Submit{Failed{message: NOT_SENT_DAPP_DETAIL, refused: false}}` on the sign path, or `SubmitFailed` on the Send path.
   - The core then deletes the record (`DeleteRecord` / `DeleteTxRecords`) and withdraws it from the tracker. The sign path does this through `SignView.tracker_withdraw`, the Send path through `SendOperation::TrackWithdrawn`.
   - Map `Admitted` / `MarkAdmitted` to `maybeSent = false` on the stored row.
   - Proof on every client: no POST before clearance; with no clearance, zero POSTs; a quit after `OpSigned` leaves one pending `maybeSent` row that the tracker resolves on the next launch (DX9).
2. **Hand-off de-duplication must include `admitted` and `maybe_sent`.** This is the review's contract, §4. Today every shell keys on `(user_op_hash, record_ids)`, and the admitted hand-off repeats both, so it would **never be fed**. The tracker would then never learn that the relay took the op, and two relay `not_found` answers would end an accepted op NotSent, with its records failed.
   - Key on `(user_op_hash, record_ids, maybe_sent, admitted)`.
   - Feed `tracker_withdraw` once per value.
   - The sites, as of this tree:

     | Shell | Site |
     |---|---|
     | desktop | `D/wallet/signing_host.rs:196/789-793` `handed_off` |
     | web | `W/signing/core/sign-resident.svelte.ts:102/315` `#lastHandoffKey` |
     | iOS | `SigningController.swift:652/760` `handoffKey` |
     | Android | `SigningController.kt:120-136` `handedRecords` (a set of record ids; it must become a key over all four) |
3. **The answer follows the tracker (RJ4).** Whenever the tracker's entry for the in-flight op changes, dispatch `OpTracked{user_op_hash, status, tx_hash, now_ms}` to the sign session. The core decides whether that answers the page. Once answered, stop the receipt wait: the late `Submit` result is dropped anyway.
4. **`TxReceipt`.** Answer it with `eth_getTransactionReceipt` through the chain pool, and pass the body's `result` verbatim as `receipt_json` (`None` when nothing came back). This closes EX13 and DX-W1: 还没上链 should not linger once the relay says `included`.
5. **Refusals (RJ3).** Set `Failed{refused: true}` when the relay refused the op. That means any submit-time NotSent reason except "relayer unavailable"; bundler-underfunded stays `Underfunded`.
   - Draw `SignEndingState::Refused` and `failure_refused` as `statusFailed` + `componentsUi.signing.refused`, with no 请重试 or Retry words.
   - Every client needs a reader for the new key.
6. **Fees (RJ12, RJ13).** Report an unanswered deployment read (`eth_getCode`) as `ChainRead{rate_limited}`, taking the rate-limit flag from the pool. Never report it as `QuoteUnavailable`.
   - Bound each automatic re-quote by `feeRequoteTimeoutMs()`.
   - Take the reason line from `feeFailureReasonKey`, and **delete the shell's own failure→words mapping**. `explore.chainDown` gets `{{chain}}`.
   - Log `fee: quote failed chain=… cause=… re-quote #n in N ms` and `fee: quote back chain=… after n re-quotes`, with no secrets.
7. **Estimate (RJ19).** Classify a failed relay estimate with `userOpEstimateFailure`. `reverts` → the danger line `simWillFail` / `simWillFailReason`. `unavailable` → today's words. `isPlainTransferCall`-style guesses no longer decide a revert.
8. **Signed deltas (RJ15).** Use `formatSignedTokenAmount`, and do not draw a `None` delta. Delete each shell's own signed-amount formatter; the desktop's is `signed_amount`, `live.rs:387-404`.
9. **Feed (RJ16).**
   - Map the stored request's first call `data` to `FeedTxRecord.call_data`.
   - Draw `counterparty_role: contract` with `componentsUi.signing.interactingLabel`, and `recipient` with today's to-label.
   - Draw no explorer control when `tx_hash` is `None`.
10. **Network health (RJ14; native shells only).** Pass the chain id as `source` and `now_ms` on every pool outcome. Keep the `NetHealthState` the core returns; do not rebuild it. A miss with no chain passes `None`.

## 6. Per client

### DESK_A (T203)
- `D/wallet/browser_host.rs:2182-2207` builds `NetHealth{misses, online}` and calls `net_health_step(health, reached)`. Move the network-back test to the four-argument form, starting from `NetHealth::default()` (online, no misses). `NetHealth` is `Clone` + `PartialEq` now, not `Copy`/`Eq`.
- The desktop does not compile until DESK_B's T217 lands, so run `cargo test` after T217.

### DESK_B (T217–T226), against the crate directly
- **T217 first:** new match arms and fields across `executor/{send,sign_request,tracker,activity_feed,pool}.rs`, `wallet/{money,live}.rs`, `signing/status.rs`, `flows/live.rs` and `contacts/live.rs`. The T217 notes in `tasks.md` say how each arm keeps today's behaviour until its task.
  - `executor/pool.rs:838` `static HEALTH: Mutex<NetHealth>` needs the new fields: `sources: Vec::new()` works in a const. `:877` `net_health_step(*health, …)` must clone, because `NetHealth` is no longer `Copy`. It then passes the chain id and the clock (T224).
- **T218/T219/T220:** write-ahead (§5.1). Also `estimate_failure` logging and `Failed{refused}` in `submit_failure`.
- **T221:** `TxReceipt`, `Withdrawn`, `admitted` (§5.4).
- **T222:** `OpTracked` (§5.3), plus the **hand-off key** in `signing_host.rs` (§5.2). Also `REQUOTE_TIMEOUT_MS`, `ChainRead` and the `fee:` lines (§5.6).
- **T223:** Refused words, `failure_reason_key`, `format_signed_token_amount`. Delete `signed_amount` and the shell's own fee reason mapping.
- **T224:** health per source. **T225:** `call_data` and `counterparty_role`.

### WEB_B (T227–T235), web + extension
- **T227:** adopt the regenerated types (`pnpm check` is red until then), including `FeeFailure`'s object form, which breaks string compares. Add a reader for `componentsUi.signing.refused`, plus the `kernels.ts` wrappers for the six new wasm exports.
- **T228/T229:** write-ahead on the sign path and the Send path (§5.1). Also the `userOpEstimateFailure` classification (§5.7) and the submit claim carrying `{opHash, chainId}`.
- **T230:** `TxReceipt`, `Withdrawn`, `admitted`, and `OpTracked` through the new `track-forward.ts`. Also the **hand-off key** in `sign-resident.svelte.ts` (§5.2).
- **T231/T232:** Refused words, the core's fee reason key and `feeRequoteTimeoutMs`, and `formatSignedTokenAmount` for deltas.
- **T233:** feed `call_data` and `counterparty_role`.
- **T235:** rebuild `extension/dist`, which still carries `80945236660e`, and re-run the event-payload ruler.
- The web has no net-health step to adopt: RJ14 is native-only, and there is no wasm export for it.

### IOS (T236–T243)
- **T236:** wire mirrors for every new variant and field. Clears the 11 ruler rows: `admitted` on `TrackerStore` `submitted`, and `refused` on the 10 `SignExecutor` `failed` sites.
  - `FeeWire`: `FeeFailure` can now be an object.
  - `ActivityWire`: `call_data`, `counterparty_role`.
- **T237:** write-ahead (§5.1); `Failed{refused}`; the store ops.
- **T238:** `TxReceipt`, `Withdrawn`, `admitted`, `OpTracked`.
- **T239:** Refused, `failure_refused`, `formatSignedTokenAmount`, `feeFailureReasonKey`.
- **T240:** `ChainRead`, `feeRequoteTimeoutMs`, `fee` log lines. Also the **hand-off key** in `SigningController.swift` (§5.2), since T240 owns that file.
- **T241:** `NetWatch.swift:67` uses the new `netHealthStep(state:reached:source:nowMs:)`, starting from `netHealthFresh()`. It does not compile until then.
- **T242:** feed.
- **T243:** gates, with `check-ios-core-fresh.sh` → ok (it is ok at this commit).

### AND (T244–T251)
- **T244:** wire mirrors. The decoders are strict, so a new variant fails the whole view until mirrored: `SignEndingState.refused`, `SignRecordClose.admitted`, `TrackOperation.tx_receipt`, `SendOperation.clear_to_post / mark_admitted / delete_tx_records / track_withdrawn`, `SendShellResult.post_cleared`, and `FeeFailure`'s object form. Add the `componentsUi.signing.refused` constant in `I18nKeys.kt`.
- **T245:** write-ahead.
- **T246:** `TxReceipt`, `Withdrawn`, `admitted`, `OpTracked`.
- **T247:** Refused, deltas, fee words.
- **T248:** `ChainRead`, the re-quote timeout, and the **hand-off key**: `handedRecords` in `SigningController.kt` (§5.2).
- **T249:** `core/net/NetHealth.kt:35` and `NetHealthTest.kt:29` move to `netHealthStep(state, reached, source, nowMs)` and `netHealthFresh()`.
- **T250:** feed. **T251:** gates.
- The `.so` and `rust/bindings/kotlin` in this worktree are current as of `b27819d0`, so do not rebuild them unless the core moves again.

## 7. What the device re-run should look at first (from CORE)
1. DX9: quit mid-submit for a dApp tx and for a split Send, then relaunch. Expect one pending "may have been sent" row, which the tracker then resolves.
2. DX-W3 / EX-W3: a relay rejection gives the page exactly one -32603 "refused" answer. The sheet says refused, with no 请重试.
3. EX-S5: when the tracker proves the op was never sent, the page gets one -32603 "not sent".
4. DX-W1 / EX13: the tx hash reaches the page as soon as the tracker knows it, including through the relay's tx hash. 还没上链 does not linger after `included`.
5. Then DX14, L2, G47, G48, G53, G49, G52.
