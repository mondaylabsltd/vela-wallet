# Tasks: Android Money Wiring

**Input**: `spec.md`, `plan.md`, `research.md` (D1–D12), `data-model.md`,
`contracts/shell-operations.md`, `quickstart.md`.

**Tests**: required. Money moves; a send path without a JVM test per machine
and a device pass per phase is the thing the founder's rule forbids
(FR-012, FR-013).

**Paths**: Android module `app-android/vela-wallet/`; Kotlin under
`app/src/main/java/app/getvela/wallet/` (abbreviated `…/`); JVM tests under
`app/src/test/java/app/getvela/wallet/` (abbreviated `…test/`); Rust under
`rust/crates/`.

**Device rule**: every phase ends with a `[device]` task. It is done when a
screenshot and the `uiautomator` text are in the scratchpad and quoted in
`results.md`. A phase with a red device task is not done.

---

## Phase 0 — Measure and the bridge (Setup)

- [X] **T001** Record the arm64-v8a `.so` baseline (14,003,760 bytes at
      042's tip) in `specs/043-android-money-wiring/results.md`; 041's
      per-machine figure (~357 KB) is the ceiling for +3.
- [X] **T002** Add `bridge_object!` for `SendCore` (`vela_core::app::send::Send`),
      `FeePolicyCore` (`fee_policy::FeePolicy`), `TxTrackerCore`
      (`tx_tracker::TxTracker`) in `rust/crates/vela-core-uniffi/src/onboarding_bridge.rs`
      beside `ManageTokensCore` (:281). Confirm the machine type names in
      each `app/*.rs` before writing.
- [X] **T003** Regenerate `rust/bindings/kotlin/` (quickstart) and rebuild
      the three ABIs; record the `.so` delta against T001.
- [X] **T004** [P] `…/feature/send/core/SendWire.kt` — `SendOperation` (18),
      `SendShellResult`, `SendEvent` (the 043 subset dispatched, all 40
      decoded where the view carries them), `SendView` and the value types
      (`SendToken`, `SendChainInfo`, `SendTreasuryProbe`, `SendFeeOutcome`,
      `SendSubmitFailure`, `SendAlertKind`, `SendHapticKind`, `SendTimerTag`,
      `SendTxRecord`, `SendQuotedFee`, `FeeCall`). snake_case `@SerialName`
      throughout; numerics as the ts-rs mirror says (u32 → Int/Long per
      041's rule, amounts as String).
- [X] **T005** [P] `…/feature/send/core/FeeWire.kt` — `FeeOperation` (6),
      `FeeShellResult`, `FeeEvent`, `FeeView`, `FeeEstimateView`,
      `FeeAssetView`, `FeeBundlerQuote`, `FeeAssetQuote`, `FeeGasOutcome`,
      `FeeTier`.
- [X] **T006** [P] `…/feature/send/core/TrackerWire.kt` — `TrackOperation`
      (6), `TrackShellResult` (incl. `receipt_with_logs` with
      `TrustReceiptLog`), `TrackEvent`, `TrackView`, `TrackPendingRecord`,
      `TrackRecordPatch`, `TrackLifecycle`.
- [X] **T007** [P] `…/feature/send/core/MtokWire.kt` — `MtokOperation` (5),
      `MtokShellResult`, `MtokEvent`, `MtokView`, `MtokTokenMeta`,
      `MtokCustomToken`.
- [X] **T008** `…test/CoreWireDriftTest.kt` — `assertFieldsExist` for the
      four views and their rows; `assertVariantsExhaustive` for the four
      operation/result families and for `SendSubmitFailure`,
      `SendAlertKind`, `SendTreasuryProbe`, `FeeGasOutcome`,
      `TrackLifecycle`; `assertVariantsExist` for the four event families.
      Run it red-then-green against the mirrors in
      `app-web/vela-wallet/src/lib/core/generated/`.
- [X] **T009** [P] `app/build.gradle.kts` + `gradle/libs.versions.toml`: add
      `androidx.work:work-runtime-ktx`; `AndroidManifest.xml`: add
      `POST_NOTIFICATIONS`. No other new dependency.

---

## Phase 1 — Foundational: transport, store, assembly, seams

- [X] **T010** `…/feature/send/core/RelayClient.kt` — bundler JSON-RPC
      (`eth_estimateUserOperationGas`, `eth_sendUserOperation`,
      `eth_getUserOperationReceipt`, `eth_getUserOperationStatus`,
      `pimlico_getUserOperationGasPrice`, `vela_getInBandGasQuote`) and REST
      (`GET /v1/treasury/{chain}` with 404 = uncovered,
      `GET /v1/account/{chain}/{safe}`) through `RpcPool` against the
      chain's bundler endpoint (`NetworkEndpointSource.kt:54`); the 8-second
      in-band quote cache with `clear()` (D8). Uses `VelaHttp` only.
- [X] **T011** [P] `…test/RelayClientTest.kt` with `…test/FakeRelay.kt` — a
      scripted transport: each method's happy answer, a 404 treasury, a
      rejection message, a rate-limit; the cache's TTL.
- [X] **T012** `…/feature/wallet/core/FeedExecutor.kt` — `writeRecords(rows)`
      and `patchRecords(ids, status, txHash)` under `writeLock`, camelCase
      shape of `:135–172`, de-dupe by id, `TX_CAP`; `pendingRecords()` for
      `load_pending_txs` (no terminal status, `type ∈ {send, dapp_tx}`) (D4).
- [X] **T013** [P] `…test/FeedExecutorTest.kt` — a pending row written then
      read by the feed; a patch flips it; the cap holds; a second write with
      the same id is one row.
- [X] **T014** `rust/crates/vela-core/src/user_op.rs` — move the pure half of
      `app-desktop/vela-wallet/src/executor/user_op.rs` (D3): the in-band
      and Tempo legs, `fallback_fee`, `key_set_of`, `to_multi_send_call`,
      the draft (nonce/gas fields, init code, dummy signature for estimate),
      `envelope`, `classify_rejection`. Re-point the desktop at the moved
      code (`cargo test` in `app-desktop/vela-wallet` stays green) — or, if
      its churn is too large for this spec, leave the desktop copy and
      record the two copies in `results.md` as owed.
- [X] **T015** `rust/crates/vela-core-uniffi/src/lib.rs` — export
      `user_op_draft(...)`, `user_op_finalize(...)`,
      `classify_relay_rejection(...)` per `contracts/shell-operations.md`;
      `cargo fmt`, regenerate bindings; a `rust/crates/vela-core/tests/`
      case that a draft's SafeOp hash equals the desktop's for the same
      inputs (a golden from 032's SC-303 send).
- [X] **T016** `…/feature/send/core/UserOpSigner.kt` — the seam (D9):
      `suspend fun sign(challenge: ByteArray, credentialIds: List<String>,
      method: KeyMethod): Assertion`; default = `PasskeyExecutor.assert`
      with rpId `getvela.app`; cancellable; exactly one call per attempt.
- [X] **T017** `…/dev/ParallelSpaceHook.kt` (main source set) — the seam the
      app calls: `active(): Boolean`, `signer(): UserOpSigner?`,
      `fixtureAccount(): FixtureAccount?`, `badge: @Composable`; release
      implementation returns false/null/nothing.

**Checkpoint**: unit tests green; the app opens on the device unchanged.

---

## Phase 2 — User Story 0: the parallel space (P1, enabling)

- [X] **T018** [US0] `rust/crates/vela-dev-fixtures-uniffi/` — new cdylib
      crate (`Cargo.toml` with `vela-core = { features = ["dev-fixtures"] }`,
      `uniffi` setup like `vela-core-uniffi`), exporting `fixture_accounts`,
      `fixture_assert(challenge, allow_credential_ids, preferred)`,
      `fixture_registration(index)` over `vela_core::dev_fixtures`
      (`resolve_signer` :226, `build_assertion` :260, `build_registration`
      :311). Add it to the workspace; `cargo clippy -D warnings` clean.
- [X] **T019** [US0] `rust/scripts/build-android.sh` — when
      `VELA_DEV_FIXTURES=1`, also `cargo ndk … -p vela-dev-fixtures-uniffi`
      into `app/src/debug/jniLibs/`; `rust/scripts/smoke-kotlin.sh`-style
      bindgen into `rust/bindings/kotlin-dev/` (gitignored like
      `bindings/kotlin`).
- [X] **T020** [US0] `app/build.gradle.kts` — debug source set:
      `kotlin.srcDir("rust/bindings/kotlin-dev")`, `jniLibs.srcDir("src/debug/jniLibs")`;
      `cargoNdkBuild` sets `VELA_DEV_FIXTURES=1` for the debug variant only;
      release packages neither (SC-010 check: `unzip -l` the release APK).
- [X] **T021** [US0] `app/src/debug/java/app/getvela/wallet/dev/ParallelSpace.kt`
      — the door (intent extra `vela.parallelSpace`, persisted key
      `vela.parallelSpace`, `vela.parallel.signWith`), the badge composable,
      the `UserOpSigner` backed by `fixtureAssert`, and the session sign-in
      as fixture account 0 (through `SessionController` with the fixture's
      credential id and derived address); `ParallelSpaceHook` binds to it.
- [X] **T022** [US0] `MainActivity.kt` — read the extra; `VelaNavHost.kt` —
      draw `ParallelSpaceHook.badge` over every route when active; sign-out
      clears the flag.
- [X] **T023** [P] [US0] `…test/ParallelSpaceTest.kt` (debug unit test source
      set) — the fixture address equals the core's `multi_address()` /
      account 0 address; an assertion from `fixtureAssert` verifies with
      `webauthn_signing_hash` + the fixture public key; `active()` false
      without the flag.
- [ ] **T024** [US0] `[device]` open the door (`quickstart.md`), screenshot:
      badge on home, receive screen address = the fixture Safe the web's
      `/parallel` shows; relaunch without the extra stays inside; sign out
      leaves.

**Checkpoint**: US0 done; every later device task runs inside the space.

---

## Phase 3 — User Story 1: send a token to someone (P1) 🎯 MVP

- [X] **T025** [US1] `…/feature/send/core/FeeExecutor.kt` — the six arms per
      `contracts/shell-operations.md` (gas signals in parallel through the
      pool; bundler quote; in-band quotes; fee recipient; gas estimate via
      `user_op_draft` with the dummy signature; TTL as a cancellable delay).
      `neutralAnswer` per arm.
- [X] **T026** [P] [US1] `…test/FeeMachineTest.kt` — the real `FeePolicyCore`
      through JNA with `FakeRelay`: a quote lands with the estimate the
      view renders; `SelectFeeAsset` re-quotes; TTL expiry flips the view.
- [X] **T027** [US1] `…/feature/send/core/SendExecutor.kt` — the 18 arms:
      `fetch_tokens` from the balance view + `NetView`; `estimate_fee` by
      dispatching `QuoteRequested` into the fee host and awaiting its
      estimate (D7); `probe_treasury`; `load_account_credential` from
      `AccountStore`; `submit_user_op` = draft → `UserOpSigner` → finalize →
      `eth_sendUserOperation`, failures via `classify_relay_rejection`;
      `cancel_passkey_sign`; `persist_tx_records` → `FeedExecutor.writeRecords`;
      `track_submitted` → the tracker (Phase 4; until then, acknowledged and
      logged); `resolve_identity`/`resolve_risk`; `simulate_calls` → null;
      `start_timer`; `haptic`; `show_alert`/`close` → controller callbacks;
      `add_network` → `Error`.
- [X] **T028** [US1] `…/feature/send/core/SendController.kt` — hosts
      `SendCore`, `FeePolicyCore` (and `ManageTokensCore` from Phase 6);
      `open(address)`, `close()`, the event intents the screens call
      (`selectToken`, `setRecipient`, `setAmount`, `tapMax`, `continue`,
      `back`, `chooseFeeToken`, `slideConfirm`, `cancelSigning`, `done`…);
      StateFlows `send`, `fee`; lifecycle: one attempt per `open`.
      Registered in `VelaWalletApplication.kt`'s container.
- [X] **T029** [P] [US1] `…test/SendMachineTest.kt` — the real `SendCore`
      through JNA with `FakeRelay` and a fake `UserOpSigner`: pick → form →
      Continue quotes → confirm shows the quoted fee → confirm signs once →
      submitted → records persisted BEFORE `track_submitted`; Max on the fee
      token equals balance minus reserve; the generation lock drops a stale
      answer.
- [X] **T030** [US1] `…/feature/send/SendLive.kt` — `sendPick(fallback,
      view, balances, chainNames, currency)`, `sendForm(...)`,
      `sendConfirm(...)`, `sendReceipt(...)`, `feeTokenSheet(...)`,
      `contactPickSheet(fallback, book)`: every figure and string from the
      core's view, the fixture only for labels; the receipt's
      `ReceiptStage` from the view's stage.
- [X] **T031** [P] [US1] `…test/SendLiveTest.kt` — the four builders against
      hand-built views: no fixture number survives; hidden balance stays
      hidden; the fee row shows the fee token; a `submitted` view renders
      the hash and the explorer link.
- [X] **T032** [US1] `navigation/VelaNavHost.kt` — `liveFlow`'s `else ->
      drawn.base/sheet` branches gain `SendPick/SendForm/SendConfirm/
      SendReceipt` and `FeeToken/ContactPick`; the 转账 entry opens
      `SendController.open`; `FlowHost` callbacks (`onContinue`, `onConfirm`,
      `onFee`, `onPickRecipient`, `onCta`) dispatch controller intents
      instead of pushing fixture steps; Back dispatches `Back`. Delete
      nothing drawn: the `Scan`/`BatchImport` branches keep their fixture.
- [X] **T033** [US1] `…/feature/flows/FlowScreens.kt` — where a send screen
      has a control with no callback (amount field, recipient field, Max,
      the slide-to-confirm), give it one; no new drawing.
- [X] **T034** [US1] `[device]` in the parallel space: 转账 → Gnosis xDAI →
      recipient = fixture account 1 → dust → Continue shows a fee →
      confirm → receipt *submitted* with a hash; the explorer shows it
      (SC-001 first half). Screenshot every screen.

**Checkpoint**: money leaves the phone.

---

## Phase 4 — User Story 2: money in flight outlives the screen (P1)

- [X] **T035** [US2] `…/feature/send/core/TrackerExecutor.kt` — the six arms:
      `poll_receipt` (cache logs per hash), `poll_status`,
      `load_pending_txs` from `FeedExecutor.pendingRecords()`,
      `update_tx_records` → `patchRecords` then dispatch
      `activity_feed::ReconcileCompleted`, `notify_confirmed` → notification
      when no activity is foregrounded + `token_trust::ReceiptLogsConfirmed`,
      `now`.
- [X] **T036** [US2] `…/feature/wallet/core/WalletController.kt` — host
      `TxTrackerCore`; `submitted(hash, recordIds, chainId)`; a 3-second
      `Tick` while foregrounded; `AppResumed` on resume; `SendExecutor`'s
      `track_submitted` now hands off here and answers `track_handed_off`;
      the receipt verdict back to the send host as `ReceiptUpdate`.
- [X] **T037** [US2] `…/feature/wallet/core/TrackerWork.kt` — WorkManager
      worker (D5): expedited one-time on background with pending records,
      ~30 s cadence for the core's 120 s window, then periodic (15-minute
      floor) until the core abandons; `NotificationChannel("transactions")`,
      the notification with a deep link that opens the wallet with the flow
      stack seeded to that receipt (`MainActivity` extra `vela.receipt`).
- [X] **T038** [US2] `…/feature/send/core/SendController.kt` — request
      `POST_NOTIFICATIONS` at the first submit (never at launch); a refusal
      is remembered and degrades to the in-app receipt.
- [X] **T039** [P] [US2] `…test/TrackerMachineTest.kt` — the real
      `TxTrackerCore` with `FakeRelay`: pending → receipt → patched →
      `ReconcileCompleted` seen; a failed receipt patches `failed`; a
      restart (`load_pending_txs`) resumes a hash with no `Submitted`.
- [X] **T040** [US2] `[device]` submit, `am force-stop`, reopen: the pending
      row is on the home; wait: *confirmed*; the receipt page says so.
      Then submit and press Home: the notification arrives; tapping it opens
      the receipt (SC-001 second half, SC-003).

**Checkpoint**: a send survives the phone.

---

## Phase 5 — User Story 3: the screen says what the core refuses (P2)

- [X] **T041** [US3] `…/feature/send/SendLive.kt` — every `SendAlertKind`,
      `SendEstimateFailure`, `SendSubmitFailure` and `SendTreasuryProbe`
      rendered as the core's wording in the drawn slots (form inline error,
      confirm banner, receipt *failed* title/caption, the treasury sheet
      states SD3B/SD3C); Continue disabled exactly when the view says so.
- [X] **T042** [US3] `…/feature/send/core/SendController.kt` — the cancel
      checkpoints: `cancelSigning` cancels the signer job and dispatches
      `CancelSigning`; `back` from confirm dispatches `Back`; a second
      `slideConfirm` while signing is ignored by the core (assert, do not
      re-implement).
- [X] **T043** [P] [US3] `…test/SendRefusalsTest.kt` — for each refusal in
      the spec's User Story 3: the view carries the core's word and the
      builder shows it; cancel at signing → signer called once, view back at
      confirm; a relay rejection → `failed` with the message; the treasury
      404 → the sheet.
- [X] **T044** [US3] `[device]` drive each refusal (`quickstart.md`
      "Refusals"), screenshot each; cancel during signing in the REAL space
      with the founder present is SC-002/SC-004 — in the parallel space
      assert the prompt count through the signer's log line (SC-004,
      SC-005).

**Checkpoint**: no silence.

---

## Phase 6 — User Story 4: the sheets, and the arms 042 left (P2)

- [X] **T045** [US4] `…/feature/send/core/MtokExecutor.kt` — the five arms
      (one Multicall3 `aggregate3` through the pool using `Abi.kt`;
      `vela.customTokens` read/write/remove; invalidate → balance refresh);
      `ManageTokensCore` hosted in `SendController` (or `WalletController`
      if the assets sheet also needs it — decide by who opens AddToken).
- [X] **T046** [US4] `…/feature/send/SendLive.kt` + `VelaNavHost.kt` —
      `AddToken` sheet live: address input → `AddressInput`/`DetectRequested`
      → result → `SaveRequested`; the new token appears in the pick list
      after `InvalidateTokenCache`.
- [X] **T047** [US4] `…/feature/send/SendLive.kt` — the fee-token sheet's
      rows from `FeeView` assets (balance, fee in that token, selected);
      `ChooseFeeToken` → `SelectFeeAsset` → re-quote; the contact-pick sheet
      from `ContactsView` (favourites first, then the book); `PickedAddress`.
- [X] **T048** [US4] `…/feature/contacts/core/ContactsExecutor.kt:119` —
      `ResolveIdentity`: index lookup by wallet ref, then the name-service
      reverse resolution the web does (`recipient-identity.ts:191`,
      `NAME_SERVICES`), cached; the same helper answers send's
      `resolve_identity`. Remove the `// live in 042` marker.
- [X] **T049** [US4] `…/feature/settings/core/NetworkAdminExecutor.kt:209`
      (+ `NetworkProbes.kt:59` if it is the same cache) — `ClearBundlerCache`
      → `RelayClient.clear()`. Remove the markers; `grep 'live in 042'`
      returns nothing.
- [X] **T050** [P] [US4] `…test/MtokMachineTest.kt`, `…test/IdentityWaterfallTest.kt`
      — add a token by address through the real machine with a fake
      multicall; the waterfall's order and cache.
- [X] **T051** [US4] `[device]` change the fee token on confirm and send in
      it (SC-006); pick a contact and see the field fill; add a token by
      address and send it. Then SC-002: the founder sends dust from their
      own wallet, one prompt — recorded as done only when it happened.

**Checkpoint**: every drawn send surface is live except Scan and BatchImport.

---

## Phase 7 — Closeout

- [ ] **T052** Gates as CI runs them: `cargo fmt/clippy/test` (workspace),
      the Android unit tests, `check-expo-residue`, `check-native-reachability`,
      `gen-onboarding-types --check`; then `build-web.mjs` rebuild + `--check`
      LAST (every Rust edit moved the fingerprint).
- [ ] **T053** Bridge size: the arm64 `.so` after, against T001; the debug
      APK's extra fixtures `.so`; the release APK inspected (SC-010).
- [ ] **T054** `grep -rn 'FlowFixtures.build' …/navigation/` — the send
      states no longer fall to `drawn.base`/`drawn.sheet` (SC-007).
- [ ] **T055** `specs/043-android-money-wiring/results.md` — per SC:
      device-verified (screenshot + UI text) or test-only; the `.so` delta;
      the D3 split's line counts and whether the desktop was re-pointed;
      what 044–046 inherit (Scan, BatchImport, `simulate_calls`,
      `add_network`); `docs/KNOWN-BUGS.md` if anything was found.
- [ ] **T056** Update the memory of the device loop and the program status.

---

## Dependencies

- Phase 0 → Phase 1 → Phase 2 (US0) → Phase 3 (US1) → Phase 4 (US2) →
  Phase 5 (US3) → Phase 6 (US4) → Phase 7. US0 is enabling: without it no
  device task after T024 can run without a finger.
- T014/T015 (assembly in the core) block T025's gas estimate and T027's
  submit. T012 blocks T027's persist and T035's load.
- T035–T036 turn T027's `track_submitted` stub into the real handoff.

## Parallel example

Within Phase 0: T004, T005, T006, T007, T009 in parallel; then T008.
Within Phase 1: T010 ∥ T012 ∥ T014, then T011 ∥ T013 ∥ T015.
Within Phase 3: T025 ∥ T027 (both need Phase 1), tests T026/T029/T031 beside
their subjects; T030 before T032.

## Implementation strategy

MVP = Phase 0–3 (US0 + US1): dust leaves the phone in the parallel space.
Then US2 (the phone can be locked), US3 (nothing is silent), US4 (the
sheets and the backfills), and the record. Each phase is committed with its
device evidence; a phase whose device task failed is committed as "wired,
not verified" and the failure is the next task.
