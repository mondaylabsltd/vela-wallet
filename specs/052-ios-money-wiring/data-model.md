# Data model: what 052 stores, and in whose bytes

The naming rule is 050's (`specs/050-ios-live-shell/data-model.md` §1): stored
JSON under a `vela.*` key is **camelCase with capitalised initialisms**
(`rpcURL`, `userOpHash`, `chainId`) — the retired Expo client's spelling, kept
by every client since. A record written on the web reads on the phone.

## Keys this cut writes

| Key | Owner | Shape | New? |
|---|---|---|---|
| `vela.transactionHistory` | `activity_feed` reads it, `send` and `tx_tracker` write it | a list, newest first, capped at **200** | key existing (051 reads it); **this cut is its first writer** |
| `vela.accounts` | onboarding | the account list, records kept **whole** | existing; the parallel space **upserts one record** and removes it on exit (FR-003) |
| `vela.activeAccountIndex` | onboarding | an integer | existing; set on enter, reset to 0 on exit |
| `vela.parallelSpace` | the door | `"1"` or absent | **new, Debug only** |
| `vela.parallelSigner` | the door | the fixture index, `"0"` if absent | **new, Debug only** |

**No `vela.tracker`.** It exists on no client. Android's string of that name is
a WorkManager task identifier, not a storage key. Pending state is derived from
`vela.transactionHistory`, which is what `load_pending_txs` documents.

## The transaction record

One shape, four clients. `TxRecords.toWire` (051) already reads it; this cut
adds the writer.

| Field | Type | Notes |
|---|---|---|
| `id` | string | the core mints it; the patch targets it |
| `userOpHash` | string | the tracker's key |
| `txHash` | string | **empty until confirmed** — that emptiness is half of `load_pending_txs`' filter |
| `from`, `to` | string | addresses as stored |
| `toName` | string \| null | the resolved name at the time of sending, never re-resolved later |
| `value` | string | a **human decimal string**, never raw units (051's unit trap) |
| `symbol` | string | |
| `decimals` | number | |
| `logoUrls` | [string] \| null | |
| `chainId` | number | |
| `timestamp` | number | **unix seconds**, not milliseconds — `dayStartMs` multiplies |
| `status` | `"pending"` \| `"confirmed"` \| `"failed"` | |
| `type` | `"send"` here; `"dapp_tx"` in 053 | an unknown value is **dropped, never guessed** |
| `usd` | string \| null | **the valuation at the time of the transfer**, frozen. Re-pricing a year-old transfer at today's rate would quietly restate history (051 phase 8) |

### Ordering, which is an invariant rather than a preference

`persist_tx_records` completes **before** `track_submitted` is emitted. The
core enforces the order; the shell must not race it by writing asynchronously.
A process death between the two leaves a row with no tracker — recoverable on
the next launch. The reverse leaves a tracker with no row — a patch that lands
on nothing, and a send the person cannot see.

## The parallel-space account record

Built from the fixtures library, written through `AccountStore.saveAccount`
(an upsert by `id`):

| Field | Value |
|---|---|
| `id` | `fixtureAccounts()[0].credentialIdHex` |
| `name` | the fixture's own name |
| `address` | `fixtureMultiAddress()` — `0x88cCA0EeDbF2C4426110bbFc998F048689266894` |
| `publicKeyHex` | key 0's |
| `createdAtISO` | now |
| `keys` | every fixture account as `{credentialId, publicKeyHex, transports: "internal"}` |

**The address is a function of every key**, which is why the whole set is
written rather than key 0 alone — the lesson from the multi-passkey work, where
a field-by-field account mapping dropped `keys` and silently derived a
different Safe.

## What the fee quote carries between screens

Not stored. It lives in the `send` model and travels to `submit_user_op` as
`quoted_fee {amount, recipient}`, which `quotedFeeUsable` validates. Persisting
it would create a second copy of the number the whole "displayed equals signed"
rule exists to keep singular.

## Storage a Release build must not contain

`vela.parallelSpace` and `vela.parallelSigner` are read only by code excluded
from the Release configuration, so a Release binary contains neither the keys
nor the reader. That is checked by symbol inspection (SC-010), not by reading
the source.
