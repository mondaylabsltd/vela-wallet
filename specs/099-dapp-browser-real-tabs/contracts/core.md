# 099 — Core contract

What changes on the wire between `vela-core` and the shells. Types are in
[../data-model.md](../data-model.md). Every rule below is decided once in the core; a shell that
re-decides one of them is a bug.

## `explore_sites`

- Model gains `recent: Vec<String>`. `TabOpened`, `TabSelected` → front; `TabClosed` → removed;
  hydration → `[selected]`.
- `ExploreView.recent_tabs: Vec<String>` (`#[serde(default)]`).

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
