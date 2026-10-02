# Tasks: 094 — the Chrome Web Store submission

**Input**: [spec.md](spec.md), [plan.md](plan.md). All done on `094-chrome-web-store` unless marked.

## Phase 1 — The package (B1, B2, S1, S11, NICE)

- [x] T001 `extension/build.mjs`: token check + wasm copy before `vite build`; derive `dist-store`
  (no `key`, no parallel space); `--zip`; `pnpm package:extension`
- [x] T002 manifest: 0.9.6, `minimum_chrome_version` 122, `incognito: not_allowed`; esbuild 122
- [x] T003 `package.test.ts`: both packages; the named wasm; store manifest = source minus `key`;
  no developer files
- [x] T004 CI: build both, run the package test, upload `chrome-extension` + `chrome-web-store`
- [x] T005 0.9.6 on desktop, Android, iOS

## Phase 2 — Core (S6, S8, S9, NICE)

- [x] T010 `dapp_rpc`: `Capabilities`, `CallsStatus`, `calls_status`, `capabilities`,
  `UNKNOWN_BUNDLE_ID`; tests
- [x] T011 `dapp_browser`: answer both; a CallsStatus read mapped through `calls_status`; tests
- [x] T012 `approval_guard`: `unlimited_warning` (permits included); tests
- [x] T013 `fee_policy`: `QUOTE_DEADLINE_MS`, `StartDeadline` / `DeadlineElapsed`; tests (the
  existing suite drops the deadline timer it never lets fire)
- [x] T014 provider: no console line; test
- [x] T015 corpus: 4 strings × 15 locales; gen/lint/verify/dump; path pin 1791; residency budget
  140,800 (owner, 2026-10-02)
- [x] T016 wasm exports; regenerate wasm, pkg-web, TS mirrors, Swift bindings (unchanged), Kotlin
  bindings (gitignored)

## Phase 3 — Extension worker (S3, S6, S7)

- [x] T020 `runtime.onInstalled` → welcome tab, `installed` mark when web tabs are open
- [x] T021 one request window per site: `holdsWindow`, `windowHolder`, `nextQueued`, `queueAfter`,
  recovery keeps a queue; `releaseWindow`, `answeredWithoutWindow`, `promoteQueued`
- [x] T022 EIP-5792 twins + routing; `calls-status.test.ts` against the core; worker tests
- [x] T023 README

## Phase 4 — Web shell (S2, S4, S5, S8, S9)

- [x] T030 `site-access.ts`, `ExtensionNotices.svelte` in the root layout; `SecurityError` sentinel
  and its corpus sentence
- [x] T031 erase → `packagedHref(welcome)`
- [x] T032 request window with nobody signed in → create / sign in in a tab; window close a backstop
- [x] T033 sheet: `unlimited_warning`, permit "can't be capped" line
- [x] T034 fee executor `start_deadline`; deployment read bounded

## Phase 5 — Native shells (S8, S9)

- [x] T040 desktop: guard warnings from the flag + permit line; fee deadline timer; tests
- [x] T041 iOS: wire field, guard blocks, fee executor; tests
- [x] T042 Android: wire field, guard blocks, fee wire + executor; tests

## Phase 6 — Docs, images, site (B3, B4, S1, S10)

- [x] T050 `docs/store-submission/chrome-web-store.md`
- [x] T051 images + `scripts/store-art/`
- [x] T052 privacy page (any site forwards; Limited Use); install docs ×15 → Chrome 122

## Phase 7 — Verification (S12)

- [x] T060 new e2e: store package, typed data v4, no-wallet connect, connect burst, landed send,
  batch status, erase
- [x] T061 every `e2e/extension-*.e2e.ts` on the final build; every suite; results.md

## Deferred

- [ ] T070 S9 balance half: the last known balance (not $0.00 / "Deposit your first asset") after an
  offline reload — owner call + four shells (results.md)
