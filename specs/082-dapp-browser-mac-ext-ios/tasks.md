# Tasks: 082 — The dApp browser holds up on the Mac, in Chrome and on the iPhone, and 079's leftovers close

**Input**: [spec.md](spec.md) (US1–US9, FR-001–FR-020, findings G1–G33, rulings 1–10),
[plan.md](plan.md) (workstreams A–H + I18N, phases 0–7, gates), [research.md](research.md)
(RA1–RA12, RB1–RB15, RC1–RC8, RD1–RD15, RE1–RE14, RF1–RF6, RG1–RG15, RH1–RH7, RI1–RI3, R0, RX),
[data-model.md](data-model.md), [contracts/core-rules.md](contracts/core-rules.md),
[quickstart.md](quickstart.md).

**Tests**: included. Every rule that moves into the core gets a core test; every client mapping
gets a unit test in that client's suite; device checks are the quickstart rows. Each task names
the test that proves it.

**Paths**: `C/` = `rust/crates/vela-core/src/app/`, `CT/` = `rust/crates/vela-core/tests/`,
`L/` = `rust/crates/vela-core/i18n/locales/<locale>/`,
`D/` = `app-desktop/vela-wallet/src/`,
`W/` = `app-web/vela-wallet/src/lib/`, `WR/` = `app-web/vela-wallet/src/routes/`,
`X/` = `app-web/vela-wallet/extension/`, `E2E/` = `app-web/vela-wallet/e2e/`,
`I/` = `app-ios/VelaWallet/VelaWallet/`, `IT/` = `app-ios/VelaWallet/VelaWalletTests/`,
`IU/` = `app-ios/VelaWallet/VelaWalletUITests/`,
`A/` = `app-android/vela-wallet/app/src/main/java/app/getvela/wallet/`,
`AT/` = `app-android/vela-wallet/app/src/test/java/app/getvela/wallet/`,
`TS/` = `app-web/trusted-signer/`, `EV/` = `specs/082-dapp-browser-mac-ext-ios/evidence/`.
Line numbers are the research's anchors on 094677f0; concurrent sessions edit this tree, so
re-grep the function name before editing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: the task can start at its phase's entry at the same time as every other [P] task of
  that phase. Its files are disjoint from theirs, and it has no unfinished dependency. A task
  without [P] follows the task(s) named in its "(after …)" note. Within one group the "after"
  chain is the order; different groups never share a file.
- **[US#]**: only client-phase and device tasks carry one (except the device pre-flight T143 and
  the optional Android smoke T169, which serve every story). US1 bad network, loads, one answer ·
  US2 connect/sign look and words · US3 dApp tx in Activity · US4 honest risk (L-D5, L-HOST,
  G14 plain send) · US5 address spelling · US6 empty History · US7 fees, slow confirmations
  and money-safety tracking · US8 logs · US9 079 evidence rows.
- Every serde addition is `#[serde(default)]` and gets the crate's serde and ts-rs derives. A
  new enum variant is **not** additive for the hand-written Swift and Kotlin wires, so every
  client's wire-mirror task ships in the same PR as the core change.
- Each task's first line names its file(s) and the test that proves it; the indented lines under
  it are detail. T180–T184 were added after the first numbering with the next free ids; each sits
  in its phase where it runs (T180 after T015, T181 after T073, T182 after T100, T183 after T122,
  T184 after T136).

---

## Phase 1: Setup

- [x] T001 Create worktree `/Volumes/data/production/vela-wallet-082` on branch `082-dapp-browser-mac-ext-ios` off main de93634f
- [x] T002 G3 fix: the Explore address bar's key handler no longer re-enters `WalletPage`'s update through `address_change`, so ⌘V, Backspace, Delete and ⌘X in the bar no longer abort the app (`D/wallet/page.rs`). Done in 35ac2b07 and re-run on the device (paste, Backspace, Enter: no crash).
- [x] T003 G29 fix: `D/webview.rs` asks WebKit for a nil `URL` before it calls `wry::WebView::url()`, so a new tab's first failed page no longer aborts inside a draw. Done in 35ac2b07; re-run showed the panel at 5 s and retries at +3/+6/+9 s.
- [x] T004 Fault injection touches only the app under test (ruling 6). Done in bea04b57: `VELA_DEV_PROXY` on the desktop (`D/executor/proxy.rs`, dev-fixtures builds) and on iOS (`I/Core/DevProxy.swift`, Debug builds), plus the chaos `mute` mode in `scripts/device/chaos-proxy.py`.
- [x] T005 Spec with findings, rulings and evidence in `specs/082-dapp-browser-mac-ext-ios/spec.md` and `EV/`: 05f55d70
- [x] T006 Plan, research, data model, core contract and quickstart in `specs/082-dapp-browser-mac-ext-ios/{plan,research,data-model,quickstart}.md` and `contracts/core-rules.md`: 094677f0
- [x] T007 [P] Record rulings 8–10 and the Phase 2 additions in `specs/082-dapp-browser-mac-ext-ios/contracts/core-rules.md`, `data-model.md`, `plan.md` and `device-pass-plan.md`, so the design docs match what Phase 2 builds (docs only; proof: the sections are reviewed against T019, T020, T024, T026, T180 and T044–T045):
  - `specs/082-dapp-browser-mac-ext-ios/contracts/core-rules.md` §3: `TrackOperation::FindOpEvent{chain_id, entry_point, user_op_hash, from_block, to_block}`, `TrackShellResult::OpEvent{logs_json, error_json, head_block}` (the pool's raw answer; the core judges a range error, T180), `submit_block` on `Submitted`/`TrackPendingRecord`/entry, and the `FIND_OP_*` constants.
  - §2: `is_log_range_error` and the `eth_getLogs` routing (T180).
  - §4: `dapp_receipt_wait_ms(elapsed_ms)`; `submit_block` on `OpSubmitted`, `SignTrackerHandoff` and `SignRecord` (T020). §5: `submit_block` on `SendShellResult::Submitted` and `SendOperation::TrackSubmitted` (T026). §1: `NOT_SENT_DAPP_DETAIL`.
  - §12: the added exports `dappReceiptWaitMs`, `userOpNotSentDetail`.
  - `data-model.md` §2: the find-event transitions and `submit_block`; §1: that the shells persist `maybe_sent` / `submit_block` with the record (T181–T184).
  - `plan.md` "Out of scope → Deferred" still lists "A relay-independent landing check (Q1)" and the High risk "Residual double pay" says it needs Q1: both are stale since ruling 8 (answered Q1) — strike the deferral and say the risk is closed by T019.
  - `device-pass-plan.md` §1 step 4 and §2's "Switch the Mac to chaos" / DX3 / IX3–IX5: one line each, "superseded by quickstart.md (ruling 6, RH1)".
- [x] T008 [P] `scripts/device/chaos-proxy.py` (RH1, RH2); proof: `python3 -m py_compile scripts/device/chaos-proxy.py`.
  - Header (required): the Android `adb shell settings put global http_proxy` lines and the iPhone "Settings > Wi-Fi > Configure Proxy" lines change every app's traffic, which ruling 6 forbids; RH1 marks them superseded. Replace them with the per-app switches (iPhone Debug build `VELA_DEV_PROXY`, extension Chrome for Testing `--proxy-server`, Android: no fault rows, RH3) and a pointer to quickstart.md §0. Proof: `rg -n 'http_proxy|Configure Proxy' scripts/device/chaos-proxy.py` finds only the "never" note.
  - Optional `stall` mode (W24): CONNECT is answered `200` and the upstream is never opened; list it in the header's mode line. Proof: one `curl -x` against the mode hangs after the 200.

**Checkpoint**: Setup done. Only docs and test infra changed.

---

## Phase 2: Foundational — the core (blocks every client phase)

Entry task, then five groups whose files are disjoint. They run in parallel: **A-core**, **C-core**,
**EF-core**, **G-core** and **I18N**. The [P] tasks below start once T009 is done (I18N's T043
does not touch `C/mod.rs` and may start with T009). After the groups
come the exports and the bindings, a sub-phase of its own whose entry is the end of every group;
it must be sequential because the binding files are shared.

- [x] T009 Register the three new pure modules in `C/mod.rs` so that no group touches `C/mod.rs` afterwards: create `C/sim_outcome.rs`, `C/net_health.rs` and `C/remote_mark.rs` as stubs holding only a module doc comment, and add their `pub mod` lines to `C/mod.rs`. Proof: `cd rust && cargo check -p vela-core --features crux`.

### Group A-core — money safety, lifecycle hole, chain notice, optional methods

Owns `rust/crates/vela-core/src/user_op.rs`, `C/rpc_pool.rs`, `C/tx_tracker.rs`,
`C/sign_request.rs`, `C/send.rs`, `CT/user_op_hash.rs` (new), `CT/user_op_submit.rs` (new),
`CT/fixtures/userop-hash-gnosis.json` (new), `CT/app_rpc_pool.rs`, `CT/app_tx_tracker.rs`,
`CT/app_sign_request.rs` and `CT/app_send.rs`.

The four file chains run in parallel:
- `user_op` (T010 → T011, T012)
- `rpc_pool` (T013 → T014 → T015 → T180)
- `tx_tracker` (T016 → T017 → T018 → T019; T019 also after T010 and T180)
- `sign_request` (T020 → T021 → T022 → T023 → T024 → T025)

`send` (T026) follows T017. All `rpc_pool` edits, including G's RG7 and ruling 8's `eth_getLogs`
routing (T180, next free id, placed after T015), live in this group because `rpc_pool.rs` is this
group's file.

- [x] T010 [P] `rust/crates/vela-core/src/user_op.rs`: add `user_op_hash(op, chain_id) -> Result<String, CoreError>` (RA6).
  - EntryPoint v0.7 `getUserOpHash`: `keccak(abi.encode(keccak(abi.encode(sender, nonce, keccak(initCode), keccak(callData), accountGasLimits, preVerificationGas, gasFees, keccak(packedPaymasterAndData))), ENTRY_POINT, chainId))`.
  - `packedPaymasterAndData` mirrors `user_op_to_json` (`:629-655`): empty, or `paymaster ‖ u128 0 ‖ u128 0 ‖ data`. The signature is excluded; the result is 0x-lowercase.
  - Add the `USER_OPERATION_EVENT_TOPIC` constant (keccak of `UserOperationEvent(bytes32,address,address,uint256,bool,uint256,uint256)`) here, so T019 only reads this file.
  - Proof: in-file `mod tests` (empty paymaster, paymaster packing, the signature excluded, a nonce/gas change moves the hash, the topic constant equals its keccak).
- [x] T011 Chain vectors for `user_op_hash` in new `CT/user_op_hash.rs` and `CT/fixtures/userop-hash-gnosis.json` (RA6, the phase-0 gate); proof: `cargo test -p vela-core --test user_op_hash`; (after T010).
  - Make one read-only Gnosis fetch per tx (`eth_getTransactionByHash`, `eth_getTransactionReceipt`): tx `0xc6f3544fc4e3ac769e92c92ab4804cd3f38ffb8d607ba07b5103a59710094dc4` (block 48478729, nonce 22, `EV/extension/w1-lost-reply-landed.txt`), and the desktop's G21 op (block 48479132, nonce 41, `0xa6180e26…`: find it by the fixture Safe's `UserOperationEvent` in that block).
  - Store the `handleOps` input and the event topics in `CT/fixtures/userop-hash-gnosis.json`.
  - Proof: new `CT/user_op_hash.rs` decodes `handleOps` into a `UserOperation` and asserts `user_op_hash(op, 100) == topics[1]` for both txs.
- [x] T012 `rust/crates/vela-core/src/user_op.rs`: add `SubmitReply`, `SubmitVerdict {Accepted, MaybeSent, NotSent{rejection}}`, `SubmitStep`, `SUBMIT_MAX_RETRIES = 3`, `SUBMIT_RETRY_DELAY_MS = 3000` and `submit_step(reply, attempt, maybe_delivered, local_hash)`, with the six ordered rules of RA1; (after T010).
  - The `[existingHash:0x…]` marker is read from the raw error JSON first, then from `relay_error_message`.
  - Add `NOT_SENT_DAPP_DETAIL`, the fixed dApp text for "relay unreachable; nothing was sent" (RA10). The clients log `userop.hash_mismatch` when an Accepted hash ≠ the local one.
  - Proof: new `CT/user_op_submit.rs` has one test per rule:
    - result → Accepted with the relay's hash;
    - marker → Accepted with that hash;
    - "currently processing" at attempt 2 → RetryAfter 3000, and at attempt 3 → not a retry;
    - an error while `maybe_delivered` → MaybeSent with the local hash;
    - AA25 on attempt 2 after a lost reply on attempt 1 → MaybeSent (RA1 rationale);
    - an error while `!maybe_delivered` → NotSent{Some(rejection)};
    - exhaustion while `!maybe_delivered` → NotSent{None}.
- [x] T013 [P] `C/rpc_pool.rs`: add `RpcTransportOutcome::NotConnected` (routed like `Network`) and `may_have_delivered(outcome)` (true for Timeout, Network, NonJson and HTTP 5xx; false for NotConnected, any JSON answer and HTTP 4xx) (RA1).
  - `RpcCallVerdict::Respond{url, maybe_delivered}` and `Failed{rate_limited, maybe_delivered}` carry a sticky OR over every POST of the call.
  - Proof: `CT/app_rpc_pool.rs`: every outcome's flag; the OR over two POSTs (Timeout then NotConnected → true); NotConnected fails over like Network; old JSON without the fields still decodes.
- [x] T014 `C/rpc_pool.rs`: the view set `unreached_chains` (RF1); (after T013).
  - A call whose first pass saw every endpoint of the chain fail on transport, with no rate-limit signal, adds the chain at "Pass swept clean" (`:1622`).
  - Any usable answer removes it (`clear_chain_failure`, `:1841-1846`); `failed_chains` keeps its meaning.
  - Expose `cooldown_ms(n)` (30 s · 2^(n−1), cap 300 s) and keep `RPC_READ_TIMEOUT_MS = 8000` public for RF2.
  - Proof: `CT/app_rpc_pool.rs`:
    - a 3-endpoint chain black-holed → in `unreached_chains` after one pass, and not yet in `failed_chains`;
    - a rate-limited pass → not added;
    - the next good answer clears it;
    - the `cooldown_ms` values for n = 1..6.
- [x] T015 `C/rpc_pool.rs`: `OPTIONAL_METHODS = ["eth_simulateV1"]` and `is_optional_method` (RG7); (after T014).
  - A JSON error to an optional method → `Route::NotServed` → `Respond{url}` with `clear_chain_failure`: no score change, no ban, and `conclude_failed` never classifies the chain.
  - A rate-limit signal still fails over.
  - Proof: `CT/app_rpc_pool.rs`: Arbitrum `-32603 "method handler crashed"` on `eth_simulateV1` → answered, and the chain is in neither `failed_chains` nor `unreached_chains`; plan words on it do not ban the endpoint for `eth_call`; 429 on it fails over.
- [x] T180 `C/rpc_pool.rs`: a range limit on `eth_getLogs` is an answer, not a chain fault (ruling 8's find-event read, RG7's pattern); proof: `CT/app_rpc_pool.rs`; (after T015).
  - Why: T019 halves its window on a range error, so the error must reach the tracker. Today it never does: a message with "exceeded" matches `is_permanent_rpc_error` (`rpc_pool.rs:600-622`) and bans the endpoint for every method, and `-32005` matches `is_transient_server_error` (`:627-641`), fails over, and after the last endpoint puts the chain in `failed_chains` — a false Gnosis notice and home banner while an op may have been sent.
  - Add the pure `is_log_range_error(error) -> bool`: a JSON error whose message names a block range or a result cap (for example `-32005 "query returned more than 10000 results"`, `"exceed maximum block range: 50000"`, `"Log response size exceeded … block range"`, `-32602 "block range is too large"`).
  - For `eth_getLogs` it is checked before the ban and fail-over checks: the call concludes `Respond{url}` carrying the error, with no ban, no score change and no chain classification. A rate-limit signal (`"rate limit exceeded"`, 429) still fails over. Other methods keep today's routing.
  - Proof: `CT/app_rpc_pool.rs`: each message above on `eth_getLogs` → answered, the endpoint not banned, the chain in neither `failed_chains` nor `unreached_chains`; `"rate limit exceeded"` on `eth_getLogs` still fails over; the same range text on `eth_call` is routed as today.
- [x] T016 [P] `C/tx_tracker.rs`: `USER_OP_STATUS_METHOD = "pimlico_getUserOperationStatus"`, `TrackStatusAnswer{status, stage, tx_hash}` and `parse_user_op_status(json)` (an unknown status string → None) (RA7).
  - `PollStatus` uses the constant; `TrackShellResult::Status` gains `tx_hash` (the 079 D2 explorer link).
  - Proof: `CT/app_tx_tracker.rs`: the live probe's `{"status":"not_found","transactionHash":null}` parses; `included` with a hash parses; `"pending?"` → None; no string `eth_getUserOperationStatus` remains in `tx_tracker.rs`.
- [x] T017 `C/tx_tracker.rs`: MaybeSent and NotSent ends (RA4); (after T016).
  - `Submitted`, `TrackPendingRecord` and the entry gain `maybe_sent`; the entry also gains `acknowledged` and `not_found_streak`.
  - `TrackOutcome::MaybeSent` holds while `maybe_sent ∧ ¬acknowledged ∧ ¬terminal ∧ ¬abandoned` (Unknown still wins at 24 h). Status polls continue past the 120 s window for such entries at `receipt_interval_ms`.
  - `NOT_FOUND_GRACE_MS = 60_000` and `NOT_FOUND_CONFIRMATIONS = 2` → the new terminal `EntryStatus`/`TrackStatus::NotSent`, with records patched `failed` through `fail_records`. `StatusUnavailable` changes nothing.
  - Proof: `CT/app_tx_tracker.rs`:
    - `not_found` at 30 s is ignored;
    - two at ≥ 60 s → NotSent; one, then `pending`, then `not_found` → streak reset;
    - a receipt wins over `not_found`;
    - a plain (non-maybe-sent) entry's `not_found` stays inert, as in 079;
    - 24 h with no answer → Unknown and records untouched ("time alone never produces a failure");
    - a reload restores `maybe_sent` from `TrackPendingRecord`.
- [x] T018 `C/tx_tracker.rs`: `TrackOperation::HoldingsMoved{chain_id}`, answered `Notified` (RE8); (after T017).
  - Emitted after `UpdateTxRecords` + `NotifyConfirmed` on a confirmed receipt, and alone after the fail patch on a failed receipt that carries a tx hash.
  - Never emitted on pending, unreachable, age or `NotSent`.
  - Proof: `CT/app_tx_tracker.rs`, one case each.
- [x] T019 `C/tx_tracker.rs`: the relay-independent landing check (ruling 8, plan Q1); proof: `CT/app_tx_tracker.rs`; (after T018, T010 and T180).
  - `submit_block: Option<u64>` on `Submitted`, `TrackPendingRecord` and the entry.
  - For an entry with `maybe_sent ∧ ¬acknowledged ∧ ¬terminal ∧ ¬abandoned`, emit `TrackOperation::FindOpEvent{chain_id, entry_point, user_op_hash, from_block, to_block}` on the status-poll cadence. The shell runs `eth_getLogs{address: entry_point, topics: [USER_OPERATION_EVENT_TOPIC, user_op_hash], fromBlock, toBlock}` through the pool and answers `TrackShellResult::OpEvent{logs_json, error_json, head_block}` with the pool's answer as it came (a result, or the JSON error the pool now returns for a range limit, T180).
  - Ranges are bounded (`FIND_OP_MAX_RANGE`), step forward from `submit_block` and are clipped to the head. The core judges the error with `rpc_pool::is_log_range_error` (no shell decides it, FR-020): a range error halves the window, down to one block; any other error retries the same range on the next tick. With `submit_block` unknown, the first op asks for the head only (`from_block: None`), and the scan starts at `head − FIND_OP_LOOKBACK_BLOCKS`.
  - The core parses the log: `success` → `Confirmed` with the log's `transactionHash` (records patched confirmed, then `NotifyConfirmed` and `HoldingsMoved`). Failure → `Dropped` (Reverted) with the tx hash (records failed, then `HoldingsMoved`).
  - A found event wins over any later relay `not_found`.
  - Proof: `CT/app_tx_tracker.rs`:
    - found success / found failure;
    - an empty range steps forward; a range error halves it; another JSON error keeps the range;
    - caught up with the head → wait for the next tick;
    - no FindOpEvent for a plain or acknowledged entry;
    - `submit_block` survives a reload.
- [x] T020 [P] `C/sign_request.rs`: `maybe_sent` and `submit_block` on `Event::OpSubmitted`, `SignTrackerHandoff` and `SignRecord`, plus `SignView.pending_op_maybe_sent` (RA3).
  - `on_op_submitted` (`:1940-1990`) copies both into the persisted pending record and the handoff.
  - Proof: `CT/app_sign_request.rs`:
    - `OpSubmitted{maybe_sent: true}` → a pending record under the local hash and a handoff with the flag;
    - the rid settles Submitted (never signs twice);
    - a swipe after commit is a dismiss, not a 4001;
    - old JSON without the fields decodes.
- [x] T021 `C/sign_request.rs`: `SignPhase {Idle, Preparing, AwaitingSignature, Submitting}` (RA9); (after T020).
  - `Event::CeremonyStarted{id}` and `CeremonyDone{id}` are id-guarded and accepted only while that inflight is in `Submitting`. `Inflight.ceremony` is `NotYet | Up | Done`, and `SignView.phase` is derived per data-model §3.
  - `is_signing` / `is_submitting` are kept until every shell reads `phase`.
  - Proof: `CT/app_sign_request.rs`: each row of the data-model phase table; a stale id is dropped; Precheck/Sponsoring → Preparing (never AwaitingSignature during the network wait, G22).
- [x] T022 `C/sign_request.rs`: `SignEnding`, `ending_of(method, payload, submitted_user_op)` (Landed also carries `user_op_hash`), `SignEndingState` and `ending_state(ending, track)` (RA8 1–2); (after T021 and T017).
  - The mapping: tracker Confirmed → Confirmed; Dropped → Reverted; NotSent/Rejected → NotSent; else `Following{outcome, fee_held}`; no entry → `Following(Landing)`.
  - Proof: `CT/app_sign_request.rs`: every row of the data-model ending table, including `Following{MaybeSent}`.
- [x] T023 `C/sign_request.rs`: `on_submit` Succeeded with a record (`:2143-2162`) answers the page only and no longer emits `UpdateRecord Confirmed` (`:2152-2156`), so the tracker alone closes on-chain records (RA8 3, W3); (after T022).
  - A receipt that reverted inside the window is still answered its tx hash (ruling 9).
  - Proof: update the `CT/app_sign_request.rs` cases that expected the Confirmed patch, and add a test: a reverted receipt → an `Ok(tx hash)` answer and no record patch from `sign_request`.
- [x] T024 `C/sign_request.rs`: `DAPP_TX_ANSWER_WINDOW_MS = 120_000` measured from `ApproveTapped`, and the pure `dapp_receipt_wait_ms(elapsed_ms) = max(10_000, window − elapsed)` (RA12); (after T023).
  - Proof: `CT/app_sign_request.rs` for 0 s, 46 s, 115 s and 200 s elapsed.
- [x] T025 `C/sign_request.rs`: `Event::TransportDropped` also stops an inflight of that transport in Precheck, Sponsoring or ReactiveSponsoring (inflight cleared, attempt bumped, effect aborted); past the commitment point nothing changes (RB2); (after T024).
  - Add `SignSubmitOutcome::AskerGone` (serde `{"type":"asker_gone"}`): nothing sent, no answer, no record, and the sheet clears if it still shows that request.
  - `EXTENSION_REQUEST_TTL_MS` is unchanged and public for export.
  - Proof: `CT/app_sign_request.rs`:
    - drop during precheck → the later `on_precheck` result does not sign (the hole at `:2014-2033`);
    - drop after the passkey → the pipeline continues;
    - AskerGone → no Respond and no PersistRecord.
- [x] T026 `C/send.rs`: `SendShellResult::Submitted{maybe_sent, submit_block}`, `SendOperation::TrackSubmitted{maybe_sent, submit_block}`, `SendTxRecord.maybe_sent`, `SendReceiptStatus::{MaybeSent, NotSent}` and `SendReceiptOutcome::Failed{rejected, not_sent}` (RA4, RA10); (after T017).
  - MaybeSent never fires the success haptic. `TrackStatus::NotSent` → `Failed{rejected: false, not_sent: true}`, never the fee-rejected words.
  - Proof: `CT/app_send.rs`:
    - MaybeSent → receipt status MaybeSent and no haptic op;
    - tracker NotSent → `not_sent`;
    - tracker Confirmed after MaybeSent → Confirmed;
    - old JSON decodes.

### Group C-core — a dApp's plain value transfer (G14)

Owns `C/clear_signing.rs` and `CT/app_clear_signing.rs`.

- [x] T027 [P] `C/clear_signing.rs`: `ClearSurface::PlainSend`, `ClearPlainSend{to, value_wei, amount, no_value}`, `ClearSigningView.plain_send` (Some iff PlainSend), `is_empty_calldata` (None, "", "0x", "0X", trimmed) and `plain_send_of` (RC1, RC2).
  - `start_tx` sets `Model.plain_send` for empty calldata whatever the recipient (no `eth_getCode`), and `surface_of` (`:1497-1519`) returns PlainSend when it is set.
  - `result` stays None; plain_send is reset wherever `result` is reset.
  - Proof: `CT/app_clear_signing.rs` rewrites the two tests that pin the bug (`:337-353`, `:2236`). Empty calldata to a contract address → PlainSend (not BlindTransaction); a non-address `to` → BlindTransaction.
- [x] T028 `C/clear_signing.rs`: the value and amount rules (RC3, RC4, RC5, RC7); (after T027).
  - Value: absent, null, "" and "0x" = 0; "0x"+hex = the exact U256; decimal text, "null", signs, non-hex or overflow → no plain send (BlindTransaction).
  - Amount: exact ÷10^18 with trailing zeros trimmed, using the `ClearLocale` separators and grouping (`:521-529, 5402-5434`).
  - `no_value` → "0" with no minus. `confirm_of`: TxPlain with `Some(p) ∧ ¬p.no_value` → `ConfirmIntent{"send", IntentSend}`, else Confirm. The first leg of `wallet_sendCalls` follows the same rule (RC7).
  - Proof: `CT/app_clear_signing.rs`:
    - `0x38d7ea4c68000` → "0.001";
    - `0x0`, absent and "0x" → `no_value` + neutral Confirm;
    - "1000" (decimal) and the text of a JSON number → BlindTransaction;
    - 2^256 → BlindTransaction;
    - de/fr separators;
    - a `wallet_sendCalls` whose first leg has empty data → PlainSend.

### Group EF-core — page loads, address bar, network health, logo misses, balance read plan

Owns `C/browser_load.rs`, `C/net_health.rs`, `C/remote_mark.rs`, `C/balance_dashboard.rs`,
`CT/app_browser_load.rs`, `CT/app_net_health.rs` (new), `CT/app_remote_mark.rs` (new) and
`CT/app_balance_dashboard.rs`. The desktop copy of `LoadWatch` is deleted later, in the desktop
phase (T058).

- [x] T029 [P] Move `LoadWatch` into `C/browser_load.rs` (RD4, RX).
  - Take the pure part of `app-desktop/vela-wallet/src/explore/load_watch.rs:34-287`: `LoadWatch`, `Asked`, `Probed`, `RetryAction`, `requested / committed / finished / watchdog / probed / give_up / schedule_retry / retry_fired / take_due / retry / busy`, and `host_of`.
  - Use ms constants `WATCHDOG_MS = 3000`, `PROBE_BUDGET_MS = 5000`, `GIVE_UP_MS = 20000`, `ENGINE_LIVE_PROGRESS = 0.15`. There is no FFI export for `LoadWatch`.
  - Proof: port `load_watch.rs:360-652` (all but the two ureq tests) into `CT/app_browser_load.rs`; they pass unchanged except for the ms units.
- [x] T030 `C/browser_load.rs`: the RD3/RD7 extensions (after T029).
  - `EngineSample{loading, progress, url}`, `LoadWatch::engine(sample) -> EngineVerdict {Nothing, PageStarted, StoppedWithoutCommit}` and `Asked::Page` for a load the wallet did not start.
  - New fields `engine_loading`, `engine_live` and `page_initiated`.
  - Retries and probes: `Probed::Deferred` for a TLS verdict while the engine loads; `RetryAction::{EngineStillLoading, NotInFront}`; `schedule_retry` returns None when `page_initiated`; give-up at 20 s only if progress never rose above `ENGINE_LIVE_PROGRESS`.
  - Proof: `CT/app_browser_load.rs`:
    - a retry due while the engine still loads the same URL → EngineStillLoading (W7);
    - the engine stopped with no commit → StoppedWithoutCommit;
    - an unasked load → PageStarted with manual retry only (W18);
    - progress 0.4 at 20 s → no give-up (DX5).
- [x] T031 `C/browser_load.rs`: `should_give_up(elapsed_ms, committed, progress)`, `stalled()` (class Timeout, `explore.loadOffline`, auto-retry) and `retry_when_network_returns(class)` (true for Offline, Timeout, Refused, Other, Proxy) (RE2, RE3); (after T030).
  - Proof: `CT/app_browser_load.rs`:
    - 19 s → false; 20 s, no commit, progress 0.1 → true;
    - committed → false; progress 0.2 → false;
    - Certificate and NotFound → no network-back retry.
- [x] T032 `C/browser_load.rs`: `LoadFailureClass::Proxy` with `reason_key = "explore.loadProxy"` on the Offline retry schedule, and `probe_code::PROXY = 6` (RD9, RE4, RX); (after T031).
  - Apple `kCFErrorDomainCFNetwork` 306–310 → Proxy and 311 → Refused; Android -5 → Proxy.
  - Apple `NSURLErrorDomain -1000` → Offline (it was NotFound); -1002, -1003 and -1006 stay NotFound.
  - Proof: `CT/app_browser_load.rs`: every row above, plus the existing -1000 test updated. The doc comment states that a proxy which answered (502, or a CONNECT closed with no reply) speaks for the host.
- [x] T033 `C/browser_load.rs`: `BarLock`, `AddressBar{url, host, lock}`, `address_bar(shown, pending, failed)`, `SiteLabel{name, host_line}` and `site_label(title, host)` (RE1, RE7); (after T032).
  - `address_bar`: a failure is up → the failed host, no lock; else the committed host, `Closed` / `Open` from `!is_insecure_public_origin`; else a pending load in an empty tab → the pending host, no lock; else empty. A non-default port is kept.
  - `site_label`: an empty title, or one equal to the host ignoring ASCII case → `{host, None}`.
  - Proof: `CT/app_browser_load.rs`:
    - the G28 spoof shape: committed jumper + pending uniswap → jumper;
    - failed → no lock;
    - loopback and private http → Closed; public http → Open;
    - `127.0.0.1:8137` keeps its port;
    - site_label: "127.0.0.1:8137" == host → one line; "Uniswap" → two lines; case folding.
- [x] T034 [P] `C/net_health.rs`: `MISSES_BEFORE_OFFLINE = 3`, `NetHealth{misses, online}`, `NetEdge {WentOffline, CameBack}` and `net_health_step`, moved from `A/core/net/NetHealth.kt:18-47` (RE3).
  - Proof: new `CT/app_net_health.rs` ports every case of `AT/NetHealthTest.kt`: the 3rd miss → WentOffline; the first reach after offline → CameBack; no edge while online.
- [x] T035 [P] `C/remote_mark.rs`: `MarkMiss`, `mark_miss_of_status(u16)` and `mark_miss_ttl_ms(miss)` (RE10).
  - NotFound (404/410), Refused (401/403) and NotAnImage → None (session). Throttled, ServerError (5xx/408), Transport and Unknown → 60 000.
  - Proof: new `CT/app_remote_mark.rs` covers every status class.
- [x] T036 [P] `C/balance_dashboard.rs`: `ReadKind`, `ReadSlot` and `read_plan(chain_id, stables, wrapped_native, custom)` (RE9).
  - Order: native (unless the chain has none, e.g. Tempo 4217), registry stablecoins (`peg_usd = 1.0`, decimals read on chain), the wrapped native unless it is the native, then custom tokens.
  - Deduplicate by lower-case contract; custom metadata wins.
  - Proof: `CT/app_balance_dashboard.rs`:
    - Base lists USDC (the G24 vector);
    - Tempo has no native slot;
    - a custom USDC entry wins over the registry one;
    - order is stable.

### Group G-core — 079 leftovers (Activity, simulation, address spelling)

Owns `C/activity_feed.rs`, `C/sim_outcome.rs`, `C/dapp_permissions.rs`, `C/dapp_browser.rs`,
`rust/crates/vela-core/provider/inpage.js`, `CT/app_activity_feed.rs`, `CT/app_sim_outcome.rs`
(new), `CT/app_dapp_permissions.rs` and `CT/app_dapp_browser.rs`.

- [x] T037 [P] `C/activity_feed.rs`: dApp transactions become rows (RG1, RG2, RG4).
  - `FeedTxRecord.dapp_origin`. `FeedItem` gains `kind: FeedTxKind {Send, Receive, DappTx}` (a folded batch is Send), `status: FeedTxStatus` (a batch uses its first line's) and `site` (DappTx only, `host[:port]`).
  - `accept()` (`:633-640`) keeps DappTx with `from == me`; `build_items()` gets a `dapp_item()` arm (`:811-832`). The value comes from `fee_policy::from_base_units` of hex or decimal wei, and 0 means no amount.
  - Message signatures and connects never become rows.
  - Proof: `CT/app_activity_feed.rs` rewrites `:265-298`, which pinned the exclusion:
    - a pending DappTx row with its site;
    - confirmed and failed statuses;
    - a MaybeSent record under a local hash → a Pending DappTx row (RG4);
    - `personal_sign` → no row;
    - a batch's status is its first line's.
- [x] T038 `C/activity_feed.rs`: `FeedView.history_empty_key` (`history.emptyTitle` when `chain_filter` is None, `history.emptyFilter` when Some) and `home_empty_key` (`home.emptyNoActivity` / `home.emptyNoActivityNetwork`) (RG5, RX); (after T037).
  - Proof: `CT/app_activity_feed.rs`, both keys for both filter states.
- [x] T039 [P] `C/sim_outcome.rs`: `SimReply`, `SimOutcome {Deltas, Reverts{reason}, NotOffered, Unreachable}`, `SimNotice`, `classify(reply, user)`, `notice(outcome)` and `revert_reason(call)` (RG6, RG8).
  - `revert_reason` decodes only `Error(string)` 0x08c379a0, strips what `name_verify::is_never_in_a_name` rejects, and caps at 64 characters.
  - Move `derive_deltas` and its helpers from `app-desktop/vela-wallet/src/executor/sim.rs:98-226` (the desktop copy is deleted in T057).
  - `notice`: Reverts → Danger `simWillFailReason` / `simWillFail`; NotOffered or Unreachable → Caution `simUnavailableWarning`; Deltas → none.
  - Proof: new `CT/app_sim_outcome.rs`:
    - the vectors of `IT/SimDeltasTests.swift` and `AT/SimDeltasTest.kt` ported;
    - Arbitrum `-32603 "method handler crashed"` → NotOffered, Caution;
    - a call with status 0x0 → Reverts, Danger;
    - a custom error → reason None;
    - bidi/control characters stripped and 64-char cap;
    - an empty array result → NotOffered.
- [x] T040 [P] `C/dapp_permissions.rs`: `dapp_spelling(addr)` = `primitives::checksum_address` (input unchanged if unparseable), applied in `popup_approved` (`:478-521`) and `popup_account_switch` (`:526-568`); `resolve_granted` is unchanged (RG10).
  - Proof: `CT/app_dapp_permissions.rs`: a lower-case grant is written EIP-55; an account switch answers EIP-55; `resolve_granted` still matches case-insensitively.
- [x] T041 `C/dapp_browser.rs`: `AccountSwitched.active_address` (`:493-496`) and `sites_listed` (`:785-789`) use `dapp_spelling` (RG10); (after T040).
  - Proof: `CT/app_dapp_browser.rs`: the account-changed event carries the same EIP-55 spelling as connect (L-D6), and loaded lower-case sites are normalised.
- [x] T042 [P] `rust/crates/vela-core/provider/inpage.js`: `applyAccounts` (`:162-170, 337, 411`) keeps the wallet's spelling as sent, and change detection compares case-insensitively (RG10).
  - Proof: the provider case in `W/dapp/core-table.test.ts` (T094) and `IT/ProviderScriptTests.swift` (T121). Every client embeds this file (`dapp_rpc::PROVIDER_JS`, `X/build.mjs`) and picks it up on rebuild.

### Group I18N — one task, one commit

Owns `L/*.json` in 15 locales, `scripts/gen-i18n.mjs` and the generated
`rust/crates/vela-core/src/i18n/paths.rs`, `rust/crates/vela-core/src/i18n_catalogs/` and
`assets/i18n/`.

- [x] T043 [P] One commit for the whole corpus change: `L/componentsUi.json`, `L/explore.json`, `L/send.json` in 15 locales, the pins in `scripts/gen-i18n.mjs` and the generated `paths.rs` / `i18n_catalogs/` / `assets/i18n/` (RI1–RI3, RA11, RG9, RG14, RX); proof: `CT/i18n_residency.rs` prints ≤ 138,800. Adding `maybeSent` alone would break the cap.
  - Keys:
    - add `componentsUi.signing.maybeSent` (L/componentsUi.json);
    - add `explore.loadProxy` and `explore.requestOpen` (L/explore.json);
    - reword `componentsUi.signing.simUnavailableWarning`;
    - delete `send.txErrorTimeout` (L/send.json).
  - Do this in all 15 locales, using the RI2 drafts (zh-HK in written Cantonese; write the other 12 locales of `loadProxy` / `requestOpen` with the same meaning).
  - Move the pins in `scripts/gen-i18n.mjs:452-454` from 1782 paths / 1693 leaves to 1784 / 1695 (89 branches), with a history comment in the `:436-451` style.
  - The six i18n steps (memory "i18n corpus gates"): (1) `npm --prefix scripts run gen:i18n`, (2) `lint:i18n`, (3) `verify:i18n`, (4) `dump:vectors`, (5) the leaf-count pin above, (6) `build:wasm` + `sync:wasm`. Steps 1–5 run here and their generated files are committed with the corpus; step 6 writes `rust/pkg-web`, which belongs to BINDINGS, so it runs in T046 (the fingerprint moves there).
  - Proof:
    - `cd rust && cargo test -p vela-core --features i18n-all,crux --test i18n_residency -- --nocapture` prints ≤ 138,800 (≈ 138,729);
    - `git diff --exit-code` is clean on `paths.rs`, `i18n_catalogs` and `assets/i18n` after a second generate;
    - `node scripts/check-ios-android-copy-parity.mjs`;
    - `grep -r txErrorTimeout` finds nothing outside history comments.

### Exports and bindings (sub-phase; entry = T010–T043 and T180 done; shared binding files, so sequential after T044 ∥ T045)

- [ ] T044 [P] UniFFI exports in `rust/crates/vela-core-uniffi/src/lib.rs` (contract §12, plus T007's additions); (after T010–T043 and T180).
  - New functions:
    - `userOpHash(draft, chainId)`, `userOpSubmitStep(reply, attempt, maybeDelivered, localHash)`, `userOpNotSentDetail()`, `userOpStatusMethod()`, `parseUserOpStatus(json)`;
    - `signEndingOf(method, payloadJson, submittedUserOp?)`, `signEndingState(endingJson, trackEntryJson?)`, `dappReceiptWaitMs(elapsedMs)`;
    - `simOutcome(user, replyJson)` → `SimOutcomeRecord`;
    - `browserLoadGiveUpMs()`, `browserLoadShouldGiveUp(elapsedMs, committed, progress)`, `browserLoadStalled()`, `browserLoadRetryWhenNetworkReturns(class)`, `browserAddressBar(shown?, pending?, failed?)`, `browserSiteLabel(title, host)`;
    - `netHealthStep(misses, online, reached)`, `markMissTtlMs(kind, status?)`, `balanceReadPlan(chainId, stablesJson, wrappedNative?, customJson)`.
  - `browserLoadClassify` gains the class `"proxy"`.
  - Proof: `cargo test -p vela-core-uniffi`, and the smoke scripts in T046.
- [ ] T045 [P] wasm exports in `rust/crates/vela-core-wasm/src/lib.rs`; (after T010–T043 and T180).
  - New functions:
    - `userOpHash(opJson, chainId)` (AttestOp shape), `userOpSubmitStep(replyJson, attempt, maybeDelivered, localHash)`, `userOpNotSentDetail()`, `userOpStatusMethod()`, `parseUserOpStatus(json)`;
    - `signEndingState(endingJson, entryJson)`, `dappReceiptWaitMs(elapsedMs)`, `signRequestTtlMs()`;
    - `rpcReadTimeoutMs()`, `rpcCooldownMs(n)`;
    - `browserSiteLabel(title, host)`, `markMissTtlMs(kind, status?)`, `balanceReadPlan(...)`.
  - Proof: `cargo test -p vela-core-wasm`, and `verify:wasm` in T046.
- [ ] T046 Regenerate every binding from this tree: `W/core/generated/`, `rust/pkg-web/`, the VelaCoreKit xcframework, `rust/bindings/kotlin` and the Android `.so`; proof: `bash rust/scripts/check-ios-core-fresh.sh` prints ok; (after T043, T044, T045).
  - Run: `npm --prefix scripts run gen:core-types` (writes `W/core/generated/`; touch `rust/crates/vela-core/src/bin/generate_wallet_state_bindings.rs` only if a new root type is needed) · `npm --prefix scripts run build:wasm && npm --prefix scripts run verify:wasm` (`rust/pkg-web`, the fingerprint moves) · `cd app-web/vela-wallet && pnpm sync:wasm` · `bash rust/scripts/build-ios-xcframework.sh` · `bash rust/scripts/build-android.sh` (the `.so`) · `bash rust/scripts/smoke-kotlin.sh` (regenerates `rust/bindings/kotlin`) · `bash rust/scripts/build-dev-fixtures.sh --host`.
  - Proof: `bash rust/scripts/check-ios-core-fresh.sh` prints ok · `bash rust/scripts/smoke-swift.sh` · `bash rust/scripts/smoke-kotlin.sh` pass.
- [ ] T047 Run the phase-0/1 gates from `specs/082-dapp-browser-mac-ext-ios/plan.md` and record the output in `EV/gates/phase-0-1.txt`; (after T046).
  - `cd rust && cargo test -p vela-core --features i18n-all,crux` (residency printed)
  - `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings`
  - `cargo fmt --all --check`
  - The i18n four, then `git diff --exit-code` on `paths.rs`, `i18n_catalogs`, `assets/i18n`
  - `node scripts/check-event-payloads.mjs` · `node scripts/check-ios-android-copy-parity.mjs`
  - The two chain vectors (T011)
  - `npm --prefix scripts run gen:core-types -- --check`
  - Expected red here, recorded and not a blocker: `pnpm check` until T075 lands, and `check-event-payloads.mjs` for the fields the shells do not send yet (`maybe_sent`, `submit_block`, `CeremonyStarted/Done`) until the client phases land. Each client gate (T101, T123, T137) re-runs `check-event-payloads.mjs` and must leave it green.

**Checkpoint**: the core suite is green; the bindings match this tree (`check-ios-core-fresh.sh`
ok, the wasm fingerprint moved). The desktop phase may start right after T010–T043 and T180, because
it uses the crate directly. The other client phases start after T047.

---

## Phase 3: Desktop — group DESK (owns `app-desktop/vela-wallet/**`)

**Goal**: D, plus the desktop halves of A, B (the abort), C, E, F and G.
**Independent test**: quickstart §2.
Chains:
- T048 (`diag`) and T049 (compile) start together; every later desktop task logs through
  `vlog!`, so everything from T050 on also comes after T048 (T067 shares `D/main.rs` with it);
- money: T050 → T051 → T054 → T056 → T057 → T070; T050 → T052 → T067; T050 → T055; T053 → T071; {T051, T055} → T072; {T051, T053, T055} → T181;
- `page.rs`: T058 → T059 → T060 → T062 → T063 → T064 → T065 → T066 (also after T051) → T068 → T069 (also after T050) → T073 (also after T072); T058 → T061 → T067;
- T074 (gates) last.

- [x] T048 [P] [US8] New `D/diag.rs` (RD12), registered in `D/main.rs`.
  - `vlog!(area, …)` prints `[vela-wallet] HH:MM:SS.mmm area: …`, using libc `localtime_r`.
  - Redaction helpers: `host_of(url)` (host[:port] only; RPC URLs carry API keys, `D/executor/pool.rs:733-735`; the Trusted Signer fragment carries the request) and `short(hash)`.
  - Each later desktop task adds its own contract §15 lines through it.
  - Proof: in-crate tests: `host_of` strips path, query, fragment and userinfo; `/v3/<key>` never survives; the time format.
- [x] T049 [P] [US1] Update `D/executor/*`, `D/signing/*`, `D/wallet/*` and `D/gallery.rs` for the new core types so the crate compiles against the Phase 2 core (the desktop's wire mirror); proof: `cargo test` in `app-desktop/vela-wallet`.
  - Exhaustive matches for:
    - `TrackStatus::NotSent`, `TrackOutcome::MaybeSent`, `TrackOperation::{HoldingsMoved, FindOpEvent}`;
    - `SignSubmitOutcome::AskerGone`, `ClearSurface::PlainSend`, `SendReceiptStatus::{MaybeSent, NotSent}`;
    - `LoadFailureClass::Proxy`, `RpcTransportOutcome::NotConnected`;
    - the new `FeedItem` / `SignView` fields.
  - Files: `D/executor/{tracker,sign_request,send,pool,clear_signing,activity_feed}.rs`, `D/signing/*`, `D/wallet/*`, and the fixture/gallery constructors (`D/signing/fixtures.rs`, `D/wallet/fixtures.rs`, `D/gallery.rs`). This is the desktop's wire mirror.
  - Proof: `cd app-desktop/vela-wallet && cargo test && cargo test --features dev-fixtures` compile and pass.
- [x] T050 [US1] Submit via the core `submit_step` in `D/executor/relay.rs`, `D/executor/user_op.rs` and `D/executor/pool.rs` (RA1, RA5, RA10, ruling 8); proof: in-crate fake-relay tests; (after T048 and T049).
  - Replace the busy loop in `D/executor/relay.rs:595-616`, and the "relay unreachable, try again" mapping in `D/executor/user_op.rs:726-729`.
  - Compute `user_op_hash` before the first POST, and read `eth_blockNumber` once for `submit_block` (best effort).
  - Map ureq errors to `NotConnected` in `D/executor/pool.rs` (HostNotFound, ConnectionFailed, Timeout(Resolve|Connect), Io ConnectionRefused / AddrNotAvailable / HostUnreachable / NetworkUnreachable, TLS handshake, proxy CONNECT failure) and OR `maybe_delivered`.
  - Call `chain::bump_nonce` (`D/executor/chain.rs:127`) only on Accepted. NotSent → -32603 with `NOT_SENT_DAPP_DETAIL`, never "All bundler endpoints failed".
  - Log `relay: submit verdict=<accepted|maybe_sent|not_sent> hash=<12> attempts=n`, and `userop.hash_mismatch` when the hashes differ.
  - Proof: in-crate tests:
    - a test pinning the exact ureq 3.4 CONNECT-failure variant → NotConnected;
    - a fake relay: mute → MaybeSent with the local hash and the nonce cache untouched;
    - refused → NotSent;
    - `[existingHash:]` → Accepted.
- [x] T051 [US1] One answer after a lost reply, and phase events, in `D/executor/sign_request.rs` (RA2, RA9, RA12, RB2, ruling 9); proof: in-crate answer tests; (after T050).
  - MaybeSent → `OpSubmitted{maybe_sent: true, submit_block}` and the same receipt wait.
  - The wait comes from `dapp_receipt_wait_ms`, replacing `RECEIPT_BUDGET` 90 s (`D/executor/sign_request.rs:156, 506`); one Ok answer (tx hash in the window, else the op hash).
  - A receipt that reverted in the window → its tx hash.
  - Dispatch `CeremonyStarted` / `CeremonyDone` around the Touch ID and Trusted Signer ceremonies.
  - Drop the result of an attempt the core aborted after `TransportDropped`.
  - Proof: in-crate tests: a mute relay → exactly one `Ok(op hash)`; a reverted receipt → `Ok(tx hash)`; the ceremony events are ordered around the passkey call; an aborted attempt never reaches the relay.
- [x] T052 [US7] Relay status in `D/executor/relay.rs` (RA7, G13); proof: `live_an_unknown_hash_is_pending_not_unreachable`; (after T050).
  - `USER_OP_STATUS_METHOD` + `parse_user_op_status` replace `D/executor/relay.rs:727-759`; delete the local parser.
  - `TrackShellResult::Status` carries `tx_hash`; log `tracker: op=<short> status=…`.
  - Proof: the live test `live_an_unknown_hash_is_pending_not_unreachable` (`relay.rs:1260-1262`) now fails on None, and passes with `-- --ignored` against the relay (`not_found`, not -32601).
- [x] T053 [US7] `D/executor/tracker.rs` runs `FindOpEvent` (ruling 8) and `HoldingsMoved` (RE8); proof: in-crate fake-pool tests; (after T049).
  - FindOpEvent: `eth_getLogs` on the EntryPoint (and `eth_blockNumber` for the head) through the pool, answered as `OpEvent{logs_json, error_json, head_block}` with the pool's answer as it came; the core judges a range error (T180).
  - HoldingsMoved → `balance_dashboard::invalidate()` (`D/executor/balance_dashboard.rs`), then `Notified`.
  - Proof: in-crate tests with a fake pool: found success, found failure, a range error, a head-only query; HoldingsMoved invalidates once.
- [x] T054 [US2] Signing sheet words and endings in `D/signing/status.rs`, `D/signing/live.rs` and `D/signing/mod.rs` (RA8, RA9, RA10, G22); proof: `live.rs` mapping tests and a resolve test; (after T051).
  - `D/signing/status.rs:91-118, 353-374` derives through `ending_of` / `ending_state`; delete the local derivation (one of the three copies).
  - `D/signing/live.rs`: stage words come from `SignView.phase`:
    - Preparing → `send.txPreparing`;
    - AwaitingSignature → `send.txSigning` or `componentsUi.signing.signing`;
    - Submitting → `send.txSubmitting` + `send.txBackgroundHint`.
  - Endings:
    - MaybeSent → title `send.txSubmitting`, caption `componentsUi.signing.maybeSent`, the op hash, `send.txCloseBackground`, no Retry;
    - NotSent → `statusFailed` + `txErrorGeneric`;
    - Reverted → `statusFailed` + `failedHint` + explorer.
  - Add the `maybeSent` reader to `SigningStrings` (`D/signing/mod.rs`).
  - Proof:
    - a resolve-without-echo test (the `signing/mod.rs:560-590` pattern);
    - a `live.rs` mapping test for every `SignEndingState`;
    - "Waiting for biometric" never shows while `phase == Preparing`.
- [x] T055 [US7] The wallet's own Send in `D/executor/send.rs` and `D/wallet/money.rs` (RA4, RA10); proof: in-crate receipt mapping tests; (after T050).
  - `D/executor/send.rs` submits through the same `submit_step` path and reports `Submitted{maybe_sent, submit_block}`.
  - `D/wallet/money.rs` draws `SendReceiptStatus::MaybeSent` (the maybeSent caption, no success haptic) and `NotSent` via `Failed{not_sent}`, never "fees stayed above…" (`money.rs:935`).
  - Proof: in-crate receipt mapping tests.
- [x] T056 [US4] Plain send in `D/signing/live.rs` and `D/wallet/signing_host.rs` (RC1, RC5, RC6, G14); proof: `live.rs` tests; (after T054).
  - `D/signing/live.rs:231, 1044-1067` draws `ClearSurface::PlainSend`: intent send, an amount card with `network_admin::builtin_native_symbol`, the recipient party, and 确认发送, or a neutral 确认 when `no_value`. The blind rung no longer shows for empty calldata.
  - `D/wallet/signing_host.rs:1025-1029` passes a present non-string `value` as text.
  - Proof: `live.rs` tests (the `:1621-1630` pattern):
    - `0x38d7ea4c68000` → −0.001 xDAI to 0x7687…D141 with no "Contract interaction";
    - `0x0` → 0 with no minus;
    - a JSON number → the contract card with no amount.
- [ ] T057 [US4] Simulation severity in `D/executor/sim.rs` and `D/signing/live.rs` (RG6, L-D5); proof: in-crate sim tests; (after T056).
  - `D/executor/sim.rs` normalises the reply into `SimReply` and calls `sim_outcome::classify`. Delete `derive_deltas` and its helpers (`sim.rs:98-226`) and the collapse at `:88-94`.
  - `D/signing/live.rs:264-268` draws `notice`: Caution `simUnavailableWarning`; Danger `simWillFail(Reason)` with the sanitised reason.
  - Proof: in-crate tests: an Arbitrum -32603 → caution tone; status 0x0 → danger with the reason; no copy of `derive_deltas` is left (`grep -n derive_deltas D/` finds only the core call).
- [ ] T058 [US1] Take `LoadWatch` out of the desktop: delete `D/explore/load_watch.rs`, new `D/explore/probe.rs`, repoint `D/explore/mod.rs`, `D/wallet/page.rs` and `D/wallet/browser_host.rs` (RD4, RD9); proof: `cargo test` incl. the ureq tests in `probe.rs`; (after T049).
  - Delete the moved pure part of `D/explore/load_watch.rs`. Rename what stays (`probe()`, `probe_code_of()`, `io_code()` and the two ureq tests) to `D/explore/probe.rs`.
  - Point `D/explore/mod.rs`, `D/wallet/page.rs` and `D/wallet/browser_host.rs` at `vela_core::app::browser_load::LoadWatch`, and take the timings from `WATCHDOG_MS` / `PROBE_BUDGET_MS` / `GIVE_UP_MS`.
  - The probe maps `ProxyFailure` Unreachable / Timeout / PacFailed → `probe_code::PROXY`; RefusedTunnel keeps refused/connect.
  - `ExploreStrings` (`D/explore/mod.rs`) reads `explore.loadProxy`.
  - Proof: `cargo test` (the ureq tests in `probe.rs`, plus a resolve test for `loadProxy`); `load_watch.rs` no longer exists.
- [ ] T059 [US1] WebKit's own load state first, in `D/webview.rs` and `D/wallet/browser_host.rs` (RD3, RD7, W7, W18); proof: fake engine-sample tests; (after T058).
  - `D/webview.rs` `engine()` reads `isLoading` / `estimatedProgress` / `URL` (the `view_url` pointer technique, `:335-353`) and wry's `can_go_back/forward`.
  - Poll every 250 ms while a load is watched or a panel is up, and every 500 ms otherwise while Explore is in front; feed `LoadWatch::engine`.
  - `retry_fired` → EngineStillLoading does not navigate: log `browser: retry … skipped (engine still loading)`.
  - PageStarted → hairline, watchdog and panel with the failed host, manual Retry only. The Windows build keeps probe-only.
  - Log lines: asked / committed / finished / watchdog / probe verdict / failed class / engine stopped without commit.
  - Proof: in-crate tests over a fake engine sample stream: one `loadRequest` for a 9 s first byte; a page-initiated load gets a panel with no auto-retry.
- [ ] T060 [US1] Hold tab switches while a request is open, in `D/wallet/browser_host.rs`, `D/wallet/page.rs`, `D/explore/components.rs` and `D/explore/mod.rs` (RD1, rulings 4 and 10, W14); proof: in-crate hold tests; (after T059).
  - `holds_navigation(view) = consent ∨ signing ∨ queued_signing > 0` in `D/wallet/browser_host.rs`.
  - One funnel, `browser_go(Go)`, in `D/wallet/page.rs` for: other tab, +, start-page tab, Enter, favourites, Recents, Reload, Open in a new tab, Back/Forward.
  - A held control is drawn at the disabled opacity (`D/explore/components.rs:357-360`) but still takes clicks. A click brings the request's column forward (re-attaching `signing_background`), shows `explore.requestOpen` in the bar's notice slot (`components.rs:459-462`) for 2.5 s, and logs `browser: navigation held`. The typed draft is kept.
  - Re-clicking the shown tab is a no-op (`page.rs:12220-12226`); closing a background tab no longer reloads (`:12248-12262`).
  - `ExploreStrings` reads `requestOpen`.
  - Proof: in-crate tests: every held path returns Held; closing the request's own tab is not held (4900 from the core); a resolve test for `requestOpen`.
- [ ] T061 [US1] The wallet's HTTP follows the system proxy with no direct fall-back: new `D/executor/proxy_macos.rs` (registered in `D/executor/mod.rs`), `D/executor/proxy.rs`, `app-desktop/vela-wallet/Cargo.toml` (RD2, RD9, ruling 3, W6, W8); proof: settings-seam tests; (after T058).
  - New `D/executor/proxy_macos.rs`: a CFNetwork `extern "C"` block over `core-foundation` 0.10 / `-sys` 0.8 (add them to `app-desktop/vela-wallet/Cargo.toml`; both are already in Cargo.lock).
    - `CFNetworkCopySystemProxySettings` (5 s cache) and `CFNetworkCopyProxiesForURL`.
    - PAC through `CFNetworkExecuteProxyAutoConfigurationURL/Script` on a `vela-pac` thread with its own run loop: 5 s cap, results cached 5 min, failures 30 s.
  - `D/executor/proxy.rs` gets `routes_for(url) -> Vec<Route {Direct, HttpConnect, Socks5h}>`.
    - Delete `Candidate`, `Candidates`, `current`, `advance`, `REDERIVE_AFTER` (`:137-247, 309-333`) and the scutil subprocess (`:512-523`).
    - No environment proxies on macOS; `VELA_DEV_PROXY` still wins in dev-fixtures builds. Loopback is direct.
    - Move to the next route only when the TCP connect to the proxy itself failed.
    - A failure carries `ProxyFailure{proxy, kind}`, logged `proxy: <host:port> unreachable|refused tunnel|timed out` / `PAC … failed`.
    - Linux and Windows follow the same no-fallback rule; their missing PAC support is logged once.
  - Proof: in-crate tests through a settings seam:
    - a connect failure moves on;
    - a timeout after connect, TLS, CONNECT 5xx and a CONNECT closed with no reply do not move on;
    - a PAC failure is a proxy failure (no DIRECT);
    - exceptions and simple hostnames;
    - no state survives between two requests.
- [ ] T062 [US1] The address bar after Enter, in `D/wallet/page.rs` and `D/explore/components.rs` (RD5, RE1, G7, G30); proof: in-crate bar tests; (after T060).
  - The bar's `on_click` ignores `event.is_keyboard()`. `address_key` returns `Go | Stop | NotMine`, and Go (when not held) or Stop calls `window.blur()`.
  - `edit_address` (`D/wallet/page.rs:12980-13002`) starts from the bar's address.
  - Host and lock come from core `address_bar`: no fixture host `app.uniswap.org` fallback (`page.rs:12510-12513`), and no lock over a failure panel.
  - While a failure panel is up, the tab title is the failed host.
  - Proof: in-crate tests: a keyboard click does not re-open edit mode; a failed first load shows the failed host with no lock (G30).
- [ ] T063 [US1] The restored tab and the nav buttons, in `D/wallet/page.rs`, `D/webview.rs` and `D/explore/components.rs` (RD6, G2); proof: in-crate launch tests; (after T062).
  - `shown_tab: Option<String>`: a restored tab waits unlit, and the strip lights only `shown_tab` or a selected start-page tab. A meta-handler visit (`page.rs:13122-13138`) goes to `shown_tab`, else the selected start-page tab, else a new tab.
  - The nav buttons take `enabled: [bool; 3]` from `webview::engine()`, all off on the start page. Back/Forward use wry's native `go_back/go_forward` (`D/webview.rs:268-278`, `D/explore/components.rs:668-672`).
  - Log `browser: navigate asked host=… view=<built|pending> composing=<b>`.
  - Proof: in-crate tests: launch with a restored tab → none lit, all nav disabled; Enter over the start page opens a new tab and leaves the restored one intact.
- [ ] T064 [US2] Stop the chrome jump, in `D/wallet/page.rs` and `D/explore/components.rs` (RD8, G6); proof: a toolbar-y layout test; (after T063).
  - The live canvas uses `.flex_1().min_h(px(0.)).w_full()` (`D/wallet/page.rs:12717-12731`); `tab_strip_with` and `toolbar` get `.flex_none()` (`D/explore/components.rs:230-236, 417-425`).
  - Proof: a layout test comparing the toolbar's y on the start page and on a live page (0 pt); then row DX12.
- [ ] T065 [US2] The connect consent shows the account and the network, in `D/wallet/page.rs` (RD11, G11); proof: a consent-view test; (after T064).
  - `consent_body` (`D/wallet/page.rs:14000-14072`) adds an account row and a changeable network row (`open_site_networks → SiteChainPicked`), both extracted from `connection_body` (`:14127-14285`).
  - The connected panel shows the grant's account (`tab.connected_address`).
  - Proof: an in-crate test that the consent view carries the address and the chain; then row U1/U2.
- [ ] T066 [US1] The Trusted Signer page could not open, in `D/executor/trusted_signer.rs` and `D/signing/trusted_signer.rs` (RD13, W16); proof: HEAD-probe tests; (after T065 and T051).
  - `D/executor/trusted_signer.rs` + `D/signing/trusted_signer.rs`: while a wait runs and the window becomes active with no answer, HEAD scheme+host+path (fragment stripped) over T061's routes within 5 s.
  - A transport failure → `mark_unreachable()` → the card shows `componentsUi.signing.signerDown` with `connect.browser.retry` as the primary action and Cancel. The request and its 5-minute clock stay.
  - Log `trusted signer: page <host> unreachable`.
  - Proof: in-crate tests: an unreachable HEAD marks it down; a cached page (HEAD ok) is untouched; the fragment is never sent.
- [ ] T067 [US1] Closing the window during a submit, in `D/executor/relay.rs` and `D/main.rs` (RD14, W17); proof: guard-counter tests; (after T048, T052 and T061).
  - `SUBMITS_IN_FLIGHT: AtomicU32` with a drop guard around the relay POST (`D/executor/relay.rs:578-615`).
  - `on_window_should_close` and the Quit action (`D/main.rs:401`) refuse once while a submit is in flight: activate, bring the submitting column forward, log `window: close held (submit in flight)`. A second close within 5 s goes through.
  - Proof: in-crate tests for the guard counter and for the second close.
- [ ] T068 [US1] The chain notice, in `D/wallet/browser_host.rs` and `D/wallet/page.rs` (RF1, RF4, G33, W11); proof: `the_chain_notice_is_for_a_chain_that_is_down_not_throttled` extended; (after T066).
  - `D/wallet/browser_host.rs` shows it for `chain ∈ failed_chains ∪ unreached_chains ∧ chain ∉ rate_limited_chains` (the pool view from `D/executor/pool.rs`).
  - Retry (`browser_host.rs:367-382`, `D/wallet/page.rs:12817`) is busy (spinner, full colour, taps ignored) until its `eth_blockNumber` settles.
  - Log `chain notice: shown/cleared/retry → ok|failed`.
  - Proof: extend `the_chain_notice_is_for_a_chain_that_is_down_not_throttled` with an unreached-only chain; test the busy state.
- [ ] T069 [US1] Network-back parity, in `D/executor/pool.rs` and `D/wallet/browser_host.rs` (RE3, RE10, FR-020); proof: three misses then a reach → one retry; (after T068 and T050).
  - Feed `net_health_step` from pool outcomes in `D/executor/pool.rs`. On `CameBack`, retry the failed shown tab when `retry_when_network_returns(class)`, and invalidate balances.
  - If the desktop's logo loading (`D/marks.rs`, `D/executor/gpui_http.rs`) remembers misses, use `mark_miss_ttl_ms`; otherwise record "no such surface" for W20 in the results matrix.
  - Proof: in-crate tests: three misses then a reach → one retry of the failed tab.
- [ ] T070 [US4] `site_label` in `D/signing/components.rs` and `D/explore/live.rs` (RE7, F14, G8 parity); proof: the spoof test in `D/explore/live.rs`; (after T057).
  - Replace the F14 copy in `D/signing/components.rs:130`, and draw Recents (`D/explore/live.rs`) with `site_label`.
  - Proof: the existing spoof test (`D/explore/live.rs:253-262`) still holds; a Recents row whose title is its host shows it once.
- [ ] T071 [US1] Balance read plan in `D/executor/balances.rs` (RE9); proof: Base's slots include USDC; (after T053).
  - `D/executor/balances.rs:338-400, 595` reads the slots from `balance_dashboard::read_plan`; delete the local list.
  - Proof: an in-crate test that Base's slots include USDC and the order is unchanged.
- [ ] T072 [US3] dApp transactions in Activity, in `D/flows/live.rs`, `D/wallet/live.rs` and `D/executor/activity_feed.rs` (RG1–RG3); proof: in-crate feed tests; (after T051 and T055).
  - Delete `kind_of` and the status lookup (`D/flows/live.rs:404`, `D/wallet/live.rs:1935-1950`). Draw rows from `FeedItem.kind/status/site`: title `history.txLabelDappTx`, subtitle site → recipient → chain, and `statusPending` / `statusFailed` + " · " for a row that is not confirmed.
  - The detail sheet shows "Requested by" (`componentsUi.signing.siweOrigin`). `D/executor/activity_feed.rs` maps the stored `dappOrigin`.
  - After the background persist of a `PersistRecord` / `UpdateRecord`, hand `ReconcileCompleted{resolved_count: 1}` over on the next tick (the `D/executor/tracker.rs:247-256` pattern), never from the tracker handoff.
  - Proof: in-crate tests: a pending dApp row with its site; a failed row; the poke fires once per write.
- [ ] T073 [US6] Empty Activity and History, in `D/wallet/page.rs` and `D/flows/live.rs` (RD10, RG5, G1, L-D7); proof: three states × two filters; (after T068 and T072).
  - Home Activity: rows; skeletons while `BalanceView.balance_unknown`; else `empty_state(Inbox, home.emptyNoActivity, home.emptySubtitle)`. Under a sidebar chain filter, the core's `home_empty_key` (no caption) (`D/wallet/page.rs`).
  - History: `history_empty_key`. Delete the desktop's own branch (`D/flows/live.rs:219-222`) and update the tests at `:4945-4946`.
  - Proof: in-crate tests for the three states and both filters.
- [x] T181 [US7] A may-have-been-sent op survives a relaunch on the desktop, in `D/executor/sign_request.rs` (`persist_record`), `D/executor/send.rs` (`PersistTxRecords`) and `D/executor/tracker.rs` (`pending_records`) (RA3, RA4, rulings 1 and 8, US3 AS2); proof: an in-crate `pending_records` round-trip test; (after T051, T053 and T055).
  - Why: each shell maps a stored row into `TrackPendingRecord` by hand with only id, hash, chain and time (`tracker.rs:82-110`). After a relaunch the entry would lose `maybe_sent` (no MaybeSent outcome, no NotSent end, no FindOpEvent) and `submit_block`, so ruling 1's "track it to its end" would stop at the first restart.
  - The stored row gains `maybeSent` and `submitBlock`, written from `SignRecord` / `SendTxRecord`. `pending_records` puts them into `TrackPendingRecord`, and the handoff into `tx_tracker` `Event::Submitted` (from `SignTrackerHandoff` and `SendOperation::TrackSubmitted`; grep `Event::Submitted` under `D/`) carries both.
  - Proof: a row written for a MaybeSent op reads back as `TrackPendingRecord{maybe_sent: true, submit_block: Some(n)}`; an old row without the keys reads back as `false` / `None`.
- [ ] T074 [US1] Desktop gates in `app-desktop/vela-wallet` (tests, dev-fixtures tests, clippy, the live relay test, the release build); (after T048–T073 and T181).
  - `cd app-desktop/vela-wallet && cargo test && cargo test --features dev-fixtures && cargo clippy --all-targets --features dev-fixtures -- -D warnings`
  - live: `cargo test live_an_unknown_hash_is_pending_not_unreachable -- --ignored`
  - `cargo build --release --features dev-fixtures` (for quickstart §1.3)

**Checkpoint**: desktop suites green; a dev-fixtures build is ready for quickstart §2.

---

## Phase 4: Extension + web — group WEB (owns `app-web/vela-wallet/**`: `src/lib/**`, `src/routes/**`, `extension/**`, `e2e/**`)

**Goal**: B, F (RF2, RF3), and the web halves of A, C, E and G; the G16 token gate.
**Independent test**: quickstart §3.
Order inside the group: wires (T075), then B's lifecycle (T076–T081), then A's may-have-been-sent
answer (T082–T083) and RF3 (T084). "Exactly one answer" holds only if the answer reaches the
live page. T075 and T076 start together; T077 follows T076 (both pin in
`W/signing/one-surface.test.ts`); T182 (persisting the may-have-been-sent flag) follows T083,
T086, T088 and T092; T100 follows T099 (it uses the fixed helper); T101 (gates) is last.

- [ ] T075 [P] [US1] Web wire mirrors over the regenerated `W/core/generated/`:
  - `W/signing/core/sign-types.ts`: `SignPhase`, `phase`, `pending_op_maybe_sent`, `CeremonyStarted/Done`, `asker_gone`, `maybe_sent`, `submit_block`.
  - `W/wallet/core/tracker-types.ts`: `NotSent`, `MaybeSent`, `HoldingsMoved`, `FindOpEvent` / `OpEvent`, Status `tx_hash`.
  - `W/wallet/core/rpc-pool-types.ts`: `NotConnected`, `maybe_delivered`, `unreached_chains`.
  - `W/signing/core/clear-types.ts`: `PlainSend`.
  - `W/wallet/core/feed-types.ts`: `kind`, `status`, `site`, `dapp_origin`, empty keys.
  - `W/flows/core/send-types.ts`: MaybeSent, NotSent, `not_sent`, `maybe_sent`.
  - Proof: `cd app-web/vela-wallet && pnpm check` green.
- [ ] T076 [P] [US1] Pure lifecycle rules in new `X/lib/request-life.js` and `X/lib/protocol.js` (RB1–RB11, RB6, RB2); proof: new `W/dapp/request-life.test.ts`, `W/dapp/instant.test.ts`, `W/signing/one-surface.test.ts`.
  - New `X/lib/request-life.js`: `nextForWindow`, `claimVerdict`, `recoveryPlan`, `affectedBy`, `surfaceAfterOpen`, `settlement`.
  - `X/lib/protocol.js`:
    - add `DOC_PORT = "vela.doc"`, `SURFACE_PORT = "vela.surface"`, `REQUEST_TTL_MS = 300000`, `CONTENT_GRACE_MS`, `CLAIM_TIMEOUT_MS`, `REQUEST_PREFIX`;
    - add `SETTLE.{page_left, surface_closed, expired, restarted, updated}`, all 4900, with the plain English of RB6;
    - add `droppedChannelAnswer(bucket, method, attempt)`, which never echoes Chrome's text;
    - remove `requestCurrent` and `nextForPanel`.
  - Proof:
    - `W/dapp/instant.test.ts` pins `REQUEST_TTL_MS` to wasm `signRequestTtlMs()`, and every `SETTLE.*.code` to `popupCloseSettlement().code` (never 4001);
    - new `W/dapp/request-life.test.ts` covers every claim verdict, recovery branch and settle cause of data-model §4;
    - `W/signing/one-surface.test.ts` pins the port names and `REQUEST_PREFIX`.
- [ ] T077 [US8] Worker logs in new `X/lib/swlog.js` (RB14, W23); proof: new `W/dapp/swlog.test.ts` and the `SW_COUNTS_KEY` pin in `W/signing/one-surface.test.ts`; (after T076, which also edits `one-surface.test.ts`).
  - New `X/lib/swlog.js`: `[vela-sw] <iso> <event> k=v…` console lines, a 200-line ring in storage.session `vela.sw.log`, and counters in `vela.sw.counts`.
  - The fixed events are those of contract §15. Never params, results, signatures, addresses or URL paths; hosts only.
  - Proof: new `W/dapp/swlog.test.ts`: the ring cap; an address or URL path passed in is dropped; `SW_COUNTS_KEY` pinned in `one-surface.test.ts`.
- [ ] T078 [US1] The worker owns each request's life, in `X/background.js` (RB1, RB3, RB4, RB5, RB7, RB8, RB10, RB11; G17, G18, G19, G23, EX6, EX7); (after T076 and T077).
  - Ledger and page link:
    - records live in storage.session under `vela.req.<tabId>:<pageRequestId>`; at first start, remove the leftover `vela.req.*` from storage.local (delete the start sweep, `:289-298`);
    - the record keeps `sender.documentId`; answers go by `tabs.sendMessage(tabId, msg, {documentId})`;
    - `vela.doc` port close → settle `page_left`, with `tabs.onRemoved` / `onReplaced` as the backstop;
    - `recover()` at start.
  - Surfaces:
    - one queue per window (`nextForWindow`), with no active-tab filter (`:235-243`);
    - `sidePanel.open` is called synchronously and `surface: 'panel'` is recorded at once, with the 2 s fallback (removes the race at `:197-202`);
    - a `vela.surface` port with a 20 s ping; a port close while the worker lives → `surface_closed`.
  - Claims and deadlines: handle claims (RB5) and push `claimed`; `abandon` → `expired`.
  - Log every step through swlog.
  - Proof: new `W/dapp/background.test.ts` with a fake `chrome` namespace:
    - a reload settles the old record (`page_left`) and shows only the new one;
    - a second tab's request queues in the same window;
    - a restart with a live page resumes (no settle);
    - a panel port close settles `surface_closed`;
    - no `vela.req.*` left in storage.local.
- [ ] T079 [US1] `X/content.js` (RB3, RB4, RB6, RB11); (after T076).
  - The `vela.doc` port is open only while the page owes a sign/connect answer. On a port close, reconnect with backoff 0/250/1000/3000 ms.
  - The answer handler replies `{ok:true}` synchronously; `alive` → the ids the document owns; `claimed` extends that id's deadline.
  - Deadlines: 5 min + 5 s unclaimed, 5 min after a claim → `abandon`.
  - A dropped channel answers through `droppedChannelAnswer`, never Chrome's `error.message` (`:62-63`). Reads are retried once, then a plain `-32603 "Vela could not finish this read — try again"`; a dropped `eth_sendRawTransaction` / `eth_sendUserOperation` answers 4900 `restarted`; the channel dropping twice on a sign/connect answers `restarted`; no reconnect at all (extension reloaded or updated) answers `updated` (data-model §4).
  - Proof: new `W/dapp/content.test.ts` with a fake runtime:
    - the "message channel closed" text never reaches the page (G19);
    - one answer per id;
    - the deadline order (worker 5 min < page 5 min + 5 s);
    - each of the four dropped-channel answers above.
- [ ] T080 [US1] The panel side of the lifecycle: new `W/dapp/panel-surface.svelte.ts`, `W/dapp/transport.ts`, `W/dapp/DappRequestHost.svelte`, `WR/+layout.svelte`, `WR/[locale]/wallet/+page.svelte` (RB4, RB7, RB9, RB10, RB15); proof: new `W/dapp/panel-surface.test.ts`; (after T075 and T076).
  - New `W/dapp/panel-surface.svelte.ts`: `start()`, `current`, `onWithdrawn(cb)`, `claim()`, `answer()`, `isPanelDocument()` (`?panel` or sessionStorage `vela.surface.panel`). Start it from `WR/+layout.svelte`; it switches to the wallet screen when a request is owed.
  - `W/dapp/transport.ts`: stop ignoring `delivered:false` (`:170-184`).
  - `W/dapp/DappRequestHost.svelte`: delete `tabId ??=` (`:130`). The pagehide settle stays only as a backstop. `withdrawn` closes the card with no words.
  - `WR/[locale]/wallet/+page.svelte:238-241` keeps panel identity via the flag.
  - Proof:
    - new `W/dapp/panel-surface.test.ts`: identity survives Wallet → Settings → Wallet (G23c); `withdrawn` clears `current`;
    - a vitest that the host serves a second tab's request.
- [ ] T081 [US1] Claims before signing, in `W/signing/core/sign-types.ts`, `W/services/dapp-submit.ts`, `W/signing/core/sign-executor.ts` and `W/dapp/core/dperm-connect.ts` (RB2, RB5); proof: `W/signing/core/sign-executor.test.ts`; (after T080).
  - Ports: `SignResponder.claim?(id, phase)` and `SignShellPorts.askerLive(id, phase)` (true when the transport has no claim) in `W/signing/core/sign-types.ts`.
  - `W/services/dapp-submit.ts`: `handleDAppRequest(…, beforeSubmit?)`, whose signFn wrappers throw `AskerGoneError`. `W/signing/core/sign-executor.ts` maps it to `asker_gone`.
  - Claim points: approve, before a connect grant (`W/dapp/core/dperm-connect.ts`); sign, before the passkey; submit, after the passkey and before the relay POST.
  - A `withdrawn` for a signing request dispatches `transport_dropped`.
  - Proof: `W/signing/core/sign-executor.test.ts`:
    - not live at sign → no signer call and `asker_gone`;
    - not live at submit → no relay POST;
    - no claim port (the web wallet) → unchanged behaviour.
- [ ] T082 [US1] Submit via wasm `userOpSubmitStep`, in `W/services/safe-transaction.ts`, `W/services/rpc-pool.ts` and `W/signing/core/sign-executor.ts` (RA1, RA5, RA10, RA12, ruling 8); proof: `W/services/safe-transaction.test.ts`; (after T081).
  - `W/services/safe-transaction.ts:3314-3354`: compute `userOpHash` before the POST, and read `eth_blockNumber` for `submit_block` (best effort).
  - `W/services/rpc-pool.ts:111-123`: a thrown fetch is `Network` (`maybe_delivered` true) unless `navigator.onLine === false` before the call → `NotConnected`.
  - NotSent answers `userOpNotSentDetail()`, never the raw `rpc-pool.ts:119` text.
  - `incrementNonceCache` runs only on Accepted (`safe-transaction.ts:1812, 2143, 2417`). The receipt wait comes from `dappReceiptWaitMs` (`:3386`).
  - Delete `classifySubmit` in `W/signing/core/sign-executor.ts`.
  - Proof: `W/services/safe-transaction.test.ts`:
    - mute → MaybeSent with the local hash and the nonce cache unchanged;
    - `[existingHash:]` → Accepted;
    - offline before the call → NotSent with the fixed detail.
- [ ] T083 [US1] One answer after MaybeSent or a revert, in `W/signing/core/sign-executor.ts`, `W/services/safe-transaction.ts` and `W/services/dapp-submit.ts` (RA2, RA8, RA9, ruling 9); proof: `W/services/dapp-submit.test.ts`, `W/signing/core/sign-executor.test.ts`; (after T082).
  - `W/signing/core/sign-executor.ts` dispatches `OpSubmitted{maybe_sent, submit_block}` and runs the same receipt wait.
  - `waitForReceipt` throws a typed `UserOpRevertedError{txHash}` instead of "dropped… try again" (`W/services/safe-transaction.ts:3427-3431`); `W/services/dapp-submit.ts:503-507` answers the hash.
  - Dispatch `CeremonyStarted` / `CeremonyDone` around the passkey. Log `submit verdict=…` on the panel console (contract §15).
  - Proof: `W/services/dapp-submit.test.ts` + `sign-executor.test.ts`:
    - mute → exactly one `Ok(op hash)` before 120 s;
    - revert → `Ok(tx hash)`, not -32603;
    - the ceremony order.
- [ ] T084 [US1] The extension translates receipt reads for the op hashes it handed out: new `X/lib/op-receipt.js`, `X/background.js`, and the panel's `answer` in `W/dapp/panel-surface.svelte.ts` (RF3, W12); proof: new `W/dapp/op-receipt.test.ts`; (after T083 and T089).
  - New pure `X/lib/op-receipt.js`.
  - The panel's `answer` carries `opHash: {chainId}` for ReceiptPending or MaybeSent. The worker records `vela.ext.op.<hash>` for 24 h.
  - `forwardRead` for `eth_getTransactionReceipt` / `eth_getTransactionByHash` with a recorded hash first asks the bundler `eth_getUserOperationReceipt`. With a receipt it forwards the same method for the real `transactionHash`; else it answers `null`.
  - Proof: new `W/dapp/op-receipt.test.ts` on the vectors of `W/services/dapp-submit.ts:987-1008`; an unrecorded 32-byte hash is forwarded as is.
- [ ] T085 [US7] Relay status on the web, in `W/services/tx-reconciler.ts`, `W/services/rpc-adapter.ts` and `E2E/stub-chain.ts` (RA7, G13); proof: `W/wallet/core/rpc-pool-executor.test.ts`; (after T075).
  - `W/services/tx-reconciler.ts:91-112` uses wasm `parseUserOpStatus`; delete the parser. `W/services/rpc-adapter.ts:28` uses the TS constant `USER_OP_STATUS_METHOD`.
  - Fix `E2E/stub-chain.ts:301-302`: answer `pimlico_getUserOperationStatus` with a valid status.
  - Proof: `W/wallet/core/rpc-pool-executor.test.ts` pins the constant to wasm `userOpStatusMethod()`; `tx-reconciler` tests use the probe's JSON.
- [ ] T086 [US7] `W/wallet/core/tracker-executor.ts` runs `FindOpEvent` and `HoldingsMoved` (ruling 8, RE8); proof: `W/wallet/core/tracker-executor.test.ts`; (after T075).
  - FindOpEvent: `eth_getLogs` through the pool, answered `OpEvent` with the pool's answer as it came (`logs_json` or `error_json`, plus `head_block`); the core judges a range error (T180), the shell never does.
  - HoldingsMoved → `ports.confirmed(chainId)`, moved from `notify_confirmed` (`:231-236`).
  - Proof: `W/wallet/core/tracker-executor.test.ts`: the found/range-error/head-only cases; `confirmed` fires on HoldingsMoved only.
- [ ] T087 [US2] Sheet words and endings on the web, in `W/signing/live.ts`, `W/signing/dapp-receipt.ts`, `W/signing/messages.ts` and `W/i18n/engine.server.ts` (RA8, RA9, RA10, G22); proof: `W/signing/dapp-receipt.test.ts`, `W/signing/live.test.ts`; (after T083).
  - `W/signing/live.ts:826-838`: stage words come from `SignView.phase`, so "Waiting for biometric" never covers the network wait.
  - `W/signing/dapp-receipt.ts`: endings from wasm `signEndingState`:
    - MaybeSent → caption `componentsUi.signing.maybeSent`, the op-hash row, `send.txCloseBackground`, no Retry;
    - NotSent;
    - Reverted → `failedHint` + explorer.
  - Add the `maybeSent` reader to `W/signing/messages.ts` and `W/i18n/engine.server.ts`.
  - Proof: `W/signing/dapp-receipt.test.ts` covers every `SignEndingState`, and `W/signing/live.test.ts` every phase.
- [ ] T088 [US7] The wallet's own Send on the web, in `W/flows/core/send-executor.ts` and `W/flows/live-send.ts` (RA4, RA10); proof: `W/flows/live-send.test.ts`, `W/flows/core/send-executor.test.ts`; (after T082).
  - `W/flows/core/send-executor.ts` reports `Submitted{maybe_sent, submit_block}`.
  - `W/flows/live-send.ts` draws MaybeSent (caption, no success haptic) and NotSent.
  - Proof: `W/flows/live-send.test.ts`, `W/flows/core/send-executor.test.ts`.
- [ ] T089 [US1] The worker's chain reads, in `X/background.js` and `X/lib/protocol.js` (RF2, G20, G33); proof: `W/dapp/protocol.test.ts`; (after T078).
  - `X/background.js`: the per-endpoint timeout is the core's 8 s (`:62`).
  - A failed endpoint enters a cooldown of `30 s · 2^(n−1)`, capped at 300 s, kept in storage.session `vela.ext.endpoints`. Cooled endpoints are tried last, and a success clears the entry.
  - The final error is `-32603 "Vela could not reach a node for chain <name> (<id>)"` with no engine text (`:452-455`).
  - swlog: `read.fail / failover / slow / exhausted`. The constants and the cooldown function live in `X/lib/protocol.js`.
  - Proof: `W/dapp/protocol.test.ts` pins `READ_TIMEOUT_MS` to wasm `rpcReadTimeoutMs()` and the cooldown to `rpcCooldownMs(n)` for n = 1..5; a worker test shows that the second call skips the cooled endpoints.
- [ ] T090 [US4] Plain send on the web, in `W/signing/live.ts` and `W/signing/core/sheet.svelte.ts` (RC1, RC5, RC6, G14); proof: `W/signing/live.test.ts`; (after T087).
  - `W/signing/live.ts:418-431` draws `PlainSend`: no red "Blind signature"; an amount card with `nativeSymbol(chain_id)` (`W/services/networks.ts:162`), the recipient, and 确认发送, or 确认 when `no_value`.
  - `W/signing/core/sheet.svelte.ts:56-65` passes `String(value)` and reads `calls[0]`.
  - Proof: `W/signing/live.test.ts`: −0.001 xDAI with the recipient; zero; a numeric value → the blind card with no amount.
- [ ] T091 [US4] `site_label` on the web, in `W/signing/live.ts` via wasm `browserSiteLabel` (RE7, F14); proof: `W/signing/live.test.ts`; (after T090).
  - The F14 copy at `W/signing/live.ts:898` is replaced by wasm `browserSiteLabel`.
  - Proof: `W/signing/live.test.ts`: host once when the name equals the host.
- [ ] T092 [US3] dApp transactions in Activity on the web, in `W/wallet/live.ts`, `W/wallet/live-detail.ts`, `W/wallet/core/feed-executor.ts` and `W/signing/core/sign-executor.ts` (RG1–RG4, L-D3); proof: `W/wallet/live-activity.test.ts`, `W/wallet/core/feed-executor.test.ts`; (after T083).
  - Delete `feedItemStatus` (`W/wallet/live-detail.ts:61-69`) and the kind guess (`W/wallet/live.ts:430-435`). Rows come from `FeedItem.kind/status/site`; the detail shows "Requested by".
  - `W/wallet/core/feed-executor.ts` maps the stored `dappOrigin`.
  - After the save, `persist_record` (`W/signing/core/sign-executor.ts:270-293`) and every `UpdateRecord` call `feedReconciled(1)`.
  - Proof: `W/wallet/live-activity.test.ts`, `live-detail.test.ts`, `W/wallet/core/feed-executor.test.ts`: a pending row within one poke; a MaybeSent row pending under the local hash.
- [ ] T093 [US6] Empty states on the web, in `W/flows/live.ts`, `W/flows/fixtures.ts`, `W/wallet/WalletDesktop.svelte` and `W/wallet/fixtures.ts` (RG5, RB13, G1, L-D7); proof: `W/wallet/live.test.ts`, `W/wallet/fixtures.test.ts`; (after T092).
  - History's empty line comes from `FeedView.history_empty_key`; delete the fixture copy of `history.emptyFilter` (`W/flows/fixtures.ts:398`, `W/flows/messages.ts:58` reader kept) and read the key in `W/flows/live.ts`.
  - `W/wallet/WalletDesktop.svelte:109-128, 134-148` draws `activitySection.mode` / `assetsSection.mode` the way `WalletHome.svelte:88-145` does.
  - `W/wallet/fixtures.ts:592-598` `buildDesktopState` gets the narrow model's `empty` copy (`:409-421`). Home uses `home_empty_key`.
  - Proof: `W/wallet/live.test.ts`, `W/wallet/fixtures.test.ts`: all networks → `emptyTitle`, filtered → `emptyFilter`, wide layout → skeleton then EmptyState.
- [ ] T094 [US5] One address spelling on the web, in `W/dapp/follow.ts` (RG10, L-D6); proof: `W/dapp/follow.test.ts`, `W/dapp/core-table.test.ts`; (after T075).
  - `W/dapp/follow.ts` gets `normalizeGrantSpelling()`, run once at wallet boot, which rewrites stored lower-case grants via wasm `checksumAddress`.
  - Proof:
    - `W/dapp/follow.test.ts`: once and idempotent;
    - `W/dapp/core-table.test.ts` loads `provider/inpage.js` (T042): `accountsChanged` keeps EIP-55, and the same account in another case fires no event.
- [ ] T095 [US1] Balance read plan on the web, in `W/services/wallet-api.ts` (RE9); proof: `W/services/wallet-api.test.ts`; (after T075).
  - `W/services/wallet-api.ts` reads the slots from wasm `balanceReadPlan`; delete the local list.
  - Proof: `W/services/wallet-api.test.ts`: Base includes USDC, and the order is unchanged.
- [ ] T096 [US1] Logo misses on the web, in `W/services/logo-cache.ts` and `W/wallet/ui/RemoteLogo.svelte` (RE10, W20); proof: new `W/services/logo-cache.test.ts`; (after T075).
  - `W/services/logo-cache.ts` keeps an expiry per URL from wasm `markMissTtlMs('unknown')` (60 s) instead of a session-long set; `W/wallet/ui/RemoteLogo.svelte` retries after it.
  - Proof: vitest: a miss is skipped for 60 s, then retried.
- [ ] T097 [US2] The consent card's look, in `W/dapp/DappRequestHost.svelte`, `W/signing/ui/SigningBody.svelte` and `WR/dev/gallery/+page.svelte` (RB12, G16); proof: the new gate in `W/tokens/tokens.test.ts`; (after T080).
  - `W/dapp/DappRequestHost.svelte:437-485` and `W/signing/ui/SigningBody.svelte:96-98` use `W/ui/Button.svelte`: Cancel secondary, Connect primary with a spinner while busy, Cancel disabled while busy.
  - Map the remaining tokens as in RB12. Remove the raw method line (panel and window). Window mode with no live request closes the window.
  - `WR/dev/gallery/+page.svelte:246`: `--layout-galleryRail` → `--layout-settingsNavW`.
  - Proof: new gate in `W/tokens/tokens.test.ts`: any fallback-less `var(--x)` under `src/**/*.svelte` that nothing defines fails (it lists today's 19 before the fix and 0 after).
- [ ] T098 [US8] Worker counters in the extension's bug report, `W/services/bug-report.ts` (RB14); proof: `W/services/bug-report.test.ts`; (after T077).
  - `W/services/bug-report.ts:44-56` adds `sw:<event>.<cause> ×N` from `vela.sw.counts`.
  - Proof: `W/services/bug-report.test.ts`: counters present; no URL or address.
- [ ] T099 [US9] The side-panel check, in `E2E/extension-helpers.ts` and `E2E/extension-live-provider.e2e.ts` (RG12, L-PANEL, FR-017); proof: a new signed-tick test; (after T080 and T097).
  - `E2E/extension-helpers.ts:106-145` finds the view with `pathname.endsWith('/wallet.html') && search.has('panel')`, reads the dialog's `aria-label` and clicks only inside the dialog. `sidePanelOpen` becomes `sidePanelShowsRequest`.
  - `E2E/extension-live-provider.e2e.ts`: `:339-341` becomes "the card goes, the panel stays"; `:343-357` becomes "a no-click request with the panel open shows in the panel" (RX, RB8).
  - Proof: a new signed-tick test, in which a MutationObserver in the panel sees `.landing-over [data-testid="dapp-receipt"]` titled "Signed", and it is gone within 5 s.
- [ ] T100 [US1] New `E2E/extension-lifecycle.e2e.ts` (RH5), using T099's fixed helper; proof: the suite on port 4174; (after T078–T084 and T099).
  - Cases: reload during a sheet (EX5); a second tab (EX4); panel ✕ (EX6); a worker stop via CDP `Target.closeTarget` on the SW target (EX8); panel identity after Settings (EX4b).
  - Each asserts one answer per request, 4900 with the plain text, and no `vela.req.*` left.
  - Proof: `npx playwright test -c playwright.isolated.config.ts e2e/extension-lifecycle.e2e.ts` (port 4174).
- [ ] T182 [US7] A may-have-been-sent op survives a reload on the web and in the extension panel, in `W/services/transactions-model.ts` (`LocalTransaction.maybeSent?`, `submitBlock?`), `W/services/dapp-history.ts` (`buildSigningRecord`), `W/signing/core/sign-executor.ts` (`persist_record`), `W/flows/core/send-executor.ts` (`persist_tx_records`) and `W/wallet/core/tracker-executor.ts` (`toPendingRecords`) (RA3, RA4, rulings 1 and 8, US3 AS2); proof: `W/wallet/core/tracker-executor.test.ts`; (after T083, T086, T088 and T092).
  - Same gap as T181: `toPendingRecords` (`tracker-executor.ts:100-116`) builds `TrackPendingRecord` from id, hash, chain and time only. Write both fields with the record, read them back into `TrackPendingRecord`, and carry them on the handoff into the tracker's `submitted` event.
  - Proof: a MaybeSent record saved, then the tracker restarted → `records_loaded` carries `maybe_sent: true` and `submit_block`; an old record without the fields loads as `false` / absent.
- [ ] T101 [US1] Web and extension gates in `app-web/vela-wallet` (unit, `pnpm check`, both builds, the isolated e2e suites, the event-payload ruler); (after T075–T100 and T182).
  - Update the expectations that assumed "failed — try again" after a lost reply, or the old status method: `E2E/relay-faults.e2e.ts`, `E2E/send-lands.e2e.ts`, `E2E/reopen-pending.e2e.ts`.
  - `cd app-web/vela-wallet && pnpm test:unit -- --run && pnpm check && pnpm build && pnpm build:extension`
  - `npx playwright test -c playwright.isolated.config.ts e2e/extension-*.e2e.ts`
  - `node scripts/check-event-payloads.mjs --machine SignEvent`, `--machine SendEvent` and `--machine TrackEvent` report no web mismatch (from the repo root; the T047 red is gone for the web).

**Checkpoint**: web unit and extension e2e green; `extension/dist` built from this tree; quickstart
§3 ready.

---

## Phase 5: iOS — group IOS (owns `app-ios/**`)

**Goal**: E, the iOS halves of A, C, F and G, and RE11 logs.
**Independent test**: quickstart §4.
Chains:
- T102 (`VelaLog`) and T103 (wires) start together; every later iOS task that logs comes after
  T102 as well;
- money: T103 → T104 → T105 → T106; T104 → T108; T105 → T109 → T110 → T115; T103 → T107 → T121; {T105, T108, T113} → T183;
- browser: T102 → T111 → T112 → T113 (also after T104 and T107) → T114 → T120, with T122 after T112;
- chrome: T114 → T116 → T118 (T111, T114, T116 and T118 all edit `IT/BrowserChromeTests.swift`), and T117;
- the rest after T103; T123 (gates) last.

- [ ] T102 [P] [US8] New `I/Core/VelaLog.swift` on `os.Logger` (RE11, W23).
  - Subsystem `app.getvela.VelaWallet`; categories browser, sign, relay, rpc, fee, tracker, balance, net. `.notice` for events and `.error` for failures.
  - Public fields: kinds, classes, chain ids, NSError domain+code, short hashes, durations. The dApp host is public in DEBUG and an FNV token in Release.
  - A ring of the last 8 "scope: kind" failures feeds the report's "Recent failures" line (`componentsUi.bugReport.previewFailures`) through `SettingsLive.redact` (`I/Features/Settings/SettingsLive.swift:1356-1361`).
  - Later iOS tasks move their own `print` calls onto it.
  - Proof: new `IT/VelaLogTests.swift`: the ring cap, the FNV token under a Release seam, and no address in a formatted line.
- [ ] T103 [P] [US1] iOS wire mirrors for every new variant and field of contract §1–§7, in `I/Features/Signing/Core/SignWire.swift`, `I/Features/Send/TrackerWire.swift`, `I/Core/RpcPool.swift`, `I/Features/Signing/Core/ClearWire.swift`, `I/Features/Wallet/ActivityWire.swift` and `I/Features/Send/SendWire.swift`; proof: `IT/CoreWireDriftTests.swift`.
  - `I/Features/Signing/Core/SignWire.swift`, `I/Features/Send/TrackerWire.swift` (incl. FindOpEvent / OpEvent / HoldingsMoved), `I/Core/RpcPool.swift` (`RpcPoolViewWire.unreached_chains`, the verdict's `maybe_delivered`, the `NotConnected` outcome), `I/Features/Signing/Core/ClearWire.swift`, `I/Features/Wallet/ActivityWire.swift` and `I/Features/Send/SendWire.swift`.
  - Proof: `IT/CoreWireDriftTests.swift` round-trips a core-emitted sample of each new variant (one unknown variant must not fail the view).
- [ ] T104 [US1] Submit on iOS, in `I/Core/RelayClient.swift`, `I/Core/RpcPool.swift`, `I/Core/UserOpSpine.swift` and `I/Features/Signing/Core/SignExecutor.swift` (RA1, RA5, RA10, ruling 8); proof: new `IT/SubmitVerdictTests.swift`; (after T102 and T103).
  - `I/Core/RelayClient.swift:514-534`: the loop is replaced by `userOpSubmitStep`, with `userOpHash` computed before the POST and `submit_block` read once.
  - Map `URLError` to `NotConnected` per contract §2 in `I/Core/RpcPool.swift` / `I/Core/CoreHTTP.swift`.
  - `I/Core/UserOpSpine.swift:429-430` and `I/Features/Signing/Core/SignExecutor.swift:256-259` no longer turn a pool give-up into "unreachable, try again". `EntryPoint.getNonce` is still read every time.
  - NotSent answers the dApp -32603 with `userOpNotSentDetail()`, never the pool's text (RA10).
  - Log `relay: submit verdict=…` through VelaLog.
  - Proof: new `IT/SubmitVerdictTests.swift` with a stub transport: mute → MaybeSent (local hash); refused → NotSent with the fixed detail; marker → Accepted.
- [ ] T105 [US1] One answer, endings and phase on iOS, in `I/Features/Signing/Core/SignExecutor.swift`, `I/Features/Signing/SigningAftercare.swift` and `I/Features/Signing/SigningLive.swift` (RA2, RA8, RA9, RA10, RA12, ruling 9, W3, G22); proof: `IT/SigningReceiptTests.swift`; (after T104).
  - `I/Features/Signing/Core/SignExecutor.swift`:
    - dispatch `OpSubmitted{maybe_sent, submit_block}`;
    - the receipt wait comes from `dappReceiptWaitMs`, replacing 120 s at `:91`;
    - `awaitReceipt` keeps `confirmed:false` (`:327-328`, `I/Core/RelayClient.swift:551-556`), answers the tx hash and hands a revert to the tracker;
    - dispatch `CeremonyStarted` / `CeremonyDone` around Face ID and the Trusted Signer.
  - `I/Features/Signing/SigningAftercare.swift:47-58` uses `signEndingOf` / `signEndingState`; delete the local derivation.
  - `I/Features/Signing/SigningLive.swift:476-480, 505-506`:
    - words come from `phase`;
    - MaybeSent → caption, op hash, no Retry;
    - NotSent;
    - Reverted → `failedHint` + explorer, where it used to be `txErrorGeneric`.
  - Proof: `IT/SigningReceiptTests.swift`: every `SignEndingState`; a reverted dApp tx never reads 已确认; phase words; `componentsUi.signing.maybeSent` resolves in zh and en without echoing its key (RI3's reader test).
- [ ] T106 [US7] Relay status on iOS, in `I/Core/RelayClient.swift` (RA7, G13); proof: `IT/RelayLiveTests.swift` and a probe-JSON parse test; (after T105, which also edits `RelayClient.swift`).
  - `I/Core/RelayClient.swift:571-587` uses `userOpStatusMethod()` + `parseUserOpStatus`; delete the parser. Status carries `tx_hash`.
  - Proof: `IT/RelayLiveTests.swift` is pinned to the method; a unit test parses the probe JSON.
- [ ] T107 [US7] Tracker on iOS, FindOpEvent and HoldingsMoved, in `I/Features/Send/TrackerExecutor.swift`, `I/Features/Send/TrackerStore.swift` and `I/App/RootView.swift` (ruling 8, RE8, G26); proof: new `IT/TrackerFindOpEventTests.swift`; (after T102 and T103).
  - `I/Features/Send/TrackerExecutor.swift` runs `FindOpEvent` (`eth_getLogs` through `RpcPool`) and answers `OpEvent` with the pool's answer as it came (`logs_json` or `error_json`, plus `head_block`); the core judges a range error (T180), the shell never does.
  - `HoldingsMoved` → `wallet.refresh(pull:false)` (`I/Features/Send/TrackerStore.swift`, `I/App/RootView.swift:1644-1647`). Log `balance refresh (holdings_moved)`.
  - Proof: new `IT/TrackerFindOpEventTests.swift`; a HoldingsMoved test in `IT/MoneyPlumbingTests.swift`.
- [ ] T108 [US7] The wallet's own Send on iOS, in `I/Features/Send/SendExecutor.swift` and `I/Features/Send/SendLive.swift` (RA4, RA10); proof: `IT/SplitVerdictTests.swift` or a new send-receipt test; (after T104).
  - `I/Features/Send/SendExecutor.swift` reports `Submitted{maybe_sent, submit_block}`.
  - `I/Features/Send/SendLive.swift` draws MaybeSent (no success haptic) and NotSent.
  - Proof: `IT/SplitVerdictTests.swift` or a new send receipt test.
- [ ] T109 [US4] Plain send on iOS, in `I/Features/Signing/SigningLive.swift` and `I/Features/Signing/Core/ClearExecutor.swift` (RC1, RC6, G14); proof: `IT/DappSigningTests.swift`; (after T105).
  - Delete the interception at `I/Features/Signing/SigningLive.swift:559-594` and draw `ClearSurface::PlainSend` with the same look. The zero-value card reads 发送 / 0 xDAI with the slide 确认.
  - An NSNumber `value` is passed as text (`I/Features/Signing/Core/ClearExecutor.swift`).
  - Proof: `IT/DappSigningTests.swift`: the look is unchanged for 0.001; zero; a number → the blind card.
- [ ] T110 [US4] Simulation severity on iOS, in `I/Features/Signing/Core/SigningController.swift`, `I/Features/Signing/Core/SimDeltas.swift` and `I/Features/Signing/SigningLive.swift` (RG6, L-D5); proof: `IT/SimulationSheetTests.swift`; (after T109).
  - `I/Features/Signing/Core/SigningController.swift:347-373` normalises the reply → `simOutcome`.
  - Delete the parser in `I/Features/Signing/Core/SimDeltas.swift:76-176` (its vectors moved to core in T039).
  - `SigningLive.swift:609-614` draws the notice tone: caution for could-not-check, danger for reverts.
  - Proof: `IT/SimulationSheetTests.swift`; `IT/SimDeltasTests.swift` becomes a mapping test.
- [ ] T111 [US1] The address bar names the committed page, in `I/Features/Explore/Core/BrowserEngine.swift`, `I/Components/Explore/AddressBarView.swift` and `I/Features/Explore/ExploreScreen.swift` (RE1, G28); proof: `IT/BrowserChromeTests.swift`; (after T102).
  - `I/Features/Explore/Core/BrowserEngine.swift:338-340, 360-376` no longer renames the bar from the KVO `webView.url`.
  - Track the committed URL (`didCommit` + same-origin SPA changes while nothing is pending), the pending URL (load, retry, back/forward, policy-allowed main-frame, `_blank`) and the failed URL. Page-initiated navigations (`:652-656`) no longer rename the bar.
  - The bar comes from `browserAddressBar`. Share, copy, favourite and edit use `bar.url` (`I/Components/Explore/AddressBarView.swift`, `I/Features/Explore/ExploreScreen.swift:154-158`).
  - Proof: `IT/BrowserChromeTests.swift`: typed uniswap over jumper keeps jumper until commit; `location.href` to a blackholed host never renames; failed → no lock.
- [ ] T112 [US1] Watchdog, Stop and a busy Retry, in `I/Features/Explore/Core/BrowserEngine.swift`, `I/Components/Explore/SiteMenuSheetView.swift` and `I/Components/Explore/BrowserWebView.swift` (RE2, RE4, RE5, G32, G31, W5); proof: `IT/BrowserLoadTests.swift`; (after T111).
  - `BrowserEngine.swift` arms a timer at every `requested()`, keyed by generation.
    - It is disarmed by commit, failure, finish, Stop and teardown; dropped when inactive and re-armed with the full budget on return.
    - On fire, `browserLoadShouldGiveUp(elapsed, committed, estimatedProgress)` → `stopLoading()` and `browserLoadStalled()` with the pending host, then retries 2/5/10 s while in front.
    - The -999 from `stopLoading` keeps the panel.
  - Stop: the refresh row in `I/Components/Explore/SiteMenuSheetView.swift` becomes Stop (`connect.dapp.stop`) while loading.
  - Retry: `I/Components/Explore/BrowserWebView.swift:82-83` uses `VelaButton(loading: retrying, busyTitle: explore.loadRetrying)`, busy and not disabled.
  - Proof: `IT/BrowserLoadTests.swift`:
    - fire at 20 s with no commit;
    - no fire with progress 0.4;
    - -999 after Stop is not a failure;
    - `-1000` → offline and CFNetwork 306 → proxy with `explore.loadProxy` resolving (the core classify through UniFFI);
    - Stop keeps the committed page.
- [ ] T113 [US1] Recovery when the network comes back, in new `I/Core/NetWatch.swift`, `I/Core/RpcPool.swift` and `I/App/RootView.swift` (RE3, W5); proof: new `IT/NetWatchTests.swift`; (after T112, T104 and T107, which also edit `RpcPool.swift` and `RootView.swift`).
  - New `I/Core/NetWatch.swift`: `netHealthStep` fed from `I/Core/RpcPool.swift` call outcomes (an unthrottled `.failed` is a miss; any answer is a reach), plus an NWPathMonitor unsatisfied→satisfied edge (500 ms debounce, path seam).
  - On `CameBack` (`I/App/RootView.swift`): reset the failed engines' attempt counts, retry the tab in front when `browserLoadRetryWhenNetworkReturns`, clear transient logo misses, and force a balance read. Log `net came back`.
  - Proof: new `IT/NetWatchTests.swift`: three misses then a reach → one retry; the path edge through the seam.
- [ ] T114 [US1] The chain notice on iOS, in `I/Features/Explore/ExploreScreen.swift` (RF1, RF4, G33); proof: `theChainNoticeShowsForAFailedChainOnly` in `IT/BrowserChromeTests.swift`; (after T113).
  - `I/Features/Explore/ExploreScreen.swift:202-210` shows it for `failed ∪ unreached ∖ rate_limited` (from the pool view, `I/App/RootView.swift:1553-1556`).
  - Retry is a `VelaButton` that stays busy until its one `eth_blockNumber` settles.
  - Proof: `IT/BrowserChromeTests.swift`, `theChainNoticeShowsForAFailedChainOnly`, extended for an unreached chain.
- [ ] T115 [US1] The fee row while the deployed state cannot be read, in `I/Features/Signing/Core/SigningController.swift` (RF5, W10); proof: `IT/SigningFeeRetryTests.swift`; (after T110).
  - `I/Features/Signing/Core/SigningController.swift:386-401, 432-438` feeds the core the same quote failure the send flow uses (`componentsUi.funding.denialNetworkError`) and re-quotes on `fee_policy::requote_delay_ms`. A refresh tap cancels the running loop first. Confirm the exact event name at implementation.
  - Proof: `IT/SigningFeeRetryTests.swift`: a reason is shown; one loop only after a refresh tap.
- [ ] T116 [US2] The consent sheet names the site once, in `I/Components/Explore/ConnectionPanelView.swift` and `I/Features/Explore/ExploreLive.swift` (RE6, G9, G10); proof: `IT/BrowserChromeTests.swift`; (after T103 and T114, which also edit `BrowserChromeTests.swift`).
  - `I/Components/Explore/ConnectionPanelView.swift:34-48`: one header row (avatar + "连接到 {host}" wrapping, lock line, ✕) and one `connect.browser.body` sentence above the buttons, with no footnote (`I/Features/Explore/ExploreLive.swift:330-336`).
  - Proof: `IT/BrowserChromeTests.swift`: the host appears once; one explainer.
- [ ] T117 [US2] The signing header at 375 pt, in `I/Components/Signing/SigningAtoms.swift` (RE13, RE7, G12); proof: `IT/SigningHeaderAndRouteTests.swift`; (after T103).
  - `I/Components/Signing/SigningAtoms.swift:61-87`: the name/host column gets `.layoutPriority(1)`, name and host `lineLimit(2)` without truncation, and the chain chip `.fixedSize()`.
  - The F14 copy (`:66-72`) is replaced by `browserSiteLabel`.
  - Proof: `IT/SigningHeaderAndRouteTests.swift`: host once; no truncation modifier.
- [ ] T118 [US2] Recents and the tab switcher, in `I/Components/Explore/SiteRowView.swift`, `I/Components/Explore/TabCardView.swift` and `I/Components/Explore/ExploreTabsScreen.swift` (RE7, RE12, G8, G25); proof: `IT/BrowserChromeTests.swift`, `IT/ExploreFixturesTests.swift`; (after T116).
  - Recents rows (`I/Components/Explore/SiteRowView.swift`) use `browserSiteLabel`.
  - `I/Components/Explore/TabCardView.swift:40-57` and `ExploreTabsScreen.swift:49-60` get one skeleton per cell:
    - the preview is sized by `Color.clear.aspectRatio(tabCardAspect, .fit)` with an overlay, and the snapshot top-cropped;
    - a tab with no snapshot shows its avatar (48 pt) and host;
    - the new-tab tile has a caption row `explore.newTab`;
    - the grid is top-aligned.
  - Proof: `IT/BrowserChromeTests.swift`, `IT/ExploreFixturesTests.swift`.
- [ ] T119 [US1] Balance read plan on iOS, in `I/Features/Wallet/BalanceExecutor.swift` and `I/Core/TokenReads.swift` (RE9, G24); proof: `IT/TokenReadsTests.swift`, `IT/WalletLiveTests.swift`; (after T103).
  - `I/Features/Wallet/BalanceExecutor.swift:160-176` and `I/Core/TokenReads.swift:61-130` read the slots from `balanceReadPlan`, so stablecoins are counted.
  - Proof: `IT/TokenReadsTests.swift` (Base includes USDC), `IT/WalletLiveTests.swift`.
- [ ] T120 [US1] Logo misses on iOS, in `I/Components/Wallet/RemoteLogoView.swift` (RE10, W20); proof: a new test in `IT/BrowserMemoryTests.swift`; (after T113).
  - `I/Components/Wallet/RemoteLogoView.swift:46-53`: `LogoStore` keeps expiries from `markMissTtlMs` and bumps an observable epoch; transient misses clear on `CameBack`.
  - Proof: a new test in `IT/BrowserMemoryTests.swift` or `IT/VelaStoreTests.swift`: a 404 stays missed for the session; a 503 retries after 60 s.
- [ ] T121 [US3] dApp transactions and empty states on iOS, in `I/Features/Flows/FlowsLive.swift`, `I/Components/Wallet/ActivityRowView.swift`, `I/Features/Wallet/WalletLive.swift`, `I/Features/Wallet/ActivityExecutor.swift` and `I/Features/Flows/WalletFlowFixtures.swift` (RG1–RG3, RG5, RG10, L-D3, L-D7); proof: `IT/ActivityLiveTests.swift`, `IT/ProviderScriptTests.swift`; (after T107).
  - Delete the record lookup at `I/Features/Flows/FlowsLive.swift:261`. Rows come from `FeedItem.kind/status/site` (`I/Components/Wallet/ActivityRowView.swift`, `I/Features/Wallet/WalletLive.swift`); the detail shows "Requested by".
  - `I/Features/Wallet/ActivityExecutor.swift` maps `dappOrigin`. The poke at `RootView.swift:1648` stays.
  - History's empty line uses `history_empty_key`: delete the fixture copy (`I/Features/Flows/WalletFlowFixtures.swift:240`, `FlowsLive.swift:183`). Home uses `home_empty_key`.
  - Proof:
    - `IT/ActivityLiveTests.swift`: a pending dApp row with its site; a failed row; `emptyTitle` on all networks;
    - `IT/ProviderScriptTests.swift`: `accountsChanged` keeps EIP-55 (T042).
- [ ] T122 [US1] A hermetic never-answering listener, in `IU/LocalDappServer.swift` and `IU/DappBrowserStabilityProbeTests.swift` (RH4, T004 of 079); proof: the panel at 20 ± 2 s (run in T168); (after T112).
  - `IU/LocalDappServer.swift` gains an in-process loopback listener that accepts and never answers, and `IU/DappBrowserStabilityProbeTests.swift` gets a test for it.
  - Proof: the panel appears at 20 ± 2 s and the busy Retry label is asserted (run via the copied-`.xctestrun` recipe, in the device phase).
- [ ] T183 [US7] A may-have-been-sent op survives a relaunch on iOS, in `I/Features/Signing/Core/SignExecutor.swift` (`persist_record`), `I/Features/Send/SendExecutor.swift` (`persist_tx_records`, `track_submitted`), `I/Features/Send/TrackerExecutor.swift` (`pendingWire`) and the dApp handoff in `I/App/RootView.swift` (RA3, RA4, rulings 1 and 8, US3 AS2); proof: a reload case in `IT/TrackerFindOpEventTests.swift`; (after T105, T108 and T113).
  - Same gap as T181: `pendingWire` (`TrackerExecutor.swift:150-162`) sends only id, hash, chain and time. Write `maybeSent` / `submitBlock` with the record, read them back into the `TrackPendingRecord` wire, and carry both on the handoff into the tracker's `submitted` event.
  - Proof: a stored MaybeSent record → the wire has `maybe_sent: true` and `submit_block`; an old record → `false` / absent.
- [ ] T123 [US1] iOS gates: the hermetic `VelaWalletTests` suite on a cloned simulator, `app-ios/scripts/check-ios-*.mjs`, `scripts/check-event-payloads.mjs` and `rust/scripts/check-ios-core-fresh.sh`; (after T102–T122 and T183).
  - `xcrun simctl clone "<base simulator>" vela-082`, then `xcodebuild test -project app-ios/VelaWallet/VelaWallet.xcodeproj -scheme VelaWallet -destination 'platform=iOS Simulator,name=vela-082' -only-testing:VelaWalletTests`. Assert the test count (Swift Testing can report a false zero).
  - `node app-ios/scripts/check-ios-dropped-judgement.mjs && node app-ios/scripts/check-ios-event-parity.mjs`
  - `node scripts/check-event-payloads.mjs` reports no iOS mismatch (the T047 red is gone for iOS)
  - `bash rust/scripts/check-ios-core-fresh.sh` ok

**Checkpoint**: iOS hermetic suite green with the counted number of tests; a Debug build is ready
for quickstart §4.

---

## Phase 6: Android parity — group AND (owns `app-android/**`)

**Goal**: the Kotlin halves of A, C, E and G (FR-020). JVM tests only; the device smoke is
optional (RH3).
Chains:
- T124 first;
- money: T125 → T126; T125 → T127 → T129 → T130 → T133 (T127 dispatches T125's verdict);
- tracker: T125 → T128 → T136 → T184 (T184 also after T127);
- browser: T131 → T132; T131 → T133 → T134 (T131, T133 and T134 all edit `AT/ExploreLiveTest.kt`);
- T137 (gates) last.

- [ ] T124 [P] [US1] Android wire mirrors in `A/feature/signing/core/SignWire.kt`, `A/feature/send/core/TrackerWire.kt`, `A/feature/wallet/core/RpcWire.kt`, `A/feature/signing/core/ClearWire.kt`, `A/feature/wallet/core/FeedWire.kt` and `A/feature/send/core/SendWire.kt`, plus key constants in `A/core/i18n/I18nKeys.kt`; proof: `AT/CoreWireDriftTest.kt`.
  - Wires: `A/feature/signing/core/SignWire.kt`, `A/feature/send/core/TrackerWire.kt` (incl. FindOpEvent / OpEvent / HoldingsMoved), `A/feature/wallet/core/RpcWire.kt`, `A/feature/signing/core/ClearWire.kt`, `A/feature/wallet/core/FeedWire.kt` and `A/feature/send/core/SendWire.kt`.
  - Add `A/core/i18n/I18nKeys.kt` constants for `componentsUi.signing.maybeSent` and `explore.loadProxy`.
  - Proof: `AT/CoreWireDriftTest.kt` round-trips each new variant (kotlinx must not refuse the whole view).
- [ ] T125 [US1] Submit on Android, in `A/feature/send/core/RelayClient.kt`, `A/feature/wallet/core/RpcPool.kt`, `A/core/net/VelaHttp.kt` and `A/feature/send/core/UserOpSpine.kt` (RA1, RA10, ruling 8); proof: `AT/RelayClientTest.kt`; (after T124).
  - `A/feature/send/core/RelayClient.kt:407-421`: the loop is replaced by `userOpSubmitStep`, with `userOpHash` and `submit_block`.
  - Map exceptions to `NotConnected` (UnknownHost, Connect, NoRouteToHost, SSLHandshake, SocketTimeout "connect timed out") in `A/feature/wallet/core/RpcPool.kt` / `A/core/net/VelaHttp.kt`.
  - `A/feature/send/core/UserOpSpine.kt:373` no longer turns a pool give-up into "unreachable, try again"; NotSent answers the dApp -32603 with `userOpNotSentDetail()`, never the pool's text (RA10). Log the verdict through `A/core/diagnostics/VelaLog.kt`.
  - Proof: `AT/RelayClientTest.kt`: mute → MaybeSent; refused → NotSent with the fixed detail; marker → Accepted.
- [ ] T126 [US7] Relay status on Android, in `A/feature/send/core/RelayClient.kt` (RA7, G13); proof: `AT/RelayClientTest.kt`; (after T125).
  - `RelayClient.kt:466-481` uses `userOpStatusMethod` / `parseUserOpStatus`; delete the parser.
  - Proof: `AT/RelayClientTest.kt:217` is fixed to the pimlico method.
- [ ] T127 [US1] One answer, endings and phase on Android, in `A/feature/signing/core/SignExecutor.kt`, `A/feature/signing/SigningAftercare.kt`, `A/feature/signing/SigningLive.kt` and `A/VelaWalletApplication.kt` (RA2, RA8, RA9, RA10, RA12, ruling 9); proof: `AT/SigningReceiptTest.kt`, `AT/SigningLiveTest.kt`; (after T125, whose verdict it dispatches).
  - `A/feature/signing/core/SignExecutor.kt:41`: the window comes from `dappReceiptWaitMs`; dispatch `OpSubmitted{maybe_sent, submit_block}`; a revert → the tx hash.
  - `A/feature/signing/SigningAftercare.kt:36-44` uses `signEndingOf` / `signEndingState`; delete the local derivation.
  - `A/feature/signing/SigningLive.kt:544-552`: words come from `phase`; MaybeSent → title `send.txSubmitting`, caption `componentsUi.signing.maybeSent` (T124's constant), the op hash, `send.txCloseBackground`, no Retry; NotSent → `statusFailed` + `txErrorGeneric`; Dropped → `failedHint` + explorer (RA10).
  - Wire the no-op dApp `signingStarted` port (`A/VelaWalletApplication.kt:690`) to `CeremonyStarted` / `CeremonyDone`.
  - Proof: `AT/SigningReceiptTest.kt`, `AT/SigningLiveTest.kt`: every `SignEndingState`; phase words.
- [ ] T128 [US7] Tracker and Send on Android, in `A/feature/send/core/TrackerExecutor.kt`, `A/feature/wallet/core/TrackerWork.kt`, `A/feature/wallet/core/WalletController.kt`, `A/feature/send/core/SendExecutor.kt`, `A/feature/send/SendLive.kt` and `A/feature/send/core/SendController.kt` (ruling 8, RE8, RA10); proof: `AT/TrackerMachineTest.kt`, `AT/SendLiveTest.kt`, `AT/SendControllerTest.kt`; (after T125).
  - `A/feature/send/core/TrackerExecutor.kt` / `A/feature/wallet/core/TrackerWork.kt` run `FindOpEvent` and answer `OpEvent` with the pool's answer as it came (`logs_json` or `error_json`, plus `head_block`); the core judges a range error (T180), the shell never does.
  - `HoldingsMoved` → `refresh(force=true)` (`A/feature/wallet/core/WalletController.kt`).
  - Send MaybeSent/NotSent in `A/feature/send/SendLive.kt` and `A/feature/send/core/SendController.kt`.
  - Proof: `AT/TrackerMachineTest.kt`, `AT/SendLiveTest.kt`, `AT/SendControllerTest.kt`.
- [ ] T129 [US4] Plain send on Android, in `A/feature/signing/SigningLive.kt` and `A/feature/signing/core/ClearExecutor.kt` (RC1, RC6, G14); proof: `AT/SigningLiveTest.kt`; (after T127).
  - Delete the interception at `A/feature/signing/SigningLive.kt:637-658` and draw `PlainSend`.
  - A present non-string `value` goes as text (`optString` → an `isNull` check, in `A/feature/signing/core/ClearExecutor.kt`).
  - Proof: `AT/SigningLiveTest.kt`: the same look for 0.001; zero; a number → blind.
- [ ] T130 [US4] Simulation severity on Android, in `A/feature/signing/core/SimDeltas.kt` and `A/feature/signing/SigningLive.kt` (RG6); proof: `AT/SimDeltasTest.kt`; (after T129).
  - Delete the parser in `A/feature/signing/core/SimDeltas.kt` → `simOutcome`. `SigningLive.kt:757` takes its tone from `notice`.
  - Proof: `AT/SimDeltasTest.kt` becomes a mapping test (caution vs danger).
- [ ] T131 [US1] Browser parity, in `A/feature/browser/ExploreLive.kt`, `A/navigation/VelaNavHost.kt`, `A/feature/browser/core/BrowserController.kt` and `A/feature/explore/components/ExploreSheets.kt` (RE1, RE2, RE5); proof: `AT/BrowserMachineTest.kt`, `AT/ExploreLiveTest.kt`; (after T124).
  - No fixture host or open lock in a fresh tab (`A/feature/browser/ExploreLive.kt:195`, `A/navigation/VelaNavHost.kt:1200-1203`): the bar comes from `browserAddressBar`.
  - The watchdog in `A/feature/browser/core/BrowserController.kt` uses `WebView.getProgress()` with `browserLoadShouldGiveUp` / `browserLoadStalled`.
  - `BrowserNotice` Retry is busy, not dimmed (`A/feature/explore/components/ExploreSheets.kt:621-627`).
  - Proof: `AT/BrowserMachineTest.kt`, `AT/ExploreLiveTest.kt`.
- [ ] T132 [US1] Network health moved to core, and logo misses, in `A/core/net/NetHealth.kt`, `A/navigation/VelaNavHost.kt` and `A/core/marks/RemoteLogo.kt` (RE3, RE10); proof: `AT/NetHealthTest.kt`, `AT/MarksTest.kt`; (after T131).
  - `A/core/net/NetHealth.kt:18-47` uses `netHealthStep`; the CameBack behaviour (`VelaNavHost.kt:530-536`) is kept, and it also clears transient misses.
  - `A/core/marks/RemoteLogo.kt` uses `markMissTtlMs`.
  - Proof: `AT/NetHealthTest.kt` becomes a mapping test; `AT/MarksTest.kt`.
- [ ] T133 [US2] Consent, header, Recents and tabs, in `A/feature/explore/components/ExploreSheets.kt`, `A/feature/signing/components/SigningComponents.kt`, `A/feature/signing/SigningLive.kt`, `A/feature/explore/components/ExploreComponents.kt` and `A/feature/explore/components/ExploreTabs.kt` (RE6, RE7, RE12, RE13); proof: `AT/SigningHeaderAndRouteTest.kt`, `AT/ExploreLiveTest.kt`; (after T130 and T131).
  - `A/feature/explore/components/ExploreSheets.kt`: the title merges into the site row.
  - `A/feature/signing/components/SigningComponents.kt:107-117`: `maxLines = 2` with no ellipsis, and `browserSiteLabel`; delete the copy at `SigningLive.kt:254`.
  - Recents (`A/feature/explore/components/ExploreComponents.kt`) use `site_label`. `A/feature/explore/components/ExploreTabs.kt` cards follow RE12.
  - Proof: `AT/SigningHeaderAndRouteTest.kt`, `AT/ExploreLiveTest.kt`.
- [ ] T134 [US1] The chain notice reads `unreached_chains`, in `A/feature/browser/ExploreLive.kt` (RF1); proof: `AT/ExploreLiveTest.kt`; (after T133, which also edits `ExploreLiveTest.kt`).
  - `A/feature/browser/ExploreLive.kt`, `chainUnreachable`.
  - Proof: `AT/ExploreLiveTest.kt`: an unreached-only chain shows it; a rate-limited one does not.
- [ ] T135 [US1] Balance read plan on Android, in `A/feature/wallet/core/BalanceExecutor.kt` (RE9); proof: `AT/BalanceMachineTest.kt`, `AT/WrappedNativeTest.kt`; (after T124).
  - `A/feature/wallet/core/BalanceExecutor.kt:260-300, 452` uses `balanceReadPlan`; delete the copy.
  - Proof: `AT/BalanceMachineTest.kt`, `AT/WrappedNativeTest.kt`.
- [ ] T136 [US3] dApp transactions and empty states on Android, in `A/feature/flows/FlowLive.kt`, `A/VelaWalletApplication.kt`, `A/feature/wallet/core/WalletController.kt`, `A/feature/wallet/core/FeedExecutor.kt` and `A/feature/flows/FlowFixtures.kt` (RG1–RG3, RG5); proof: `AT/FlowLiveTest.kt`, `AT/FeedExecutorTest.kt`; (after T127 and T128).
  - Delete the tx-hash heuristic at `A/feature/flows/FlowLive.kt:265`; rows come from `kind/status/site`: title `history.txLabelDappTx`, subtitle site → recipient → chain, `statusPending` / `statusFailed` + " · " when not confirmed, and the detail's "Requested by" fact (`componentsUi.signing.siweOrigin`) (RG2).
  - `A/VelaWalletApplication.kt:691`: `recordsPersisted()` passes 1; `WalletController.kt:722` re-reads the feed. `A/feature/wallet/core/FeedExecutor.kt` maps `dappOrigin`.
  - History and home empty keys: delete the fixture choice at `A/feature/flows/FlowFixtures.kt:298`.
  - Proof: `AT/FlowLiveTest.kt`, `AT/FeedExecutorTest.kt`: a Failed dApp row is now possible; the row appears after one write.
- [ ] T184 [US7] A may-have-been-sent op survives a restart on Android, in `A/feature/signing/core/SignExecutor.kt` (`recordRow`), `A/feature/send/core/SendExecutor.kt` (`PersistTxRecords`, the `trackSubmitted` port), `A/feature/send/core/SendController.kt`, `A/feature/send/core/TrackerExecutor.kt` (`LoadPendingTxs`) and `A/feature/wallet/core/FeedExecutor.kt` (RA3, RA4, rulings 1 and 8, US3 AS2); proof: a reload case in `AT/TrackerMachineTest.kt`; (after T127, T128 and T136).
  - Same gap as T181: `LoadPendingTxs` (`TrackerExecutor.kt:50-60`) builds `TrackPendingRecord` from id, hash, chain and time, and `trackSubmitted(userOpHash, recordIds, chainId)` drops both new fields. Write `maybeSent` / `submitBlock` with the row, read them back, and carry them on the handoff into the tracker's `Submitted`.
  - Proof: a stored MaybeSent row → `TrackPendingRecord(maybe_sent = true, submit_block = n)`; an old row → `false` / null.
- [ ] T137 [US1] Android gates: `./gradlew testDebugUnitTest` in `app-android/vela-wallet`, `scripts/check-android-*.mjs` and `scripts/check-event-payloads.mjs`; (after T124–T136 and T184).
  - `bash rust/scripts/build-dev-fixtures.sh --host && cd app-android/vela-wallet && ./gradlew testDebugUnitTest` (JAVA_HOME = Android Studio JBR)
  - `node scripts/check-android-dropped-judgement.mjs && node scripts/check-android-event-parity.mjs`
  - `node scripts/check-event-payloads.mjs` reports no Android mismatch (the T047 red is gone for Android)

**Checkpoint**: the JVM suite is green; parity recorded for every shared rule.

---

## Phase 7: Signer page — group SIGNER (owns `app-web/trusted-signer/**`; then one core file)

The `TS/` tasks touch no other group's file and may start at any time. T141 edits a core file
(`rust/crates/vela-core/src/trusted_signer/integrity.rs`), and that moves the core fingerprint. It
therefore waits for the Phase 2 checkpoint, and T046 re-runs after it before any device build.

- [x] T138 [P] [US4] L-HOST: the signing page names the site once, in `TS/src/lib/resolve.js` and `TS/src/lib/render.js` (RG11, FR-013); proof: new `TS/samples/origin-line-test.mjs`.
  - `TS/src/lib/resolve.js` `baseView` (`:169-212`) adds `originShown: !!host && host.toLowerCase() !== ((known && known.name) || '').toLowerCase()`; the ceremony view (`:1034-1036`) sets it too.
  - `TS/src/lib/render.js:448-449, 490` draws the host line only when it is set. `view.dapp.origin` stays for `warn.claimedOrigin`.
  - Proof: new `TS/samples/origin-line-test.mjs`: name == host → one line; a different name → two lines; the claimed-origin warning is still drawn.
- [x] T139 [US4] A no-calldata call is a send, in `TS/src/lib/resolve.js` (RC8, G14); proof: new `TS/samples/plain-send-test.mjs`; (after T138).
  - `TS/src/lib/resolve.js:242-287`: `hasCalldata` is false for absent, "", "0x" and "0X"; every no-calldata call takes the send path, including value 0 (amount "0", no out-minus, no `ui.simNoOther`). The blind ladder (`:348-366`) sees only calldata.
  - Proof: new `TS/samples/plain-send-test.mjs`.
- [x] T140 [US4] Rebuild `TS/dist` (`bun TS/samples/build-single.mjs`, `_headers` included) and record the new hash; (after T139). Built `58556162286e6174af717248b7898b226d4477b880dcd5351080c2b9a73ed035` (b8e74266); it replaces `9df169ea…` (e1cc53a8), which is never to be allowed.
  - Proof: every sample suite passes: `hostile-test.mjs`, `takeover-test.mjs`, `fee-leg-test.mjs`, `unlimited-line-test.mjs`, `single-file-test.mjs`, `slider-test.mjs`, `ceremony-test.mjs`, `channels-test.mjs`, `origin-line-test.mjs` and `plain-send-test.mjs`.
- [ ] T141 [US4] Put the new hash at the front of `BUILD_ALLOWED` in `rust/crates/vela-core/src/trusted_signer/integrity.rs`; `LAUNCH` is unchanged; (after T140 and T047).
  - Then re-run T046 (pkg-web, xcframework, `.so`).
  - Proof: `cd rust && cargo test -p vela-core --test trusted_signer`, the integrity `--check`, and `check-ios-core-fresh.sh` ok.
- [ ] T142 [US4] Owner step (outward-facing): deploy `TS/dist/` from the 082 tree to sign.getvela.app; (after T141).
  - Verify `curl -I https://sign.getvela.app/b/<hash>/sign` → 200 with the `immutable` header.
  - Then move `LAUNCH` (`integrity.rs:135`) to that hash and rebuild the wasm and native bindings (T046).
  - Proof: the curl output in `EV/signer/deploy.txt`; `check-ios-core-fresh.sh` ok.

**Checkpoint**: sample suites green; `BUILD_ALLOWED` carries the new build; `LAUNCH` moves only
after the owner's deploy is verified.

---

## Phase 8: Device verification (quickstart.md)

This phase needs every client phase done for its client, and T046 current (`check-ios-core-fresh.sh`
ok before any device row).
- Faults touch only the app under test (ruling 6, RH1). One client at a time goes on port 8899, with
  `chaos mode=pass` between rows.
- Screenshots and log excerpts go in `EV/<client>/post-<row>.jpg` / `.txt`.
- Every failure row names its log line (FR-018).
- Rows run in the parallel space unless marked 👆.

- [ ] T143 Pre-flight (quickstart §0–§1): freshness via `rust/scripts/check-ios-core-fresh.sh`, `$S/proxy-before.txt`, the test dApp, `scripts/device/chaos-proxy.py` and the three builds; proof: the `.err` first line and each client's CONNECTs in `$S/logs/chaos.log`.
  - Freshness: `check-ios-core-fresh.sh` ok, and the `WASM_URL` hash equals `extension/dist/*.wasm`.
  - `scutil --proxy > $S/proxy-before.txt`.
  - Serve the test dApp on 8137 and 8138; start `chaos-proxy.py` with the `chaos()` helper.
  - Builds: desktop dev-fixtures (§1.3), CfT with its own profile (§1.4), iPhone Debug with `VELA_DEV_PROXY` (§1.5).
  - Proof: `.err` first line `dev proxy: all traffic via 127.0.0.1:8899`; chaos.log shows each client's CONNECTs.

### Desktop (§2)

- [ ] T144 [US1] Desktop page loads, proxy, tabs and chain (quickstart §2); evidence in `EV/desktop/post-*` and `$S/logs/desktop-*.err`.
  - Rows: DX0, DX-G3, DX13, DX14, DX11, L1, DX1, L2–L4, L5, DX2, DX4, DX5, DX3′, C1, C2, DX8, DX9, DX-T5.
  - Regressions: **G3, G7, G29, G30, G2, G33**, W6, W7, W8, W14, W16, W17, W18.
  - Evidence: `EV/desktop/post-*`, plus the `.err` lines `browser:`, `proxy:`, `chain notice:`, `window:`.
- [ ] T145 [US1] Desktop money under faults (quickstart §2); evidence in `EV/desktop/post-*` and `$S/logs/desktop-*.err`.
  - Rows: DX-W1, DX6, DX7, S5, DX-G13 (+ optional S6), DX-W3, S7/S8.
  - Regressions: **G21, G22, G13**, W1, W3.
  - Also check that the fee row shows the fee coin and fiat before signing (L-D4, US7 AS1).
  - Also (T181; not a quickstart row): during DX-W1, relaunch while the may-have-been-sent caption shows and before `pass` → Activity still lists the dApp row as 处理中; after `pass` it turns 已确认 with no tap (the chain-log check found it even if the relay stays mute).
  - Evidence: explorer links, the fixture Safe's nonce before and after each row, and `relay: submit verdict=` lines.
- [ ] T146 [US2] Desktop look and words (quickstart §2); screenshots in `EV/desktop/post-*`.
  - Rows: DX12, U1/U2, U5·S1·S2.
  - Regressions: **G6, G11**. Screenshots in zh and en.
- [ ] T147 [US4] Desktop honest risk (quickstart §2); evidence in `EV/desktop/post-*`.
  - Rows: DX-G14, G14-zero/num, DX-LD5.
  - Regressions: **G14**, L-D5.
- [ ] T148 [US3] Desktop Activity: row DX-LD3 (after DX-W1 and S4); L-D3 live; evidence in `EV/desktop/post-DX-LD3.jpg`.
- [ ] T149 [US5] Desktop address spelling: row DX-LD6; L-D6 live, compared with `p30-dapp-log.png`; evidence in `EV/desktop/post-DX-LD6.txt`.
- [ ] T150 [US6] Desktop empty states: row DX-LD7; **G1**, L-D7; evidence in `EV/desktop/post-DX-LD7.jpg`.
- [ ] T151 [US9] Desktop evidence rows: SC-006a/b (10 failed + 10 good loads, Recents) and SC-007-en (no "Secure site", "Not secure" or "Encrypted"); evidence in `EV/desktop/post-SC-006*.jpg` and `post-SC-007-en.jpg`.

### Chrome extension (§3)

- [ ] T152 [US1] Extension request lifecycle (quickstart §3); evidence in `EV/extension/post-*` and the `vela.sw.log` dump.
  - Rows: EX0, EX2, EX4, EX4b, EX5, EX6, EX7 (5 min), EX8, EX8b, EX3-idle, EX9.
  - Regressions: **G17, G18, G19, G23**. EX8b's `sw.start` / `req.arrived` lines settle G23(d).
  - Keep DevTools off the worker for the lifecycle rows.
- [ ] T153 [US1] Extension reads and money (quickstart §3); evidence in `EV/extension/post-*`.
  - Rows: EX10/EX11, S7/S8, EX-W1, EX-S5, EX12, EX13, EX-W3.
  - Regressions: **G20, G33, G21, G22**, W3, W12.
  - EX-S5 is the expected web difference (may-have-been-sent until `not_found` ×2).
  - Also (T182; not a quickstart row): during EX-W1, reload the side panel while the caption shows and before `pass` → the panel's Activity still lists the pending dApp row, and it turns 已确认 after `pass`.
- [ ] T154 [US2] Extension consent card: row U2 in zh and en at panel width; **G16**; evidence in `EV/extension/post-U2-{zh,en}.jpg`.
- [ ] T155 [US4] Extension plain send: row E-G14 (+ zero, numeric); **G14**; evidence in `EV/extension/post-E-G14*.jpg`.
- [ ] T156 [US5] Extension address spelling: row EX-LD6 (old grants rewritten at boot); L-D6; evidence in `EV/extension/post-EX-LD6.txt`.
- [ ] T157 [US6] Extension empty states: row EX-G1 in a wide tab; **G1**, L-D7; evidence in `EV/extension/post-EX-G1.jpg`.
- [ ] T158 [US8] Extension logs: row EX-LOG; W23; evidence in `EV/extension/post-EX-LOG.txt`. The `vela.sw.log` excerpts are host-only; the report preview lists `sw:req.settled.page_left ×1` and no URL or address.
- [ ] T159 [US9] Extension evidence rows: S3 (the panel tick and the e2e suites green on port 4174; L-PANEL, SC-009) and SC-007-en; evidence in `EV/extension/post-S3.jpg` and `post-SC-007-en.jpg`.

### iPhone (§4)

- [ ] T160 [US1] iPhone browser, chain and fee (quickstart §4); evidence in `EV/ios/post-*` and the `log collect` archive in `$S/logs/`.
  - Rows: IX0/IX1, E-G28a, E-G28b, E-G28c, IX2, E-Stop, E-W5-back, E-G31, L5, C1/C2, IX6, S7/S8, E-W20.
  - Regressions: **G28, G32, G31, G33**, W5, W10, W20.
  - No `--console` session during these rows.
- [ ] T161 [US1] iPhone money: rows IX-W1, IX8, S5, IX7; **G21, G22**, W1, W3; evidence in `EV/ios/post-*`.
  - Also (T183; not a quickstart row): during IX-W1, force-quit and relaunch (same `-e` JSON) while the caption shows and before `pass` → Activity still lists the pending dApp row; after `pass` it turns 已确认.
- [ ] T162 [US2] iPhone look (quickstart §4); screenshots in `EV/ios/post-*`.
  - Rows: E-G9/G10 (zh and en), E-G12 (also at the largest text size), E-G25, E-G8.
  - Regressions: **G9, G10, G12, G25, G8**.
- [ ] T163 [US4] iPhone plain send: row I-G14 (+ zero-value card); G14, now drawn from the core; evidence in `EV/ios/post-I-G14*.jpg`.
- [ ] T164 [US3] iPhone Activity and balance after a dApp tx: rows E-LD3 and E-G26; L-D3, **G26**; evidence in `EV/ios/post-E-LD3.jpg`, `post-E-G26.jpg`.
- [ ] T165 [US5] iPhone address spelling: row E-LD6; L-D6; evidence in `EV/ios/post-E-LD6.txt`.
- [ ] T166 [US6] iPhone empty states: row E-LD7; L-D7; evidence in `EV/ios/post-E-LD7.jpg`.
- [ ] T167 [US8] iPhone logs: row E-W23; W23; evidence in `EV/ios/post-E-W23.txt`. Every failure row has an `app.getvela.VelaWallet` line in the `log collect` archive; the report preview shows "最近失败: browser: timeout; …" with no hosts.
- [ ] T168 [US9] iPhone evidence rows (quickstart §4); evidence in `EV/ios/post-*`.
  - Rows: E-L1 (SC-003, frame-counted ≤ 0.5 s), E-probe (079 T004 plus T122's listener, via the copied `.xctestrun`), SC-006a/b, SC-007-en.

### Android (§5, optional)

- [ ] T169 [P] Android smoke on the Xiaomi `9d5f42fb`, parallel space, no proxy (RH3): rows A-G14, A-G28, A-LD3, A-LD5/6/7, A-G26; regressions **G14, G28, G26**, L-D3, L-D5, L-D6, L-D7 (FR-020 parity). Evidence in `EV/android/post-*`. This task is optional; the JVM suite (T137) is the gate.

### Owner batch (§6, real passkeys, no faults)

- [ ] T170 [US2] 👆 The Mac, on the `/Applications` build signed per the macOS platform-passkey recipe; evidence in `EV/desktop/post-O-D*`.
  - Rows: O-D1 (**G22**: 等待生物识别… only while Touch ID is up), O-D2 (**L-D3 live**: the dApp tx in Activity survives a relaunch; the balance moved), O-D3 (**G11**, **L-D6 live**).
- [ ] T171 [US2] 👆 The owner's Chrome with `app-web/vela-wallet/extension/dist` from this tree; evidence in `EV/extension/post-O-E*`.
  - Rows: O-E1 (load unpacked from the 082 `extension/dist`), O-E2 (L-PANEL tick; **L-D3 live**: Activity lists the dApp tx, where it read "No activity yet"), O-E3 (optional worker stop; **G19** in the real profile).
- [ ] T172 [US2] 👆 The iPhone with `"VELA_PARALLEL_SPACE":"0"` and no `--console`; evidence in `EV/ios/post-O-I1.jpg`.
  - Row O-I1: sign + dust, G26 balance within 5 s, **G24** USDC on Base ≈ 0.470005.
- [ ] T173 [US4] 👆 Row O-T1 on the iPhone and the Mac, only after T142; evidence in `EV/signer/post-O-T1.jpg`.
  - The page header names the host once (L-HOST); the self-reported-site warning is still there; a zero-value plain send reads Send 0 (RC8).

### Close-out (§7)

- [ ] T174 [US8] Close-out (FR-018, FR-019, SC-007): the proxy diff, the secret scan over `$S/logs`, and the SC-007 table in `EV/sc-007-log-lines.md`.
  - `chaos mode=pass`; stop the proxy and the dApp servers.
  - `diff $S/proxy-before.txt <(scutil --proxy)` prints "system proxy untouched"; the owner confirms once that the iPhone's Wi-Fi proxy setting was never touched (quickstart §0, ruling 6).
  - The secret scan `rg -n '0x[0-9a-fA-F]{130,}|/v3/[0-9a-f]{20,}|#[A-Za-z0-9_-]{40,}|signature=0x|privateKey|mnemonic|seed' $S/logs` prints nothing.
  - Write the SC-007 table (failure row → log line, one per row in §2–§4) into `EV/sc-007-log-lines.md`.

---

## Phase 9: Results, hand-offs, delivery

- [ ] T175 Write `specs/082-dapp-browser-mac-ext-ios/results.md`.
  - A verdict per SC-001–SC-010, with evidence links.
  - The **client matrix**: for every G#, L-… and W#, one of fixed / already right / no such surface, for desktop, extension, iPhone and Android.
  - The gates run in each phase (T047, T074, T101, T123, T137, T140, T141).
  - The regression table of quickstart §7 (finding → rows → verdict), and the SC-007 table from T174.
  - What was not done and why: G4 (RD15), W9/W21/W22 (RF6), W25 (b)/(c) (speculative; its (a) is covered by T111's pending URL), os_log on the desktop, one webview per desktop tab, PAC on Windows and Linux, proxy words in `FeeFailure` (RD9), the `tx_call_of` core move (RC6), CS26, Android fault rows (RH3), the spec's edge cases with no quickstart row (Mac sleep/wake, passkey cancel/timeout, unsupported chain).
- [ ] T176 Relay hand-offs in `results.md` (RG13, plan "Out of scope"), each with its evidence file:
  - L-D2(a): Arbitrum in-band ops are accepted and never mined;
  - L-D4, ruling 2: on Gnosis and Arc the native coin is the stablecoin, so the relay's pricing and floor are the relay's;
  - an `eth_getUserOperationStatus` alias for builds already shipped (`EV/relay-status-probe.txt`);
  - confirm that admission stores an op before `not_found` can be read (the 60 s grace), and that `AlreadyQueued` covers included ops.
- [ ] T177 Optional cleanup, once all four shells read `SignView.phase`: remove `is_signing` / `is_submitting` from `C/sign_request.rs` and from `SignWire` (iOS, Android) and `sign-types.ts`, then re-run T046. Otherwise record it as a follow-up in `results.md`. Proof: every client suite green after the removal.
- [ ] T178 Memory: a project note `project_082_dapp_browser_mac_ext_ios.md` in `~/.claude/projects/-Volumes-data-production-vela-wallet/memory/`, indexed in its `MEMORY.md`.
  - A project note for 082: rulings 1–10, the new core rules (submit_step, MaybeSent/NotSent, FindOpEvent, SignPhase, the extension lifecycle, PlainSend, address_bar/site_label/give-up, read_plan, sim_outcome), and the device recipe (`VELA_DEV_PROXY`, chaos `mute`/`stall`, CfT profile, the owner batch).
  - Update the device-harness, i18n-corpus-gates and concurrent-sessions references where 082 changed them.
- [ ] T179 Delivery: commits by path, `git cherry origin/main 082-dapp-browser-mac-ext-ios`, push, and the PR with `specs/082-dapp-browser-mac-ext-ios/results.md`'s evidence table and client matrix; CI green.
  - Commit by path per client, in reviewable steps (the corpus is one commit, T043).
  - Before deleting the worktree, run `git cherry origin/main 082-dapp-browser-mac-ext-ios`.
  - Push, and open the PR to main with the evidence table and the client matrix; CI green.

---

## Dependencies & execution order

- **Phase 1** → Phase 2. T007 and T008 are independent of everything else.
- **Phase 2**:
  - Entry: T009, which blocks A-, C-, EF- and G-core (they need `C/mod.rs` settled). I18N does not touch `C/mod.rs` and may start with T009.
  - Groups: A-core, C-core, EF-core, G-core and I18N run in parallel.
  - Inside A-core:
    - `user_op` T010 → {T011, T012};
    - `rpc_pool` T013 → T014 → T015 → T180;
    - `tx_tracker` T016 → T017 → T018 → T019 (T019 also needs T010 and T180);
    - `sign_request` T020 → T021 → T022 (also needs T017) → T023 → T024 → T025;
    - `send` T026 needs T017.
  - C-core: T027 → T028.
  - EF-core: T029 → T030 → T031 → T032 → T033; T034, T035 and T036 are independent.
  - G-core: T037 → T038; T039; T040 → T041; T042.
  - Exports T044 ∥ T045 after every group (T010–T043 and T180) → bindings T046 → gates T047.
- **Desktop (Phase 3)** needs T010–T043 and T180 only (the crate, not the bindings); the rest of the client phases need T047.
- **Client phases 3–6 run in parallel**; they own disjoint trees.
  - The extension orders B before A: T076–T081 → T082 → T083 → T084 (T084 also after T089).
  - Inside each client group the "(after …)" notes are the order; they also keep two tasks off one file: the desktop `page.rs` chain and `D/main.rs` (T048 → T067); the web's `one-surface.test.ts` (T076 → T077); iOS `RelayClient.swift` / `RpcPool.swift` / `RootView.swift` and `IT/BrowserChromeTests.swift` (T111 → T114 → T116 → T118); Android `SigningLive.kt` and `AT/ExploreLiveTest.kt` (T131 → T133 → T134).
  - Each client's submit task comes before the tasks that dispatch its verdict: desktop T050 → T051/T055, web T082 → T083/T088, iOS T104 → T105/T108, Android T125 → T127/T128.
  - The persistence tasks close each client's money chain: T181 (desktop), T182 (web), T183 (iOS), T184 (Android); each client's gate task waits for it.
  - `pnpm check` is expected red between T046 and T075; `check-event-payloads.mjs` is expected red from T047 until the last client gate (T101, T123, T137) passes.
- **Signer page (Phase 7)**: T138–T140 any time; T141 after T047 and T140, then T046 again; T142 is the owner's, after T141.
- **Device (Phase 8)** per client after that client's phase and a fresh T046. The money rows (T145, T153, T161) are the MVP proof. T173 needs T142. T174 comes after all device rows.
- **Phase 9** after Phase 8. T177 needs Phases 3–6.

## Parallel groups

| Group | Files it owns (exclusively) | Can run with |
|---|---|---|
| SETUP-DOCS (T007) | `specs/082-…/contracts/core-rules.md`, `specs/082-…/data-model.md`, `specs/082-…/plan.md`, `specs/082-…/device-pass-plan.md` | everything |
| SETUP-CHAOS (T008) | `scripts/device/chaos-proxy.py` | everything |
| CORE-ENTRY (T009) | `C/mod.rs`, stubs `C/sim_outcome.rs`, `C/net_health.rs`, `C/remote_mark.rs` | SETUP-*, I18N, SIGNER-PAGE |
| A-core (T010–T026, T180) | `rust/crates/vela-core/src/user_op.rs`, `C/rpc_pool.rs`, `C/tx_tracker.rs`, `C/sign_request.rs`, `C/send.rs`, `CT/user_op_hash.rs`, `CT/user_op_submit.rs`, `CT/fixtures/userop-hash-gnosis.json`, `CT/app_rpc_pool.rs`, `CT/app_tx_tracker.rs`, `CT/app_sign_request.rs`, `CT/app_send.rs` | C-core, EF-core, G-core, I18N, SIGNER-PAGE |
| C-core (T027–T028) | `C/clear_signing.rs`, `CT/app_clear_signing.rs` | A, EF, G, I18N, SIGNER-PAGE |
| EF-core (T029–T036) | `C/browser_load.rs`, `C/net_health.rs`, `C/remote_mark.rs`, `C/balance_dashboard.rs`, `CT/app_browser_load.rs`, `CT/app_net_health.rs`, `CT/app_remote_mark.rs`, `CT/app_balance_dashboard.rs` | A, C, G, I18N, SIGNER-PAGE |
| G-core (T037–T042) | `C/activity_feed.rs`, `C/sim_outcome.rs`, `C/dapp_permissions.rs`, `C/dapp_browser.rs`, `rust/crates/vela-core/provider/inpage.js`, `CT/app_activity_feed.rs`, `CT/app_sim_outcome.rs`, `CT/app_dapp_permissions.rs`, `CT/app_dapp_browser.rs` | A, C, EF, I18N, SIGNER-PAGE |
| I18N (T043) | `L/*.json` (15 locales), `scripts/gen-i18n.mjs`, generated `rust/crates/vela-core/src/i18n/paths.rs`, `rust/crates/vela-core/src/i18n_catalogs/`, `assets/i18n/` | A, C, EF, G, SIGNER-PAGE |
| EXPORTS (T044 ∥ T045) | `rust/crates/vela-core-uniffi/src/lib.rs` ∥ `rust/crates/vela-core-wasm/src/lib.rs` | each other, DESK, SIGNER-PAGE |
| BINDINGS (T046–T047) | `rust/pkg-web/`, `W/core/generated/`, the xcframework, Android `.so` + Kotlin bindings, `rust/crates/vela-core/src/bin/generate_wallet_state_bindings.rs` | DESK, SIGNER-PAGE |
| DESK (T048–T074, T181) | `app-desktop/vela-wallet/**` (incl. `Cargo.toml`, new `D/diag.rs`, `D/executor/proxy_macos.rs`, `D/explore/probe.rs`) | EXPORTS, BINDINGS, WEB, IOS, AND, SIGNER-PAGE |
| WEB (T075–T101, T182) | `app-web/vela-wallet/**` (`src/lib/**`, `src/routes/**`, `extension/**`, `e2e/**`) except the generated dirs BINDINGS writes | DESK, IOS, AND, SIGNER-PAGE |
| IOS (T102–T123, T183) | `app-ios/**` | DESK, WEB, AND, SIGNER-PAGE |
| AND (T124–T137, T184) | `app-android/**` | DESK, WEB, IOS, SIGNER-PAGE |
| SIGNER-PAGE (T138–T140) | `app-web/trusted-signer/**` | every group |
| SIGNER-CORE (T141–T142) | `rust/crates/vela-core/src/trusted_signer/integrity.rs` (+ a BINDINGS re-run) | DESK, WEB, IOS, AND (their device builds wait for its re-run) |
| DEVICE (T143–T174) | `specs/082-…/evidence/**`, the scratchpad logs | T169 (Android) with any other device task; the fault rows run one client at a time |
| RESULTS (T175–T179) | `specs/082-…/results.md`, memory | — |

## Implementation strategy

1. **MVP: the P0 money-safety fix on the three clients the owner tested.** G21 (a landed payment
   reported "failed — try again") was measured on the desktop, in Chrome and on the iPhone, and
   G17/G19 are the extension's own P0s. The MVP is:
   - Phase 2's A-core (T009–T026, T180), the feed rows of G-core (T037, because the money rows
     expect the pending `dApp 交易 · 处理中` row, RG4), plus I18N (T043, which carries `maybeSent`)
     plus T044–T047. For an MVP cut, T044–T046 may export only that surface (`userOp*`,
     `parseUserOpStatus`, `signEnding*`, `dappReceiptWaitMs`, `signRequestTtlMs`, `rpcReadTimeoutMs`,
     `rpcCooldownMs`) and run again when the rest of Phase 2 lands; in practice those groups run
     in parallel and are usually done first.
   - Desktop: T048–T055, T072 and T181.
   - Extension: T075–T089, T092 and T182, with B's lifecycle (T076–T081) before A's answer (T082–T083) and RF3 (T084, which also needs T089).
   - iOS: T102–T108, T121 and T183.
   - Android: T124 (the wire mirrors) at least. The new tracker and sign variants are not additive
     for the hand-written Kotlin wires, so an MVP that ships without T124 breaks Android's whole
     view; T125–T128 and T184 follow as soon as possible.
   - Proof: rows DX-W1, S5, DX6/DX7 and the relaunch check (T145), EX5, EX8, EX-W1, EX-S5 and the
     reload check (T152–T153), and IX-W1, IX8, S5 and the relaunch check (T161).
   Stop and demo when those rows pass: no "try again" for a landed op, one answer per request, the
   op tracked to its end (relay or chain log) across a restart, and no signature for a page that left.
2. Next, **page loads and the address bar** (US1): EF-core, then desktop
   T058–T063/T068, iOS T111–T114/T122, Android T131/T134. Proof: rows T144 and T160.
3. Then the **079 leftovers and honesty** (US3–US6, US4 plain send): C-core and G-core, then each
   client's feed, sim, plain-send and spelling tasks. Proof: rows T147–T150, T155–T157, T163–T166.
4. Then **polish and logs** (US2, US8): consent, header, tabs, G16, the worker and iOS logs, the
   bug-report counters. Proof: rows T146, T154, T158, T162, T167.
5. Android parity (Phase 6) follows each shared rule as it lands, gated by its JVM suite.
6. The signer page (Phase 7) runs alongside. Its release waits for the owner's deploy (T142).
7. Last come the owner's passkey batch (T170–T173), the close-out (T174), the results and the PR.

## Coverage (research id → tasks)

| Research | Tasks |
|---|---|
| RA1 | T012, T013, T050, T082, T104, T125 |
| RA2 | T051, T083, T105, T127 |
| RA3 | T020, T181, T182, T183, T184 |
| RA4 | T017, T026, T055, T088, T108, T128, T181–T184 |
| RA5 | T050, T082, T104 (unchanged getNonce) |
| RA6 | T010, T011, T044, T045 |
| RA7 | T016, T052, T085, T106, T126 |
| RA8 | T022, T023, T054, T083, T087, T105, T127 |
| RA9 | T021, T051, T054, T083, T087, T105, T127 |
| RA10 | T012, T050, T054, T055, T082, T087, T088, T104, T105, T108, T125, T127, T128 |
| RA11 | T043 |
| RA12 | T024, T051, T082, T105, T127 |
| Ruling 1 | T012, T017, T026, T050–T055, T082–T088, T104–T108, T125–T128, T181–T184 |
| Ruling 8 | T007, T019, T180 (range errors reach the tracker), T050, T053, T082, T086, T104, T107, T125, T128, T181–T184 (flag and `submit_block` survive a restart) |
| Ruling 9 | T023, T051, T083, T105, T127 |
| Ruling 10 | T043, T060 |
| RB1–RB11 | T025, T076, T078, T079, T080, T081 |
| RB12 | T097 |
| RB13 | T093 |
| RB14 | T077, T098 |
| RB15 | T080 |
| RC1–RC7 | T027, T028, T056, T090, T109, T129 |
| RC8 | T139 |
| RD1–RD14 | T058–T069, T073 |
| RD15 | T175 (deferred) |
| RE1–RE2 | T030–T033, T062, T111, T112, T131 |
| RE3 | T031, T034, T069, T113, T132 |
| RE4 | T032, T112 |
| RE5 | T112, T131 |
| RE6 | T116, T133 |
| RE7 | T033, T070, T091, T117, T118, T133 |
| RE8 | T018, T053, T086, T107, T128 |
| RE9 | T036, T071, T095, T119, T135 |
| RE10 | T035, T069, T096, T120, T132 |
| RE11 | T102 |
| RE12 | T118, T133 |
| RE13 | T117, T133 |
| RE14 | T160–T168 |
| RF1 | T014, T068, T114, T134 |
| RF2 | T014, T045, T089 |
| RF3 | T084 |
| RF4 | T068, T114 |
| RF5 | T115 |
| RF6 | T175 |
| RG1–RG4 | T037, T072, T092, T121, T136 |
| RG5 | T038, T073, T093, T121, T136 |
| RG6, RG8 | T039, T057, T110, T130 |
| RG7 | T015 |
| RG9, RG14 | T043 |
| RG10 | T040, T041, T042, T094, T121 |
| RG11 | T138, T140, T141, T142 |
| RG12 | T099 |
| RG13 | T145, T176 |
| RG15, RH1 | T143 |
| RH1 (chaos header), RH2 | T008 |
| RH3 | T169 |
| RH4 | T122, T168 |
| RH5 | T100 |
| RH6 | T143–T173 |
| RH7 | T174 |
| RI1–RI3 | T043 |
