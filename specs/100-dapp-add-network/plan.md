# Implementation Plan: 100 — A dApp can ask Vela to add a network

**Branch**: `100-dapp-add-network` | **Date**: 2026-10-04 | **Spec**: [spec.md](spec.md)
**Design**: [research.md](research.md) · [data-model.md](data-model.md) · [contracts/core.md](contracts/core.md) · [quickstart.md](quickstart.md)

## Summary

`wallet_addEthereumChain` for a chain the wallet lacks stops being 4902 "add it in Settings" (070
[D]) and becomes Vela's add-network sheet: the core reads the page's ask, checks the chain with the
SAME step functions Settings' wizard uses (catalog first; the page's https RPC only when the catalog
does not know the chain, and only if it answers the chain id), shows the verdict, and on approval
saves through the wizard's own save, moves the site to the chain and answers `null`. Every rule is
in `vela-core` (`dapp_rpc`, `dapp_browser`, `network_admin`); each shell carries three messages
between the two machines and draws the sheet with its consent-sheet components.

## Technical Context

- **Core**: Rust `vela-core` (Crux). Touched: `dapp_rpc` (pure: ask, RPC rule, outcome table),
  `dapp_browser` (route, one-at-a-time, answer, cancel, record), `dapp_record` (two reasons),
  `network_admin` (the check as shared steps; the page's add beside the wizard). Bindings: wasm
  (`dappAddChainAsk`, `dappAddOutcomeError`); UniFFI unchanged (JSON bridges).
- **Shells**: desktop gpui (the Connection column, where the consent shows); Android Compose
  (`VelaModalSheet` like the consent); iOS SwiftUI (the Explore sheet like the consent); web
  SvelteKit + the extension (the request window / side panel like a connect).
- **Testing**: `cargo test -p vela-core` (`tests/app_dapp_add_network_100.rs`, one test per rule, and
  both machines end to end); desktop `cargo test` (`browser_host` against the real cores); Android
  `testDebugUnitTest` (real cores over JNA); iOS `xcodebuild test` (real cores); web vitest (the
  worker's routing, content's holding, the surface's relay over the real wasm).
- **Constraints**: the i18n residency budget (SC005 144,400; 144,350 measured before) — not moved;
  Android/iOS typed enums must learn new variants in the same commit; generated TS via
  `gen-core-types`; committed wasm via `build-web.mjs`, rebuilt last.

## Constitution Check

`.specify/memory/constitution.md` is the unfilled template; the project's standing rules apply:
- **The core decides, shells draw.** The ask, the RPC rule, the check, the verdict, the codes, the
  record — all core. The extension's worker keeps only what must be synchronous (known chain → switch;
  one open → -32002), from the core-published catalog, as it does for connect.
- **One copy of a rule.** The wizard's check is shared, not copied (research R2).
- **Money safety / trust.** A page never adds a network by itself; its RPC must prove the chain id;
  the catalog's data wins; the save is the wizard's.
- **i18n.** 15 locales; residency within budget by removing two dead keys.

Status: **pass**.

## Project Structure

```text
specs/100-dapp-add-network/   spec · research · plan · data-model · contracts/core.md · tasks · quickstart · results
rust/crates/vela-core/src/app/
  dapp_rpc.rs        DappChainAsk, add_chain_ask, usable_rpc_url, url_host, DappAddOutcome, add_outcome_error
  dapp_browser.rs    Route::AddChain → ForwardToAddNetwork / -32002; AddNetworkAnswered; CancelAddNetwork; adding_network
  dapp_record.rs     DbrReason::{NotCompatible, BadRpc}
  network_admin.rs   shared check steps; dapp_add state, events, DappAddSettled, NetView.dapp_add
rust/crates/vela-core/tests/app_dapp_add_network_100.rs
rust/crates/vela-core/i18n/locales/*/{connect,componentsUi,onboarding}.json
rust/crates/vela-core-wasm/src/lib.rs   dappAddChainAsk, dappAddOutcomeError
app-desktop/vela-wallet/src/  executor/dapp_browser.rs, executor/network_admin.rs, wallet/browser_host.rs, wallet/page.rs, explore/mod.rs
app-android/vela-wallet/app/src/main/java/app/getvela/wallet/  feature/browser/core/{DbrWire,BrowserExecutor,BrowserController}.kt, feature/settings/core/{NetWire,NetworkAdminExecutor,SettingsController}.kt, VelaWalletApplication.kt, feature/explore/…, navigation/VelaNavHost.kt
app-ios/VelaWallet/VelaWallet/  Features/Explore/Core/{DbrWire,DbrExecutor,BrowserController}.swift, Features/Settings/{SettingsWire,NetworkAdminExecutor,SettingsStore}.swift, Features/Explore/ExploreScreen.swift, App/RootView.swift
app-web/vela-wallet/  extension/{background,content}.js, extension/lib/protocol.js, src/lib/dapp/DappRequestHost.svelte, src/lib/settings/core/network-admin*.ts
```

## Phases

1. **Core** (tests first): the pure ask/RPC/outcome; the browser's route, answer, cancel, record;
   the wizard's check as shared steps (existing tests unchanged); the page's add in `network_admin`;
   corpus + residency; wasm exports; generated types.
2. **Desktop**: executors carry the three messages; the Connection column draws the sheet.
3. **Android**: wire mirrors, executors, the relay in `VelaWalletApplication`, the sheet in Explore.
4. **iOS**: wire mirrors, executors, the relay in `RootView`, the sheet in Explore.
5. **Web**: the worker's mirror and route, content's hold, the surface's sheet and relay.
6. **Verify**: every gate in the task list; results.md with real counts.

## Complexity Tracking

| Choice | Why | Simpler alternative rejected |
|---|---|---|
| The page's add lives in `network_admin`, beside the wizard | The ledger and the check are there; one writer of `vela.customNetworks` | A copy in `dapp_browser`: two checks, two writers |
| The check refactored into step functions | Same check for both callers, provably (old tests unchanged) | A second pipeline that "mirrors" the wizard |
| Three messages relayed by each shell | Two machines, as `ForwardToSigning` already is | Merging the machines |
