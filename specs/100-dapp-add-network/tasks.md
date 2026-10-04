# Tasks: 100 — A dApp can ask Vela to add a network

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/core.md](contracts/core.md), [quickstart.md](quickstart.md)

Rule for every task: the decision is the core's; a shell task only executes, relays or draws.

## Phase 1 — Core (`rust/crates/vela-core`)

- [x] T001 [US2] `dapp_rpc.rs`: `DappChainAsk`, `add_chain_ask` (decimals 18, list shapes, cleaned
  texts, first four usable RPCs, first https explorer), `usable_rpc_url` (`offers_wallet` rule),
  `url_host`.
- [x] T002 [US1] `dapp_rpc.rs`: `DappAddOutcome`, `add_outcome_error` (4001 / 4902 / -32602 /
  -32002), `ADD_NOT_COMPATIBLE`, `ADD_BUSY`.
- [x] T003 [US1][US4] `dapp_browser.rs`: `Route::AddChain` unknown → `ask_to_add` (-32602 /
  -32002 / `ForwardToAddNetwork`, row re-classed `consent`, `OpenKind::AddNetwork`);
  `AddNetworkAnswered` → added (chain joins `chains`, `set_site_chain`, `null`) or the table's error;
  `retire_document` → `CancelAddNetwork`; `DbrView.adding_network`.
- [x] T004 [US4] `dapp_record.rs`: `DbrReason::{NotCompatible, BadRpc}` with keys.
- [x] T005 [US1] `network_admin.rs`: the wizard's check as shared steps (`Step`, `probe_candidates`,
  `begin_probes`, `probed_step(must_report)`, `code_step`, `p256_step`, `contracts_verdict`,
  `unverified`); every existing `app_network_admin` test unchanged.
- [x] T006 [US1][US2][US3] `network_admin.rs`: `dapp_add` + `dapp_gen`; `DappAddRequested` (store
  gate, dedup → added, catalog), catalog-known vs site path, `NetDappAddPhase`, `DappAddApproved`
  (wizard's save), `DappAddDeclined` (by phase), `DappAddRetried`, `DappAddCancelled`,
  `DappAddSettled`, `NetView.dapp_add`.
- [x] T007 Tests `tests/app_dapp_add_network_100.rs`: one per rule (switch; sheet; -32002; added +
  chainChanged + null; 4001; 4902 / -32602 + record; page left; malformed; RPC rule; outcome table;
  catalog wins; site RPC must name the chain; no usable RPC; incompatible adds nothing; approve saves
  the Settings way; decline; unable to verify + retry; already has it; wizard untouched; busy +
  cancel; before the store loads; both machines end to end ×2). Update
  `app_dapp_browser.rs`'s old 4902-add assertion.
- [x] T008 Corpus (15 locales): `connect.browser.{addLead,addFromSite}`,
  `componentsUi.browserStatus.reason.badRpc`, `consentBusy` generalised, minus
  `onboarding.login.alertNotFound{Title,Body}`; `gen-i18n.mjs` pin 1877 / 1780; `gen-i18n`,
  `i18n.dump`; residency test.
- [x] T009 wasm `dappAddChainAsk`, `dappAddOutcomeError`; `gen-core-types`; `build-web.mjs` last.

## Phase 2 — Desktop (`app-desktop/vela-wallet`)

- [x] T010 `executor/dapp_browser.rs`: `ForwardToAddNetwork` / `CancelAddNetwork` → outbound orders.
- [x] T011 `executor/network_admin.rs`: `DappAddSettled` → a settle order (answer `Written`).
- [x] T012 `wallet/browser_host.rs` + `wallet/page.rs`: relay forward → resident
  `DappAddRequested`, settle → `AddNetworkAnswered`, cancel → `DappAddCancelled`; the Connection
  column draws the sheet (after a consent); tests against the real cores.

## Phase 3 — Android

- [x] T020 `DbrWire.kt` / `NetWire.kt`: new ops, events, view fields, reasons, phase.
- [x] T021 `BrowserExecutor.kt` / `NetworkAdminExecutor.kt` / `BrowserController.kt` /
  `SettingsController.kt` / `VelaWalletApplication.kt`: the relay.
- [x] T022 The sheet in Explore (`VelaModalSheet`, the consent's components); `ExploreLive` model.
- [x] T023 Tests: the relay against the real cores; the sheet model.

## Phase 4 — iOS

- [x] T030 `DbrWire.swift` / `SettingsWire.swift`; `DbrExecutor` / `NetworkAdminExecutor` op lists.
- [x] T031 The relay (`BrowserController`, `SettingsStore`, `RootView`).
- [x] T032 The sheet in `ExploreScreen` (the consent's sheet slot).
- [x] T033 Tests (Swift Testing) against the real cores.

## Phase 5 — Web (extension)

- [x] T040 `protocol.js`: `addChain` held like connect (`droppedChannelAnswer`), docs.
- [x] T041 `background.js`: catalog in the mirror; route unknown add → surface, -32002 while one is
  open; a `null` answer to an add record switches the site before delivery.
- [x] T042 `content.js`: hold `addChain`.
- [x] T043 `DappRequestHost.svelte` + a sheet component; `network-admin` executor relays
  `dapp_add_settled`; publish the catalog on `added`.
- [x] T044 Tests: worker routing, content holding, the surface relay over the real wasm.

## Phase 6 — Verify

- [x] T050 All gates (rust fmt/clippy/test, gen-core-types --check, build-web + sync + --check,
  desktop, Android, iOS on `Vela-s100`, web check + unit, event payloads, lint/verify i18n);
  results.md with real counts; quickstart.
