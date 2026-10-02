# Implementation Plan: 093 — every dApp interaction in Activity

**Branch**: `093-dapp-activity` | **Date**: 2026-10-02 | **Spec**: [spec.md](spec.md) |
**Tasks**: [tasks.md](tasks.md) | **Results**: [results.md](results.md)

## Summary

The core already writes a `SignRecord` per approved dApp request and folds `dapp_tx` into Activity, but
dropped signatures, never knew what a request was beyond a recorded intent, and left four shells to
word the second line, guess titles and cut the stored request their own way. 093 moves every rule into
the core: a `DappSummary` read once at approve time; the feed deciding each dApp row's verb, place,
subtitle, allowance, detail facts and technical lines; a swap's receipt folding into its row; one
8 KB stored-request cut. Shells copy, store, map back and draw.

## Technical Context

- Core: `rust/crates/vela-core/src/app/` — new `dapp_activity.rs` (summary, protocol lookup, stored
  request); `sign_request.rs` (summary + stored request on the record, empty signature result,
  `SignApproveOpts.token_meta`); `clear_signing.rs` (`ClearSigningView.record_intent`, batch headline,
  seven headline `ClearTerm`s); `activity_feed.rs` (signature rows, `FeedLine` subtitle, `FeedDapp`
  place / allowance / facts / technical, receipt folding).
- Bridges are JSON (wasm `wallet_state`, uniffi `onboarding_bridge`): new fields ride the existing
  machines; no new uniffi exports. TS mirrors regenerate (`gen-core-types`).
- i18n: `history.dappRowTitle` only. Budget: `SC005_BUDGET` 138,800 → 140,800 (owner, 2026-10-02).

## Core design

| Piece | Decision |
|---|---|
| `DappSummary` | `action` ∈ Call · Batch · Approve · Permit · SignIn · Message · TypedData · BlindSign; `calls`; `contract`; `spender`; `token`, `symbol`, `decimals`; `amount` (raw) or `unlimited`; `revoke`; `expires_at` (epoch s, `None` = none readable / never); `signin_domain` (only when the SIWE domain is the origin's host); `primary_type` (`[A-Za-z0-9_]`, ≤ 32). Built by `summarize(method, params, origin, token_meta)` with `approval_guard::detect_approval` / `detect_calldata_approval`, `clear_signing::analyze_message`, `typed_data_request::canonical`. `decreaseAllowance` is a Call. |
| Place | `protocol_of(addr)` = the built-in table's owner, except Permit2, WETH, Coinbase smart wallet. Grants: the spender's; batch: the first known call target, else the spender's; call / typed data: the contract's; else the site's host. |
| Verb | Call/Batch/Approve: the recorded intent (term or text), else `intentContractCall` / `batchIntent` / `intentApprove` (`intentRevoke` for a revoke). Signatures by what they are: `permitIntent`, `signInIntent`, `messageIntent`, `typedDataIntent`, `ethSignIntent` — never a descriptor's verb. |
| `record_intent` | ClearSign result's intent unless best effort (and not built-in); "Send" for a plain send; batch = the one verb shared after setting aside Approve / Approve NFT / Approve all NFTs / Authorize spending (all approvals → that verb); else `None`. |
| Subtitle (`FeedItem.subtitle`) | `[status ≠ confirmed, never on a signature] + dApp: [site if ≠ place] + [network]`; transfer: `[To/From name-or-address]` else `[network]`. |
| Right column | money (`value` …, as before); else a grant's `allowance` (value + symbol, or unlimited → danger); signatures carry no money. |
| Detail | `facts` (≤ 6): site, network, recipient (who got the money — a plain send's or a transfer's, with the row's name) or contract or spender, spending cap, expires (grants; not a revoke), balance changes, date. `technical`: operation, content, primary type, tx hash, UserOp hash. `off_chain` replaces the status chip with `connect.detail.offChainNote`. |
| Leniency | `FeedTxRecord.summary` / `.balance_changes` are read leniently: what this build cannot read is `None` for that record, never a feed that fails to load. |
| Receipts | A Receive of this account whose tx hash is a non-failed dApp row's real tx hash is folded; exactly one sets `received` (exact). The record stays in `transactions`. |
| Stored request | `stored_request(params_json)`: verbatim ≤ 8,192 bytes; else string values clipped (6,144 → 2,048 → 512 → 0 bytes, char boundary) until it fits; else `""` (nothing kept). |
| Signature result | `SignRecord.result = ""` for `SignMessage` / `SignTypedData` (the page still gets the signature). |

## Shell contract (every shell; no rule in a shell)

**Approve** (`SignApproveOpts`): `intent = clearView.record_intent` (delete any shell rule — desktop's
`recorded_intent`); `token_meta = guardView.meta` whenever a guard view exists; `balance_changes` = the
sheet's own simulation judgments when the shell runs one (unchanged where it already does).

**Persist** (`PersistRecord`): store `record.stored_request` as the request text (web: parse it into
`signedRequest.params`) and `record.request_truncated` — delete the shell's own cut; store
`record.summary` verbatim as `dappSummary` (JSON object); store `record.balance_changes` verbatim as
`balanceChanges` where the shell has no stored form yet (desktop keeps `assetChanges`); `txHash =
record.result` (empty for a signature).

**Read** (`ReadTxStore` → `FeedTxRecord`): for `dapp_tx`, `sign_message`, `sign_typed_data` map
`dapp_url` (iOS / Android: `dappUrl ?? dappOrigin` for old records; web: `dappUrl` only), `intent`,
`summary` (= `dappSummary` verbatim), `balance_changes`, and `call_data` (dApp tx, as today).

**Row**: a row with `item.dapp` is a dApp row (signatures too). Verb = `componentsUi.signing.<intent_term>`
?? `dapp.intent`; title = `place ? history.dappRowTitle{intent, place} : verb`. Subtitle = `item.subtitle`
joined " · " (status → the shell's existing status words; to/from → `history.toName/fromName` with
`name ?? short(address)`; site verbatim; network → chain name). Figure: money as today, else the
allowance (`unlimited` → `componentsUi.signingApprove.unlimitedValue` [+ symbol], danger tone; masked
when hidden unless unlimited).

**Detail**: header as the row; no status chip when `dapp.off_chain` (show `connect.detail.offChainNote`);
facts from `dapp.facts` in order — labels: site `connect.detail.labelApp`, network
`componentsTx.detail.labelChain`, recipient `componentsTx.detail.to`, contract `tokenDetail.labelContract`
(a noun — the 083 F3 review), spender
`componentsUi.signing.labelSpender`, spending cap `componentsUi.signingApprove.spendingCap`, expires
`componentsUi.signingApprove.expiresLabel` (`noExpiry` for none), balance changes
`componentsUi.signing.balanceChangesTitle`, date `componentsTx.detail.labelDate`; explorer link when
`tx_hash`. "Technical details" (`componentsUi.signing.advancedToggle`), collapsed: operation
`componentsTx.detail.labelOperation` (`opContractInteraction` / `componentsUi.signing.batchSubtitle{count}` /
`opSignature` / `opTypedDataSignature`), content `connect.detail.content{CallData,TypedData,Message}` with
the stored request read only when opened (`contentMissing` when none or `""`), type `componentsUi.signing.typeLabel`,
hash `componentsTx.detail.labelHash`, UserOp hash `componentsTx.receipt.userOpHash`.

## Shells

| Shell | Files (entry points) |
|---|---|
| Web + extension | `src/lib/signing/SigningHost.svelte`, `signing/core/sign-executor.ts`, `services/dapp-history.ts`, `wallet/core/feed-executor.ts`, `wallet/live.ts`, `wallet/live-detail.ts`, `flows/screens/TxDetail.svelte`, `wallet/messages.ts`, `i18n/engine.server.ts`, `signing/terms.ts` |
| Desktop | `src/wallet/signing_host.rs`, `src/executor/sign_request.rs`, `src/executor/activity_feed.rs`, `src/wallet/live.rs`, `src/flows/live.rs` |
| iOS | `Features/Signing/SigningController.swift`, `Signing/Core/SignExecutor.swift`, `Core/TxRecords.swift`, `Features/Wallet/ActivityWire.swift`, `Features/Wallet/WalletLive.swift`, `Features/Flows/FlowsLive.swift` |
| Android | `feature/signing/core/SignWire.kt`, `SigningController.kt`, `SignExecutor.kt`, `feature/wallet/core/FeedWire.kt`, `FeedExecutor.kt`, `feature/wallet/WalletLive.kt`, flows detail |

## Risks / merge notes

- `scripts/gen-i18n.mjs` path count, the i18n catalogs, `assets/wasm/*.wasm`, `rust/pkg-web/*` and the
  TS mirrors conflict with 090–092 (each adds keys / regenerates); regenerate after merging.
- History cap 200 is shared by sends and (now) signatures on web / Android / iOS — not changed here.
