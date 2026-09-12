# Contracts: what the Android shell answers

Each machine's operations (what the core asks) and results (what the shell
sends back), with the Kotlin file that mirrors them and the transport that
answers. Rust line refs are to the merged tree. Every family below is an
`assertVariantsExhaustive` entry in `CoreWireDriftTest`.

## `send` — `SendWire.kt` ↔ `send.rs` (`SendOperation` :682, `SendShellResult` :782)

| Operation | Answer | Android answers with |
| --- | --- | --- |
| `fetch_tokens {address}` | `tokens_loaded {tokens?, chains}` | `BalanceDashboard` view (held tokens) + `NetView` (chains); no second walk |
| `clear_token_cache {address}` | `token_cache_cleared` | `BalanceDashboard` refresh |
| `resolve_token_metadata {chain_id, address}` | `token_metadata {meta?}` | pool `eth_call` symbol/decimals |
| `add_network {chain_id}` | `network_added {outcome}` | `Error` in 043 (entry is the scanner, 046) |
| `estimate_fee {chain_id, account, tx?, batch?, gas_fee_token?, public_key_hex?}` | `fee_estimated {outcome}` | the live `fee_policy` session (D7) |
| `probe_treasury {chain_id}` | `treasury_probed {probe}` | `RelayClient` `GET /v1/treasury/{chain}` (404 = uncovered) |
| `load_account_credential {account_id}` | `account_credential {public_key_hex?}` | `AccountStore` |
| `submit_user_op {…}` | `submitted {user_op_hash, now_ms}` / `submit_failed {failure}` | draft (core) → sign (`UserOpSigner`) → finalize (core) → `eth_sendUserOperation`; failures classified by the core's `classify_rejection` |
| `cancel_passkey_sign` | `passkey_cancel_acknowledged` | cancel the signing job |
| `persist_tx_records {records}` | `records_persisted` | `FeedExecutor.writeRecords` |
| `track_submitted {user_op_hash, record_ids, chain_id}` | `track_handed_off` | `tx_tracker` `Submitted` |
| `resolve_identity {address}` | `identity_resolved {identity?}` | D10 waterfall |
| `resolve_risk {chain_id, address}` | `risk_resolved {risk?}` | `eth_getCode` + local records |
| `simulate_calls {…}` | `sim_resolved {sim_json: null}` | none in 043 (046) |
| `start_timer {ms, tag}` | `timer_elapsed {tag}` | coroutine delay, cancellable |
| `haptic {kind}` | `haptic_played` | `Haptics` (a `send` variant per kind) |
| `show_alert {kind}` | `alert_acknowledged` | rendered by `SendLive` as the core's wording |
| `close` | `closed` | flow stack pop |

Events dispatched by the live screens: `Open, SelectToken, SetRecipient,
SetAmount, ToggleFiatInput, TapMax, OpenContactPicker, CloseContactPicker,
PickedAddress, Continue, Back, EditAmount, ChooseFeeToken, FeeUpdated,
FeeBusyChanged, SlideConfirm, SigningStarted, CancelSigning,
RetryAfterBootstrap, DismissTreasurySheet, RetryAfterError, ReceiptUpdate,
Done, RefreshTokens, DisplayChanged`. Not dispatched in 043: the multi/split
family, `OpenScanner`/`ScanResolved`, `OpenBatchImport`, `AddNetworkTapped`.

## `fee_policy` — `FeeWire.kt` ↔ `fee_policy.rs` (`FeeOperation` :370, `FeeShellResult` :398)

| Operation | Answer | Android answers with |
| --- | --- | --- |
| `fetch_gas_price {chain_id, want_tip}` | `gas_price {eth_gas_price?, base_fee?, priority_fee?}` | pool: `eth_gasPrice` ∥ latest block `baseFeePerGas` ∥ `eth_maxPriorityFeePerGas` |
| `fetch_bundler_quote {chain_id, tier}` | `bundler_quote {quote?}` | `pimlico_getUserOperationGasPrice` |
| `fetch_in_band_quotes {chain_id, account}` | `in_band_quotes {quotes?}` | `vela_getInBandGasQuote [{safeAddress}]`, 8 s cache |
| `fetch_fee_recipient {chain_id, account}` | `fee_recipient {recipient?}` | `GET /v1/account/{chain}/{safe}` → `settlementRecipient ?? depositAddress` |
| `estimate_user_op_gas {chain_id, account, deployed, calls}` | `user_op_gas {outcome}` | draft with dummy signature → `eth_estimateUserOperationGas` |
| `start_ttl {ms}` | `ttl_elapsed` | coroutine delay |

## `tx_tracker` — `TrackerWire.kt` ↔ `tx_tracker.rs` (`TrackOperation` :88, `TrackShellResult` :128)

| Operation | Answer | Android answers with |
| --- | --- | --- |
| `poll_receipt {user_op_hash, chain_id}` | `receipt_with_logs` / `receipt` / `receipt_failed` / `receipt_pending` / `receipt_unreachable` | `eth_getUserOperationReceipt`; logs cached per hash for `notify_confirmed` |
| `poll_status {…}` | `status {status, stage?, now_ms}` / `status_unavailable` | `eth_getUserOperationStatus` |
| `load_pending_txs` | `records_loaded {records, now_ms}` | rows without terminal status, `type ∈ {send, dapp_tx}` |
| `update_tx_records {ids, patch}` | `records_patched` | patch rows; then `activity_feed::ReconcileCompleted` |
| `notify_confirmed {user_op_hash, chain_id, tx_hash}` | `notified` | notification (if backgrounded) + `token_trust::ReceiptLogsConfirmed` |
| `now` | `clock {now_ms}` | wall clock |

Events: `Submitted, Tick (3 s foreground / worker in background), AppResumed,
HomeFocused, Abort`.

## `manage_tokens` — `MtokWire.kt` ↔ `manage_tokens.rs` (`MtokOperation` :128, `MtokShellResult` :153)

| Operation | Answer | Android answers with |
| --- | --- | --- |
| `multicall_erc20_meta {chain_id, address}` | `chain_meta_resolved {…, meta?}` | one Multicall3 `aggregate3` through the pool (`Abi.kt` has the encoders) |
| `read_custom_tokens` | `custom_tokens_loaded {tokens}` | `vela.customTokens` |
| `write_custom_token {token}` | `saved` / `save_failed` | replace-by-id |
| `remove_custom_token {id}` | `removed {id}` / `remove_failed {id}` | — |
| `invalidate_token_cache` | `cache_invalidated` | `BalanceDashboard` refresh |

## The fixtures cdylib (debug only) — `uniffi.vela_dev_fixtures`

| Function | Returns |
| --- | --- |
| `fixtureAccounts()` | `[ {index, credentialIdHex, publicKeyHex, address} ]` |
| `fixtureAssert(challenge, allowCredentialIds, preferred?)` | `{credentialIdHex, signatureDerHex, authenticatorDataHex, clientDataJsonHex}` — the onboarding `Assertion` shape |
| `fixtureRegistration(index)` | `{credentialIdHex, attestationObjectHex, clientDataJsonHex}` |

Constants come from the core (`RP_ID = getvela.app`, `ORIGIN`, platform
attachment, `internal` transport).

## The UserOp assembly (main `.so`, from `vela-core::user_op`)

| Function | Purpose |
| --- | --- |
| `userOpDraft(chainId, keys, account, calls, feeMode, quotes, nonce, deployed, gas)` | the unsigned operation + its SafeOp hash (the challenge) |
| `userOpFinalize(draft, assertion, keys, credentialId)` | the packed signature; JSON for the relay |
| `classifyRelayRejection(message)` | `SendSubmitFailure` |

Exact signatures are settled in phase 2 with the desktop's `user_op.rs` open;
the contract is that no Kotlin computes a hash, a leg, or a signature.
