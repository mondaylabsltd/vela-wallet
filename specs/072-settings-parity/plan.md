# Implementation Plan: 072 — Settings parity

**Branch**: `072-settings-parity` (on `071-clear-signer`) | **Spec**: [spec.md](spec.md) | **Research**: [research.md](research.md) | **Tasks**: [tasks.md](tasks.md)

## Summary

Move the rules four shells copied — preference spellings, the storage catalog
and erase rule, the provider-key merge — into the core; then make every
shell's Settings live at web phone's granularity, P0 defects first.

## The shared surface (contract)

| Rule | Core | UniFFI (Kotlin / Swift) | wasm | Desktop |
|---|---|---|---|---|
| Preferences read | `prefs::read` | `prefsRead(entries: Map<String,String>) -> PrefsRecord` | `prefsRead(entriesJson) -> json` | direct |
| Legacy rewrite | `prefs::migrations` | `prefsMigrations(entries) -> [KeyWrite{key, value?}]` | `prefsMigrations` | direct |
| Text-size stops | `prefs::TEXT_SCALE_LEVELS` | `prefsTextScaleLevels()` | — | direct |
| Locale record | `prefs::locale_prefs_json` | `prefsLocaleJson(n, d, t)` | — | direct |
| Storage rows | `storage_catalog::ITEMS` | `storageItems()` | `storageItems()` | direct |
| Key → row | `item_of_key` | `storageItemOfKey(key)` | `storageItemOfKey` | direct |
| Clear-caches rule | `is_cache_key` | `storageIsCacheKey` | `storageIsCacheKey` | direct |
| Erase rule | `is_erasable_key` | `storageIsErasableKey` | `storageIsErasableKey` | direct |
| Records / bytes | `records_in`, `bytes_display` | `storageRecordsIn`, `storageBytesDisplay` | — | direct |

Keys (record format): `vela.theme` (system/light/dark), `vela.language`
(auto/tag), `vela.localePrefs` ({numberFormat,dateFormat,timeFormat}),
`vela.textScale` (compact…xlarge), `vela.avatarStyle` (initials/identicon).

Destructive actions that must confirm: erase; clear a user-data row; disconnect
one / all sites; remove a network; reset endpoints.

## Phases

| Phase | Deliverable | Gate |
|---|---|---|
| 0 | Core: provider merge, `prefs`, `storage_catalog`, exports | core tests, clippy, wasm size |
| 1 | Web wide fixes + core rules | vitest, e2e settings |
| 2 | Android | JVM tests; device pass |
| 3 | iOS P0s + parity | hermetic tests; simulator |
| 4 | Desktop parity | `cargo test` |

Risk: **High** for erase and provider keys (data loss if wrong); each change
there carries a test that fails without it.
