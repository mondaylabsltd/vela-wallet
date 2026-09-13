# Implementation Plan: Android Settings Audit and the Send Path (049)

**Branch**: `049-android-settings-audit-send` | **Date**: 2026-09-13 | **Spec**: [spec.md](spec.md)

## Summary

Make every preference take effect where it shows (avatar style read by every
avatar; the number/date/time presets applied to every figure, observable, with
live examples and a real clear-signing locale), drive the whole settings
inventory on the Xiaomi, and prove the send path with a real 0.001 XDAI
transfer on Gnosis.

## Technical Context

**Language/Version**: Kotlin (Compose), Android minSdk 29; no core change.
**Primary Dependencies**: existing `Preferences`, `Formats`, `IdenticonImage`, the live builders.
**Storage**: unchanged keys (the web's).
**Testing**: JUnit (`:app:testDebugUnitTest`), the two rulers, the phone pass.
**Target Platform**: Android (Xiaomi alioth `9d5f42fb`, parallel space).
**Constraints**: web = checklist; no fixture redraw; the editable amount field keeps raw digits.

## Constitution Check

- Web parity by reading the web's code, not memory (R1, R2, R4).
- Every phase ends on the phone (the founder's standing rule).
- One implementation: the builders keep reading `Formats.current`; only the
  holder's nature changes.

## Project Structure

```text
specs/049-android-settings-audit-send/
├── plan.md · research.md · data-model.md · quickstart.md · tasks.md · results.md
├── contracts/settings-inventory.md
└── checklists/requirements.md

app-android/vela-wallet/app/src/main/java/app/getvela/wallet/
├── core/format/Formats.kt                       # Compose-state holder, decimalMark/plain/examples
├── core/data/Preferences.kt                     # publish order
├── core/identicon/IdenticonImage.kt             # LocalAvatarStyle, name, InitialsDisc, viewer (seed,name)
├── feature/wallet/components/IdenticonAvatar.kt, IdenticonViewerSheet.kt, BalanceDisplay.kt, WalletHeaderRow.kt, AccountSwitcherSheet.kt
├── feature/wallet/WalletModels.kt (decimalMark), WalletLive.kt (mark on hero/amounts)
├── feature/flows/FlowModels.kt, FlowLive.kt, components/FlowBlocks.kt, FlowRows.kt
├── feature/send/SendLive.kt                     # trim/fromBase/summary marks
├── feature/signing/core/ClearWire.kt, SigningController.kt   # ClearLocale.fromFormats
├── feature/signing/components/SigningComponents.kt
├── feature/contacts/components/ContactRow.kt, ContactDetailScreen.kt
├── feature/explore/components/BrowserChrome.kt, ExploreSheets.kt
├── feature/onboarding/flow/DoneScreen.kt
├── feature/settings/SettingsLive.kt, components/SettingsRows.kt, SettingsScreen.kt
├── navigation/VelaNavHost.kt                    # viewer host (seed,name), remember keys, sheet ids
└── MainActivity.kt                              # LocalAvatarStyle provider
```

## Phases

1. **Avatar style** (US1): the composition local, `InitialsDisc`, the viewer,
   nineteen sites → phone: four sites in each style + relaunch.
2. **Formats** (US2): Compose-state holder, marks on hero/token amounts/fees,
   remember keys, live sheet examples with wire-key ids, tx-detail fiat via
   `Money`, `ClearLocale.fromFormats` → unit tests → phone: hero/asset/feed/
   send/confirm/detail under `dot_comma`, flip back, `h12`.
3. **Inventory pass** (US3): the 19 rows of the contract driven on the phone.
4. **Send** (US4): 0.001 XDAI to 觉得九点半 on Gnosis; RPC balance before/after.
5. **Closeout**: rulers, results.md, KNOWN-BUGS (web clear-signing locale), memory.

## Complexity Tracking

None: no new machine, no new module; one composition local and one Compose
state holder replace a static and a never-read preference.
