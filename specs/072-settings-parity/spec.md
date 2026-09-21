# Feature Specification: Settings, the same on every Vela

**Feature Branch**: `072-settings-parity`
**Created**: 2026-09-22
**Status**: Core done; shells in progress
**Input**: Owner, 2026-09-22 — "还有设置页面功能也要做好，对齐颗粒度，现在看起来最完善的设置在 web 版本，desktop ios iphone 感觉都是个残缺品。"

## Why

The web's Settings is the reference; the three native Settings are partly
drawn from fixtures, partly inert, and partly wired to different rules. The
audit (research.md) found worse than "incomplete": **data loss** (saved RPC
provider keys deleted by focusing a field — web's wide layout, Android),
**events the core cannot read** (iOS: endpoint edits, provider keys, tests,
the RPC-fix save never reach the core), **settings that do nothing** (iOS
theme "Auto"; the whole desktop Appearance panel; erase on the desktop and on
web's wide layout), **destructive actions without a question** (web wide's
per-row clear of the whole address book; desktop disconnects; removing a
network on web / Android / iOS), and **one preference meaning four things**
(Android's `system` vs `auto`, its text size inside `vela.localePrefs`, its
theme outside the store; the desktop's `vela.formats`, and no stored theme,
language, text size or avatar at all).

The owner's rule for this program: *a stable experience in an unstable
environment; a clear, simple, maintainable architecture; less duplicated
code.* So the rules that four shells copied move into the core, and each
shell draws what the core says.

## User stories

### US1 — Nothing I saved disappears (P0)
Opening Settings on any shell, in any layout, and touching any field never
removes a saved RPC provider key, endpoint or override. A destructive action
(erase, clear a user-data row, disconnect a site or all sites, remove a
network) always asks first, in words that name what goes.

### US2 — Every row does what it says (P0/P1)
Every row a shell draws is live: a choice is stored and applied, a link
opens, a button acts. What a shell cannot do yet it does not draw.

### US3 — One preference, one meaning (P1)
Theme, language, text size, avatar style and the three formats are stored
under the same keys with the same values on every shell; a store written by an
older build reads correctly and is migrated once.

### US4 — The same rows on every shell (P1)
Web phone's granularity is the target: account (switcher with totals, create
/ sign in another, keys, backup, sign out, erase), appearance (language, text
size, theme incl. system, avatar), localization (currency, number/date/time),
transactions (speed, sign with, Clear Signer page), networks (list, detail
with RPC/explorer edit and remove, add wizard with its checks), RPC providers,
service endpoints, device storage (per-row clear, clear caches, connections),
about (version, links).

## Requirements

### Core (done in this branch)
- **FR-001** `network_admin`: a provider-key blur merges into the SAVED keys
  (only providers with a draft change); the view shows the saved key when no
  draft is open. No shell can wipe keys by forgetting `ProvidersOpened`.
- **FR-002** `vela_core::prefs`: keys, vocabularies, defaults, text-scale
  stops; `read(entries)` accepts every shell's older spelling;
  `migrations(entries)` rewrites only known legacy spellings (a newer build's
  value survives). UniFFI `prefsRead` / `prefsMigrations` /
  `prefsTextScaleLevels` / `prefsLocaleJson`; wasm `prefsRead` /
  `prefsMigrations`.
- **FR-003** `vela_core::storage_catalog`: the rows, groups, keys and
  prefixes (the union of every shell's keys), `is_cache_key`,
  `is_erasable_key` (scan `vela.`, keep `vela.pendingUploads`),
  `records_in`, `bytes_display` (1024s). UniFFI `storage*`; wasm `storage*`.
  `vela.balanceHidden` is a preference, never a cache; the "custom" row is
  tokens, networks and network overrides — not provider keys or endpoints.

### Shells
- **FR-010** Every destructive action confirms (erase; clear a user-data row;
  disconnect one / all sites; remove a network; reset endpoints).
- **FR-011** Erase = scan every store for `is_erasable_key`, delete, verify
  (report what survived), revoke live dApp sessions, then the first-run
  state — no cancellable sign-out afterwards, no hand-kept list.
- **FR-012** Preferences read through `prefs_read`, migrated at launch with
  `prefs_migrations`, written under the shared keys (Android's DataStore theme
  moves to `vela.theme`).
- **FR-013** Storage page from the catalog: rows, groups, per-row clear,
  clear-all-caches, bytes in 1024s formatted in the person's number format.
- **FR-014** Shell-specific parity lists in tasks.md (from research.md's gap
  list), P0 first.

## Success criteria
- **SC-001** No shell's Settings contains an inert control or fixture value
  that a person can mistake for their own data.
- **SC-002** The provider-key, iOS-event and theme-Auto defects each have a
  test that fails without the fix.
- **SC-003** A store written by Android, iOS, desktop or web reads the same
  five preferences on every other shell (core tests over each spelling).
- **SC-004** Device pass on the Android phone and the iOS simulator: erase,
  storage clears, theme/language/text size, provider keys, network edit/remove.

## Out of scope (owner decisions)
- Terms / Privacy rows (no corpus, no URLs exist).
- Feedback / bug report send (the row was removed by the founder's ruling on
  web and Android; not reinstated here).
- A developer section in Settings.
