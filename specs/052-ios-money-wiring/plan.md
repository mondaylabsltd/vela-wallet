# Implementation Plan: iOS Money Wiring

**Branch**: `052-ios-money-wiring` | **Date**: 2026-09-14 | **Spec**: [spec.md](./spec.md)

## Summary

Wire three Crux machines — `send`, `fee_policy`, `tx_tracker` — into the
SwiftUI client so the drawn send journey moves real money, and pay once for two
pieces of plumbing every later cut reuses: the **user-operation spine** (which
053's dApp transactions and 054's split/sweep both go through) and the
**parallel space** (which is how 053–057 get device-verified without a finger).
No Rust machine changes and, unlike 050 and 051, **no bridge export and no
regenerated `vela_core_uniffi.swift`** — every export this needs shipped with
Android's 043–049 and is already in the committed bindings.

## Technical Context

**Language/Version**: Swift 5.9 / SwiftUI on iOS 17.4+; Rust 1.97.1 for the
(unchanged) machines.

**Primary Dependencies**: existing only — `VelaCoreKit`, `CoreDriver`,
`CoreStore`, `VelaStore`, `CoreHTTP`, `RpcPool`, `PasskeyExecutor`,
`Foundation.URLSession`, `UserNotifications`, `BackgroundTasks`. No new
packages. One new **local** binary artefact: the Debug-only fixtures
xcframework (research D1).

**Storage**: `UserDefaults`, JSON text under `vela.*` keys. This cut writes
`vela.transactionHistory` (new writer, existing key and shape) and, in Debug
only, `vela.parallelSpace` / `vela.parallelSigner`.

**Testing**: Swift Testing in `VelaWalletTests` (executors, the spine, the
relay client's classification, the tracker's scheduler, drift); the live suite
behind `-DVELA_LIVE_TESTS`; XCUITest `LiveWiringAcceptanceTests` on the
founder's iPhone for every P1 scenario.

**Target Platform**: iPhone, iOS 17.4+.

**Performance Goals**: no regression to cold start; the confirm screen's fee
appears within one quote round trip; a foreground tick costs one bundler call
per pending operation per three seconds and stops when nothing is pending.

**Constraints**: zero lines under `rust/crates/vela-core/src/app/`; zero corpus
delta; tokens only (the literal audit baseline is 35 and the gate is "no new
ones"); no business `if` in executors; the four other clients untouched; a
Release archive must contain no fixture symbol.

**Scale/Scope**: 3 machines, 30 operations (18 + 6 + 6), 6 screens + 2 sheets,
one new build configuration wrinkle. Roughly 12 new Swift files; the Swift test
count should grow by ~45.

## Constitution Check

`.specify/memory/constitution.md` is still the unfilled Spec Kit template, so
the gate is the repo's rules plus `docs/agent-rules/AI-CODING-RULES.md`, as
050 and 051 recorded.

| Rule | This feature |
| --- | --- |
| **One authoritative implementation per capability per platform** | ✅ Rules stay in `vela-core`. The spine is one file serving this cut's transfers and 053's dApp transactions; the fee quote is the `fee_policy` machine, not a second estimator. |
| **Tokens only** | ✅ No new colour, spacing or type literal; `audit-literals.mjs` stays at 35. |
| **i18n through vela-core** | ✅ Spec 021 drew these screens with their text; zero corpus delta ([contracts/i18n-keys.md](./contracts/i18n-keys.md)). |
| **Generated files are regenerated, not hand-edited** | ✅ `vela_core_uniffi.swift` is not touched at all; `vela_dev_fixtures.swift` comes from the new script and is committed as generated. |
| **Fixtures are the single canon for UI state** | ✅ `SendLive` is a sibling of `WalletFlowFixtures`, as `WalletLive` is of `WalletFixtures`. The gallery and the screenshot sweep are unchanged. |
| **Core decides, shell performs** | ✅ [contracts/shell-operations.md](./contracts/shell-operations.md) answers all 30 operations; expected failures are the core's own variants; unknown tags answered loudly. |
| **One PR solves one problem** | ✅ Eight phases, each one commit with its own gate. |
| **High-risk changes carry risk, evidence, rollback** (§3) | ⚠️ **High**: this is the first iOS code path that spends money and the first that signs anything outside onboarding. Mitigations: the assembly and the classification are the core's (no Swift arithmetic on a fee); the spine refuses before the prompt on any unreadable precondition; `quoted_fee_usable` gates the signed fee against the displayed one; device sends are dust on one chain; the parallel space is the default test subject so the founder's wallet is not the experiment. Rollback is per phase — each is one commit and the flow host falls back to fixtures by reverting one file. |

No violations to justify — Complexity Tracking left empty.

## Project Structure

```text
specs/052-ios-money-wiring/
├── spec.md  plan.md  research.md  data-model.md  quickstart.md  results.md
├── contracts/{shell-operations.md, i18n-keys.md}
├── checklists/requirements.md
└── tasks.md                      # the cold-pickup handoff

rust/crates/vela-dev-fixtures-uniffi/Cargo.toml   # +staticlib in crate-type
rust/scripts/build-ios-dev-fixtures.sh            # NEW, sibling of the core's

app-ios/VelaDevFixturesKit/Artifacts/             # NEW, gitignored
app-ios/VelaWallet/VelaWallet/
├── Core/
│   ├── RelayClient.swift          # NEW — bundler JSON-RPC + REST, two caches
│   ├── UserOpSpine.swift          # NEW — assembly order, one implementation
│   ├── ParallelSpaceHook.swift    # NEW — the seam; always compiled
│   ├── TrackerNotifier.swift      # NEW — permission, post, tap
│   ├── BackgroundClock.swift      # NEW — grace + BGTaskScheduler
│   ├── CoreHTTP.swift             #   + a REST GET with one header
│   └── TxRecords.swift            #   + writeRecords
├── Dev/
│   ├── vela_dev_fixtures.swift    # NEW, generated + committed, Debug only
│   └── ParallelSpaceBinding.swift # NEW, Debug only
├── Features/Send/
│   ├── SendExecutor.swift         # NEW — 18 operations
│   ├── SendStore.swift            # NEW — resident
│   ├── SendWire.swift             # NEW — Decodable view mirrors
│   ├── SendLive.swift             # NEW — SendView → FlowScreenModel
│   ├── FeeExecutor.swift          # NEW — 6 operations
│   ├── FeeStore.swift  FeeWire.swift
│   └── Core/UserOpSigner.swift    # NEW — the seam
├── Features/Wallet/
│   ├── TrackerExecutor.swift      # NEW — 6 operations
│   └── TrackerStore.swift         # NEW — resident, booted at launch
├── Features/Contacts/{ContactsExecutor,ContactsLive}.swift   # markers down
├── Features/Settings/NetworkAdminExecutor.swift              # marker down
├── App/{RootView,VelaWalletApp}.swift                        # composition
└── Info.plist                                                # BG + notifications
```

**Structure decision**: `Features/Send/` is new and holds the three money
machines' shells together, because they are one journey and their executors
call each other (`estimate_fee` → the fee session). The spine and the relay
client live in `Core/` instead, because 053 drives them from the browser and
nothing about them is the send screen's.

## Phases

Each phase is one commit ending on a green gate: build, `xcodebuild test`, the
literal audit, and — from phase 3 — a device run.

| # | What lands | Proves |
|---|---|---|
| 0 | Baselines measured; the 30-operation inventory; contracts written | Nothing is claimed that was not measured |
| 1 | `RelayClient`, `CoreHTTP.get`, `TxRecords.writeRecords`, `UserOpSigner`, `UserOpSpine` — with tests, no screen | The money plumbing, provable before anything can spend |
| 2 | **The parallel space**: the crate's `staticlib`, the build script, the Debug link, the hook, the binding, the door, the badge | SC-010 and US0 — and every later phase becomes drivable |
| 3 | `fee_policy` + `send` wired: stores, executors, wires, `SendLive`, the flow host reading the core | SC-001 to the confirm screen |
| 4 | Sign and submit: the spine behind `submit_user_op`, records persisted, `track_submitted` handed off | SC-001 on the device — money moves |
| 5 | `tx_tracker`: resident, foreground tick, launch resume, background grace, notification, tap-to-receipt | SC-003, US2 |
| 6 | Refusals and the two sheets: every alert the core raises, the fee-token sheet, the contact picker | SC-005, SC-006, US3/US4 |
| 7 | The three markers; closeout: device pass, SC table, baselines re-measured, results.md | SC-009, SC-011, FR-015 |

The order differs from a naive "smallest first" for one reason: **the parallel
space is phase 2, not phase 1**, because it is worthless until there is
something to sign, and phase 1's spine is testable without it. Android put it
first and then had to stub the thing it was gating.

## Complexity Tracking

> No Constitution Check violations. Left empty.
