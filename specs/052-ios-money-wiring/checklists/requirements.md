# Requirements checklist — 052

Ticked at the phase that lands the behaviour; re-read at closeout.

## The door and the keys

- [ ] A Debug build enters the parallel space from `VELA_PARALLEL_SPACE=1` and from the persisted flag (FR-001)
- [ ] Every screen carries the badge while it is active (FR-002)
- [ ] Leaving removes exactly one `vela.accounts` record and touches nothing else (FR-003)
- [ ] An archived Release binary contains no fixture symbol and no badge string (SC-010)
- [ ] The Release configuration **compiles** with the binding excluded

## The send

- [ ] Token pick, form validation, Max, 继续, confirm, sign, submit, persist, track all run on `send` (FR-004)
- [ ] The fee on confirm is the fee signed; `quotedFeeUsable` gates it (FR-005)
- [ ] The fee-token sheet offers only the chain's accepted assets, each with a balance, and a change re-quotes (FR-005, FR-010)
- [ ] The contact picker reads the person's own book and fills the recipient (FR-010)
- [ ] The pending row is written before tracking starts, and the feed shows it (FR-006)

## In flight

- [ ] Tracking resumes from storage on launch (FR-007)
- [ ] The background grace keeps polling; the verdict is never lost (FR-007)
- [ ] A confirmation while away posts a notification that opens the receipt (FR-007)
- [ ] Notification permission is asked at the first submit, never at launch

## Refusals

- [ ] Every `SendAlertKind` reaches the screen as the core's wording (FR-008)
- [ ] No sentence contains a raw unit (FR-008)
- [ ] A cancel ends the attempt; the prompt count for that attempt is one, proven by a counting test (FR-009)
- [ ] An unreadable nonce refuses **before** the prompt

## The contracts

- [ ] All 30 operations answered exactly once, with a `neutralAnswer` twin per machine (FR-013)
- [ ] `CoreWireDriftTests` exhaustive for `send`, `fee_policy`, `tx_tracker` (SC-008)
- [ ] No business `if` in any executor
- [ ] The three `// live in 052` markers are gone (FR-011, SC-009)

## The invariants

- [ ] Zero lines under `rust/crates/vela-core/src/app/` (FR-016)
- [ ] Zero corpus delta (FR-016)
- [ ] `vela_core_uniffi.swift` unchanged (SC-011)
- [ ] Zero lines under the other four clients (FR-016)
- [ ] Every `*Fixtures.swift` diff additive; the gallery and sweep unchanged (FR-014)
- [ ] `audit-literals.mjs` still 35

## The record

- [ ] Every phase exercised on the founder's iPhone before it is called done (FR-015)
- [ ] results.md says **device-verified** or **test-only** for every SC (FR-015)
- [ ] Anything needing a finger is named as such, with what was done instead
