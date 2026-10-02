# Tasks: 093 — every dApp interaction in Activity

**Input**: [spec.md](spec.md), [plan.md](plan.md)
Format: `- [ ] T### [P] [US#] description (files) — test`

## Phase 1 — Core (blocks everything)

- [x] T001 [US1] `DappSummary` + `summarize` + `protocol_of` + `stored_request` (`src/app/dapp_activity.rs`) — unit tests in the module; `tests/app_dapp_activity.rs`
- [x] T002 [US1] `ClearSigningView.record_intent`, `batch_headline`, headline `ClearTerm`s (`src/app/clear_signing.rs`) — `tests/app_clear_signing.rs` (+2 tests, 3 assertions)
- [x] T003 [US2] `SignApproveOpts.token_meta`; `SignRecord.{summary, stored_request, request_truncated}`; empty signature `result` (`src/app/sign_request.rs`) — `tests/app_sign_request.rs`, `tests/app_dapp_activity.rs`
- [x] T004 [US1] [US2] Feed: signature rows, `FeedLine` subtitle, `FeedDapp.{action, place, allowance, off_chain, facts, technical}`, receipt folding (`src/app/activity_feed.rs`) — `tests/app_activity_feed.rs` (3 pinned tests rewritten, +7)
- [x] T005 i18n `history.dappRowTitle` × 15; gen / lint / verify / vectors; `SC005_BUDGET` 140,800; gen-i18n path count 1787
- [x] T006 Artefacts: fmt, gen:i18n, build-web + sync-wasm, gen-core-types, Swift bindings

## Phase 2 — Shells (parallel after Phase 1)

- [x] T010 [P] [US1] [US2] [US3] Web + extension: approve opts, persist, read back, rows, detail + technical (`app-web/vela-wallet/src/lib/...`) — vitest
- [ ] T011 [P] [US1] Web dApp sheet simulation → `balance_changes` (SHOULD) — not done: `sim_outcome` has no wasm export (results.md)
- [x] T012 [P] [US1] [US2] [US3] Desktop (`app-desktop/vela-wallet/src/...`) — cargo test
- [x] T013 [P] [US1] [US2] [US3] iOS (`app-ios/VelaWallet/...`) — XCTest
- [x] T014 [P] [US1] [US2] [US3] Android (`app-android/vela-wallet/...`) — JVM unit tests

- [x] T015 [US3] Follow-up: `recipient` fact + `tokenDetail.labelContract`; lenient stored summary; `""` stored request = not recorded (core + four shells)

## Phase 3 — Verification

- [x] T020 Full suites (core, web, desktop, iOS, Android) + `check-native-reachability`, `check-event-payloads`, `check-dead-controls`
- [x] T021 Screenshots: swap / permit / SIWE rows + a detail — iOS simulator, web phone + desktop width, desktop headless; Android has no harness
- [x] T022 results.md
