# Tasks: 099 — The dApp browser: real tabs, and layers you can see

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/core.md](contracts/core.md), [quickstart.md](quickstart.md)

Rule for every task: the decision is the core's; a shell task only executes or draws, and deletes
the shell's own copy of a rule it replaces. Core tests fail on the old core before the change.

## Phase 1 — Core rules (`rust/crates/vela-core`)

- [ ] T001 [US1] `src/app/browser_tabs.rs` (new): `EngineInput`, `EnginePlan`, `LIVE_TABS_CAP`,
  `plan_engines`; registered in `app/mod.rs`; tests `tests/app_browser_tabs_099.rs` (selected
  never suspended, busy never suspended, cap, pressure → 1, recency order, unknown recency in strip
  order, nothing live → nothing to suspend).
- [ ] T002 [US1] `explore_sites.rs`: `recent` + `ExploreView.recent_tabs`; tests in
  `tests/app_explore_sites.rs` (open/select/close/hydrate).
- [ ] T003 [US2] `dapp_browser.rs`: clock (`now_ms` on the events in contracts), `DbrRequestRow`
  ring per tab (`REQUEST_RECORD_CAP` 200), `DbrRequestClass`, `DbrLayer`, `DbrReason`, rows opened in
  `handle_request`, closed in `deliver_result`/`deliver_error`/retire with layer+reason;
  `ConsentRejected { now_ms }`.
- [ ] T004 [US1] `dapp_browser.rs`: `DbrTabView.busy`, `page`, `provider` (`DbrProviderState`,
  `DbrNotOfferedReason`), `open_requests`, `failed_recent`, `last_failure`.
- [ ] T005 [US3] `dapp_browser.rs`: `Read.deadline_ms` (`READ_DEADLINE_MS` 30 000),
  `ReadAnswered { now_ms, failure }`, `UserOpResolved { now_ms }`; read failures → network/relay
  reasons; endpoint errors (incl. 429 / −32005) → `rate_limited` / `endpoint_error`.
- [ ] T006 [US2] `dapp_browser.rs`: `DbrOperation::Log { line }` per row end and page/provider
  change; `InspectorOpened/Closed`, `DbrView.inspector` with rows and `report` text.
- [ ] T007 [US2] Tests `tests/app_dapp_browser_099.rs`: one per layer/reason path, ring cap, the
  inspected-tab-only rows, busy across consent/read/signing/queued, provider states, deadline in
  the op, log lines contain no params/addresses/full URLs.
- [ ] T008 [US4] `sign_request.rs`: `confirm_state(ConfirmInput) -> ConfirmState` with
  `ConfirmBlock` order from data-model (incl. off-chain, another tier, reading); tests that pin
  each block and that it equals the old shells' `confirm_enabled` on their own test cases.
- [ ] T009 [US4] `sign_request.rs`: `Failed.signer: Option<FailureKind>`,
  `SignErrorKind::SignerFailed`, `SignErrorNotice.signer`; dapp_browser maps it to `signer / *`;
  tests.
- [ ] T010 [US4] `tx_tracker.rs`: `FIRST_STATUS_POLL_MS`; `relay_sent_at_ms` (entry + view);
  forgotten ops (`forgotten_since_ms`, `FORGOTTEN_NOT_SENT_MS`, scan required); `landing_pace` +
  `LandingPace`/`LandingLine`; tests (first ask at 3 s; sent time from submitted/included/bundle
  tx/receipt; forgotten ends only with scan caught up; unanchored stays pending; ladder & ring
  values match the old desktop `ring_progress` and ladder).
- [ ] T011 [US4] `send.rs` `SendReceiptView.relay_sent_at_ms`; `sign_request.rs`
  `Following.relay_sent_at_ms`; tests.
- [ ] T012 [US2] Corpus: `componentsUi.browserStatus.*`, `componentsUi.signing.confirmBlock.*`,
  `componentsUi.signing.signer.*`, `send.txRelayQueued`/`txRelaySending` in 15 locales;
  `gen-i18n` pin; residency test.
- [ ] T013 Bindings: UniFFI `browser_engine_plan`, `sign_confirm_state`, `landing_pace`; wasm
  `signConfirmState`, `landingPace`; `gen-core-types`; `build-web.mjs`; uniffi/wasm crate tests.

## Phase 2 — Desktop (`app-desktop/vela-wallet`)

- [ ] T020 [US1] `src/webview.rs`: `BROWSERS: BTreeMap<String, Browser>`; every fn takes the tab
  id; sinks (`on_message_to`, `on_load_to`, `on_meta_to`, `on_leave_to`, ipc) carry it; one shared
  `WebContext` on Windows; per-view `NavigationGate` on macOS; `show(tab)` hides the rest.
- [ ] T021 [US1] `src/wallet/browser_host.rs` + `page.rs`: the explore tab id replaces
  `BROWSER_TAB`; a switch shows/hides, never navigates; remove the hold (`holds_navigation`) and
  the switch veil; closing a tab destroys its view and sends `TabClosed`.
- [ ] T022 [US1] Engine plan: call `plan_engines` on tab/DbrView changes; suspend = destroy view,
  keep URL/title; wake on select with the "reloaded to save memory" note.
- [ ] T023 [US3] Per-tab `LoadDriver`, chain notice, crash/engine-failure panels, Back floor.
- [ ] T024 [US3] `src/executor/dapp_browser.rs`: `deadline_ms` (race the pool), `failure` kind,
  `now_ms` on every event/result, `Log` → `vlog!`.
- [ ] T025 [US2] Status entry in the browser chrome (page · wallet · last issue) + inspector panel
  (rows, copy `report`), `InspectorOpened/Closed`.
- [ ] T026 [US4] `src/signing/live.rs`: `confirm_enabled` → core `confirm_state`, block line +
  action under the slide; signer kind from `platform_macos`/`user_op` into `Failed.signer`.
- [ ] T027 [US4] `src/flows/live.rs` + `signing/status.rs`: ladder and ring → `landing_pace`,
  counted from `relay_sent_at_ms`; the relay's state line before it.
- [ ] T028 Desktop tests (live/status/browser_host) and the gallery entries for status + inspector.

## Phase 3 — Phones

- [ ] T030 [P] [US1] iOS `BrowserController.swift`: `browser_engine_plan` on changes and on
  `didReceiveMemoryWarning` (`pressure`); suspended note on wake.
- [ ] T031 [P] [US2] iOS: `DbrWire` mirrors (new fields/variants), `DbrExecutor` deadline + failure
  + `now_ms` + `Log` → `os_log`; status entry + inspector sheet.
- [ ] T032 [P] [US4] iOS signing/send: `sign_confirm_state`, `landing_pace`, signer kind.
- [ ] T033 [P] [US1] Android `BrowserController.kt`: engine plan on changes and `onTrimMemory`.
- [ ] T034 [P] [US2] Android `DbrWire`/`BrowserExecutor`: mirrors, deadline, failure, `now_ms`,
  `Log` → `Log.i`; status entry + inspector sheet.
- [ ] T035 [P] [US4] Android `SigningLive`/`SendLive`: `sign_confirm_state`, `landing_pace`, signer
  kind; `I18nKeys`.
- [ ] T036 Phone tests (BrowserMachineTest, DappBrowserTests, SigningLive tests, wire drift).

## Phase 4 — Web

- [ ] T040 [P] [US4] `src/lib/signing/*`: confirm via `signConfirmState`; `flows/live-send.ts`,
  `signing/dapp-receipt.ts`, `flows/ui/ring.ts`: `landingPace`; vitest over the real wasm.

## Phase 5 — Verify and record

- [ ] T050 Suites: core, desktop, iOS, Android, web; `build-web --check`; clippy/fmt.
- [ ] T051 Quickstart on the Mac (signed bundle), iPhone 11, Xiaomi; memory before/after (SC-003).
- [ ] T052 `docs/dapp-browser/ARCHITECTURE.md`: the six layers, where each is seen and logged.
- [ ] T053 `results.md`; PR.

## Order

T001–T002 → T003–T007 (one machine, in sequence) ∥ T008–T011 → T012–T013 → desktop T020–T028 →
phones T030–T036 ∥ web T040 → T050–T053.
