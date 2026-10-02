# 092 results — networks the wallet cannot reach

Branch `092-unreachable-network-notice` (from `origin/main` @ `ec033f231`). The device check (Xiaomi, iPhone) is the lead's (T023).

## What each shell did before, and does now

| Shell | Home line before | Tap before | Now |
|---|---|---|---|
| Android (F08) | "4 个网络 RPC 不可用" | Settings + the RPC fix for `banner_chain_ids[0]` (BNB) only | 「4 个网络暂时连不上」 → SR6 list of all four, a sheet over the wallet (no tab switch); a row → its own fix in the same sheet → back to the list; SR3 also over the wallet |
| iOS | "部分余额仍在更新" for any failed chain (never named the networks) | the RPC fix for the first chain only | the core's line → SR6 in the same sheet; a row swaps to its SR2, ✕/Save step back |
| Web + extension panel | "N networks RPC unavailable" | sheet/dialog with the RPC fix for the first chain only | the core's line → list sheet (phone) / dialog (wide); row → SR2 → Done back to the list |
| Desktop | "N networks RPC unavailable" | FixRpc dialog for the first chain only | the core's line → list dialog; row → FixRpc over it; the Settings banner and the wizard's no-endpoint line also take the new copy |

None of the four shells already did the right thing: every one opened only the first network's fix.

## Core

- `BalanceView.unreachable_networks` / `unreachable_key` replace `banner_chain_ids`. `UnreachableNetwork` has `{chain_id, last_known: held|empty|not_read, last_seen_usd, line_key}`.
- The order is: networks last seen holding something first, by worth; then the rest in `BUILTIN_CHAINS` order; added networks by id.
- `last_read`: per account and per run. A chain that answered replaces its entry. A failed chain keeps its old entry. Rows a shell carried over (web, desktop) seed the entry only when it is empty. Spam does not count as a holding.
- `FetchSettled.read_chain_ids` (serde default) lets the core tell "held nothing" apart from "not read". All four executors send it.
- `UnreachableListOpened` / `UnreachableListClosed`: a forced quiet read on open, then `UNREACHABLE_RECHECK_MS` = 10 s after each read. An armed partial retry goes first. The timer stops on close, on an account switch, or when the list is empty. No reads happen while the app is backgrounded.

## Device pass 2026-10-02 (lead) — two follow-ups

**iPhone: the Home line never appeared under the chaos proxy.** Reproduced on a simulator. I used `chaos-proxy.py` in `mode=drop` with the lead's match list, `VELA_PARALLEL_SPACE=1`, and `VELA_DEV_PROXY=127.0.0.1:8941`.
- The fetch round did settle (`chains_failed chains=56` within seconds), and the Home line's gating was right.
- The failures were classified as **rate-limited**. Of the 23 BNB Chain endpoints the iOS pool sweeps, 22 were dropped. The one that got through, `lb.routeme.sh`, answers 429 ("public rate limit exceeded"). `rpc_pool` called a chain rate-limited if *any* node in the final pass throttled, and 092's list (rightly) leaves rate-limited chains out. So BNB Chain was hidden, Home said 「部分余额仍在更新。」, and the 0.0643 BNB stayed missing.
- **Fix (core, all shells):** a chain counts as rate-limited only when the final pass's nodes *answered*, if only to throttle. A pass in which any node could not be reached at all (timeout, refused, dropped, 5xx) is a failed chain, offered the fix. A chain whose every node says 429 is still the quiet, busy chain (invariant ④ kept).
- Test: `a_throttle_among_unreachable_nodes_is_a_failed_chain_not_a_busy_one`.
- After the fix, the same simulator run shows 「暂时连不上 BNB Chain」 (`agent092/shots/ios-chaos-{before,after}-fix.png`).
- The core rule is shared, so web, desktop and Android had the same blind spot and are fixed by the same change.

**A held-open connection kept the round from settling on iOS and Android (lead, follow-up).**
- Web and desktop gave up on a chain after 18 s; iOS and Android had no limit. A connection held open (chaos `stall`, and a real mode behind a blocking network) makes every POST wait for its 8 s timeout. A chain with 23 endpoints and 3 passes then takes minutes per call, and the round and Home wait on it.
- The limit is now the core's rule, `balance_dashboard::CHAIN_READ_DEADLINE_MS` = 18 000. It is exported as `balanceChainReadDeadlineMs()` (UniFFI) and as the wasm function of the same name.
- Every shell reads that one value: web `wallet-api.ts`, desktop `balances.rs`, iOS `TokenReads.bounded` (home round and switcher rows), Android `BalanceExecutor`.
- A chain past the limit is failed for the round, so it joins the unreachable list. The other chains' answers still land.
- Tests, one per shell, each with a transport that never answers:
  - web `wallet-api-chain-outage.test.ts`, fake timers at the core's deadline;
  - desktop `a_chain_that_never_answers_is_failed_at_the_deadline_and_the_rest_land`;
  - iOS `ChainDeadlineTests` (4);
  - Android `aChainThatNeverAnswersIsFailedAtTheDeadlineAndTheRestLand`, with `FakeRpcTransport(stall = …)`.
- Simulator check: chaos `mode=stall` on every BNB Chain and Polygon host. The round settled 21 s after launch with `chains_failed chains=137,56`, and Home shows 「2 个网络暂时连不上」 (`agent092/shots/ios-stall-after-fix.png`).

**Android: ✕ on a row's fix closed everything.**
- Before: the wallet route sent the sheet's ✕ AND Material's own dismissal (swipe, scrim, Back) through one callback, "back to the list". Material calls that only after it has already hidden the sheet. Its path therefore left the list open but invisible, which looks like "the whole sheet closed, Home shown". Its re-reads kept running, and the next tap on the Home line changed nothing, because the state was already "list".
- Now the two ways out are separate, as on the iPhone:
  - the sheet's ✕ (and Done after a restore) steps back: fix → list → closed;
  - a swipe down, the scrim, or Back closes the whole sheet.
- New `WalletRescueSheet` composable (the wallet route hosts it). `SettingsSheet` gains `onClose` for its ✕.
- Tests: the unit test `WalletRescueTest` (+1). The new instrumented `WalletRescueSheetTest` drives the real ✕ and Back on the sheet. It compiles here; run it on a device with the `am instrument` line in its header. I did not use adb.

## Copy (zh / en)

| Key | zh | en |
|---|---|---|
| unreachableOne | 暂时连不上 {{name}} | Can't reach {{name}} right now |
| unreachableMany | {{n}} 个网络暂时连不上 | Can't reach {{n}} networks right now |
| unreachableBody | 资产不受影响，只是现在读不到。 | Your assets there aren't affected — we just can't read them right now. |
| unreachableNone | 所有网络都已连上。 | Every network is reachable again. |
| lastSeen | 上次读到 {{amount}} | Last seen {{amount}} |
| lastSeenUnpriced | 上次读到时有资产 | Held tokens when last read |
| lastSeenEmpty | 上次读到时没有资产 | Held nothing when last read |
| notReadYet | 还没读到 | Not read yet |

- All 15 locales are covered. zh-HK uses written Cantonese (暫時連唔到…、仲未讀到).
- The row action reuses `assets.rpcFix` (修复 / Fix).
- The list's title is the Home line itself, so the list and Home cannot disagree.
- `assets.rpcUnavailable{Single,Multiple}` are removed.
- i18n size, ja + en JSON: +649 B (en +322, ja +327). The runtime route measures 139,205 against the new 139,800 budget (owner, 2026-10-02).

## Suites

| Suite | Result |
|---|---|
| core `cargo test --workspace` (i18n-all, dev-fixtures) | 2,318 passed, 0 failed (balance_dashboard: 62, of which 10 are new; rpc_pool +1; the deadline export pinned in the UniFFI and wasm tests) |
| core clippy `-D warnings` / fmt | clean / clean |
| web vitest | 170 files, 2,393 passed, 5 skipped |
| web `pnpm check` | 0 errors, 0 warnings |
| web e2e `home-truth.e2e.ts` (chromium, own preview on :4192) | 5 / 5 |
| desktop `cargo test` | 881 passed, 49 ignored (one pool test re-aimed: a 5xx is a node not reached); clippy has no warning on changed lines; fmt clean |
| Android `testDebugUnitTest` | 921 passed, 0 failed; the instrumented `WalletRescueSheetTest` compiles (it runs on a device) |
| iOS `VelaWalletTests` (own cloned iPhone 16 simulator) | 1,106 tests in 142 suites passed |
| check-native-reachability / check-event-payloads / check-dead-controls | pass / 0 mismatches / 0 dead controls |

## Screenshots

Under the agent scratchpad, `agent092/shots/`:
- iOS (simulator, gallery): `ios-home-h9-{zh,en}.png` (Home line), `ios-list-sr6-{zh,en}.png` (the list).
- Desktop (gallery DSR6, one-frame non-activating window): `desktop-dsr6-{zh,en}.png`. Chain logos are still loading in that single frame.
- Web (live preview, stubbed RPCs): `web-phone-zh-{home,list}.png`, `web-desktop-{zh,en}-{home,list}.png`, plus the e2e's `092-home-notice.png` / `092-list.png`.
- Android: no screenshot harness exists (no Robolectric / Paparazzi); gallery H9 (Home line) and SR6 (the list) are there for the device; the device check is the lead's.
