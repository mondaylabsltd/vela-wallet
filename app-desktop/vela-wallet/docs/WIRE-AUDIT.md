# Desktop wire audit — constant answers and undispatched events (F0.3)

Base `feat/multisig-account` @ `811aa55c3`, 2026-10-30. Reference: the web shell
(`app-web/vela-wallet/src/lib`). Paths are relative to `app-desktop/vela-wallet/src`
unless they start with `app-web/` or `rust/`.

Two questions, asked of every core machine the desktop drives:

1. **Constant answers.** Where does the desktop answer a core operation with a
   hardcoded value (`false`, `None`, `vec![]`, a fixed literal, a stub) where the
   web computes it from real data?
2. **Undispatched events.** Which core `Event` variants does the web dispatch that
   the desktop never does?

Status column: **fixed** (this branch, `nat-desktop-0`), **open** (still a gap,
with what it needs), **by design** (the web answers the same, or the desktop has
no such surface).

## 1. Constant answers

| # | Desktop (at base) | Operation → field | Desktop answered | Web computes | Status |
|---|---|---|---|---|---|
| A1 | `executor/send.rs` `recipient_risk` | `ResolveRisk` → `history_unread`, `known_name`, `own` | `false`, `None`, `false`; an unreadable history read as "never sent" → *first transfer* | `recipient-risk.ts` + `knownName`/`ownAddress` ports (`routes/[locale]/wallet/+page.svelte:882-901`) | **fixed** — own accounts are `own` (no first-transfer cue), `known_name` = own account / named contact, unreadable history = `history_unread` + `first_time: None`; `eth_getCode` bounded at 3 s and cached. Multisig members: open (multisig) |
| A2 | `executor/send.rs` `ResolveIdentity` | `IdentityResolved` | skipped the person's contact names | `send-executor.ts:826-839` (own names first, `self`) | **fixed** |
| A3 | `executor/relay.rs` `parse_quote_row` | `InBandQuotes` → `native_usd_floor_price` | `None` | `fee-executor.ts:128-136,206` (`getCachedNativePriceUsd`) | **fixed** — added per answer in `executor/fee.rs` from the last settled balances |
| A4 | `executor/network_admin.rs` `read_store` | `StoreLoaded` → `shown_networks` | read the record, never settled one | `services/shown-networks.ts:127-140` | **fixed** — `settle_shown_networks` at launch and in `read_store` |
| A5 | `executor/tracker.rs` `ReadNonce` | `NonceRead` → `nonce`, `head_block` | `None`, `None` | `tracker-executor.ts:214-233` | **fixed** — `EntryPoint.getNonce` at the head (`chain::nonce_at`) |
| A6 | `executor/user_op.rs` `SubmitFailure` → `SendSubmitFailure` | `SubmitFailed` | never `ChainUnreachable{busy}`, `PasskeyTimedOut`, `NonceUsed` | `send-executor.ts:914-976` + `prepare-bound.ts` | **partly fixed** — `ChainUnreachable{busy}` via `executor/prepare_bound.rs`. Open: `PasskeyTimedOut` (the ceremony has no own deadline signal yet), `NonceUsed` (the relay's bare AA25 is not classified apart from `Refused`) |
| A7 | `executor/send.rs` `SimulateCalls` | `SimResolved` → `sim_json` | `None` ("no simulation engine") | `simulateAssetChanges` (`send-executor.ts:859-871`) | **open** — the desktop has an engine (`executor/sim.rs`, used by dApp signing); wire it into Send |
| A8 | `executor/balance_dashboard.rs` `settled_result` | `FetchSettled` → `own_node_chain_ids`, `fallback_chain_ids` | `vec![]`, `vec![]` | `balance-executor.ts:159-160` from the pool view | **open** — `executor/pool.rs` has no accessor for which chains answered from the person's own node / fell back; needs one (the core's `rpc_pool` view holds it) |
| A9 | `executor/sign_request.rs` `submit_failure` | `SignSubmitOutcome::Underfunded.funding` | `None` | `sign-executor.ts:639-668` | **open** |
| A10 | `executor/sign_request.rs` `submit_failure` | `SignSubmitOutcome` | never `PasskeyTimedOut` / `PageDeclined` | `sign-executor.ts:615-619` | **open** (round 5/6 signing) |
| A11 | `wallet/signing_host.rs` `clear_kickoff` | `ResolveTransaction/Batch/TypedData` → `from`, `probe_code` | `None`, `false` | `signing/core/sheet.svelte.ts:107,120-121` | **open** — then A12 needs a real answer |
| A12 | `executor/clear_signing.rs` | `RpcGetCode` → `result` | `None` | `clear-executor.ts:121-138` | **open** (unreached until A11) |
| A13 | `wallet/money.rs` `add_network_settled` | `NetworkAdded` | `CheckFailed` → `NotCompatible` | `send-executor.ts:437-441` (`unverified` → `error`) | **open** |
| A14 | `executor/contacts.rs` `read_send_history` | `HistoryLoaded` | unreadable → `[]` | `history_failed` (`contacts-executor.ts:303`) | **open** |
| A15 | `executor/contacts.rs` `read_list` | `StoreLoaded` | unreadable → empty book | `store_unreadable` (`contacts-executor.ts:295`) | **open** |
| A16 | `executor/contacts.rs` `write_list` | `Written` | always, error dropped | `write_failed` (`contacts-executor.ts:296-301`) | **open** |
| A17 | `executor/token_trust.rs:578-588` | `ReadCustomTokens` | always `Some` | `null` on a failed read (`token-trust-executor.ts:285-289`) | **open** — fails open |
| A18 | `executor/send.rs` `ClearTokenCache` | `TokenCacheCleared` | no-op | clears + forces (`send-executor.ts:411-415`) | **open** (low) |
| A19 | `executor/token_trust.rs:597` | `InvalidateTokenCache` | ack only | `clearTokenCache(address)` | **open** (low) |
| A20 | `executor/relay.rs` `read_in_band_quotes` | `InBandQuotes` | drops every stablecoin row when the native row is unpriced | hands every row over UNFILTERED (`fee-executor.ts:70-77`) | **open** — the core's `picker_rows` decides; the shell filter predates it |
| A21 | `executor/send.rs` `ProposeSafeTx` | `ProposeFailed` | always | `proposeTransfers` | **by design until multisig lands** (F0.4) — kept honest |
| A22 | `executor/network_admin.rs` `read_store` | `multisig_service`, `genesis_service_url` | `false`, `None` | `true`, stored URL | **by design until multisig lands** |
| — | `relay.rs` `operator_served: false`, `coin: None`; `network_admin.rs` `vela: false`; `signing_host.rs` `bundler_cost_wei`/`fee_collector: None`, `dapp`/`requested_address`/`request_ts_ms: None`; `balances.rs` `spam: false` | | | the web hardcodes the same | **by design** |
| — | contacts `note: None` (`contacts/live.rs:274`) | | | | not a finding: inside `#[cfg(test)]`; the live path keeps the stored note |

## 2. Events the web dispatches and the desktop did not

Found by diffing every `pub enum Event` variant in `rust/crates/vela-core/src/app/*.rs`
against `type: '<snake>'` in the web's non-test sources and `Event::<Variant>` in the
desktop's non-test, non-fixture sources.

| Machine | Event | Status on this branch |
|---|---|---|
| send | `SubmitSlow` | **fixed** — `executor/prepare_bound.rs` watch inside `SubmitUserOp` |
| send | `ChainAnswered` | **fixed** — `SendHost::watch_recovery` (5 s / 4 s read) |
| send | `RetryRiskCheck` | **fixed** — the confirm's "Check again" |
| send | `SendAgain` | **fixed** — the receipt's "Send again" / "Review again" |
| send | `SendQueued` | by design — one window, no cross-tab send lock |
| send | `RetryAfterError` | **open** — the desktop's error notice offers no "Try again" of its own yet |
| balance_dashboard | `HoldingsRestored` | **fixed** — `vela.holdingsCache` per account + space |
| balance_dashboard | `AccountNoted` | **fixed** — opened (or rebuilt) on Settings / Contacts |
| balance_dashboard | `SpaceChanged` | **fixed** — a space switch (was a bare forced refresh) |
| tx_tracker | `SubmitFacts` | **fixed** — the signed nonce of a may-have-been-sent op |
| session | `SyncPending` | **fixed** — the web's widening retry timer. Open: the home's "not yet findable — Try now" line |
| sign_request | `LandingClosed` | **fixed** |
| sign_request | `DismissTapped` | **open** |
| approval_guard | `GrantDeliberatelyChosen`, `LegGrantDeliberatelyChosen` | **fixed** — the cs-17 grant-all card (Allow all / Revoke) |
| network_admin | `OnlyMineChanged` (from a screen) | **fixed** — Settings → network → "Only use my node" |
| network_admin | `DappAddApprovedUnverified` | **open** — the in-app browser's add-network sheet |
| fee_policy | `LeaveConfirm`, `ChainChanged` | **by design** — the web defines `leaveConfirm()`/`chainChanged()` (`flows/core/fee-quote.svelte.ts:463-470`) but nothing calls them |
| contacts | `HistoryChanged`, `StoreChanged`, `UseSharedStore` | by design — multi-document sync on the web; one window here |
| dapp_permissions | `PopupRequest`, `PopupApproved`, `PopupAccountSwitch` | by design — the extension's popup |
| create_wallet | `CancelPending` | **open** |
| payment_request | `LinkOpened` | **open** |
| send | `SlideConfirm` | by design — the person's confirm goes through the intents (`submit_transfer`, spec 103) |

## 3. Core changes these need (not made here — CORE lane)

- **rpc_pool**: an own-node / fallback accessor for a shell that runs the pool
  in-process (A8), or the pool view's `own_node_chain_ids`/`fallback_chain_ids`
  exported as a pure function of the pool state.
- **user_op / relay**: a `RelayRejection::NonceUsed` (bare AA25) so a shell can
  answer `SendSubmitFailure::NonceUsed` (A6) — today only the web's TS classifier
  knows it.
- **send**: `SendRecipientRisk.own`/`known_name` for multisig members arrive with
  F0.4 (the members list is web-only state).
- None of the fixed rows needed a core change.
