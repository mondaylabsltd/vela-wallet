# Research — 072 Settings parity (the audit, 2026-09-22)

Read-only audit of `071-clear-signer` (the base of this branch). File aliases:
W = `app-web/vela-wallet/src` (W:home `lib/settings/SettingsHome.svelte`, W:desk
`lib/settings/SettingsDesktop.svelte`, W:page `routes/[locale]/settings/+page.svelte`),
A = `app-android/…/feature/settings` + `navigation/VelaNavHost.kt` (A:nav),
I = `app-ios/VelaWallet/VelaWallet/Features/Settings` + `App/RootView.swift` (I:root),
D = `app-desktop/vela-wallet/src` (D:page `wallet/page.rs`), core:net =
`rust/crates/vela-core/src/app/network_admin.rs`.

Status words: LIVE (wired), PART (live with a hole), FIX (fixture), INERT (drawn,
does nothing), BROKEN (does the wrong thing), MISSING.

## Most serious findings

1. **Saved RPC-provider keys deleted by focusing a field** (web wide, Android).
   The core showed provider keys only from `provider_drafts`, filled only on
   `ProvidersOpened`; a blur wrote the whole draft map. Web wide never sends
   `ProvidersOpened` (W:desk 140-145); Android never does (A:ctl 321 uncalled).
   **Fixed in the core** (FR-001): merge into saved keys; view falls back.
2. **iOS sends network events the core can't decode** (I:store 249-290):
   `endpoint_edited/_blurred`, `provider_key_edited/_blurred`,
   `provider_test_requested` send `"id"` (core wants `field` / `provider`);
   `override_field_edited` omits `field`; `add_by_chain_id_requested` sends
   `text` (core wants `chain_id` + `now_iso`). The endpoints and providers pages
   are pure fixture ("Alchemy · Connected · alch_k3y…9fQ2" for everyone).
3. **iOS theme "Auto" is a no-op** — segment id `auto`, `ThemeChoice` has
   `system/light/dark` (I:prefs 22, I:root 2681-2684); once Light/Dark is
   picked, "follow the system" is unreachable.
4. **Erase is inconsistent**: web wide INERT (W:desk 215-219); desktop INERT
   (D:page 6850); iOS deletes a hand-kept list of 18 keys then raises a
   cancellable sign-out AFTER the data is gone (I:root 2942-2960, 786-793), no
   verification; Android sweeps its KV store but the DataStore theme survives
   and live dApp grants are not revoked.
5. **Preference keys differ per shell** (table below).
6. **Destructive actions without confirmation**: web wide per-row storage clear
   (W:desk 353-357); desktop site disconnect / disconnect-all (D:page
   7992-8061); remove a custom network on web / Android / iOS (desktop confirms).
7. **Android's add-network wizard never shows why a check failed**: live model
   never sets callout / recheck / setup tool (A:fix 925-929); an unverifiable
   chain is badged "Incompatible" (A:live 288-299); the recheck handler would
   parse search text as a chain id and add without confirming (A:nav 1823).

## Parity matrix (condensed)

| Row | Web phone | Web wide | Android | iOS | Desktop |
|---|---|---|---|---|---|
| Account switcher | LIVE | LIVE | LIVE; no per-account totals | LIVE | LIVE; USD only |
| Create / sign in another | LIVE | LIVE | LIVE | LIVE | MISSING in live panel |
| Keys / Ethereum backup | LIVE | LIVE | backup jumps to Wallet tab | LIVE | LIVE |
| Sign out | LIVE | LIVE | LIVE | LIVE | LIVE |
| Erase | LIVE | INERT | PART | BROKEN | INERT |
| Language | LIVE | LIVE | LIVE | LIVE; row reads "System" | INERT |
| Text size | LIVE | LIVE | LIVE | LIVE | INERT |
| Theme | LIVE | LIVE | LIVE | BROKEN (Auto) | INERT |
| Avatar | LIVE | LIVE | LIVE | LIVE | INERT |
| Currency | LIVE | LIVE | LIVE (8 fixed) | LIVE (8 fixed; USD fallback wording) | PART (no menu) |
| Currency search | INERT | — | INERT | INERT | — |
| Number/date/time | LIVE | LIVE | LIVE | LIVE | LIVE (`vela.formats`) |
| Speed / Sign with / Clear Signer page | LIVE | LIVE | LIVE | LIVE | LIVE |
| Networks count | FIX 12 | — | LIVE | FIX 12 | — |
| RPC/explorer override edit | LIVE | LIVE | LIVE | INERT | LIVE per keystroke |
| Remove custom network | no confirm | no confirm | no confirm; a11y "Add network" | INERT | confirm |
| Add-network wizard | LIVE; setup-tool INERT | same | PART (finding 7) | PART; recheck doesn't re-check | LIVE |
| Provider keys | LIVE | BROKEN | BROKEN | FIX + undecodable | LIVE per keystroke |
| Provider test / Get key | "Get key" runs a test | same | same | undecodable; link text | test LIVE; no link |
| Service endpoints | LIVE | pills blank (no `endpoints_opened`) | same | FIX | LIVE per keystroke |
| Storage headline/bar | LIVE | LIVE | LIVE (1000s) | LIVE | PART (bar fixture) |
| Clear a user row | confirm | NO confirm | confirm | confirm | INERT |
| Caches / clear all | confirm | per-row no confirm | LIVE | LIVE | INERT |
| Connections | confirm | no confirm | LIVE | LIVE | no confirm |
| About version | LIVE | LIVE | LIVE | LIVE | LIVE |
| About links | INERT | INERT | LIVE | INERT | INERT |
| Tab bar on Settings | LIVE | sidebar | LIVE | only Wallet responds | sidebar |

## Storage keys per preference (before 072)

| Preference | Web | Android | iOS | Desktop |
|---|---|---|---|---|
| Theme | `vela.theme` system/light/dark | DataStore `theme_preference` light/dark/auto | `vela.theme` | not stored |
| Language | `vela.language` auto/tag | `vela.language` **system**/tag | auto/tag | not stored |
| Formats | `vela.localePrefs` {numberFormat,dateFormat,timeFormat} | same **+ textScale** | same | **`vela.formats` {number,date,time}** |
| Text scale | `vela.textScale` | inside localePrefs | `vela.textScale` | not stored |
| Avatar | `vela.avatarStyle` | same | same | not stored |
| Hide balance | user data | balances cache | cache | — |

## Rules moved into the core (this branch)

- network_admin provider merge (FR-001).
- `prefs` codec (FR-002) and `storage_catalog` (FR-003).

Candidates left for later: wizard presentation state and health badge in
`NetView`; provider metadata (display name, key URL); About content with
canonical URLs; a destructive-action confirmation table.
