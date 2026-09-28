# Data model — spec 082

The state that changes in 082. The *rules* that move it are the core's
([contracts/core-rules.md](contracts/core-rules.md)); where a piece of state lives in a shell or in
the extension's worker, it says so. Ids in brackets point to [research.md](research.md).

## 1. Submit verdict (one userOp submit) — [RA1, RA2, RA5, RA6]

Held by the submitting executor for the length of one submit; decided by `user_op::submit_step`.

| Field | Meaning |
|---|---|
| `local_user_op_hash` | `user_op_hash(op, chain_id)`, computed before the first POST |
| `attempt` | 0-based POST count for this op (busy retries re-POST the identical op) |
| `maybe_delivered` | sticky OR over every POST of this op of `may_have_delivered(transport outcome)` |
| `submit_block` | the chain head from one `eth_blockNumber` before the first POST (best effort; `None` if it failed) |

| Transport outcome of one POST | `may_have_delivered` |
|---|---|
| `NotConnected` (DNS, refused, TLS handshake, proxy CONNECT failure, connect/resolve timeout) | false |
| any JSON answer (result or error) | false |
| HTTP 4xx | false |
| `Timeout`, `Network` (reset after write), `NonJson`, HTTP 5xx | **true** |

Web note: `fetch` cannot tell a refusal from a lost reply, so a thrown fetch is `Network` (true)
unless `navigator.onLine === false` before the call.

```
POST ─► reply ─► submit_step(reply, attempt, maybe_delivered, local)
  result hash                         ─► Accepted{relay hash}        (mismatch → log userop.hash_mismatch)
  error with [existingHash:0x…]       ─► Accepted{that hash}
  "currently processing"/"Retry later", attempt < 3 ─► RetryAfter 3000 ms ─► POST (same op)
  error | pool exhausted, maybe_delivered   ─► MaybeSent{local}
  error, ¬maybe_delivered             ─► NotSent{Some(relay rejection)}
  pool exhausted, ¬maybe_delivered    ─► NotSent{None}          ("definitely not sent")
```

| Verdict | Local nonce | `OpSubmitted` | Record | dApp answer | Sheet |
|---|---|---|---|---|---|
| `Accepted` | bumped (desktop/web cache) | yes | pending, tracked | tx hash (receipt in window) or op hash | 079 states (waiting → landed / still confirming) |
| `MaybeSent` | **not** bumped | yes, `maybe_sent: true`, local hash | pending, `maybe_sent`, tracked | tx hash if a receipt arrives in the window, else the local op hash (`ReceiptPending`) — never 4900/-32603 | title "Submitting…", caption `maybeSent`, op hash, "Close · keep running", no Retry |
| `NotSent{rejection}` ("definitely not sent") | not bumped | no | none | -32603: the relay's rejection, or the fixed "relay unreachable; nothing was sent" (`NOT_SENT_DAPP_DETAIL`) | cross, `statusFailed` + `txErrorGeneric` (or the rejection's reason) |

**What the record keeps.** A pending record written for an `Accepted` or `MaybeSent` op carries
`maybe_sent` and `submit_block` (from `SignRecord` / `SendTxRecord`), and every shell persists both
with the stored row: desktop T181, web and extension panel T182, iOS T183, Android T184. On a
relaunch the shell's pending-record loader puts them back into `TrackPendingRecord`, and the
hand-off into the tracker's `Submitted` carries both, so a may-have-been-sent op keeps its
`MaybeSent` outcome, its `NotSent` end and its find-event (§2) across a restart. A row stored
before 082 reads back as `false` / `None`.

## 2. Tracked operation (a `tx_tracker` entry) — [RA4, RA7, RE8, ruling 8]

| Field | Meaning |
|---|---|
| `user_op_hash`, `chain_id`, `record_ids`, `submitted_at_ms` | as in 079 |
| `maybe_sent` (new) | the submit ended MaybeSent; restored from `TrackPendingRecord.maybe_sent` on reload |
| `acknowledged` (new) | any receipt, or any relay status other than `not_found`, has been seen |
| `not_found_streak` (new) | consecutive `not_found` answers at age ≥ 60 s with no receipt |
| `submit_block` (new) | the head read before the first POST; where the find-event scan starts. Restored from `TrackPendingRecord.submit_block`; `None` = unknown |
| find-event window (new) | the next `from_block` and the current window width (≤ `FIND_OP_MAX_RANGE`, halved on a range error) |
| `status` | `Pending` → terminal `Confirmed` / `Dropped` (reverted, incl. Safe `ExecutionFailure`) / `Rejected` / **`NotSent`** (new) |

`TrackEntryView.outcome`:

| Condition (first match) | Outcome |
|---|---|
| terminal | `Final` |
| abandoned (24 h) | `Unknown` |
| `maybe_sent ∧ ¬acknowledged` | **`MaybeSent`** (new) |
| in the 120 s window | `Landing` |
| otherwise | `StillConfirming` |

```
Submitted{maybe_sent} ─► Pending
Pending ──status ≠ not_found──► acknowledged = true, streak = 0
Pending ──not_found, age < 60 s──► (ignored)
Pending ──not_found, age ≥ 60 s──► streak += 1 ──streak = 2 ∧ no receipt──► NotSent (records → failed)
Pending ──receipt success──► Confirmed            (+ NotifyConfirmed + HoldingsMoved)
Pending ──receipt reverted──► Dropped             (records → failed, + HoldingsMoved)
Pending ──status rejected──► Rejected             (records → failed)
Pending ──StatusUnavailable──► (nothing changes)
Pending ──age 24 h──► abandoned (outcome Unknown; records untouched)
```

**Find-event** (ruling 8, T019): the relay-independent landing check, for an entry with
`maybe_sent ∧ ¬acknowledged ∧ ¬terminal ∧ ¬abandoned`, on the status-poll cadence:

```
submit_block unknown ──► FindOpEvent{from_block: None} (head only) ──OpEvent{head}──► from = head − FIND_OP_LOOKBACK_BLOCKS
tick ──► FindOpEvent{from, to = min(from + width − 1, head)}
  OpEvent logs: the op's UserOperationEvent, success ──► Confirmed   (log's tx hash; records → confirmed, + NotifyConfirmed + HoldingsMoved)
  OpEvent logs: the op's UserOperationEvent, failure ──► Dropped     (reverted; log's tx hash; records → failed, + HoldingsMoved)
  OpEvent logs: none                                 ──► from = to + 1 (caught up with the head → wait for the next tick)
  OpEvent error, rpc_pool::is_log_range_error        ──► same from, width halved (down to 1 block)
  OpEvent error (other) / no answer                  ──► same window next tick
Confirmed / Dropped by a found event ──later relay not_found──► (ignored: the found event wins)
```

The shell only runs `eth_getLogs` (EntryPoint address, `topics: [USER_OPERATION_EVENT_TOPIC,
user_op_hash]`) and `eth_blockNumber` through the pool and answers what came back; the core judges
the range error (FR-020). The pool answers a range error on `eth_getLogs` instead of banning the
endpoint or classifying the chain (T180), so it reaches this machine.

Polling: status polls (and the find-event) continue past the 120 s window while
`maybe_sent ∧ ¬acknowledged`, at `receipt_interval_ms(false, age)`; a plain entry's `not_found`
stays inert (079 unchanged). Status comes from `pimlico_getUserOperationStatus` only
(`USER_OP_STATUS_METHOD`); an unknown status string parses to nothing. `TrackShellResult::Status`
may carry the relay's `tx_hash`.

## 3. Signing request presentation — [RA3, RA8, RA9, RB2, RC1]

**Phase** (`SignView.phase`, from the inflight stage and the new `ceremony` field):

| Inflight stage | `ceremony` | `phase` | Words |
|---|---|---|---|
| none | — | `Idle` | the form |
| Precheck, Sponsoring | — | `Preparing` | `send.txPreparing` |
| Submitting | `NotYet` | `Preparing` | `send.txPreparing` |
| Submitting | `Up` (after `CeremonyStarted{id}`) | `AwaitingSignature` | `send.txSigning` (tx) / `componentsUi.signing.signing` (message) |
| Submitting | `Done` (after `CeremonyDone{id}`) | `Submitting` | `send.txSubmitting` + background hint |
| ReactiveSponsoring, PersistingResult | — | `Submitting` | same |

Ceremony events carry the request id and are dropped unless that inflight is in `Submitting`.
`SignView.pending_op_maybe_sent` mirrors the pending record's flag.

**Ending** — `ending_of(method, payload, submitted_user_op) -> SignEnding` then
`ending_state(ending, tracker entry) -> SignEndingState`:

| `SignEnding` | Tracker entry | `SignEndingState` | Drawn as |
|---|---|---|---|
| `Signed` (message) | — | `Signed` | tick, "已签名", closes itself |
| `Landed{tx, op?}` / `StillConfirming{op}` | `Confirmed` | `Confirmed{tx}` | tick + short hash + explorer |
| same | `Dropped` | `Reverted{tx}` | cross, `statusFailed` + `failedHint` + explorer |
| same | `NotSent` / `Rejected` | `NotSent` | cross, `statusFailed` + `txErrorGeneric` |
| same | pending | `Following{op, outcome, fee_held}` | outcome `MaybeSent` → `maybeSent` caption; `Landing` → ring; `StillConfirming` → `stillConfirming`; `Unknown` → unknown sentence |
| same | none yet | `Following{…, Landing}` | ring |

The core no longer patches a dApp record Confirmed from `on_submit`; the tracker closes on-chain
records both ways.

**Asker gone** — `SignSubmitOutcome::AskerGone`: the shell proved, before the passkey or between the
passkey and the relay POST, that the asker no longer exists. Inflight cleared, sheet cleared if it is
still that request, no answer, no record. `TransportDropped` now also stops a pipeline in Precheck,
Sponsoring or ReactiveSponsoring; past the commitment point the pipeline continues.

## 4. Extension request (the worker's ledger) — [RB1–RB11, RF3]

Lives in `chrome.storage.session` under `vela.req.<tabId>:<pageRequestId>`; in memory the worker
keeps only `docPorts` (documentId → port) and `surfaces` (windowId/rid → port).

| Field | Meaning |
|---|---|
| `v`, `rid`, `id` | schema version, worker request id, the page's own request id |
| `method`, `params`, `origin` | the request (never logged) |
| `tabId`, `windowId`, `documentId` | from `sender`; `documentId` is Chrome's per-document id |
| `sentAt`, `at` | content.js send time; `at = min(sentAt, now)` — the 5-minute clock |
| `surface`, `surfaceWindowId` | `panel` or `window`, set synchronously on arrival |
| `state` | `created` → `shown` → `claimed` |

```
created ──panel/window draws it──► shown ──claim(approve|sign) live──► claimed ──claim(submit) live──► claimed
   │                                 │                                   │
   └──────────── answer delivered (tabs.sendMessage by documentId) ──────┴──► answered   (record removed)
   └──────────── settle(cause) ──────────────────────────────────────────────► settled    (record removed, 4900 to the page, `withdrawn` to the surface)
```

| Trigger | Cause | Code / message to the page |
|---|---|---|
| `vela.doc` port closes while the worker lives; `tabs.onRemoved`/`onReplaced`; `alive` probe fails at recovery | `page_left` | 4900 "The page navigated away" |
| the surface's `vela.surface` port closes (Chrome's ✕, panel reload); window closed; recovery finds no panel/window | `surface_closed` | 4900 "The browser closed before the request finished" |
| claim at age ≥ 5 min; content.js deadline (`abandon`); recovery past 5 min | `expired` | 4900 "Vela did not answer in time — check its activity" |
| content.js sees the channel drop twice on a sign/connect | `restarted` | 4900 "Vela restarted before the request finished — check its activity" |
| content.js cannot reconnect (extension reloaded/updated) | `updated` | 4900 "Vela was updated — reload this page and try again" |

All settlement codes equal `popupCloseSettlement().code` (4900), never 4001. A dropped read channel
is retried once, then answers a plain `-32603 "Vela could not finish this read — try again"`; a
dropped `eth_sendRawTransaction`/`eth_sendUserOperation` answers `restarted`.

**Claim verdict** (`claimVerdict(record, {now, ttlMs, docAttached, callerWindowId, phase})`): live
only if the record exists ∧ the caller's window matches ∧ its page port is attached (or answers
`alive`) ∧ (phase `submit` ? state = claimed : age < 5 min). Not live → `{live: false, cause}`; no
answer within 5 s (after one reconnect) counts as not live.

**Recovery at worker start** (`recoveryPlan`): per record — past TTL → settle `expired`; panel window
without a side-panel context or dedicated window gone → settle `surface_closed`; else probe `alive`
(kept if the page still owns the id).

**content.js deadlines** (per outstanding sign/connect id): unclaimed → 5 min + 5 s; claimed → 5 min
after the claim. The `vela.doc` port is open only while the map is non-empty.

**Op-hash map** (RF3): `vela.ext.op.<hash>` = `{chainId, at}` for 24 h, written when an answer
carries `opHash`; read by `forwardRead` for receipt lookups.

**Endpoint health** (RF2): `vela.ext.endpoints` = `{<url>: {failures, until}}`; cooldown
`30 s · 2^(failures−1)` capped at 300 s; a success deletes the entry.

**Worker log** (RB14): `vela.sw.log` (ring of 200 lines), `vela.sw.counts` (`<event>.<cause>` → n).

## 5. Activity row — [RG1–RG5]

`FeedTxRecord` gains `dapp_origin: Option<String>` (shells map the stored `dappOrigin`).
`FeedItem` gains:

| Field | Rule |
|---|---|
| `kind` | `Send` / `Receive` / `DappTx` (a folded batch is `Send`) |
| `status` | the record's status (`Pending` / `Confirmed` / `Failed`); a batch uses its first line's |
| `site` | `DappTx` only: `host[:port]` of `dapp_origin` |

`DappTx` rows (from == me): direction Out; counterparty `to` if any; value from hex or decimal wei
through `from_base_units`, and 0 → no amount (value `None`, symbol ""). Message signatures and
connects never become rows.

`FeedView` gains `history_empty_key` (`history.emptyTitle` | `history.emptyFilter`) and
`home_empty_key` (`home.emptyNoActivity` | `home.emptyNoActivityNetwork`), both from `chain_filter`.

Row words (shells): title `history.txLabelDappTx` for `DappTx`; subtitle `site` → recipient →
chain; a not-confirmed row prefixes `statusPending` / `statusFailed` + " · ". A MaybeSent op is a
Pending `DappTx` row under its local hash until the tracker patches it.

## 6. Simulation verdict — [RG6–RG8]

`SimReply = Result(json) | Error{code, message} | Unreachable` → `SimOutcome`:

| Reply | `SimOutcome` | `notice` |
|---|---|---|
| pool gave up | `Unreachable` | Caution, `simUnavailableWarning` |
| JSON-RPC error (-32601, -32602, -32603 …) or a result that is not a non-empty array of blocks with calls | `NotOffered` | Caution, `simUnavailableWarning` |
| any call with status `0x0` or an error | `Reverts{reason}` | Danger, `simWillFailReason` (sanitised, ≤ 64 chars) or `simWillFail` |
| otherwise | `Deltas{deltas}` (empty = nothing moves) | none; the balance block from `token_trust` as today |

`eth_simulateV1` is an optional method in the pool: its JSON error is an answer, never a chain
failure or a ban; a rate-limit signal still fails over.

## 7. Plain send — [RC1–RC5]

`ClearSurface::PlainSend` with `ClearSigningView.plain_send: Option<ClearPlainSend>` (Some iff the
surface is PlainSend):

| Field | Rule |
|---|---|
| `to` | EIP-55 of the request's `to`; not an address → no plain send (BlindTransaction) |
| `value_wei` | exact decimal of absent/""/"0x" (= 0) or "0x"+hex; anything else → no plain send |
| `amount` | `value_wei / 10^18`, exact, trailing zeros trimmed, `ClearLocale` marks; "0" when zero |
| `no_value` | value = 0 → "Send · 0 <coin>", no minus, neutral `Confirm` |

Reset wherever `result` is reset. Symbol: the shell's fee-row native symbol.

## 8. Address spelling toward dApps — [RG10]

| Place | Spelling |
|---|---|
| grant written (`popup_approved`, `popup_account_switch`) | EIP-55 (`dapp_spelling`) |
| grant loaded (`sites_listed`) | normalised to EIP-55 |
| `AccountSwitched.active_address` | EIP-55 |
| extension's stored grants | rewritten once at wallet boot (`normalizeGrantSpelling`) |
| `inpage.js` `session.accounts`, `selectedAddress`, legacy `eth_accounts` | the wallet's spelling as sent; change detection case-insensitive |
| `resolve_granted` and its JS twin | unchanged (case-insensitive match) |

## 9. Page load and the address bar — [RE1, RE2, RE3, RD3–RD7, RD9]

Per tab (iOS/Android engine, desktop `LoadWatch` in core):

| Field | Meaning |
|---|---|
| `committed_url` | the document on screen (commit callback; iOS also same-origin SPA changes while nothing is pending) |
| `pending_url` | what a load, retry, back/forward, policy-allowed main-frame navigation or `_blank` asked for; cleared on commit, failure, finish, Stop, teardown |
| `failed_url` | the address of the failure on screen (falls back to `pending_url`) |
| `generation` | per load; timers carry it and stale ones are ignored |
| `engine_loading`, `engine_progress` | WebKit `isLoading` / `estimatedProgress` (desktop poll; iOS engine), Android `getProgress()` |
| `engine_live` | progress rose above `ENGINE_LIVE_PROGRESS` (0.15) for this generation |
| `page_initiated` | desktop: a load the wallet did not ask for (`PageStarted`) — manual Retry only |
| `failure`, `retrying`, `attempt` | as in 079 |

**Address bar** — `address_bar(committed, pending, failed) -> {url, host, lock}`:

| Situation (first match) | host | lock |
|---|---|---|
| a failure panel is up | failed host | none |
| a document committed | committed host | `Closed` (https, loopback/private http) / `Open` (public http) |
| a load pending in an empty tab | pending host | none |
| nothing | "" | none |

```
idle ──load/reload/link──► requested (pending_url, hairline; bar keeps committed host)
requested ──commit──► committed ──finish──► finished
requested ──platform failure | engine stopped without commit | should_give_up(20 s, no commit, progress ≤ 0.15)──► failed(class)
failed ──2/5/10 s timer (in front) | Retry | network CameBack (healing class)──► requested (retrying)
requested ──Stop──► idle (committed page and bar kept, no panel)
desktop: engine loading with no wallet request and a new URL ──► requested (page_initiated)
desktop: retry timer while the engine still loads the same URL ──► no navigate (EngineStillLoading)
```

New failure class `proxy` (reason `explore.loadProxy`, Offline retry schedule); Apple -1000 → `offline`.

## 10. Desktop tab and hold — [RD1, RD6]

| Field | Meaning |
|---|---|
| `shown_tab: Option<String>` | the tab whose page is in the one webview; None at launch and after closing the page |
| `holds_navigation(view)` | `consent ∨ signing ∨ queued_signing > 0` (core `DbrView`) |
| `browser_notice` | `(explore.requestOpen, seq)` for 2.5 s after a held click |

A restored tab waits unlit; a typed address over the start page opens a new tab; clicking the shown
tab does nothing; closing a background tab does not reload.

## 11. Network health, logo misses, balance read plan — [RE3, RE9, RE10]

- `NetHealth{misses, online}`; `net_health_step(state, reached)` → edge `WentOffline` at the 3rd
  miss, `CameBack` on the first reach after offline.
- `MarkMiss = NotFound | Refused | NotAnImage | Throttled | ServerError | Transport | Unknown`; TTL
  None (session) for the first three, 60 000 ms otherwise; transient misses clear on `CameBack`.
- `ReadSlot{kind: Native|Stable|Wrapped|Custom, contract, symbol, known_decimals, peg_usd}`; order
  native, stables, wrapped, custom; deduplicated by lower-case contract.
- `TrackOperation::HoldingsMoved{chain_id}`: on a confirmed receipt, or a failed receipt with a tx hash.

## 12. Desktop route (wallet HTTP) — [RD2]

`routes_for(url) -> Vec<Route>`, `Route = Direct | HttpConnect{host, port} | Socks5h{host, port}`,
from the system settings per request (5 s cache; PAC results 5 min, PAC failures 30 s). Within one
request the next route is tried only when the TCP connect to the proxy itself failed. A failure
carries `ProxyFailure{proxy: "host:port", kind: Unreachable | RefusedTunnel(status) | Timeout |
PacFailed}`. `Unreachable`, `Timeout` (connecting to the proxy) and `PacFailed` map to probe code 6
(`proxy`); `RefusedTunnel` — the proxy answered 502 or closed the CONNECT — keeps the site's own
class (`refused`/`connect`), as iOS -1000 does (research RX, RD9). No state survives a request.

## 13. Chain notice — [RF1, RF4]

`shown = chain ∈ (failed_chains ∪ unreached_chains) ∧ chain ∉ rate_limited_chains`, for the tab in
front. `unreached_chains` gains a chain when one RPC call's first pass saw every endpoint fail on
transport with no rate-limit signal; any usable answer removes it. Retry is busy until its one read
settles.
