# Tasks — 072 Settings parity

## Core
- [x] T001 `network_admin`: provider blur merges into saved keys; view falls back to the saved key — regression test
- [x] T002 `prefs`: keys, vocabularies, text-scale stops, `read`, `migrations` (legacy only) — tests over every shell's spelling
- [x] T003 `storage_catalog`: rows, groups, keys/prefixes (union), cache + erase rules, records, bytes — tests
- [x] T004 UniFFI `prefs*` / `storage*`; wasm `prefsRead` / `prefsMigrations` / `storage*`

## Web (reference, fixed where it is wrong)
- [x] T010 P0 wide layout: send `providers_opened` / `endpoints_opened`; confirm per-row storage clears
- [x] T011 Wide erase: dialog, sweep, verify, failure callout (as phone)
- [x] T012 Confirm before removing a network; "Get key" opens the provider's page (a test is its own button)
- [x] T013 About links open; networks count + About's network count from the live list
- [x] T014 Storage from `storageItemOfKey` / `storageIsCacheKey` / `storageIsErasableKey` / `storageIsOurs` / `storageRecordsIn`; erase from the core rule
- [x] T015 Preferences via `prefsRead` / `prefsMigrations`
- [x] T016 Currency search box filters (or is not drawn)

## Android (me)
- [x] T020 Preferences: `vela.theme` (DataStore migrated once), `vela.language` = auto, `vela.textScale` own key — via `prefsRead` / `prefsMigrations`
- [x] T021 Storage page + erase from the catalog (1024s; erase includes the theme and live dApp grants; verify)
- [x] T022 Send `ProvidersOpened` / `EndpointsOpened`; confirm before removing a network; fix the delete label
- [x] T023 Add-network wizard: error / couldn't-verify wording (never "Incompatible" for unverified), recheck that re-checks, reset on open
- [x] T024 Account switcher: per-account totals
- [x] T025 Currency search; JVM tests (640); device pass — see quickstart

## iOS
- [x] T030 P0 the seven network events in the core's shape; providers + endpoints pages live
- [x] T031 P0 theme Auto (= system) selectable and stored as `system`
- [x] T032 P0 erase: catalog scan + verify + dApp revoke + first-run, no cancellable sign-out
- [x] T033 Network detail: RPC / explorer edit, remove with confirm; recheck re-checks; wizard reset
- [x] T034 Preferences via `prefsRead` / `prefsMigrations`; storage from the catalog
- [x] T035 Tab bar on Settings; language row names the language; currency row wording; links tappable; networks count
- [x] T036 Hermetic tests (718); simulator pass (4 UI tests + screenshots)

## Desktop
- [x] T040 Appearance panel live: language, text size, theme (system / light / dark), avatar — stored under the shared keys, applied
- [x] T041 Formats move to `vela.localePrefs` (migration via the core)
- [x] T042 Account panel: create / sign in another; totals in the display currency
- [x] T043 Erase (dialog, sweep, verify); per-row storage clears + clear all caches (confirm), bar from the catalog
- [x] T044 Confirm disconnect / disconnect-all; currency picker; About links; providers / endpoints / overrides save on blur or Enter, not per keystroke
- [x] T045 `cargo test` (492)

## Found while merging (all shells)
- [x] T050 Core: blurring a provider field that was never edited kept writing "" over its saved key (FR-001 protected only the OTHER providers) — fixed, rule test fails without it
- [x] T051 Reset endpoints asks first on every shell (FR-010): `settingsModals.endpoints.reset{Title,Body,Confirm,Cancel}` in all fifteen locales (pin 1713 → 1717)
- [x] T052 `storageIsOurs` (UniFFI + wasm), `storageRecordsIn` / `storageBytesDisplay` (wasm): web and iOS stop re-deriving the namespace and the record count

## Later (found, not in this pass)
- Android: the Ethereum backup row jumps to the Wallet tab before signing (sign in place, as web / iOS); RPC-fix sheet's provider chips and report link are not tappable; the relayer sheet is never opened live; the endpoints self-host guide link.
- Desktop: the dApp webview's own website data is not a `vela.` key, so erase does not clear it; the "Contacts" row counts `vela.contacts` only (web counts groups and dismissed suggestions too).
- Web: storage sizes are not in the person's number format (FR-013); it keeps a KB floor under one kilobyte where the core says B.
- iOS (DEBUG only): the parallel-space badge survives an erase until relaunch.
- Core: locale endonyms and provider "Get key" URLs are copied between web, desktop and iOS.
- Core: the add-network wizard's presentation (which callout, recheck, CTA) is still decided per shell — move it into `NetView`; provider metadata (display name, key URL); About content with canonical URLs.
