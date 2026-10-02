# Tasks: 091 — Debug mode: the wallet for http dApps on the local network

**Input**: [spec.md](spec.md), [plan.md](plan.md).
Format: `- [x] T### [P] [US#] description (files) — test`. US1 = reveal the switch, US2 = a LAN dApp gets
the wallet, US3 = the setting changes while a page is open.

## Phase 1 — Core (decided once)

- [x] T001 [US2] `offers_wallet(origin, debug_mode)`; the private-host rule turned into tables, same behaviour (`rust/crates/vela-core/src/app/dapp_permissions.rs`) — `tests/app_dapp_permissions.rs::offers_wallet_table` (43 origins, both modes), `insecure_origin_classification_table` unchanged
- [x] T002 [US2] `private_host_js()`: the same tables written as a JS host test (`dapp_permissions.rs`) — `dapp_rpc::tests::the_scripts_host_test_is_the_cores_tables`
- [x] T003 [US2] `provider_script(host, debug_mode)`: off = spec 088 byte for byte, on = secure context or `http:` + the host test (`dapp_rpc.rs`) — `dapp_rpc::tests::script_is_one_classic_script_per_host`
- [x] T004 [US2][US3] `Event::DebugModeChanged`; the gate asks `offers_wallet`; off retires documents no longer offered; tab view shows no connection where not offered (`dapp_browser.rs`) — `tests/app_dapp_browser.rs`: `debug_mode_offers_the_wallet_to_a_lan_page`, `debug_mode_never_offers_the_wallet_to_public_http`, `without_debug_mode_a_lan_page_is_not_answered`, `turning_debug_mode_off_withdraws_the_wallet_from_an_open_lan_page`, `turning_debug_mode_on_applies_from_the_next_document`
- [x] T005 [US1] `prefs::DebugMode` (`vela.debugMode`: hidden/off/on) and `version_tapped` (7 taps, ≤ 1,000 ms gap) (`src/prefs.rs`) — `prefs::tests::{debug_mode_reads_hidden_until_revealed, seven_quick_taps_reveal_the_switch, a_slow_tap_starts_the_count_again, once_revealed_nothing_counts}`
- [x] T006 [P] Bindings: UniFFI `dapp_provider_script(host, debug_mode)`, `dapp_offers_wallet`, `PrefsRecord.debug_mode`, `prefs_debug_mode_value`, `prefs_version_tapped`; wasm `dappProviderScript(host, debugMode)`, `dappOffersWallet`, `prefsRead().debugMode`
- [x] T007 [US1] Corpus `about.{debugMode,debugModeBody,debugModeRevealed}` × 15 locales; `gen-i18n` count 1789; SC-005 budget 139,800 — gen/lint/verify i18n, `dump:vectors`, `i18n_residency`
- [x] T008 [P] Regenerated: `rust/pkg-web`, `assets/wasm`, TS mirrors (`DbrEvent.ts`), Swift bindings — `build-web --check`, `gen-onboarding-types --check`
- [x] T009 [US2] Web: the generated script runs in a VM against `dappOffersWallet` on ~50 origins, both modes (`app-web/vela-wallet/src/lib/dapp/core-table.test.ts`)

## Phase 2 — Desktop

- [x] T010 [US1] `preferences::{debug_mode, set_debug_mode}` (`src/executor/preferences.rs`) — `the_debug_mode_switch_is_stored_in_the_cores_spelling`
- [x] T011 [US2][US3] `webview::set_debug_mode` retires a view built for the other mode; the next `place` rebuilds it at the page it showed (`src/webview.rs`, `src/webview_absent.rs`) — `a_change_of_debug_mode_rebuilds_the_view`, `the_injected_script_is_the_cores_desktop_script`
- [x] T012 [US2][US3] `BrowserHost::follow_debug_mode`: core event + webview; a retired view is `page_gone` (`src/wallet/browser_host.rs`) — `debug_mode_from_the_preferences_reaches_the_gate`
- [x] T013 [US1] About: the version is the hidden entry; `toggle_row` (the app's first switch); the 2 s notice; erase follows (`src/wallet/page.rs`, `src/settings/{mod,components}.rs`) — `the_debug_mode_words_resolve`; GUI run: 7 taps → stored `off`, row + notice; a click on the row → stored `on`

## Phase 3 — iOS

- [x] T014 [US1] Preferences read/store debug mode; `VersionTapCounter` feeds the core (`Core/Preferences.swift`, `Core/VelaStore.swift`)
- [x] T015 [US2][US3] `ProviderBridge` script per mode + swap; `BrowserEngine`/`BrowserController` follow the mode (`Features/Explore/Core/*`)
- [x] T016 [US1] About: hidden entry, `SettingsSwitchRow`, `NoticeCapsule` (shared with Explore) (`Features/Settings/*`, `Components/*`, `App/RootView.swift`)
- [x] T017 Tests — `ProviderScriptTests` (incl. the JavaScriptCore run against `dappOffersWallet`), `DebugModeTests`, `BrowserWireDriftTests`; full `VelaWalletTests` 1113 passed

## Phase 4 — Android

- [x] T018 [US1] Preferences read/store debug mode (`core/data/Preferences.kt`)
- [x] T019 [US2][US3] `ProviderBridge` keeps its `ScriptHandler` and swaps the script; the controller follows the mode (`feature/browser/core/*`)
- [x] T020 [US1] About: hidden entry, `VelaSwitchRow`, the page's notice bar (now draws a notice with no action); gallery ST14B (`feature/settings/*`)
- [x] T021 Tests — `BrowserMachineTest` (scripts per mode; two fake WebViews against the real core), `PreferencesTest`, `SettingsFixturesTest` (ST14B), `SettingsLiveTest`; full suite 917 passed

## Phase 5 — Docs

- [x] T022 `specs/088-store-readiness/results.md`: owner ruling 2026-10-02 → spec 091
- [x] T023 `docs/dapp-browser/ARCHITECTURE.md` (who is offered the wallet); `docs/store-submission/privacy-and-review.md` §3b (the hidden debug mode, Apple 2.3.1(a)); iOS `Info.plist` comment
