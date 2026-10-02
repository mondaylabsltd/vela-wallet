# Tasks: 092 — Networks the wallet cannot reach

## Phase 1 — Core (US1, US2, US3)
- [x] T001 `balance_dashboard`: `unreachable_networks` (ordered: held by worth, then network order) + `unreachable_key`, replacing `banner_chain_ids`.
- [x] T002 Per-account `last_read` record. Add `FetchSettled.read_chain_ids` (serde default). Carried-over rows stand in for an unseen read. Spam is not a holding.
- [x] T003 `UnreachableListOpened` / `UnreachableListClosed`: forced quiet re-read, then `UNREACHABLE_RECHECK_MS` after each read while open. Partial retries come first.
- [x] T004 Machine tests (10 new in `tests/app_balance_dashboard.rs`).
- [x] T005 Corpus: 8 new keys, 2 removed, 15 locales. Run gen / lint / verify / dump. Set the residency budget to 139,800.
- [x] T006 Artefacts: wasm, `rust/pkg-web`, TS mirrors, Swift bindings (unchanged), i18n outputs.

## Phase 2 — Shells (parity)
- [x] T010 [P] Web: Home line, `UnreachableBody` sheet / dialog, row → SR2 → back to the list, `read_chain_ids`, open/close events. Vitest + e2e (`home-truth.e2e.ts`).
- [x] T011 [P] Desktop: Home line, `unreachable_dialog` under FixRpc, Settings banner + wizard on the new copy, `read_chain_ids`, gallery DSR6, tests.
- [x] T012 [P] iOS: Home line (was "still updating"), SR6 sheet content with step to SR2, `read_chain_ids`, gallery H9 + SR6, tests.
- [x] T013 [P] Android: Home line, SR6 overlay on the Settings rescue route, row → RpcFix → back, `read_chain_ids`, SR6 fixture, tests.

## Phase 3 — Docs and checks
- [x] T020 087 `results.md` / `tasks.md`: F08, F12 and F32 owner rulings.
- [x] T021 Suites: core, web, desktop, Android, iOS, and the three CI scans.
- [x] T022 Screenshots (iOS simulator, desktop gallery, web e2e).
- [ ] T023 Device check on the Xiaomi and the iPhone (lead).
