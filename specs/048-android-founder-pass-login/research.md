# Research — 048 Android Founder Pass and Login Recovery

Every unknown in the plan, resolved from the source (the two audits of
2026-09-12 and the reads that followed).

## R1. Why login hangs after the passkey signs

- **Finding**: `wallet.getvela.app` moved from the retired Expo bundle to the
  SvelteKit Worker on 2026-09-11 (commit `950b8f4c`). Same origin → same
  `localStorage`. The Expo client wrote `vela.accounts` in camelCase
  (`publicKeyHex`, `createdAt`, `keys[].credentialId`, `keys[].publicKeyHex`);
  the core's `Account`/`AccountKey` (`rust/crates/vela-core/src/app/mod.rs:131-168`)
  are snake_case with `#[serde(default)]` only on `keys` and `transports`.
  Serde fails with `missing field credential_id` (records with keys) or
  `missing field public_key_hex` (records without).
- **Where it dies**: `login.rs:294-304` requests `LoadAccounts` and renders
  `busy`; the web executor returns the raw list (`onboarding/core/executor.ts:140`);
  the wasm bridge refuses (`vela-core-wasm/src/bridge.rs:85-88`); the effect
  loop catches and calls `options.onError?.()` (`core/effect-loop.ts:109-116`)
  which the login page and the session store never pass. The machine stays in
  `LoadingAccounts`; `login.rs:338-340` (`StorageFailed → SignInFailed`) is
  unreachable because only a *thrown executor* maps to it. The session boot
  (`session.rs:336-348`) hangs the same way → `allowed_route: 'loading'`.
- **Decision**: fix both layers.
  1. Core: `#[serde(alias = "...")]` for the old names on `Account`,
     `AccountKey`, `PendingUpload`, `PendingUploadMember`. Reads both, writes
     snake_case only. No exported type changes → no ts-rs/uniffi regen; wasm
     rebuilt (`node rust/scripts/build-web.mjs`, then `--check`).
  2. Web: `storage.ts::loadAccounts` normalises old records (keeping `keys`)
     and writes the list back once when any record was old; `services/accounts.ts`
     treats `keys` as optional. The file header's "byte-compatible with Expo"
     claim is corrected.
  3. Never silent: `effect-loop.ts` makes `onError` required and, when the
     core refuses a resolved answer, feeds `toFailure(effect, error)` once for
     that effect id (login → `storage_failed` → `SignInFailed`; session →
     `accounts_unavailable` → `Empty` → onboarding). The login page and the
     session store show a `fatal` prompt sheet with "sign in again" and "reset
     this browser's copy" (the `CreateFlow.svelte:62` pattern).
  4. Android: `CoreDriver.kt:152-159` answers the machine with the escaped
     failure (same contract); `SessionController` gets an `onFault` that logs
     and renders a visible state.
- **Alternatives rejected**: a `SessionView.storage_unreadable` field — a
  four-shell view change for what the failure answer already gives; a
  storage-version key alone — does not help the core-level refusal on other
  shells.
- **Tests**: core `tests/app_login.rs` + `tests/app_session.rs` with camelCase
  fixtures (with and without `keys`), a `mod.rs` equality test; web vitest for
  `loadAccounts` (normalise + rewrite) and for the effect loop's refusal path;
  Playwright: seed the old shape → `/en/wallet` leaves loading and the
  parallel-space sign-in lands on the wallet; Android: an Expo-shaped
  `vela.accounts` in a `FakeStore` → `SessionController.view` leaves loading,
  plus the Xiaomi check with the store seeded through `run-as`.
- **Desktop**: not exposed (`executor/storage.rs:226-238` skips unreadable
  records). The skip is silent — recorded in results, not changed.

## R2. Clipboard

- Today three sites write the clipboard (`VelaAddressStrip.kt:54`,
  `IdenticonViewerSheet.kt:144`, `DoneScreen.kt:251`), two ways; every flow and
  contact copy control only flips a tick.
- **Decision**: `core/platform/Clipboard.kt` — `Clipboard.copy(context, label,
  text): Boolean` through `ClipboardManager` with a `VelaLog.event("clipboard",
  "copied", "chars" to n)`; the existing three sites and every audit row 1
  control call it; the copied tick shows only when it returned true.
- **Device check**: paste into the send recipient field with
  `input keyevent 279` and read the field back from the dump.

## R3. List rows that open nothing

- `FlowHost.kt:137-140,150` drop the tapped indices (`onSelect = { _, _ -> onNavigate(TxDetail) }`),
  `FlowNav.push(step, id = null)` → `FlowLive.txDetail(null)` returns null → no
  sheet, but the step is pushed (phantom Back).
- **Decision**: the list bodies pass the row's id (`holdingId` / feed row id)
  through `onNavigate(step, id)`; `FlowNav.push` refuses a detail step without
  an id (logs) instead of pushing it.

## R4. The share image

- Web (`share-image.ts`): 480×700 at 2×, accent field, white curved foot with
  the app icon and the ink wordmark, the chain logo in its pill, a REAL code
  (`encodeQr(address)`), name, address; "保存图片" downloads a file.
- Android: `ShareCardArtwork.kt:80-82` calls `QrCard(label)` without a
  payload → the gallery's placeholder pattern; 360dp wrap-height capture;
  sailboat logo on the accent, no foot.
- **Decision**: `ShareCardModel` gains `code: String` (the address); the
  artwork passes it; the composition is redrawn after `share-image.ts`
  (foot, icon, wordmark, pill with the chain logo via `RemoteLogo`, 480×700
  at 2× = 960×1400 px capture); "保存图片" writes through
  `core/platform/Gallery.kt` (MediaStore `Pictures/Vela`, no permission on
  API 29+) and shows a confirmation; the share sheet remains a second action.
- **Device check**: pull the saved file (`/sdcard/Pictures/Vela/…png`) and
  decode with `zbarimg`.

## R5. Contacts: dock, 群发转账, groups

- Action ids exist (`contacts.action.Send/Receive/Qr`, `contacts.batchSend`,
  `contacts.manage`, `contacts.groupMenu`, `contacts.swipeSend/Delete`,
  `history.filterAll`) with no `when` branch in `VelaNavHost.kt:1063-1121`.
  The machine side exists: `ContactsController.saveGroup(ContactGroupInput(id?, name, color?, members?))`,
  `deleteGroup(id)`; `SendController.seedSplit(List<SendRecipientDraft>)`,
  `SendOpenParams(preselected_symbol, preselected_network, prefilled_recipient…)`.
- **Decision**: branches for each id; a `GroupEditSheet` (name + colour) for
  new/rename; the group ⋯ menu sheet (rename, delete with confirm); 群发转账
  → `send.open(params)` then `seedSplit(members)` (one member → the plain form
  with `prefilled_recipient`); the label for the list's section action follows
  the web's `contacts.groupNew` (新建分组).

## R6. The identicon viewer everywhere

- `IdenticonViewerSheet(address, onDismiss)` is hosted only in `WalletScreen`.
- **Decision**: hoist one host to the NavHost (`identiconViewer: String?`
  state + the sheet) and pass `onIdenticon(address)` down through the twelve
  sites the audit lists; `IdenticonAvatar` gains an optional `onTap`.

## R7. Chain and class filters

- Web: the chain filter is shell render state (`$lib/wallet/chain-filter.svelte`)
  applied to the feed (via `chain_filter_changed`), the assets and the send
  pick; the class filter is `sendClassFilter` with `sendTokenClass(token)`
  (`live-send.ts:85`) deciding stable / gas / other.
- Android: `FeedEvent.ChainFilterChanged` is wired but never dispatched;
  `FlowStep.Chains` reaches nowhere; the SD1 chips have no `onFilter`.
- **Decision**: a `ChainFilter` state in the NavHost (mirroring the web) with
  a chain sheet (the receive network list reused); it narrows the assets list
  and the send pick in the live builders and dispatches
  `ChainFilterChanged` to the feed; the class rule ported verbatim into
  `SendLive` and applied to `visibleSendTokens` so picker indices stay true.

## R8. Explorer links

- Every network row carries `explorer_url` (`NetWire.kt`); the web builds
  `address/`, `tx/`, `token/` paths.
- **Decision**: `FlowLive` fills `explorerUrl` on R2/A2/T2 models from the
  chain's `explorer_url`; the buttons open it through the existing
  `context.openUrl`.

## R9. Scanner flip

- `CameraScanner.kt:78` binds `DEFAULT_BACK_CAMERA`; `LiveScanSurface.kt:69`
  ignores `ScanTool.Flip`.
- **Decision**: a `lensFacing` state; flip rebinds with the other selector
  when `cameraProvider.hasCamera(front)`; otherwise the tool is disabled.

## R10. Slider and haptics

- Web: `TextScaleSlider.svelte` is a native range input over tick dots.
- Android: `VelaTextScaleSlider` draws tappable dots only; haptics exist in
  `VelaButton` (VirtualKey on press), `AlphaIndexRail` (SegmentTick), and
  `Haptics.success/error/moneyIn` (predefined effects).
- **Decision**: `VelaSlider(steps, index, onChange)` — drag with snapping,
  tap on dots, live preview through the existing `textScale` commit; one
  `VelaHaptic` helper (`Detent`, `Select`, `Success`, `Reject`) routed through
  `View.performHapticFeedback` (honours the system switch) with the constants
  per API level, logging `haptic <class>`; applied to: slider detents, the
  signing slider threshold, switches, class/chain filter chips, network /
  fee-token / account picks, favourite, copy. The policy goes into the
  design-system doc.

## R11. Settings fields and links

- `VelaUrlField` has no `onValueChange`/`onAction`; the list-foot add row and
  several link texts have no `clickable`.
- **Decision**: `VelaUrlField(value, onValueChange, action, onAction)`;
  `network_admin`'s `override_field_edited/blurred` and
  `provider_test_requested` events dispatched (the web's); links through
  `context.openUrl`.
