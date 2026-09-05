# Tasks: Android Live Shell — Settings & Contacts on the Core

**Input**: `spec.md`, `plan.md`, `research.md`, `data-model.md`,
`contracts/shell-operations.md`

**Tests**: required. This feature's whole claim is that a drawn screen now
tells the truth, and an untested claim of truth is just a different fixture.

## Format: `[ID] [P?] [Story] Description`

- **[P]** — parallelisable (different files, no dependency)
- **[Story]** — US1 (settings), US2 (contacts), US3 (the paved road), or
  `INFRA` for work every story needs

**Paths** are relative to the repository root. The Android module root is
`app-android/vela-wallet/`; Kotlin sources live under
`app/src/main/java/app/getvela/wallet/` (abbreviated `…/wallet/` below).

---

## Phase 0 — Measure before committing (GATE)

- [X] **T001** `INFRA` Build A (baseline, 3 machines): `cargo ndk -t arm64-v8a
      --platform 29 build --release -p vela-core-uniffi`, strip, record bytes.
- [X] **T002** `INFRA` Build B (+ `Contacts`, `NetworkAdmin`,
      `DisplayCurrency`), strip, record.
- [X] **T003** `INFRA` Build C (all 24 machines), strip, record — so specs 041
      and 042 budget from data, not estimate.
- [X] **T004** `INFRA` Build D (scaffolding isolation: three duplicate objects
      over machines already linked) — the gate tripped, so the alternative was
      priced with a build instead of an argument.
- [X] **T004b** `INFRA` Verdict written into `results.md`: a uniffi object costs
      5,920 bytes, a machine 356,885; multiplexing would save 1.7%. Proceeding.

---

## Phase 1 — The road, part one: bridge + plumbing (US3)

- [X] **T005** `US3` Add the three `bridge_object!` registrations to
      `rust/crates/vela-core-uniffi/src/onboarding_bridge.rs`, doc comments
      matched to the wasm siblings in `vela-core-wasm/src/wallet_state.rs`.
- [X] **T006** `US3` Regenerate `rust/bindings/kotlin/` and confirm the three
      new classes are present (`ContactsCore`, `NetworkAdminCore`,
      `DisplayCurrencyCore`).
- [X] **T007** `US3` Create `…/wallet/core/crux/` and **move** `CoreDriver.kt`
      and the `CoreBridge` interface out of `feature/onboarding/core/`; update
      onboarding's imports. No copies, no behaviour change.
- [X] **T008** `US3` Add `…/core/crux/Wire.kt`: the single `Json` instance and
      its rules (`ignoreUnknownKeys`, `explicitNulls = false`, no
      `coerceInputValues` — research D2), with the reasoning in the header.
- [X] **T009** `US3` Add `…/core/crux/CoreHost.kt`: one machine's driver plus a
      `StateFlow` of its decoded view; typed on the view class, blind to the
      product.
- [X] **T010** `US3` Add the Kotlin serialization plugin + `kotlinx-serialization-json`
      to `gradle/libs.versions.toml` and `app/build.gradle.kts`.
- [X] **T011** `US3` **Gate**: `:app:testDebugUnitTest` and `:app:assembleDebug`
      green with the 118 existing tests unchanged.

---

## Phase 2 — The road, part two: store + drift gate (US3)

- [X] **T012** `US3` Extract `…/core/data/VelaStore.kt` — `read`/`write`/`remove`
      over the existing `vela_onboarding` DataStore file; `AccountStore` keeps
      its whole-record invariant and rides on top (research D9).
- [X] **T013** `US3` `VelaStoreTest`: absent reads as null, a write round-trips,
      a removed key is absent, and `AccountStore`'s existing behaviour is
      unchanged (its own tests stay green).
- [X] **T014** `US3` `CoreWireDriftTest`: for every declared Kotlin wire class,
      compare `@SerialName`s and nullability against the ts-rs mirror in
      `app-web/vela-wallet/src/lib/core/generated/`, via the `vela.repo.root`
      system property `DesignTokenDriftTest` already uses.
- [X] **T015** `US3` **Demonstrate SC-008**: rename a field in a Rust wire type,
      watch `CoreWireDriftTest` go red, revert, record the output in
      `results.md`.

---

## Phase 3 — Currency, end to end (US1)

- [X] **T016** `US1` `…/feature/settings/core/CurrencyWire.kt` — `CurrencyView`,
      `CurrencyOperation`, `CurrencyShellResult`, `CurrencyEvent`.
      **`rate` stays `Double?`** (data-model §1).
- [X] **T017** `US1` `CurrencyExecutor.kt` — four operations, exactly as
      `contracts/shell-operations.md` specifies; `read_device_currency`
      answered for real from the primary locale (research D8);
      `resolve_rate` fail-closed with `// live in 041`.
- [X] **T018** `US1` `CurrencyExecutorTest` — one case per operation, including
      a regionless locale (`en`) answering `null` and a stored code that
      round-trips.
- [X] **T019** `US1` Wire the currency host into `AppContainer` (lazy — research
      D5) and the currency overlay in `SettingsScreen` to it: the list, the
      chosen row, and the write on selection.
- [~] **T020** `US1` **Device check (SC-001)** — **BLOCKED**: needs a wallet on
      the device (a passkey ceremony is a human action) and MIUI refuses the
      instrumented-test APK. `CurrencyPersistenceTest` is written and waiting;
      the in-process half is green. See `results.md`.

---

## Phase 4 — Fields that accept typing (US1)

- [X] **T021** `US1` `VelaUrlField` gains an optional `onValueChange`; when
      absent, the rendering is byte-identical to today (research D7).
- [X] **T022** `US1` Compose test: with `onValueChange` the field accepts text
      and reports it; without it, the same tree renders as before.
- [X] **T023** `US1` **Gate**: every settings gallery state still renders
      (`settings-gallery`, all 28 states, light and dark).

---

## Phase 5 — Networks, read path (US1)

- [X] **T024** `US1` `NetWire.kt` — `NetView` and everything it reaches
      (data-model §1), plus `NetOperation`/`NetShellResult`/`NetEvent`.
- [X] **T025** `US1` `NetworkAdminExecutor.kt` — six storage operations live
      against `VelaStore`, one real debounce, ten network operations
      fail-closed with `// live in 041`, each answering the shape the contract
      names.
- [X] **T026** `US1` `NetworkAdminExecutorTest` — one case per operation;
      unreadable storage reads as "nothing configured"; a write failure still
      answers `written`.
- [X] **T027** `US1` `SettingsLive.kt` — `NetView` + `CurrencyView` + session →
      `SettingsScreenModel`, sibling of `SettingsFixtures`.
- [X] **T028** `US1` `SettingsLiveTest` — the networks list, the detail page,
      the endpoints and providers pages render from a core view; health tiles
      render their **unknown** state (FR-013), never a staged value.
- [X] **T029** `US1` `VelaNavHost`: the `settings` route reads the live builder;
      `settings-gallery` keeps its fixtures.

---

## Phase 6 — Networks, write path (US1)

- [~] **T030** `US1` Add-network wizard — **MOVED TO 041**: both routes into it
      need `fetch_chain_info` / `fetch_search_index`, which are fail-closed
      here, so the core never resolves a candidate and `can_add` stays false.
      That is the core refusing to save a chain it has not confirmed, not a
      wiring gap.
- [X] **T031** `US1` Network detail: edit the RPC and explorer URLs; removal
      with its confirm.
- [X] **T032** `US1` Provider keys and service endpoints: edit and save through
      the core, `vela.serviceEndpoints` written as the complete merged record.
- [X] **T033** `US1` Refusal path: a network the core rejects shows the core's
      refusal and writes nothing (FR-012) — unit-tested on the view, and seen
      once on a device.
- [~] **T034** `US1` **Device check (SC-002)** — **BLOCKED** with T020, and
      partly moot: the "add" half moves to 041 with T030.

---

## Phase 7 — Contacts, read path (US2)

- [X] **T035** `US2` `ContactsWire.kt` — `ContactsView`, `Contact`,
      `ContactGroupView`, `ContactRecipientView`, the enums, and the
      operation/result/event hierarchies.
- [X] **T036** `US2` `ContactsExecutor.kt` — three storage operations live (the
      camelCase stored shapes and the `address → ms` **map** verbatim);
      `load_send_history` truthfully empty; identity and classification
      fail-closed. All marked per the contract.
- [X] **T037** `US2` `ContactsExecutorTest` — round-trip each stored shape;
      a corrupt record reads as empty, never as a partial book.
- [X] **T038** `US2` `ContactsLive.kt` — `ContactsView` → `ContactsHomeModel`,
      `ContactDetailModel`, `GroupDetailModel`, with letter sectioning and
      search narrowing documented as the presentation judgements they are.
- [X] **T039** `US2` `ContactsLiveTest` — **parity cases ported from**
      `app-web/vela-wallet/src/lib/contacts/live.test.ts`: `#` sorts last,
      core order survives inside a letter, an unnamed address introduces itself
      by its short form.
- [X] **T040** `US2` `VelaNavHost`: the `contacts` route reads the live builder;
      `contacts-gallery` keeps its fixtures.
- [X] **T041** `US2` **Connect the Contacts tab.** It refuses to navigate today
      because the screen showed fixtures (research D11); that reason expires
      with T038. Settings' `通讯录` row keeps working and now reaches the same
      live screen.
- [X] **T041b** `US2` **Device check (SC-003)** ✅ **done on device**: the live
      contacts route on an empty store shows the empty state and its two calls
      to action — no fixture people, no fixture groups. (First attempt showed
      the fixture book: a silently incomplete `adb install`, not the code.)

---

## Phase 8 — Contacts, actions (US2)

- [X] **T042** `US2` Detail and group-detail actions take their target from the
      **rendered model** (data-model §3). A test proves the detail page's
      delete acts on the contact it is displaying.
- [X] **T043** `US2` Delete + dismiss + group membership reach the core and
      write through the executor.
- [~] **T044** `US2` **Device check (SC-004)** — **BLOCKED** with T020. The
      machine test already deletes the middle of three and asserts both the
      view and the stored bytes.
- [X] **T045** `US2` Record in `results.md` which contact actions have **no
      artwork** on Android (add/edit form, favourites) — as a debt for the
      designer, not as scope silently absorbed (FR-016).

---

## Phase 9 — Honesty pass and closeout

- [X] **T046** `INFRA` `grep -n 'Fixtures\.' navigation/VelaNavHost.kt`: every
      remaining hit is a developer route or belongs to 041/042, and is listed.
- [X] **T047** `INFRA` `grep -rn 'live in 041'`: the count is recorded as the
      budget spec 041 inherits.
- [X] **T048** `INFRA` **FR-018 proof**: the aeroplane-mode device pass behaves
      identically to the connected one.
- [X] **T049** `INFRA` `results.md`: ten success criteria verdicted with
      evidence, the size measurements, the debts, and a handover section naming
      the first thing spec 041 must do.
- [X] **T050** `INFRA` Final gate: `:app:testDebugUnitTest`, `:app:assembleDebug`,
      and every gallery state rendering.

---

## Legend

`[X]` done · `[~]` not done, with the reason recorded here and in `results.md`.
Nothing is marked done that is not.

## Dependencies

```text
Phase 0 (GATE) ─► Phase 1 ─► Phase 2 ─┬─► Phase 3 ─► Phase 4 ─► Phase 5 ─► Phase 6   (US1)
                                      └─► Phase 7 ─► Phase 8                          (US2)
                                                                    Phase 9 ◄─ both
```

US1 and US2 are independent after Phase 2, and either alone is a shippable
increment: settings that remember, or an address book that is the person's own.
US3 is not a screen — it is Phase 1 + Phase 2, and its proof is that Phase 3
costs four files.

## Parallelisable

`[P]` pairs within a phase: T016/T024/T035 (wire files, disjoint), T018/T026/T037
(executor tests, disjoint), T028/T039 (live tests, disjoint). Everything else is
sequential because it shares `VelaNavHost`, `AppContainer` or the gate.
