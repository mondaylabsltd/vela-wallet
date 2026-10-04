# 099 — Data model

Everything here is core (`vela-core`) state or wire. Shells hold only engines (webviews) and draw.
New wire fields are `#[serde(default)]` so an older reader keeps decoding; new enum variants land
in every shell's mirror in the same change (Android's typed enums refuse a whole view on an
unknown name).

## As built (2026-10-04) — where the code differs from the plan below

The code is the contract; these are the places the plan below was changed while building it.

- **Layers** are `browser · provider · wallet · network · relay · sheet · signer` (`sheet`, not
  `signing`: the person's decision on Vela's sheet, consent included).
- **Reasons** (`dapp_record::DbrReason`, each with `key()` → `componentsUi.browserStatus.reason.*`):
  `navigated_away, page_crashed, tab_closed, wallet_withdrawn, insecure_origin, not_connected,
  account_mismatch, no_account, consent_busy, unsupported_method, unknown_chain, bad_params,
  too_many_reads, unknown_batch, wallet_refused, no_endpoint, timed_out, rate_limited,
  endpoint_error, reverted, rejected_by_person, relay_refused, relay_unreachable,
  not_confirmed_yet, relay_failed, signer_unavailable, signer_not_discoverable, signer_failed`.
  `tab_closed` has no line of its own (a closed tab has no panel). No `signer_cancelled`: a
  cancelled prompt is still `PasskeyCancelled` (never an answer).
- **Provider state** is flat: `pending · offered · insecure_origin · no_hello`. **Page state**:
  `blank · loading · ready · crashed`. Both have `key()`.
- **Rows** carry `code` and `outcome: open · answered · failed`; `DbrFailureNote` carries `key`;
  the inspector carries `connected: bool`, not an address.
- **Signer** kinds are three `SignErrorKind`s — `signer_unavailable`, `signer_not_discoverable`,
  `signer_failed` — not one kind plus a notice field; `signer_failed` stays retryable.
- **Confirm** — `sign_confirm::confirm_state` / `confirm_state_of` (borrowed views); `ConfirmState`
  carries `key` (`componentsUi.signing.confirmBlock.*`, `answeredRetry` when the failure is
  retryable); `SignView.confirm_block` names the signing machine's own block. A stale fee stays
  advisory (research R7).
- **The relay's send time** lives only on `TrackEntryView.relay_sent_at_ms`. `SendReceiptView` and
  `SignEndingState::Following` do NOT carry it (their outcome types are `Eq`, and every shell
  already holds the tracker entry for its op): each landing reads the entry and calls
  `landing_pace`. `LandingPace` is flat — `{ line: waiting | none | remaining | elapsed | slow,
  seconds, progress }`.
- **Corpus**: 51 keys (41 `browserStatus`, 9 `confirmBlock`, `send.txRelaySending`); no layer-name
  keys. ja+en runtime residency 144,350 of 144,400.
- **Shell-side leftovers** (presentation, guarded by tests, not exported yet): the status line's
  priority (reloaded → wallet not offered → latest trouble; "reloaded" is the shell's own fact),
  and the fee row's `off_chain` / another-tier display test on iOS, Android and web — the gate no
  longer reads them.


## Tabs and engines

| Item | Where | Shape | Rule |
|---|---|---|---|
| Recency | `explore_sites::Model.recent` (not persisted) | `Vec<String>` tab ids, most recent first | `TabOpened` and `TabSelected` move the id to the front; `TabClosed` removes it; hydration seeds it with the selected tab. |
| `ExploreView.recent_tabs` | view | `Vec<String>` | the order above, only ids that exist |
| `DbrTabView.busy` | `dapp_browser` view | `bool` | the tab has an open request, a read in flight or queued, the consent sheet's entry, the signing job or a queued signature |
| `LIVE_TABS_CAP` | `browser_tabs` | `6` | most live engines kept |
| `EngineInput` | `browser_tabs` | `{ tabs: Vec<String>, selected: Option<String>, recent: Vec<String>, busy: Vec<String>, live: Vec<String>, pressure: bool }` | `live` = tabs whose engine exists now; `pressure` = the OS asked to free memory |
| `EnginePlan` | `browser_tabs` | `{ suspend: Vec<String> }` | see below |

`plan_engines(input) -> EnginePlan`:
1. Keep: the selected tab; every busy live tab.
2. Fill: other live tabs in recency order (unknown recency last, in strip order) while the kept
   count is below the cap — `1` under pressure, else `LIVE_TABS_CAP`.
3. Suspend: every live tab not kept. Never the selected tab, never a busy tab, never a tab that is
   not live (nothing to suspend).

Waking is the shell's: a selected tab with no engine gets one. The shell remembers which tabs it
suspended, to say "reloaded to save memory" when one comes back.

## Requests (per tab)

```text
DbrRequestRow {
  id: String,                 // the page's JSON-RPC id, as text
  method: String,             // e.g. eth_call, personal_sign
  class: DbrRequestClass,     // local | consent | read | relay_read | signing
  started_ms: f64,            // 0 = unknown
  ended_ms: Option<f64>,      // None while open
  outcome: DbrOutcome,        // open | answered | failed { code: i64 }
  layer: Option<DbrLayer>,    // who ended it, for a failure
  reason: Option<DbrReason>,  // why, for a failure
}
```

- `DbrRequestClass`: `local` (accounts, chain id, permissions, switch/add chain — answered by the
  wallet at once), `consent` (connect), `read` (node RPC), `relay_read` (bundler/relay reads:
  receipts, op status, `wallet_getCallsStatus`), `signing`.
- `DbrLayer`: `browser` (page or tab gone), `provider` (the page is not offered the wallet),
  `wallet` (the wallet's own rules refused it), `network` (the chain's RPC), `relay`,
  `signing` (the person, on the sheet), `signer` (the passkey).
- `DbrReason` (one corpus key each, `componentsUi.browserStatus.reason.*`):
  `navigated_away` (4900, page left/closed/crashed), `not_offered`, `unsupported_method` (4200),
  `not_connected` (4100), `unknown_chain` (4902), `bad_params` (−32602), `too_many_reads` (−32005),
  `no_endpoint` (no RPC answered), `timed_out` (no answer in `READ_DEADLINE_MS`),
  `rate_limited` (429/−32005 from the endpoint), `endpoint_error` (the endpoint's own error,
  passed through), `rejected_by_person` (4001), `relay_refused`, `relay_failed`,
  `signer_cancelled`, `signer_unavailable`, `signer_not_discoverable`, `signer_failed`,
  `wallet_refused` (any other wallet refusal, e.g. an unlimited approval).
- Ring: the last `REQUEST_RECORD_CAP` = 200 rows per tab (open + settled), oldest dropped first.
  Kept across documents (a navigation is when one wants it most); dropped with the tab.
- Never stored: params, results, signatures, addresses, full URLs (the tab's origin only).

Counts in every view (cheap): `DbrTabView.open_requests: u32`, `DbrTabView.failed_recent: u32`
(failures among the last 20 rows), `DbrTabView.last_failure: Option<DbrFailureNote>`
(`{ layer, reason, method }` — the status entry's one line).

The full rows reach the view only for the inspected tab: `DbrView.inspector: Option<DbrInspectorView>`
`{ tab, origin, page: DbrPageState, provider: DbrProviderState, connected_address, chain_id, rows,
report }`, where `report` is the copyable plain-text record (English, stable, host-only, one line
per row) — the same text on every client and in a bug report.

## Provider and page

- `DbrProviderState`: `pending` (a load began, no hello yet), `offered` (the document said
  hello), `not_offered { reason: DbrNotOfferedReason }` — `insecure_origin` (an origin
  `offers_wallet` refuses), `no_hello` (a load finished in a secure origin with no hello: the
  script did not run).
- `DbrPageState`: `loading`, `loaded`, `crashed`, `blank` (no document yet).

## Time

`dapp_browser` keeps `clock_ms`, the latest `now_ms` any event carried. Events that start or end a
request carry `#[serde(default)] now_ms: f64`: `PageMessage`, `NavigationStarted`, `LoadFinished`,
`TabClosed`, `RendererGone`, `ConsentRejected`, `SigningAnswered`; shell results `ReadAnswered`,
`UserOpResolved` carry it too. `0` = unknown: the record keeps the last known time.

## Reads

- `DbrOperation::Read.deadline_ms: f64` = `READ_DEADLINE_MS` (30 000). The executor answers by then.
- `DbrShellResult::ReadAnswered { body_json, now_ms, failure: Option<DbrReadFailure> }` —
  `no_endpoint` | `timed_out` | `rate_limited` when `body_json` is `None`.

## Log

`DbrOperation::Log { line }` — one line per request end and per tab page/provider change,
`dapp tab=<id> req=<id> method=<m> class=<c> outcome=<o> layer=<l> reason=<r> ms=<d>` /
`dapp tab=<id> page=<p> provider=<p> origin=<host>`. The shell writes it to its log as is.

## Signing sheet

- `ConfirmInput { sign: SignView, guard: GuardView, clear: ClearSigningView, fee: FeeView,
  speed_tier: Option<FeeTier> }` → `ConfirmState { enabled: bool, block: Option<ConfirmBlock> }`.
- `ConfirmBlock` (first that applies, in this order; one corpus key and one action each):
  `in_flight` (signing / submitting), `reading` (the request is still being read),
  `refused` (spec 081 block), `account_switching` (reconcile pending), `funding` (funding sheet),
  `answered` (the request was answered or its failure is held — action: try again, when offered),
  `approval_choice` (an editable approval with no choice), `batch_unsettled`,
  `fee_measuring` (estimating / another speed's figure), `fee_failed` (action: retry),
  `fee_short` (action: pick another coin), `fee_missing`.
- Stale fee: `FeeView.stale` does not shut confirm (unchanged, `fee_policy` invariant); the sheet
  shows the refresh affordance it already has.

## Signer

`SignSubmitOutcome::Failed.signer: Option<FailureKind>` (`#[serde(default)]`) — the same
`FailureKind` create/login use (`cancelled`, `not_supported`, `not_discoverable`, `other`), from
the shell's existing passkey classifier. `SignErrorKind::SignerFailed` (−32603) answers the page;
`SignErrorNotice.signer: Option<FailureKind>` picks the sheet's words.

## Tracker

- `FIRST_STATUS_POLL_MS` = 3 000.
- `TrackEntryView.relay_sent_at_ms: Option<f64>` — when the tracker first learned the bundle is on
  the network (`submitted`, `included`, a named bundle tx, or a receipt).
- `Entry.forgotten_since_ms: Option<f64>` — the first `not_found` (past the grace) for an
  acknowledged op. `FORGOTTEN_NOT_SENT_MS` = 600 000.
- `SignEndingState::Following.relay_sent_at_ms`, `SendReceiptView.relay_sent_at_ms` pass it on.
- `landing_pace(sent_at_ms, typical_s, now_ms) -> LandingPace { line, progress }` — the one
  countdown ladder (`waiting` · `remaining{s}` · `elapsed{s}` · `slow`) and ring curve, counted from
  the relay's send, never from acceptance.
