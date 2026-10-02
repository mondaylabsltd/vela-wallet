# 092 results — networks the wallet cannot reach

Branch `092-unreachable-network-notice` (from `origin/main` @ `ec033f231`). The device check (Xiaomi, iPhone) is the lead's (T023).

## What each shell did before, and does now

| Shell | Home line before | Tap before | Now |
|---|---|---|---|
| Android (F08) | "4 个网络 RPC 不可用" | Settings + the RPC fix for `banner_chain_ids[0]` (BNB) only | 「4 个网络暂时连不上」 → SR6 list of all four (same rescue route), each row → its own fix → back to the list |
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
| core `cargo test --workspace` (i18n-all, dev-fixtures) | 2,317 passed, 0 failed (balance_dashboard: 62, of which 10 are new) |
| core clippy `-D warnings` / fmt | clean / clean |
| web vitest | 170 files, 2,392 passed, 5 skipped |
| web `pnpm check` | 0 errors, 0 warnings |
| web e2e `home-truth.e2e.ts` (chromium, own preview on :4192) | 5 / 5 |
| desktop `cargo test` | 880 passed, 49 ignored; clippy has no warning on changed lines; fmt clean |
| Android `testDebugUnitTest` | 913 passed, 0 failed |
| iOS `VelaWalletTests` (own cloned iPhone 16 simulator) | 1,102 tests in 141 suites passed (targeted run first: 109 passed) |
| check-native-reachability / check-event-payloads / check-dead-controls | pass / 0 mismatches / 0 dead controls |

## Screenshots

Under the agent scratchpad, `agent092/shots/`:
- iOS (simulator, gallery): `ios-home-h9-{zh,en}.png` (Home line), `ios-list-sr6-{zh,en}.png` (the list).
- Desktop (gallery DSR6, one-frame non-activating window): `desktop-dsr6-{zh,en}.png`. Chain logos are still loading in that single frame.
- Web (live preview, stubbed RPCs): `web-phone-zh-{home,list}.png`, `web-desktop-{zh,en}-{home,list}.png`, plus the e2e's `092-home-notice.png` / `092-list.png`.
- Android: no screenshot harness exists (no Robolectric / Paparazzi); the device check is the lead's.
