# Contracts: what the iOS shell answers

Thirty operations across three machines. Each row is one operation, the result
variant that answers it, and the transport that produces it. Every family below
is an exhaustiveness entry in `CoreWireDriftTests`.

**The four rules** (`specs/024-web-live-shell/contracts/shell-operations.md`,
unchanged since):

1. Every operation is answered **exactly once** — a skipped one leaves the core
   waiting forever.
2. An expected failure is answered with the machine's own failure variant; the
   executor never throws and never classifies.
3. Executors hold **no business `if`** — one operation, one call.
4. The `switch` is exhaustive over the operation tags, with a loud `default`
   that still answers (the `neutralAnswer` twin).

---

## `send` — `SendWire.swift` ↔ `send.rs` (`SendOperation`, `SendShellResult`)

| Operation | Answer | iOS answers with |
| --- | --- | --- |
| `fetch_tokens {address}` | `tokens_loaded {tokens?, chains}` | the `balance_dashboard` view's holdings + the `network_admin` view's chains — **no second walk** |
| `clear_token_cache {address}` | `token_cache_cleared` | `WalletStore.refresh()` |
| `resolve_token_metadata {chain_id, address}` | `token_metadata {meta?}` | `TokenMetadata.symbolAndDecimals` (051) over `RpcPool` |
| `add_network {chain_id}` | `network_added {outcome}` | **`Error`** — the scanner is this operation's only entry, and it is 055 |
| `estimate_fee {chain_id, account, tx?, batch?, gas_fee_token?, public_key_hex?}` | `fee_estimated {outcome}` | the live `fee_policy` session (research D6) |
| `probe_treasury {chain_id}` | `treasury_probed {probe}` | `RelayClient.probeTreasury` — REST, **404 = uncovered**, not an error |
| `load_account_credential {account_id}` | `account_credential {public_key_hex?}` | `AccountStore` |
| `submit_user_op {…}` | `submitted {user_op_hash, now_ms}` / `submit_failed {failure}` | `UserOpSpine.submit` — assembly is the core's, transport is `RelayClient`'s, the one seam is `UserOpSigner` |
| `cancel_passkey_sign` | `passkey_cancel_acknowledged` | cancel the signing `Task` |
| `persist_tx_records {records}` | `records_persisted` | `TxRecords.writeRecords` — **one atomic write**, never per record |
| `track_submitted {user_op_hash, record_ids, chain_id}` | `track_handed_off` | `TrackerStore.submitted(…)` |
| `resolve_identity {address}` | `identity_resolved {identity?}` | `RecipientIdentity` (051) — the same instance the address book uses |
| `resolve_risk {chain_id, address}` | `risk_resolved {risk?}` | `eth_getCode` through the pool, **raw bytes to the core** (051's EIP-7702 finding), plus `first_time` from `TxRecords` |
| `simulate_calls {…}` | `sim_resolved {sim_json: null}` | **nothing** — no simulation engine on this base (055) |
| `start_timer {ms, tag}` | `timer_elapsed {tag}` | `Task.sleep`, cancellable |
| `haptic {kind}` | `haptic_played` | `UIImpactFeedbackGenerator` / `UINotificationFeedbackGenerator` |
| `show_alert {kind}` | `alert_acknowledged` | rendered by `SendLive` as the core's wording |
| `close` | `closed` | `FlowNav.close()` |

**Events the live screens dispatch**: `Open, SelectToken, SetRecipient,
SetAmount, ToggleFiatInput, TapMax, OpenContactPicker, CloseContactPicker,
PickedAddress, Continue, Back, EditAmount, ChooseFeeToken, FeeUpdated,
FeeBusyChanged, SlideConfirm, SigningStarted, CancelSigning,
RetryAfterBootstrap, DismissTreasurySheet, RetryAfterError, ReceiptUpdate,
Done, RefreshTokens, DisplayChanged`.

**Not dispatched in 052**: the multi/split family (054), `OpenScanner` /
`CloseScanner` / `ScanResolved` and `AddNetworkTapped` (055),
`OpenBatchImport` (054).

---

## `fee_policy` — `FeeWire.swift` ↔ `fee_policy.rs`

| Operation | Answer | iOS answers with |
| --- | --- | --- |
| `fetch_gas_price {chain_id, want_tip}` | `gas_price {eth_gas_price?, base_fee?, priority_fee?}` | pool: `eth_gasPrice` ∥ latest block `baseFeePerGas` ∥ `eth_maxPriorityFeePerGas`. `want_tip` is false on Tempo — asking there corrupts the stablecoin reimbursement |
| `fetch_bundler_quote {chain_id, tier}` | `bundler_quote {quote?}` | `pimlico_getUserOperationGasPrice`; `nil` = unsupported → the core's local fallback |
| `fetch_in_band_quotes {chain_id, account}` | `in_band_quotes {quotes?}` | `vela_getInBandGasQuote [{safeAddress}]`, 8-second cache |
| `fetch_fee_recipient {chain_id, account}` | `fee_recipient {recipient?}` | `GET /v1/account/{chain}/{safe}` → `settlementRecipient ?? depositAddress` |
| `estimate_user_op_gas {chain_id, account, deployed, calls}` | `user_op_gas {outcome}` | a draft with the dummy signature → `eth_estimateUserOperationGas` |
| `start_ttl {ms}` | `ttl_elapsed` | `Task.sleep` |

**The bundler is the price authority.** A `nil` from any of these is a fact the
core degrades on; a shell that substituted its own number would be a second
opinion nobody diffed.

---

## `tx_tracker` — `TrackerWire.swift` ↔ `tx_tracker.rs`

| Operation | Answer | iOS answers with |
| --- | --- | --- |
| `poll_receipt {user_op_hash, chain_id}` | `receipt_with_logs` / `receipt` / `receipt_failed` / `receipt_pending` / `receipt_unreachable` | `eth_getUserOperationReceipt`. The mapping is the core's doc: RPC error or throw → **unreachable**; no result or no `txHash` → **pending**; `success !== false` → **receipt**; `success === false` → **failed**. iOS answers `receipt_with_logs` (it can read them), so the core can catch an `ExecutionFailure` inside a "successful" op |
| `poll_status {user_op_hash, chain_id}` | `status {…}` / `status_unavailable` | `eth_getUserOperationStatus`; an older relay answers `status_unavailable`, which is not a failure |
| `load_pending_txs` | `records_loaded {records, now_ms}` | `vela.transactionHistory` rows with `status == "pending"`, a `userOpHash` and an empty `txHash`; an unreadable store answers an **empty list**, as every client does |
| `update_tx_records {ids, patch}` | `records_patched` | one atomic patch, then `activity_feed::ReconcileCompleted` |
| `notify_confirmed {user_op_hash, chain_id, tx_hash}` | `notified` | the local notification when backgrounded, **and** the polled receipt's authentic logs to `token_trust::ReceiptLogsConfirmed` — the single auto-add entry point |
| `now` | `clock {now_ms}` | the wall clock |

**Events**: `Submitted, Tick, AppResumed, HomeFocused, Abort`.

**The cadence is the core's**; the shell only supplies the clock. Foreground:
3 s while pending. Background: the grace window, then whatever
`BGTaskScheduler` grants (research D8).

---

## The fixtures library (Debug only) — `vela_dev_fixtures`

| Function | Returns |
| --- | --- |
| `fixtureAccounts()` | `[{index, credentialIdHex, name, publicKeyHex, address}]` |
| `fixtureMultiAddress()` | the golden multi-key Safe — one address for all keys |
| `fixtureAssert(challenge, allowCredentialIds, preferred?)` | `{credentialIdHex, signatureDerHex, authenticatorDataHex, clientDataJsonHex}` — the onboarding `Assertion`'s four hex fields |
| `fixtureRegistration(index)` | a registration, unused by this cut |

`RP_ID`, `ORIGIN`, the platform attachment and the `internal` transport are the
core's constants, not the shell's.

---

## The user-operation assembly (already exported, called for the first time)

| Function | Purpose |
| --- | --- |
| `userOpFloors(chainId, deployed, subCalls)` | per-chain gas floors |
| `userOpDraft(sender, nonce, deployed, keyHexes, calls, fee, floors)` | the unsigned draft |
| `userOpApplyEstimate(draft, vgl, cgl, pvg, floors)` | fold the bundler's estimate in, floor-clamped |
| `userOpWithCalls(draft, calls, fee)` | re-write the calls and the **settled** fee leg |
| `userOpSafeOpHash(draft, chainId)` | **the 32 bytes the passkey signs** |
| `userOpSign(draft, assertion, credentialIdHex, keys)` | pack the assertion into the operation's signature |
| `userOpRelayJson(op, feeToken)` | what the bundler receives |
| `userOpHasContractCall(calls)` | gates whether a failed estimate is fatal |
| `quotedFeeUsable(amount, recipient)` | the displayed-equals-signed gate |
| `parseExistingUserOpHash(message)` | an idempotent re-submit |
| `classifyRelayRejection(message)`, `relayErrorMessage(json)` | the relay's words → the core's verdict |
| `isChainWithoutNativeCoin(chainId)` | the Tempo branch |
| `safeMessageHash`, `eip1271Signature` | ported with the spine, **called in 053** |

No Swift computes a hash, a leg, a fee or a signature.
