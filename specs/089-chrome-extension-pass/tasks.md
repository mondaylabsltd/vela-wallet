# Tasks: 089 — the Chrome extension pass

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md)
Format: `- [ ] T### [P] [US#] description (files) — test`

## Phase 1 — Set-up

- [x] T001 Local `089-integration` = `origin/main` @ `67e2d193d` + PR #342 + PR #343; `pnpm build:extension` — package 37.7 MB, 120 pages
- [x] T002 Port-shifted copy of the extension suites (5186–5189), serial, no web server; baseline 39/42 (the 3 G35 cases red)
- [x] T003 [P] Exploration kit: real window sizes, CDP side-panel screenshots, stand-in chain + relay, chaos proxy per browser (scratch, not committed)

## Phase 2 — US1: functional (P1)

- [x] T010 [US1] Fix the G35 e2e cases to 083's answer (`e2e/extension-lifecycle.e2e.ts`, comments in `extension/background.js`, `extension/lib/request-life.js`) — extension e2e 35/35 on `main` → `fix/089-lifecycle-e2e`
- [x] T011 [US1] Functional matrix, window surface: discovery, connect, chains (known / unknown / malformed / add), refusals (eth_sign, eth_signTransaction, …), watchAsset, 085 typed-data refusals, other-account requests, SafeTx, sendCalls, revoke, second tab, other origin — 40 rows, `functional.json`
- [x] T012 [US1] A granted connect answered by the worker (`extension/background.js` `grantMirror`, `grantedAccounts`, `instantConnect`; `extension/lib/protocol.js` `instantConnectAnswer`; README row) — `background.test.ts` +5, `instant.test.ts` +12, `extension-connect` e2e +1 → `fix/089-instant-connect`
- [x] T013 [US1] Toolbar reuses only a normal window's tab (`extension/background.js` `openWallet`) — `background.test.ts` +2 → `fix/089-toolbar-tab`
- [x] T014 [P] [US1] Unstable network (chaos proxy): latency, blackhole, drop, recovery — `chaos.json`
- [x] T015 [P] [US1] Worker restart, reload, second tab, panel ✕ — covered by `extension-lifecycle` EX4/EX5/EX6/EX8 (green)
- [ ] T016 [US1] Requests without a gesture each open a window (F03) — owner: design a join/queue for the window surface after `dapp_browser`'s rule (same origin joins, another is told no)
- [ ] T017 [US1] `wallet_getCallsStatus` / `wallet_getCapabilities` 4200 everywhere (F04) — owner: a core route + the worker's twin
- [ ] T018 [P] [US1] Offline states in the shared wallet (F06, F07) — owner

## Phase 3 — US2: security (P1)

- [x] T020 [US2] Hostile pages: forged origin, replayed id, cross-origin frame, page probes of the package, long host, IDN host, flood — `security.json`, `security-longhost.json`
- [x] T021 [US2] The surface asks the core: `PopupRequest { origin, params_json }`, `requested_address`, `popup_origin_refusal` / `InsecureOrigin` (`rust/crates/vela-core/src/app/dapp_permissions.rs`; `src/lib/dapp/request.ts`, `core/dperm-*.ts`; wasm, pkg-web, TS mirrors) — core +5, `popup-sign-gate.test.ts` +5, `extension-signing` e2e +2 → `fix/089-popup-sign-gate`
- [x] T022 [P] [US2] The sheet header never truncates the host, and wraps it at its dots / colon (`src/lib/signing/ui/SigningHeader.svelte`) — `SigningHeader.svelte.test.ts` +3 → `fix/089-origin-never-truncated`
- [x] T023 [P] [US2] Nothing web-accessible (`extension/manifest.json`) — `package.test.ts` +1, `extension-security` e2e +1 → `fix/089-manifest-war`
- [x] T024 [US2] Permissions reviewed one by one — research.md R3
- [x] T027 [US2] `storage.local` closed to content scripts at worker start (`extension/background.js`) — `background.test.ts` +2, `extension-security` e2e +1 (isolated world over CDP) → `fix/089-storage-trusted`
- [ ] T025 [US2] A batch's sheet describes its first call only (F28) — owner: every shell's clear signing (core summary of every leg)
- [ ] T026 [P] [US2] Permit2 `PermitSingle` unlimited: red hero, no banner / cap (F22) — owner: shared approval guard

## Phase 4 — US3: usability & visuals (P2)

- [x] T030 [US3] Screenshots: request window (420 × 728), side panel (360 × 765, CDP), wallet tab (1440 × 900); en / zh / ru × light / dark; ru at the largest text — 101 shots in all (curated set in `evidence/`)
- [x] T031 [P] [US3] Whole address and slide label at 360 px (`PartyRow.svelte`, `SlideToConfirm.svelte`) — `PartyRow.svelte.test.ts` +2, `SlideToConfirm.svelte.test.ts` +4 → `fix/089-sheet-narrow-fit`
- [x] T032 [P] [US3] Font family tokens never fall back to a serif (`scripts/gen-tokens.mjs`, `tokens.css`) — `tokens.test.ts` +1 → `fix/089-font-fallback`
- [ ] T033 [US3] The request window's two dialects (bare consent page vs. bottom sheet over an empty scrim), consent without account / network (F26, F27) — owner: design

## Phase 5 — US4: store readiness (P3)

- [x] T040 [US4] Checklist and permission justifications — research.md R4, R3

## Phase 6 — Lead's extra item

- [x] T050 Reproduce "Workers Builds: vela-wallet-web" locally for `892c4839e` and `67e2d193d` (fresh install, `pnpm build`, `wrangler deploy --dry-run`, `wrangler versions upload --dry-run`) — both clean and identical; research.md R5 lists what the owner must check in the dashboard

## Phase 7 — Integration

- [x] T060 Local `089-integration-fixes` = `089-integration` + every fix branch, conflicts resolved (plan.md merge notes); extension e2e 47/47, web unit 2306 passed, svelte-check 0 errors; re-shot the fixed surfaces
