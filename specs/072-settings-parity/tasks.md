# Tasks — 072 Settings parity

## Core
- [x] T001 `network_admin`: provider blur merges into saved keys; view falls back to the saved key — regression test
- [x] T002 `prefs`: keys, vocabularies, text-scale stops, `read`, `migrations` (legacy only) — tests over every shell's spelling
- [x] T003 `storage_catalog`: rows, groups, keys/prefixes (union), cache + erase rules, records, bytes — tests
- [x] T004 UniFFI `prefs*` / `storage*`; wasm `prefsRead` / `prefsMigrations` / `storage*`

## Web (reference, fixed where it is wrong)
- [ ] T010 P0 wide layout: send `providers_opened` / `endpoints_opened`; confirm per-row storage clears
- [ ] T011 Wide erase: dialog, sweep, verify, failure callout (as phone)
- [ ] T012 Confirm before removing a network; "Get key" opens the provider's page (a test is its own button)
- [ ] T013 About links open; networks count + About's network count from the live list
- [ ] T014 Storage from `storageItemOfKey` / `storageIsCacheKey` / `storageIsErasableKey`; erase from the core rule
- [ ] T015 Preferences via `prefsRead` / `prefsMigrations`
- [ ] T016 Currency search box filters (or is not drawn)

## Android (me)
- [ ] T020 Preferences: `vela.theme` (DataStore migrated once), `vela.language` = auto, `vela.textScale` own key — via `prefsRead` / `prefsMigrations`
- [ ] T021 Storage page + erase from the catalog (1024s; erase includes the theme and live dApp grants; verify)
- [ ] T022 Send `ProvidersOpened` / `EndpointsOpened`; confirm before removing a network; fix the delete label
- [ ] T023 Add-network wizard: error / couldn't-verify wording (never "Incompatible" for unverified), recheck that re-checks, reset on open
- [ ] T024 Account switcher: per-account totals
- [ ] T025 Currency search; JVM tests; device pass

## iOS
- [ ] T030 P0 the seven network events in the core's shape; providers + endpoints pages live
- [ ] T031 P0 theme Auto (= system) selectable and stored as `system`
- [ ] T032 P0 erase: catalog scan + verify + dApp revoke + first-run, no cancellable sign-out
- [ ] T033 Network detail: RPC / explorer edit, remove with confirm; recheck re-checks; wizard reset
- [ ] T034 Preferences via `prefsRead` / `prefsMigrations`; storage from the catalog
- [ ] T035 Tab bar on Settings; language row names the language; currency row wording; links tappable; networks count
- [ ] T036 Hermetic tests; simulator pass

## Desktop
- [ ] T040 Appearance panel live: language, text size, theme (system / light / dark), avatar — stored under the shared keys, applied
- [ ] T041 Formats move to `vela.localePrefs` (migration via the core)
- [ ] T042 Account panel: create / sign in another; totals in the display currency
- [ ] T043 Erase (dialog, sweep, verify); per-row storage clears + clear all caches (confirm), bar from the catalog
- [ ] T044 Confirm disconnect / disconnect-all; currency picker; About links; providers / endpoints / overrides save on blur or Enter, not per keystroke
- [ ] T045 `cargo test`
