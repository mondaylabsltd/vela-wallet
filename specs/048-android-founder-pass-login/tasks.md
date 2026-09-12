# Tasks: Android Founder Pass and Login Recovery (048)

**Input**: spec.md, plan.md, research.md, data-model.md, contracts/, quickstart.md
**Rule**: every phase ends on the Xiaomi (parallel space, dust only) before the next starts; the login fix ends on the web with an old-shaped seed as well.

## Phase 1 — Setup

- [ ] T001 Confirm the stacked branch `048-android-founder-pass-login` on 047, `.specify/feature.json`, and the test toolchains (cargo with `i18n-all,crux`, the web's vitest/Playwright with an isolated `--output` and port, gradle with the JBR `JAVA_HOME`)

## Phase 2 — Foundational

- [ ] T002 [P] `core/platform/Clipboard.kt` — one `Clipboard.copy(context, label, text): Boolean` with a `VelaLog` line; the three existing writers (`VelaAddressStrip.kt`, `IdenticonViewerSheet.kt`, `DoneScreen.kt`) call it
- [ ] T003 [P] `core/platform/VelaHaptic.kt` — `Detent | Select | Success | Reject` through `performHapticFeedback` (API-level constants), a `haptic <class>` log line, and `docs/design-system/haptics.md` with the policy; `VelaButton` and `AlphaIndexRail` route through it
- [ ] T004 [P] `core/platform/Gallery.kt` — `Gallery.savePng(context, name, bytes): Uri?` through MediaStore `Pictures/Vela`
- [ ] T005 [P] NavHost hosts: `identiconViewer: String?` + `IdenticonViewerSheet`, and `ChainFilter` state (`chainId: Int?`) with the chain sheet, in `navigation/VelaNavHost.kt`; `IdenticonAvatar` gains `onTap: ((String) -> Unit)?`

## Phase 3 — US1 Login recovery (P1)

- [ ] T006 [US1] Core: `#[serde(alias)]` on `Account.public_key_hex/created_at_iso`, `AccountKey.credential_id/public_key_hex`, `PendingUpload*` camelCase names in `rust/crates/vela-core/src/app/mod.rs`; unit test that both spellings deserialize equal
- [ ] T007 [US1] Core tests: `tests/app_login.rs` `an_expo_era_record_still_opens_the_wallet` (with and without keys); `tests/app_session.rs` `an_expo_era_record_restores_the_session`
- [ ] T008 [US1] Web: `onboarding/core/storage.ts` normaliser (`fromStoredAccount` restored, keys kept, transports `""`), one-time rewrite when any record was old, header corrected; `services/accounts.ts` `keys ?? []`; `storage.test.ts`
- [ ] T009 [US1] Web: `core/effect-loop.ts` — `onError` required; a refused `resolve` feeds `toFailure(effect, error)` once per effect id; `effect-loop.test.ts`
- [ ] T010 [US1] Web: the login page and the session store pass `onError` and show the prompt (sign in again / reset this browser's copy) via the existing prompt sheet; `starting`/`loginView` reset so the button is pressable; corpus keys `onboarding.storage.*` in all locales (the i18n gates: wasm bytes + fingerprint)
- [ ] T011 [US1] Web e2e `e2e/login-old-shape.e2e.ts`: seed the old shape → `/en/wallet` leaves loading; parallel-space sign-in lands on the wallet; an unreadable seed shows the prompt
- [ ] T012 [US1] Android: `CoreDriver.resolve` answers the machine with `escapedFailure` when the bridge refuses a result; `SessionController` passes `onFault` (log `session.fault` + visible fault view); `SessionOldShapeTest` with a FakeStore
- [ ] T013 [US1] Rebuild the web wasm (`node rust/scripts/build-web.mjs` + `--check`), the Android `.so`, and run `cargo test`, vitest, gradle; device: seed the old shape into the Xiaomi's store (debug hook) → the session leaves loading; web manual check per quickstart

## Phase 4 — US2 Every button does what it says (P1)

- [ ] T014 [US2] Copy controls in `feature/flows/FlowScreens.kt` (R1 rows, R2 address/contract, A2 address/hash, T2 facts, SD4 hash) and `feature/contacts/ContactDetailScreen.kt` (`contacts.copyAddress` branch) call `Clipboard.copy`; tick only on success; `VelaHaptic.Select`
- [ ] T015 [US2] List rows carry ids: `feature/flows/FlowHost.kt` (`onSelect` → `onNavigate(step, id)`), `FlowNav.push` refuses a detail step without an id; `FlowLive.txDetail/tokenDetail` by id
- [ ] T016 [US2] Share image: `ShareCardModel.code` + `chainLogoUrl` (`FlowModels.kt`), `ShareCardArtwork.kt` redrawn after `share-image.ts` (foot, app icon, wordmark, logo pill, real `QrCard(payload)`), `ShareCardCapture` at 960×1400, 保存图片 → `Gallery.savePng` + snackbar, share as the second action
- [ ] T017 [US2] Contacts dock: `VelaNavHost.kt` branches for `contacts.action.Send` (send.open prefilled recipient), `Receive`, `Qr` (contact code sheet)
- [ ] T018 [US2] 群发转账: `contacts.batchSend` → `send.open` + `SendController.seedSplit(members as SendRecipientDraft)`; one member → prefilled form; group of zero → the split entry empty
- [ ] T019 [US2] Groups: `contacts.manage` relabelled 新建分组 (`ContactsFixtures.kt` via corpus `contacts.groupNew`) → `GroupEditSheet` (name, colour) → `saveGroup`; `contacts.groupMenu` → menu sheet (rename → `saveGroup(id, name)`, delete → confirm → `deleteGroup`)
- [ ] T020 [US2] Identicon viewer from the twelve sites: `ContactRow.kt`, `ContactDetailScreen.kt`, `SettingsRows.kt` account row, `SettingsScreen.kt` accounts sheet, `AccountSwitcherSheet.kt`, `FlowBlocks.kt` address card + recipient field, `FlowRows.kt` split card + contact-pick row + fact lead, `FlowScreens.kt` breakdown, `SigningComponents.kt` signer row — each `onTap` → the NavHost host
- [ ] T021 [US2] Device pass for US2 per quickstart (clipboard paste-back, list rows + Back count, saved PNG decoded with `zbarimg`, contact dock, 群发转账 rows, group create/rename/delete, viewer from each site)

## Phase 5 — US3 Filters, links and second-tier controls (P2)

- [ ] T022 [US3] Class filter: `SendClassFilter` + `sendTokenClass` ported into `feature/send/SendLive.kt`; `FlowHost.SendCallbacks.onFilter`; chips selected state; `VelaHaptic.Select`
- [ ] T023 [US3] Chain filter: 全部网络 pill (`FlowChrome.kt`) → the NavHost chain sheet; `SendLive.pick`, `FlowLive.assets` narrow by it; `WalletController.chainFilterChanged` dispatched to the feed; pill shows the chosen network; sweep pin wins
- [ ] T024 [US3] Explorer links: `FlowLive` fills `explorerUrl` (from `explorer_url` of the chain) on R2/A2/T2 models; `FlowHost` passes `onExplorer` → `context.openUrl`
- [ ] T025 [US3] Token detail: 转账 → `send.open(preselected_symbol, preselected_network)`; 收款 → the token's own code (R3); activity rows tappable → tx detail; 删除记录 on A2 → feed delete with confirm
- [ ] T026 [US3] Home hero: `WalletScreen` passes `onToggleVisibility` → `togglePrivacy`, and `onStatusClick` → a rescue sheet hosted from the home (RpcFix / BalanceDetail bodies made reachable from `feature/wallet`)
- [ ] T027 [US3] Scanner flip: `feature/scan/CameraScanner.kt` `lensFacing` state + rebind; `LiveScanSurface.kt` `ScanTool.Flip`; disabled without a front camera
- [ ] T028 [US3] Recipient picker: 扫码 row in the live send (`FlowHost.kt:273`), group rows (`SendLive.kt` `groups`, `onGroup` → `seedSplit`), per-row 通讯录 pick on split cards (`FlowRows.kt` → `openRowPicker(id)`)
- [ ] T029 [US3] Add token: 原生币 tab + chain suggestion pick (`FlowHost`/`FlowLive`/`FlowScreens` → the network_admin wizard events)
- [ ] T030 [US3] Settings: `VelaUrlField(onValueChange, onAction)`; network detail overrides → `override_field_edited/blurred`; "+ 添加网络" foot clickable; add-network custom RPC + 重新检查; provider 检查密钥/获取密钥 (`provider_test_requested`) + drpc link; language sheet contribute link; feedback box editable (`SettingsScreen.kt`, `SettingsPrimitives.kt`, `SettingsController.kt`)
- [ ] T031 [US3] Contacts: 最近往来 · 全部 (`history.filterAll` → history filtered to the contact), swipe 转账/删除 branches, export CSV/JSON choice sheet
- [ ] T032 [US3] Device pass for US3 per quickstart

## Phase 6 — US4 Slider and haptics (P2)

- [ ] T033 [US4] `core/designsystem/components/VelaSlider.kt` — drag with snapping, tap on dots, `Detent` per step crossed; `SettingsPrimitives.VelaTextScaleSlider` replaced; the sample text scales live
- [ ] T034 [US4] Apply `VelaHaptic.Select` to switches, class/chain filter chips, network / fee-token / account picks, favourite, copy; `Detent` on the signing slider threshold; audit that no navigating tap, tab, Back or sheet has one
- [ ] T035 [US4] Device pass: `haptic` log lines per quickstart (one per step crossed, one per selection, zero for scroll/tab/back/sheet); system haptics off → silent

## Phase 7 — US5 Visual parity (P3)

- [ ] T036 [US5] Scanner look: bracket weight at the web's, one status line (`feature/scan/ScanSurface.kt`)
- [ ] T037 [US5] Settings home rows absent on the live route (`VelaNavHost.kt:1255`), feedback box editable — screenshots against the web

## Phase 8 — Closeout

- [ ] T038 Rulers re-run (`scripts/check-android-event-parity.mjs`, `check-android-dropped-judgement.mjs`); no new strong diffs
- [ ] T039 `results.md`: the 40-row after-state table, the login fix evidence (core/web/Android), deviations; `docs/KNOWN-BUGS.md` entries (the login bug, the desktop's silent skip); tasks marked; memory
- [ ] T040 Commits per phase on 048; the web changes flagged for the web owner's review (the founder said the web needs the storage fix too)

## Dependencies

- Phase 2 before 4–6 (helpers and hosts). US1 is independent of the phone phases and goes first.
- T015 before T025 (ids); T005 before T020/T023; T003 before T034; T016 uses T004.

## Parallel opportunities

- T002/T003/T004/T005 together; T006–T009 together (different layers); T014/T016/T017–T019 together after Phase 2; T022–T031 largely independent files.

## Implementation strategy

US1 first and shipped on its own (it blocks people today), then the P1 phone phase, then P2, then the slider/haptics, then looks. Each phase: build → install → scripted device pass → commit.
