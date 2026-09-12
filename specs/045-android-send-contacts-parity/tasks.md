# Tasks: Android Send Parity, Batch and Contacts Parity

**Input**: spec.md, plan.md, research.md, data-model.md, contracts/, quickstart.md
**Rule**: every phase ends on the Xiaomi (serial 9d5f42fb), parallel space, Gnosis.

## Phase 0 — Wires (blocking)

- [x] T001 Bridge `batch_import` as `BatchImportCore` in `rust/crates/vela-core-uniffi/src/onboarding_bridge.rs`; regenerate Kotlin bindings; move the web wasm fingerprint (`node rust/scripts/build-web.mjs`)
- [x] T002 [P] `app-android/.../feature/send/core/BatchWire.kt`: `BatchEvent` (10), `BatchOperation` (3), `BatchShellResult` (6, `FileContent` text|matrix), `BatchView` (+ preview rows, recipients); numerics from Rust (`u32` → Int, `f64` → Double)
- [x] T003 [P] `app-android/.../feature/contacts/core/ContactsWire.kt`: + `ImportFile`, `ImportAcknowledged`, `ExportRequested`, `ExportTaken`, `AddGroupMembers`, `RemoveGroupMember`, `SetContactGroups`; view + `sections`, `import_failure`, `export`; types `ContactExportFile`, `ContactExportScope`, `ContactFileFormat`, `ContactImportFailure`, `ContactSection`
- [x] T004 `CoreWireDriftTest`: the batch family (events/operations/results exhaustive, view subset) and the contacts additions; `BridgeSmokeTest` opens `BatchImportCore`
- [x] T005 Rebuild the device .so (`./gradlew :app:assembleDebug`), install, smoke on the Xiaomi

## Phase 1 — US1 Split (P1)

- [x] T006 [P] [US1] `feature/send/core/SplitRows.kt`: `amountEdited(rows, id, amount)`, `rowRemoved(rows, id)`, `rowAppended(rows)` (new id `""`), untouched rows byte-identical; `SplitRowsTest`
- [x] T007 [US1] `SendController`: `enterSplit()`, `splitAmount(id, text)`, `splitRemove(id)`, `splitAdd()`, `seedSplit(recipients)`, `pickContacts`-seeding via `SeedSplitRecipients`; the split form's recipient address edit through `RecipientsChanged`
- [x] T008 [US1] `SendLive`: split form from `SendView.recipients` (cards: ordinal, name/short address, identicon seed, amount, remove label), summary (count, total, fee), actions Add/Contacts/Import, CTA gate = `can_continue`; the confirm's `To` = `RECIPIENT_COUNT`; `SendLiveTest` split cases
- [x] T009 [US1] `FlowHost` + `VelaNavHost`: row callbacks (`onRecipientAmount(id)`, `onRecipientRemove(id)`, `onRecipientAction(Add|Contacts|Import)`), the mode switch from the form (single → split)
- [x] T010 [US1] Device: XDAI split to founder + Safe (0.001 each) → one op hash, two feed rows (SC-001); record in results.md

## Phase 2 — US2 Sweep pick (P1)

- [x] T011 [US2] `SendController`: `sweepPicking` flag, `startSweep()`, `toggleSweep(tokenId)` (first tick → `SetMultiNetwork(chain)` then `ToggleMultiToken`), `selectAllValuable(visibleIds)`, `clearSweep()`, `confirmSweep()` → `ConfirmMultiSelection`; `SendMachineTest` sweep cases (pin, dim refused, select-all scoped)
- [x] T012 [US2] `SendLive`: pick in multi mode — `SendSelectionModel(selected, dimmed, selectAll)`, rows off-chain dimmed and not tappable, notice `MULTI_SEND_SUMMARY`-style chain line, CTA counts `multi_selected_ids`; sweep form (SD2D) rows from `multi_specs` + shared recipient; `SendLiveTest`
- [x] T013 [US2] `FlowHost` + `VelaNavHost`: pick callbacks (`onToggle`, `onSelectAll`, `onSweepContinue`), the "send several" entry from the pick
- [x] T014 [US2] Device: tick XDAI pins Gnosis, other chains dim, select-all scoped, clear re-pins (SC-002); sweep send test-covered with two scripted tokens

## Phase 3 — US3 Batch import (P2)

- [ ] T015 [P] [US3] `feature/documents/DocumentPorts.kt`: Activity-bound `pick(mimes) → Picked(name, bytes)?`, `create(name, mime, bytes) → Boolean`, `share(name, mime, bytes)` (FileProvider + `ACTION_SEND`); `res/xml/file_paths.xml`; manifest provider
- [ ] T016 [P] [US3] `feature/send/core/XlsxMatrix.kt`: zip + `xl/sharedStrings.xml` + `xl/worksheets/sheet1.xml` → `List<List<String>>`; `XlsxMatrixTest` with a tiny generated workbook
- [ ] T017 [US3] `feature/send/core/BatchExecutor.kt`: `FetchUsdFiatRate` via the wallet's fiat-rate port, `PickFile` → `DocumentPorts.pick` (csv/txt → Text, xlsx → Matrix; cancelled/failed), `SaveTemplateFile` → `DocumentPorts.create`; `BatchMachineTest`
- [ ] T018 [US3] `SendController` hosts `batch` (`openBatch()` sends `OpenBatchImport` + `BatchEvent.Open{token, currency, max}`; `batchUnit/fiat/rawText/pickFile/saveTemplate/editRate/resetRate/apply/close`); `Apply` → `SeedSplitRecipients` from `BatchView.recipients`
- [ ] T019 [US3] `SendLive.batchImport(view)` mirroring the web's `liveBatchImport` (units, rate states loading/ok/failed, preview rows ok/bad, rejected text, CTA count/disabled); `FlowHost` batch sheet callbacks; `SendLiveTest`
- [ ] T020 [US3] Device: push `two-rows.csv`, pick through DocumentsUI, preview 2, apply → 2 split rows → send lands (SC-003); template saved through the creator

## Phase 4 — US4 Treasury exit + stale quote (P2)

- [ ] T021 [US4] `SendController.dismissTreasury()` → `DismissTreasurySheet`; confirm notice gets the second action (`componentsUi.funding.cancel`); `SendLive` confirm `noticeSecondary`; stale `FeeView.stale` on the confirm page → `requestQuote()` once per stale flip; `SendMachineTest`
- [ ] T022 [US4] Device: stale quote refreshes on the confirm page (wait the TTL, fee line updates, slide still works) (SC-004)

## Phase 5 — US5a Contact form and star, drawn (P1)

- [ ] T023 [P] [US5] `feature/contacts/ContactsModels.kt`: `ContactFormModel`, `ContactDetailModel.favourite: FavouriteControlModel(on, label)`, `ContactInspectionModel(tag, firstTime?)`
- [ ] T024 [P] [US5] `feature/contacts/ContactsFixtures.kt`: C7 add (empty, save disabled), C8 edit (filled), C9 detail with the star on + inspection; gallery entries in the contacts gallery list
- [ ] T025 [US5] `feature/contacts/components/ContactForm.kt` (018 vocabulary: sheet, two fields, error line, Save/Cancel), the star in the detail header, the inspection tag row in `ContactDetailScreen`; `ContactsFixturesTest` counts
- [ ] T026 [US5] Device: gallery C7/C8/C9 screenshots on the Xiaomi (drawn states before wiring)

## Phase 6 — US5b/US6/US7 Contacts wired (P1/P2)

- [ ] T027 [US5] `ContactsController`: `openAdd()`, `openEdit(address)`, `formName/formAddress`, `saveForm()` → `Save{input, now_ms}` (core validation; the view's outcome shown), `toggleFavourite(address)`, `closeForm()`; `ContactsLive.form(view, draft)`; `ContactsMachineTest` save/favourite
- [ ] T028 [US6] `ContactsController`: `export(scope, format)` → `ExportRequested` → view `export` → `DocumentPorts.share` → `ExportTaken`; `import(intoGroup?)` → `DocumentPorts.pick` → `ImportFile` → `last_import`/`import_failure` shown → `ImportAcknowledged`; `ContactsLive` report/failure words (`importDone*`, `importFail*`)
- [ ] T029 [US6] Groups both ways: detail chips editable → `SetContactGroups{address, group_ids}`; group sheet members → `AddGroupMembers`/`RemoveGroupMember`; `ContactsMachineTest`
- [ ] T030 [US7] Inspection on open: `InspectRecipient{chain_id, address}` on detail open; `ContactsLive.detail` draws `contractTag`/`walletTag` from `recipient.is_contract`/`kind`, `firstTimeTagNeutral` from `first_interaction`
- [ ] T031 [US5] `VelaNavHost` + `ContactsRoute`: form sheet raised from Add/Edit, star, ⋯ menu Import/Export, group member editing; `DocumentPorts` bound in `MainActivity`
- [ ] T032 Device: form add + edit + star + force-stop (SC-005); export via share sheet → Files, import back → existing wins (SC-006); founder = Wallet, USDC contract = Contract (SC-007)

## Phase 7 — Closeout

- [ ] T033 Gates: unit suite 0 failures, `CoreWireDriftTest`, `check-native-reachability.mjs` (android set + reasons), `build-web.mjs --check`, .so size under 19.5 MB
- [ ] T034 `specs/045-android-send-contacts-parity/results.md` (phases, device evidence, owed), memory update

## Dependencies
Phase 0 → all. Phase 1 → Phase 3 (batch feeds split). Phase 5 → Phase 6. Phases 2, 4 independent after 0.
