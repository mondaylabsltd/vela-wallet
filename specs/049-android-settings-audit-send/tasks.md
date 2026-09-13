# Tasks: Android Settings Audit and the Send Path (049)

**Input**: spec.md, plan.md, research.md, data-model.md, contracts/settings-inventory.md, quickstart.md
**Rule**: every phase ends on the Xiaomi (parallel space) before the next starts; the send is real money on Gnosis.

## Phase 1 — Setup

- [ ] T001 Branch `049-android-settings-audit-send` on 048, `.specify/feature.json`, the spec set

## Phase 2 — US1 Avatar style (P1)

- [ ] T002 [US1] `core/identicon/IdenticonImage.kt`: `LocalAvatarStyle`, `name` parameter, `InitialsDisc` (accent disc, letter at 0.34, `V` fallback), viewer local `(seed, name)`; `feature/wallet/components/IdenticonAvatar.kt` passes `name`
- [ ] T003 [US1] `MainActivity.kt` provides `LocalAvatarStyle` from `preferences.view`; `VelaNavHost.kt` hosts the viewer as `(seed, name)`; `IdenticonViewerSheet.kt` draws the styled avatar
- [ ] T004 [US1] Sites pass the name: `SettingsRows.kt` account row, `SettingsScreen.kt` accounts sheet, `WalletHeaderRow.kt`, `AccountSwitcherSheet.kt`, `ContactRow.kt`, `ContactDetailScreen.kt`, `SigningComponents.kt` signer row, `BrowserChrome.kt`, `ExploreSheets.kt`, `FlowBlocks.kt` (address card, recipient field via `RecipientFieldModel.name`), `FlowRows.kt` (fact lead via `FactLead.Identicon.name`, split card, contact pick row), `DoneScreen.kt`
- [ ] T005 [US1] Unit: `IdenticonInitialsTest` (letter rule); phone: four sites in each style, relaunch

## Phase 3 — US2 Formats (P1)

- [ ] T006 [US2] `Formats.kt`: `current` as Compose state; `decimalMark()`, `plain()`, `example()/dateExample()/timeExample()`; `Preferences.publish` sets formats before the view
- [ ] T007 [US2] `WalletModels.kt` `BalanceModel.decimalMark`; `BalanceDisplay.kt` draws it; `WalletLive.kt` sets it and marks `trimAmount/signedAmount`
- [ ] T008 [US2] `SendLive.kt` `trim/fromBase/splitSummary/confirm/splitRow/amountModel.fiat` marks; `FlowLive.kt` tx-detail amount mark + fiat via `Money` (NavHost passes `currency`)
- [ ] T009 [US2] `SettingsLive.withPreferences`: live examples, wire-key row ids; `VelaNavHost.kt` sheet select by key; remove `numberKeyAt/dateKeyAt/timeKeyAt`
- [ ] T010 [US2] `VelaNavHost.kt`: `Formats.current` in both flow `remember` key lists
- [ ] T011 [US2] `ClearWire.kt` `ClearLocale.fromFormats`; `SigningController.clearKickoff` uses it
- [ ] T012 [US2] Unit: `FormatsTest` (mark/plain/examples), `WalletLiveTest` (hero mark, feed mark under dot_comma), `SendLiveTest` (trim mark), `SettingsLiveTest` (sheet ids/examples), `ClearLocaleTest`; phone per quickstart

## Phase 4 — US3 Inventory pass (P1)

- [ ] T013 [US3] Drive rows 1–19 of `contracts/settings-inventory.md` on the phone; fix what fails; record reasons

## Phase 5 — US4 Send (P1)

- [ ] T014 [US4] 0.001 XDAI → 觉得九点半 on Gnosis; balance before/after; receipt 已确认; feed row

## Phase 6 — Closeout

- [ ] T015 Rulers; `results.md`; `docs/KNOWN-BUGS.md` (WEB-CLEAR-LOCALE-1); memory; commits per phase
