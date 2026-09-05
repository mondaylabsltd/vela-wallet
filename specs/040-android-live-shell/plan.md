# Implementation Plan: Android Live Shell — Settings & Contacts on the Core

**Branch**: `040-android-live-shell` | **Date**: 2026-09-05 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/040-android-live-shell/spec.md`

## Summary

Make the Android Settings and Contacts surfaces read and write the person's own
wallet instead of fixtures, by driving three existing `vela-core` machines —
`contacts`, `network_admin`, `display_currency` — over the uniffi bridge, and
by paving the road that the remaining twenty-one machines (specs 041, 042) will
travel unchanged.

Three parts, in this order because each is the previous one's proof:

1. **The road** — the bridge carries three more machines (size-measured first);
   the Crux plumbing moves out of the onboarding feature into a shared package
   and gains a per-machine host; a single key-value store serves everyone; a
   drift test makes a Rust rename fail an Android build.
2. **Settings** — currency and networks read from and write to the core, with
   the display-only fields given the ability to be typed into.
3. **Contacts** — the address book, its groups and its detail pages read the
   device's own store, and the actions the drawn surface can reach act on the
   entity that is on screen.

No business rule is written on either side of the bridge. No pixel moves.

## Technical Context

**Language/Version**: Kotlin 2.2.10 (Compose, AGP 9.3.1, JVM target 11); Rust
1.x for the bridge crate only (registration lines, no logic).

**Primary Dependencies**: existing — `uniffi 0.32`, JNA, DataStore Preferences
1.1.7, Navigation Compose, `org.json`. **Added by this feature** —
`org.jetbrains.kotlin.plugin.serialization` 2.2.10 and
`kotlinx-serialization-json` (research D2).

**Storage**: `androidx.datastore` Preferences, one file (`vela_onboarding`),
keys and payload shapes shared byte-for-byte with the web and desktop clients
(research D9).

**Testing**: JVM unit tests (`app/src/test`, JUnit4) — the existing 118-test
suite is the floor; Compose UI tests only where a rendering claim needs one;
one on-device pass per user story for the persistence criteria (SC-001…SC-004),
because "survives a force-stop" cannot be asserted on the JVM.

**Target Platform**: Android, `minSdk 29`, `targetSdk 36`, ABIs
`arm64-v8a`/`armeabi-v7a`/`x86_64`.

**Project Type**: mobile app inside a five-client monorepo whose single source
of business rules is a Rust crate.

**Performance Goals**: no regression in cold-start time (the launch animation
already covers the first frame); a settings or contacts screen shows settled
data within one frame of its machine reporting `loaded`; no main-thread I/O.

**Constraints**: **zero network calls** from the wired surfaces (FR-018); the
stripped `arm64-v8a` size delta measured and inside the budget agreed in
research D6 before any machine is added; no business `if` in any executor.

**Scale/Scope**: 3 of 24 machines wired (bringing Android to 6); ~40 Kotlin
wire declarations; 2 screens, 8 subpages, 14 overlays already drawn; 1 Rust
file touched (three registration lines).

## Constitution Check

`.specify/memory/constitution.md` is the unfilled Spec Kit template — it
defines no project-specific gates, so there is nothing to violate and nothing
to claim compliance with. The gates this feature actually answers to are the
ones the sibling specs made normative, and they are enumerated here so the
absence of a constitution does not read as an absence of rules:

| Gate | Source | How this plan meets it |
| --- | --- | --- |
| Rules live in Rust; shells render and perform I/O | 024 Why / 019 FR | FR-019; no Kotlin `if` on a business condition — the drift test and code review are the enforcement |
| Every operation answered exactly once | 019 `contracts/shell-operations.md` §0.1 | one `when` arm per operation, exhaustive on a sealed hierarchy |
| Executors never throw for an expected failure | §0.2 | failures map to the machine's own failure variant |
| Executors hold no business `if` | §0.3 | classification stays in the core; the shell reports what it observed |
| A Rust wire change is a compile error, not a silent default | §0.4 | sealed `when` without `else` + the drift test (research D3) |
| Committed generated artefacts are regenerated and checked | 031 handover | not triggered — this feature changes no `rust/` type, only registrations. If that changes, `gen-core-types.mjs --check`, `build-web.mjs --check` and `verify-web.mjs` run before commit |

## Project Structure

### Documentation (this feature)

```text
specs/040-android-live-shell/
├── spec.md
├── plan.md              # this file
├── research.md          # D1–D11, decisions with their evidence
├── data-model.md        # wire types, stored shapes, display models
├── quickstart.md        # build it, run it, prove it
├── contracts/
│   └── shell-operations.md   # what each executor answers, per operation
├── checklists/
│   └── requirements.md
└── results.md           # written as the work lands: verdicts, measurements, debts
```

### Source Code (repository root)

```text
rust/crates/vela-core-uniffi/src/
└── onboarding_bridge.rs          # +3 bridge_object! registrations (no logic)

app-android/vela-wallet/app/src/main/java/app/getvela/wallet/
├── core/
│   ├── crux/                     # NEW — the paved road
│   │   ├── CoreBridge.kt         #   the interface uniffi objects satisfy
│   │   ├── CoreDriver.kt         #   MOVED from feature/onboarding/core/
│   │   ├── CoreHost.kt           #   NEW — machine + driver + view StateFlow
│   │   └── Wire.kt               #   NEW — the shared Json and its rules
│   └── data/
│       └── VelaStore.kt          # NEW — the key-value half of AccountStore
├── feature/
│   ├── onboarding/core/          # imports the promoted classes; keeps no copy
│   ├── settings/
│   │   ├── core/
│   │   │   ├── NetWire.kt        # NEW — network_admin wire types
│   │   │   ├── NetworkAdminExecutor.kt
│   │   │   ├── CurrencyWire.kt   # NEW — display_currency wire types
│   │   │   ├── CurrencyExecutor.kt
│   │   │   └── SettingsController.kt   # the two hosts, one screen state
│   │   ├── SettingsLive.kt       # NEW — core views → SettingsScreenModel
│   │   └── components/SettingsPrimitives.kt   # VelaUrlField gains input
│   └── contacts/
│       ├── core/
│       │   ├── ContactsWire.kt   # NEW
│       │   ├── ContactsExecutor.kt
│       │   └── ContactsController.kt
│       └── ContactsLive.kt       # NEW — core view → ContactsHomeModel
├── navigation/VelaNavHost.kt     # fixture call sites → live builders
└── VelaWalletApplication.kt      # lazily holds the three hosts

app-android/vela-wallet/app/src/test/java/app/getvela/wallet/
├── CoreWireDriftTest.kt          # NEW — Kotlin ↔ ts-rs mirrors (FR-008)
├── ContactsExecutorTest.kt       # NEW
├── NetworkAdminExecutorTest.kt   # NEW
├── CurrencyExecutorTest.kt       # NEW
├── ContactsLiveTest.kt           # NEW
└── SettingsLiveTest.kt           # NEW
```

Everything else — 158 existing Kotlin files, six fixture galleries, the design
system — is untouched by construction.

## Phase plan

Each phase ends green (`assembleDebug` + unit tests) and is a commit. The order
is not arbitrary: every phase is the evidence the next one needs.

**Phase 0 — Measure before committing.** Build A/B/C of research D6; record the
stripped deltas. If B exceeds the budget, stop and price the multiplexed-bridge
alternative before writing a line of Kotlin. *(Gate for the whole plan.)*

**Phase 1 — The road, part one: bridge + plumbing.** Three `bridge_object!`
lines; regenerate the Kotlin bindings; move `CoreDriver`/`CoreBridge` into
`core/crux/`; add `CoreHost` and `Wire`; onboarding keeps working through the
promoted code. Proof: existing tests green, no new behaviour.

**Phase 2 — The road, part two: store + drift gate.** Extract `VelaStore`;
`AccountStore` rides on it unchanged in behaviour; `CoreWireDriftTest` lands
with the first wire declarations and is *demonstrated* to go red on a rename
(SC-008).

**Phase 3 — Currency.** The smallest machine (413 lines, four operations)
proves the whole road end to end: wire types, executor, host, live builder,
the currency sheet reading and writing a real preference, `read_device_currency`
answered for real (research D8), `resolve_rate` fail-closed with `// live in 041`.

**Phase 4 — Fields that accept typing.** `VelaUrlField` gains `onValueChange`;
every existing call site and gallery state renders unchanged (research D7).

**Phase 5 — Networks, read path.** `network_admin` wire types, executor
(six storage operations live, ten network operations fail-closed), host, and
the networks list + network detail + endpoints + providers pages reading the
core.

**Phase 6 — Networks, write path.** Add / edit / remove a custom network,
provider keys, service endpoints — each through the core, each surviving a
force-stop, each showing the core's refusal when it refuses.

**Phase 7 — Contacts, read path.** Wire types, executor (three storage
operations live; `load_send_history` truthfully empty until 041;
`resolve_identity` and `classify_recipient` fail-closed), host, and the home,
search, group and detail surfaces reading the stored book.

**Phase 8 — Contacts, actions.** Delete, group membership and dismissal act on
the entity the screen is showing — with the target taken from the rendered
model (the desktop sibling shipped a detail page that deleted a *different*
contact; the same shape of bug is what this phase's tests are aimed at).

**Phase 9 — Honesty pass and closeout.** Every remaining fixture reachable
from a signed-in route is either gone or carries its `// live in 041` marker
with a rendered unknown state; `results.md` records the seven verdicts, the
size measurements, and the debts handed to 041.

## Complexity Tracking

One dependency is added (`kotlinx-serialization-json`) and one file is moved
across packages. Both are justified in research D2 and D4 respectively, and
both are cheaper than the alternative they replace (thousands of lines of
hand-written JSON walking; a second copy of the driver). No other complexity is
introduced: no DI framework, no code generator, no new module, no abstraction
that has exactly one implementation.
