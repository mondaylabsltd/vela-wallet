# 099 — Core contract

What changes on the wire between `vela-core` and the shells. Types are in
[../data-model.md](../data-model.md). Every rule below is decided once in the core; a shell that
re-decides one of them is a bug.

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


## `explore_sites`

- Model gains `recent: Vec<String>`. `TabOpened`, `TabSelected` → front; `TabClosed` → removed;
  hydration → `[selected]`.
- `ExploreView.recent_tabs: Vec<String>` (`#[serde(default)]`).
- Closing many (Phase 6): `TabCloseScope` = `{"type":"others","keep":id}` ·
  `{"type":"right","of":id}` · `{"type":"all"}`; `tabs_closed_by(tabs, scope) -> Vec<id>` (UniFFI
  `explore_tabs_closed_by(tabs_json, scope_json) -> Option<String>`); `Event::TabsClosed { ids }`
  closes them in one step: a surviving selection stays; a closed one moves to the nearest
  surviving tab on its right in the old strip, else its left; none left → no selection (the start
  page); `recent` pruned; one persist. Shells close each closed tab's engine and tell
  `dapp_browser` `TabClosed` per tab, as for one.

## `browser_tabs` (new, pure)

- `pub const LIVE_TABS_CAP: usize = 6;`
- `pub fn plan_engines(input: &EngineInput) -> EnginePlan`
- UniFFI `browser_engine_plan(input_json: String) -> String` (EnginePlan JSON); the desktop calls
  the Rust function directly.

Called by a shell after any of: a tab selected/opened/closed, a request opened/settled (the
`DbrView` changed), an engine created, a memory warning. The shell destroys each tab in
`suspend` (and sends nothing to `dapp_browser` — the tab is not closed; its next document says
`hello` when it is woken).

## `dapp_browser`

Events (all new fields `#[serde(default)]`):

| Event | Change |
|---|---|
| `PageMessage` | `now_ms: f64` |
| `NavigationStarted`, `LoadFinished`, `TabClosed`, `RendererGone` | `now_ms: f64` |
| `ConsentRejected` | becomes `ConsentRejected { now_ms }` (unit variant → struct; the shells' encoders change in the same commit) |
| `SigningAnswered` | `now_ms: f64` |
| `InspectorOpened { tab }` / `InspectorClosed` | new: which tab's full record the view carries |

Operations / results:

| Item | Change |
|---|---|
| `DbrOperation::Read` | `deadline_ms: f64` — the executor must answer by then |
| `DbrShellResult::ReadAnswered` | `now_ms: f64`, `failure: Option<DbrReadFailure>` (`no_endpoint` · `timed_out` · `rate_limited`) |
| `DbrShellResult::UserOpResolved` | `now_ms: f64` |
| `DbrOperation::Log { line }` | new: write `line` to the shell's log, answer `Ack` |

View:

| Field | Meaning |
|---|---|
| `DbrTabView.busy` | never suspend this tab |
| `DbrTabView.page` | `DbrPageState` |
| `DbrTabView.provider` | `DbrProviderState` |
| `DbrTabView.open_requests`, `failed_recent`, `last_failure` | the status entry |
| `DbrView.inspector` | the inspected tab's rows + `report` text |

Rules:

1. Every request a document sends gets a row when it is classified, before any answer.
   A message that is ignored (sub-frame, not offered, closed tab) gets no row; a not-offered
   page shows in `provider` instead.
2. A row ends exactly where the answer is delivered (`deliver_result` / `deliver_error`), with the
   layer and reason the code path names; a 4900 from `retire_document` is `browser /
   navigated_away`.
3. Read failures: `ReadAnswered.failure` names `network / no_endpoint | timed_out |
   rate_limited`; an endpoint's own `{"error":…}` is `network / endpoint_error` with its code.
   Bundler reads are `relay_read` and fail as `relay / …` with the same reasons.
4. Signing answers: `SignResponsePayload::Err.kind` → layer/reason
   (`UserRejected` → `signing / rejected_by_person`; `SubmitFailed` with a refusal →
   `relay / relay_refused`; `SignerFailed` → `signer / signer_*` from the notice's kind;
   `UnauthorizedAccount` → `wallet / not_connected`; `UnsupportedChain` → `wallet /
   unknown_chain`; others → `wallet / wallet_refused`).
5. `READ_DEADLINE_MS` = 30 000 is sent in every `Read`; a shell that cannot cancel the I/O still
   answers at the deadline and drops the late body.
6. One `Log` line per row end and per change of a tab's `page` or `provider`.

## `sign_request`

- `SignSubmitOutcome::Failed { message, refused, signer: Option<FailureKind> }`.
- `SignErrorKind::SignerFailed` (code −32603) when `signer` is `not_supported`,
  `not_discoverable` or `other`; `SignErrorNotice.signer` carries the kind.
- `pub fn confirm_state(input: &ConfirmInput) -> ConfirmState` — replaces each shell's
  `confirm_enabled`; UniFFI `sign_confirm_state(sign_json, guard_json, clear_json, fee_json,
  speed_tier_json) -> String`; wasm `signConfirmState(...)`.
- `SignEndingState::Following.relay_sent_at_ms: Option<f64>`.

## `tx_tracker`

- `FIRST_STATUS_POLL_MS = 3_000`: an op's first relay status ask is 3 s after acceptance.
- `TrackEntryView.relay_sent_at_ms: Option<f64>` — when the tracker first learned the bundle is on
  the network (status `submitted`/`included`, a named bundle tx, or a receipt).
- Forgotten ops: an acknowledged, non-terminal op whose status answers `not_found` (past the
  grace) is asked again at its receipt pace (also past the wait window) and runs its find-event
  scan; it ends `NotSent` when `FORGOTTEN_NOT_SENT_MS` (600 000) have passed since the first such
  answer, at least `NOT_FOUND_CONFIRMATIONS` answers were `not_found`, and the scan is caught up
  with no event. Any other status, a receipt or an event clears `forgotten_since_ms`.
- `pub fn landing_pace(sent_at_ms: Option<f64>, typical_s: Option<u16>, now_ms: f64) -> LandingPace`
  — `{ line: LandingLine, progress: Option<f32> }`, `LandingLine` = `waiting` (not sent yet: the
  shell shows the relay's state line) · `remaining { s }` · `elapsed { s }` · `slow`. Replaces the
  shells' `elapsed < typical` ladders and `ring_progress`. UniFFI `landing_pace(...)`, wasm
  `landingPace(...)`.

## `send`

- `SendReceiptView.relay_sent_at_ms: Option<f64>` (from the tracker entry).

## Corpus

New keys (15 locales, residency budget unchanged unless measured over):
`componentsUi.browserStatus.*` (status entry, inspector headings, layer names, `reason.*`,
`provider.*`, `page.*`, "reloaded to save memory", "copy record"),
`componentsUi.signing.confirmBlock.*` (one line per `ConfirmBlock`),
`componentsUi.signing.signer.*` (one line per signer `FailureKind`),
`send.txRelaySending` / `send.txRelayQueued` (the landing before the relay has sent).
