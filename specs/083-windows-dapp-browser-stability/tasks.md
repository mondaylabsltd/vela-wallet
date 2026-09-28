# Tasks: 083 — the dApp browser works, and tells the truth, on Windows

Paths: `D/` = `app-desktop/vela-wallet/src/`, `C/` = `rust/crates/vela-core/`. Format `[ID] [P?] [Story]`.
Done is `[x]` with "— done: …"; a task not run says **NOT RUN:** and why.

## Phase 0: Device pass

- [x] T001 Build the installer from main and the parallel-space release — done: `VelaWallet-Setup-0.9.5-x64.exe` (de93634f)
- [x] T002 Driver: posted input + PrintWindow for the wallet, CDP for the page, fault proxy upstream the machine's proxy — done: see quickstart.md
- [x] T003 Device pass on Windows 11, findings W1–W19 with evidence — done: spec.md, `evidence/`
- [x] T004 Owner's fingerprint session on the installed build — done: exposed W19 (phone QR never drawn)

## Phase 1: User Story 1 — the installed browser starts (P1) 🎯 MVP

**Goal**: an installed app opens pages; a failed engine start is said once. **Independent test**: E1–E3.

- [x] T010 [US1] Profile under local app data / `VELA_STATE_DIR` via `wry::WebContext` (`D/webview.rs`, `D/executor/storage.rs`) — done: dbf5a48c
- [x] T011 [US1] `EngineFailure` + FAILED state, retried only on Retry (`D/explore/engine.rs`, `D/webview.rs`) — done: dbf5a48c
- [x] T012 [US1] Engine panel with the code, Retry, 在系统浏览器中打开 (`D/wallet/page.rs`) — done: dbf5a48c
- [x] T013 [US1] Erase deletes the profile folder when no view exists — done: dbf5a48c
- [x] T014 [US1] No demo host or demo tabs in a signed-in session (`page.rs`, `D/explore/live.rs::pending_tab`) — done: dbf5a48c
- [x] T015 [US1] Device: E1, E2 — done: read-only folder opens the dApp; runtime missing → panel, 1 log line, 0.12 CPU-s/10 s
- [x] T016 [US1] Device: E3 on the installed build (owner's UAC) — done: installed 24578479 opens the dApp, profile in %LOCALAPPDATA%

**Checkpoint**: E1–E3.

## Phase 2: User Story 2 — the address bar (P1)

- [x] T020 [US2] Ignore keyboard clicks; Enter/Esc leave the bar (`page.rs`) — done: 885867f4
- [x] T021 [US2] `LoadWatch::named_url`; the bar names and edits the load being opened — done: 885867f4
- [x] T022 [US2] An untouched open bar follows a commit (`address_follows`) — done: 885867f4
- [x] T023 [US2] Device: B1, B2 — done: 3/3 new host at 0.35 s; 4/4 exact typed site

## Phase 3: User Story 3 — WebView2's own events (P1)

- [x] T030 [US3] Core `LoadPlatform::WebView2` + table + tests (`C/src/app/browser_load.rs`, `C/tests/app_browser_load.rs`) — done: 9e0fe081
- [x] T031 [US3] `D/webview2_events.rs`: ContentLoading / NavigationCompleted / ProcessFailed / certificate CANCEL — done: 9e0fe081
- [x] T032 [US3] `LoadWatch::error_page` / `engine_failed`; page keeps the panel; crash panel on Windows — done: 9e0fe081
- [x] T033 [US3] Warning lock beside a refused certificate — done: 9e0fe081
- [x] T034 [US3] Device: C1–C5 — done: see results.md

## Phase 4: The signing column (US5, W19, D1)

- [x] T040 [US5] W19: the phone QR / touch / timeout cards inside the signing column; a scan that ran out is not an answer; ✕/Esc/Drop stop the scan — done: 545621d7, 4198ec6b
- [x] T041 [US5] D1: Esc never answers a pending request — done: 545621d7
- [x] T042 [US5] W11: "正在准备交易…" until the authenticator is asked — done: 545621d7
- [x] T043 [US5] W10: a native-coin transfer reads as a transfer on the desktop (phones unchanged) — done: 25ed87f6, d5661249
- [x] T044 [US5] Device: S1, S2; S3 with the owner's phone — done: Esc ×3 no answer, ✕ one 4001; preparing → landed; iPhone signed after W20 (24578479)

## Phase 5: Links, chrome, network (US4, D2–D4)

- [x] T050 [US4] New windows → a new tab on a gesture; mailto/tel only on a tap; other schemes refused; downloads refused — done: e6285fc9, a6b1b2fb
- [x] T051 W15: the chrome never shrinks; back/forward from the engine's history — done: e6285fc9, a6b1b2fb, 84e09838
- [x] T052 D4: the consent shows account and network — done: e6285fc9, a6b1b2fb
- [x] T053 D3: route choice per host (`D/executor/proxy.rs`) — done: 4bc67c85, 3cd27313, 3d22bcd1
- [x] T054 D2: menus over the page cut a region hole on Windows instead of hiding the page — done: 5c9b42a4
- [x] T055 Device: N1–N3 — done: see results.md

## Phase 6: Release and results

- [x] T060 Desktop + core suites; `cargo fmt` — done: desktop 718/0, core suites green, fmt clean
- [x] T061 Rebuild the installer, install (owner's UAC), E3 + S3 on the installed build — done: 24578479 installed
- [x] T062 results.md (SC table, before/after evidence), memory note — done; PR: see results.md
- [x] T063 W20 (found at T061): iPhone caBLE tunnel ids upper-case — done: 24578479

## Phase 7: Uniswap on Base and the hand-off (2026-09-28/29)

- [x] T070 Trace the owner's USDC → ETH on Base (parallel space): fee coin, relay answers, page answers — done: U1–U8 in results.md
- [x] T071 Receipts: reverted → one error; wait on an outcome, cap 10 min; never another op's hash — done: ddb52da8
- [x] T072 Fee: pay from what the op leaves; a relay refusal is "would fail"; coin list over a failed quote — done: 380d9014, 5825e443
- [x] T073 H1 sign-in copy, H7 write probe, H8 tab title — done: d9ab6e80, 672d1dbb, 0ff7fbed, 5c79c6b3
- [x] T074 H4/H5 phone stops and "check your phone" — done: a22e1b30, 95b8e573, 1063909f
- [x] T075 H6 hedged reads (desktop) — done: 8872b915, e6b7469e
- [x] T076 H2 dApp transactions in 活动 — done: 43fd67a4, ff660c2d
- [x] T077 H3 web plain send + preparing — done: 3707e196, 1618a9f6, 4d4c76c4
- [x] T078 H9 "Install for me only" — done: d344d847, 5adf0c17 (moving an existing install waits on D5)
- [x] T079 Merge all streams; receipts review #2 (only the op's own logs fail it) and #3 (revert receipt at once); README fee claim — done: e85859a9, 2448738a, ffc9ac9f; desktop 779/0, core 1896/0
- [x] T080 Device on `ffc9ac9f`: max USDC → ETH, ETH → USDC, 0.1 USDC → ETH landed; H1, H7, H8 pass — done: results.md
- [ ] T081 F1–F3: a dApp row says what it moved; the detail's hash fits; a contract is not 接收方 — in progress
- [ ] T082 H4/H5 with the owner's iPhone; install the rebuilt installer (owner's UAC) — **NOT RUN:** needs the owner
- [ ] T083 H6 on the device — **NOT RUN:** the pool never asks a slower node once faster ones answer; not stageable here without faking chain data (results.md)

## Dependencies

Phase 1 before everything on the installed build; Phase 3 depends on Phase 1's `webview2-com`
dependency; T054 after T050/T051 (same files); T061 after all code.
