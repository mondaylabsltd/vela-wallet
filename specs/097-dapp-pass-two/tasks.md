# Tasks: 097 — dApp pass two

**Input**: [spec.md](spec.md), [plan.md](plan.md), [findings.md](findings.md)

Part A tasks (User Story 1, branch `097-signing-readable`). Parts B and C list theirs on their own branches.

## Phase 1 — Evidence

- [x] T001 [US1] Copy the pass's requests into `rust/crates/vela-core/tests/fixtures/dapp097/` (1inch native order and `Order`, Aave borrow/repay, PancakeSwap/Uniswap BNB swaps, PancakeSwap USDC batch, Curve approve/swap) and the two descriptors the service served (`descriptors/`).
- [x] T002 [US1] Verify the outside facts: 1inch `MakerTraitsLib.sol` v4 layout (tag 4.0.0 = master 50bd1300 for the bits used); the pass order's expiry decodes to 2026-10-02 16:42:10 UTC; Curve router `0xA72C85…51CC` in `curve-router-ng` README (BSC) and `curve-js` `ALIASES_BSC.router`.

## Phase 2 — Core rules (`rust/crates/vela-core/src/app/clear_signing.rs`)

- [x] T010 [US1] N1: `uint256` → address (≤160 bits) for `addressName` and `tokenPath`; wider numbers shown whole.
- [x] T011 [US1] N1: an amount with no nameable token is unverified (em dash); any unverified amount makes the reading `partial`.
- [x] T012 [US1] N1: "beneficiary" reads as a recipient.
- [x] T013 [US1] N2: a borrow is `ReceiveAmount`; a repay stays `SendAmount`.
- [x] T014 [US1] N3: `ClearAmountBound` + `ClearSignField.bound`, graded from the label's words.
- [x] T015 [US1] N6: built-in typed reading of 1inch LOP v4 `Order` (receiver, unwrap-to-native, minimum, expiry).
- [x] T016 [US1] N8: Curve router known on chain 56; `token_name` (registry symbol alone, chain symbol + short address) on Token rows and batch targets.
- [x] T017 [US1] Core tests `tests/app_clear_signing_dapp097.rs` (each fails on the old core); 096 tests updated to the N8 rule.

## Phase 3 — Shells

- [x] T020 [P] [US1] Web `src/lib/signing/live.ts`: hero caption from `bound`, unverified tone, `warnPartial` + `warnUnverifiedAmount`; `messages.ts`, `engine.server.ts`; tests in `live.test.ts` over the real wasm core.
- [x] T021 [P] [US1] Desktop `src/signing/live.rs`: wiring tests over the real core (and the test literal's new field).
- [x] T022 [P] [US1] iOS `VelaWalletTests/DappSigningTests.swift` (`SigningLiveTests`): wiring tests over the real core.
- [x] T023 [P] [US1] Android `SigningLiveTest.kt`: wiring tests over the real core.

## Phase 4 — Regenerate, verify, report

- [x] T030 [US1] Regenerate: fmt, `gen:i18n`, wasm + sync, TS mirrors, Swift bindings; `build-web --check`, `gen-onboarding-types --check`.
- [x] T031 [US1] Suites: core, web, desktop, Android, iOS; reachability / event-payload / dead-control checks.
- [x] T032 [US1] Screenshots: web and iOS simulator for the 1inch native order, the 1inch `Order`, the Aave borrow and a single-call swap.
- [x] T033 [US1] `results-a.md`.
