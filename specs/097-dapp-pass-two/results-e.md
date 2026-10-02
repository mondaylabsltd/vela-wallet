# 097 part E — the desktop dApp browser after a real-money pass (S2, S3) and the same on every shell

**Branch:** `097-desktop-dapp-fixes`, on top of `097-dapp-activity-amounts` (097 A + C + B). Not pushed.
**Evidence:** the desktop pass (`scratchpad/desk097/`: `logs/app-01.log` line 641, shots 123/129/131/132/134/136/137/138).

## Plan

### S2 — `wallet_getCallsStatus(<the id the page got>)` read "Unknown bundle id"

**Root cause (verified in code and log).** The desktop's landing wait ended `Landed(tx)` → `after_landing` → `Succeeded{result: tx}` for every method, so `sign_request` answered the PancakeSwap batch with the **tx hash**. The browser core knows a batch only by the hash the shell names (`user_op_hash_of` names the answer only when it equals the op), so neither the tx hash nor the op hash was a known bundle: every `wallet_getCallsStatus` read 5730 "Unknown bundle id" and the page sat on "Proceed in your wallet" over a landed swap. iOS (`SignExecutor.afterReceiptWait`) and Android (`SignExecutor.afterReceiptWait`) do the same — the same defect on both phones' in-app browsers. The web/extension answered the op hash at acceptance (`handleSendCalls` → `receipt_pending`), which is why the extension worked.

**The one rule (core).**

| Question | Rule | Where |
|---|---|---|
| What is a batch answered with? | Its id = its user operation's hash (EIP-5792). The moment the relay **accepts** it (`OpSubmitted`, not `maybe_sent`, through the write-ahead) the core answers the page with it — every shell, whatever its own receipt wait would say; the shell's late `Submit` is dropped (the RJ4 path). A batch that may only have been sent is answered when its fate is known, still with the id: `Succeeded{tx}` / `NotConfirmed` / tracker `Confirmed{tx}` → the op hash; `Reverted` / `Rejected` / `NotSent` stay the 083 errors. | `sign_request.rs`: `answer_batch_id`, `batch_id`, `on_op_submitted`, `on_submit_outcome`, `on_op_tracked` |
| How is the id known to a status lookup? | The browser core records a `wallet_sendCalls` answered with a 32-byte hash **by the method**, with the chain it was forwarded on — no shell has to name it. The shell-named op hash is still recorded (receipt translation). | `dapp_browser.rs`: `signing_answered`, `SignJob.chain_id`, `user_ops: BTreeMap<hash, chain>` |
| How does the lookup resolve? | Unchanged rule (`dapp_rpc::calls_status`, spec 094/096 F3): receipt → 200 + the operation's receipt (500 if it reverted); none and the relay refused before any block → 400; else 100. Now read on the **batch's** chain, not the site's current one (the extension already did). | `dapp_browser.rs` `Route::CallsStatus`, `start_read` |

The sheet follows the tracker after an early answer (`ending_of(batch, op, op)` = `StillConfirming` → `Following` → `Confirmed`); the tracker alone closes the record.

**Shells.** Web: the executor remembers the batch's chain *before* reporting `op_submitted` (the core now answers inside that call), so the extension's answer still carries `opHash: {chainId}` and the worker records the id (`rememberOp`). Desktop / iOS / Android: no change needed in the executors — their waits end answered (RJ4) — docs updated; each gets a wiring test through its own browser host.

### S3 — the disconnect dialog under the page

`dialog_over_browser()` named only the account switcher and the artwork viewer; the Connection panel's confirm (`self.confirm`) was painted under the native webview. Now `dialog_up()` lists **every** centred dialog the page root draws (confirm, settings dialogs, network remove, import result, group form, pick, export, explore form/groups, contact QR, balance detail, unreachable, feedback viewer, sign-out, the dApp column's Trusted Signer prompt), and a source test fails if a new root dialog is added without saying whether it hides the page. Desktop only: the phones' webviews are SwiftUI/Compose children, not native overlays; the extension has no in-page dialogs over a webview.

### S3 — the Connection panel for a disconnected site

The panel read `connected` from the core (`DbrTabView.connected_address`) but used it only for the status word. Now, not connected (and not asking): the status says **"No active connection"** (existing key `home.connEmptyTitle`, 0 new bytes), and the account row, the "can see your address" explainer, Disconnect and "requests appear here" are gone; the network row stays (the page reads its chain either way). Same on iOS (`ConnectionModel.connected`) and Android (`ConnectionModel.connected`), which drew the same full panel. Desktop also: the toolbar ⋯ menu no longer offers Disconnect for a site with no grant (the phones already hid it), and the account chip on the start page no longer opens the drawing's mock ("app.uniswap.org · Connected · Gnosis" — found while taking the screenshots). Web: its Explore is gallery-only (no live connection panel); the extension lists only connected sites — no such surface.

### S3 — times on the signing sheet in UTC

`clear_kickoff` passed `ClearLocale::default()` (comma-dot, **MDY**, 24 h, offset **0**). Now `clear_locale()` = the person's presets (`format_prefs::current()`, Settings → Localization, machine conventions on "Automatic") + this machine's offset now (`executor::local_utc_offset_minutes`, the same `localtime_r` the feed's day boundary uses). Verified the others: iOS (`SigningController.defaultLocale`: presets + `TimeZone.current.secondsFromGMT()`), Android (`ClearLocale.fromFormats`: presets + `TimeZone.getDefault().getOffset(now)`) pass the real offset; the web/extension passed the real offset but **fixed presets** (`comma_dot/iso/h24`) — now `toClearLocale(resolvedFormatKeys())`.

### S4 / not in scope

- S4 "Interacting with 0x8AC76a51…580d" (BSC USDC as a batch leg target): left to 097 D's chain-scoped token registry; not confirmed here (D has not landed on this branch).
- A request the dApp already timed out on still signs and lands (page-internal timeout, invisible to the wallet) — note only; the S2 early answer removes the batch case's long wait on the phones and desktop.
- Minimums to 18 decimals, lowercase recipients — unchanged (earlier decisions).

## Tasks

- [x] T1 core `sign_request`: batch answered with its id at acceptance; with its id after any wait; tracker `Confirmed` → id.
- [x] T2 core `dapp_browser`: batch ids recorded by method, with their chain; status and receipt lookups on that chain.
- [x] T3 core tests `tests/app_batch_id_097.rs` (5, the pass's real batch `req-pcs-usdc-bnb.json` + its real receipt; 4 fail on the old core).
- [x] T4 regenerate wasm (`vela_core_bg.c6f1e81602a4.wasm`), `rust/pkg-web`; TS mirrors and Swift bindings unchanged (no type change).
- [x] T5 web: executor remembers the batch chain before `op_submitted` (accepted) or at return (maybe sent); sheet locale from the person's presets; 3 tests.
- [x] T6 desktop: batch wiring test through `BrowserDriver` + the real sign core; `clear_locale`; `dialog_up`; not-connected panel; ⋯ menu; start-page chip; 5 tests (+1 extended).
- [x] T7 iOS: not-connected panel (`ConnectionModel.connected`, `statusLine`); batch-id browser test; 2 tests (+1 updated).
- [x] T8 Android: not-connected panel; batch-id browser test; 2 tests.
- [x] T9 screenshots (desktop live app, iOS renderer); suites; CI scripts.

## Results

| Suite | Command | Result |
|---|---|---|
| core | `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,485 passed, 0 failed |
| core lint | `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings`; `cargo fmt --all --check` | clean |
| i18n | `i18n_residency` | ja+en 137,675 B (budget 141,800) — **0 bytes added**, no corpus change (reuses `home.connEmptyTitle`) |
| web | `npx vitest run`; `pnpm check` | 176 files, 2,594 passed (5 skipped); 0 errors / 0 warnings |
| extension | `pnpm build:extension` | built (release + store) |
| desktop | `cargo fmt --check && cargo clippy --all-targets && cargo test` | 919 passed, 49 ignored; clippy: no new warning (59 pre-existing, same files) |
| Android | `:app:testDebugUnitTest -PvelaSkipRustBuild` | 970 tests, 0 failures |
| iOS | `xcodebuild test -only-testing:` `DappBrowserTests`, `BrowserMemoryTests`, `BrowserChromeTests` (own clone of iPhone 16 Pro), then `VelaWalletTests` | 53 tests in 3 suites passed; full: 1,166 tests in 147 suites passed |
| CI scripts | reachability / event payloads / dead controls | reachable; 0 mismatches (539 sites); 0 dead controls |
| artefacts | `build-web --check`, `gen-onboarding-types --check` | current |

## Screenshots (session scratchpad)

- Desktop, the real app on PancakeSwap (`desk097e/shots/`): `e02-panel-connected.png` (connected panel, unchanged), `e03-disconnect-dialog-over-page.png` (the confirm over the column — the page steps aside), `e04-panel-after-disconnect.png` ("No active connection", network only; the page shows Connect), `e05-site-menu-not-connected.png` (⋯ without Disconnect), `e06-panel-not-connected-zh.png` (Aave, 暂无活跃连接).
- iOS (renderer of the live model, `shots097e/ios/`): `{en,zh}-connection-not-connected.png`, `{en,zh}-connection-connected.png`.
- Android: no screenshot harness (covered by `ExploreLiveTest`).

## Decisions (for the lead)

1. A batch is answered at **acceptance**, not at landing, on every shell (the web already did). The page follows it by `wallet_getCallsStatus`; the sheet follows the tracker. A lost relay reply (`maybe_sent`) still waits for the fate (so a never-sent batch is answered "not sent", not an id that reads 100 forever).
2. A reverted batch found by a wait (only on the maybe-sent path now) keeps 083's error answer; an accepted batch that later reverts reads **500** from `wallet_getCallsStatus`.
3. The disconnected panel reuses `home.connEmptyTitle` ("No active connection" / 暂无活跃连接) — no new key; no explainer sentence (the panel offers nothing to explain).

## Open questions / not done

1. The desktop's disconnect confirm reuses Settings' copy (title = host, body "Connections · clearing disconnects dApps"); `connect.browser.disconnectTitle/Body` exist and read better in the panel. Left as is (copy change beyond the defect).
2. `app-web/.../services/dapp-submit.ts` `handleReadOnlyRPC` (with its own `wallet_getCallsStatus`, no 400 rule) is referenced by nothing — dead duplicate of the core rule; not removed here.
3. S4 to be confirmed once 097 D lands.
4. Files of 097 D's set touched: none (`clear_signing.rs`, `token_trust.rs`, `activity_feed.rs`, `dapp_activity.rs` unchanged).
