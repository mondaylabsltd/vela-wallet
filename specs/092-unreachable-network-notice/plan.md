# Implementation Plan: 092 — Networks the wallet cannot reach

**Branch**: `092-unreachable-network-notice` | **Date**: 2026-10-02 | **Spec**: [spec.md](spec.md)

## Summary

`balance_dashboard` already knew which chains failed (`failed_chain_ids` minus `rate_limited_chain_ids`, then called `banner_chain_ids`). Every shell turned that into "N networks RPC unavailable". On tap, web, desktop, iOS and Android all opened the RPC fix for `banner_chain_ids[0]` only. iOS did not even show the line; it said "still updating".

The core now owns the list, and the shells only draw it:
- `banner_chain_ids` is replaced by `unreachable_networks`: ordered rows, each with `chain_id`, `last_known` (held / empty / not_read), `last_seen_usd` and the corpus key of its line.
- `unreachable_key` is the Home sentence: one network or several.
- The core keeps a per-account record of what each chain last answered with (`last_read`). Shells now report the chains each round asked (`FetchSettled.read_chain_ids`).
- `UnreachableListOpened` / `UnreachableListClosed` make the core re-read while the list is on screen. It reads once on open, then again 10 s after each read. A partial retry, when armed, comes first.

## Technical Context

- **Core**: `rust/crates/vela-core/src/app/balance_dashboard.rs`. All state is in the machine; no new effect (the re-read timer is the existing `StartRetryTimer` with its own id).
- **Shells**: web + extension side panel (`app-web/vela-wallet`), desktop (`app-desktop/vela-wallet`), iOS (`app-ios`), Android (`app-android/vela-wallet`).
- **i18n**: `assets.json` in 15 locales. The new strings are + `unreachable{One,Many,Body,None}` and + `lastSeen`, `lastSeenUnpriced`, `lastSeenEmpty`, `notReadYet`. `rpcUnavailable{Single,Multiple}` are removed. The count variable is `{{n}}`, not `{{count}}`, because the count is always ≥ 2 and each locale words it without plural forms (lint A5). The ja + en budget moves 138,800 → 139,800 (owner, 2026-10-02).

## Where the list lives, per shell (follows each shell's existing rescue navigation)

| Shell | Tap on Home | Row "Fix" | Back from the fix |
|---|---|---|---|
| Web (phone) | BottomSheet over Home (`rescue = 'unreachable'`) | swaps the sheet to SR2 for that chain | Done after restore → back to the list |
| Web (wide) + extension panel | Dialog over Home | same | same |
| Desktop | dialog over Home (`unreachable_dialog`), under the settings dialog layer | `SettingsDialog::FixRpc` opens over the list | closing the editor shows the list again |
| iOS | the existing rescue `.sheet` over Home, content `.unreachable` (SR6) | content swaps to SR2 (`rescueStep`); no second sheet | ✕ / Save → back to the list; a swipe closes the whole sheet |
| Android | one sheet over the Wallet route (the settings sheet host, `SettingsSheet`, made internal); overlay `Unreachable` (SR6); no tab switch | the same sheet swaps to RpcFix for that chain (`WalletRescue.fix`) | Done / ✕ → back to the list over Home; swipe, scrim or Back close the whole sheet (as on iOS) |

Android used to push Settings for every status-line rescue (spec 048), and F08 named that as part of the defect. On the lead's ruling (2026-10-02), the rescues (SR6, a row's SR2, and SR3 when nothing is down) are now one sheet over the wallet. Its content swaps and never stacks, the same pattern as the account switcher. `pendingSettingsOverlay` and the Settings route's rescue wiring are removed. Settings → Networks is unchanged.

## Constitution Check

1. **Rules decided once in the core.** The core alone decides set, order, count, the Home sentence, each row's sentence, the last-known record and the re-read cadence. **PASS**
2. **Parity.** Four shells. The extension side panel is the web route. **PASS**
3. **Less duplication.** `banner_chain_ids` is removed, not kept beside the new list. The shells' carry-over rows feed the core's record and are not re-derived. **PASS**
4. **Stable in an unstable environment.** The list never claims emptiness it cannot know. "Not read yet" is said plainly. **PASS**

## Verification

- Core machine tests: count, order, live removal, one vs several, held / unpriced / empty / not-read, privacy, spam, account switch, carried rows, re-read cadence.
- Shell unit tests on each shell. A web e2e on a real preview build: the line, the list with live data, a network coming back and leaving the open list.
- Screenshots: iOS simulator (gallery H9 / SR6), desktop gallery (DSR6), web e2e at phone and desktop width. Android has no screenshot harness; the device check is the lead's.
