# Data Model: Android Money Wiring

Core-owned state is not restated; the shapes below are what the SHELL holds
or persists, and where each mirrors the core's type.

## Send attempt (Kotlin `SendController`)

| Field | Type | Notes |
| --- | --- | --- |
| `sendHost` | `CoreHost<SendView>` | one per attempt lifetime; `Open` on 发送, `Close` on Done/Back-out |
| `feeHost` | `CoreHost<FeeView>` | the quote session for this attempt (research D7) |
| `mtokHost` | `CoreHost<MtokView>` | AddToken sheet |
| `generation` | `Long` | the core's single-flight lock is authoritative; the shell's counter only drops stale executor answers |
| `signing` | `Job?` | the assertion in flight; `CancelPasskeySign` cancels it |

State transitions are the core's (`send.rs` module doc): Open → SelectToken →
EnterDetails → Continue{estimate ∥ treasury} → Confirm → sign → Submitted →
Persist → TrackSubmitted → Done. The shell holds no step of its own.

## Fee quote (mirror of `FeeEstimateView`, core `fee_policy.rs`)

Rendered on confirm and handed back unchanged to `SubmitUserOp.quoted_fee`
(`SendQuotedFee { amount, recipient }`) — the "displayed = signed" gate. Fee
asset rows (`FeeAssetView`) feed the fee-token sheet: symbol, balance,
decimals, usd, selected.

## Pending record (persisted, `vela.transactionHistory`)

The feed's own camelCase row (`FeedExecutor.kt:135–172`):

| Key | Type | Written at submit | Patched by tracker |
| --- | --- | --- | --- |
| `id` | string | record id from `SendTxRecord` | — |
| `userOpHash` | string | yes | — |
| `txHash` | string? | null | `TrackRecordPatch.tx_hash` |
| `from`, `to`, `toName` | string | yes | — |
| `value`, `symbol`, `decimals`, `logoUrls`, `chainId`, `usd` | as feed | yes | — |
| `timestamp` | ms | submit time | — |
| `status` | `pending` (absent) / `confirmed` / `failed` | absent | `TrackRecordPatch.status` |
| `type` | `send` | yes | — |

Invariant: written and acknowledged (`RecordsPersisted`) before the core
emits `TrackSubmitted`. `TrackPendingRecord` for `LoadPendingTxs` is derived
from rows with no terminal status and `type ∈ {send, dapp_tx}`.

## Custom token (mirror of `MtokCustomToken`, persisted `vela.customTokens`)

Replace-by-id on write; `InvalidateTokenCache` → `BalanceDashboard` refresh.

## Parallel space (debug source set only)

| Item | Where | Notes |
| --- | --- | --- |
| door | intent extra `vela.parallelSpace` + store key `vela.parallelSpace` | persisted so a relaunch stays inside; cleared by sign-out |
| preferred signer | store key `vela.parallel.signWith` | index into the fixture accounts (web's `vela.parallel.signWith(n)`) |
| badge | every screen | answers to the flag, not to the build type |
| fixture account | from the fixtures cdylib | index, credential id, P-256 public key, derived Safe address — same as web/desktop |

## Notification

Channel `transactions`; one notification per confirmed/failed hash; tap
opens the wallet with the flow stack seeded to that receipt.
