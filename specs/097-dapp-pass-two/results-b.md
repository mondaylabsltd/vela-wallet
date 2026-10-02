# 097 part B — results: Activity says what happened, with the figures the chain proved

**Branch:** `097-dapp-activity-amounts` (from `origin/main` `cca03b56`) · **Findings:** N5, N7, N8 (Activity part), N4 (Activity part) · **User Story 2** of [spec.md](spec.md) (on part A's branch).

## Plan

| Layer | Change |
|---|---|
| `tx_tracker` (core) | `proven_moves(logs, op)`: a landed op's OWN receipt logs (scoped by the EntryPoint's `BeforeExecution` / `UserOperationEvent` boundaries; account = the op event's `sender`) → per-token net of ERC-20 `Transfer`s + the coin the Safe logs arriving (`SafeReceived`, emitted by the account itself). The closing patch carries `settlement { moved, failure }`; `failure` = `Reverted` / `Refused` / `NotSent` from the tracker's own verdict. |
| `activity_feed` (core) | `FeedTxRecord.settlement`. A confirmed dApp record with `moved` is drawn from it alone: exact lines, the call's own `value` netted in, the in-band token fee taken back out, the one trusted outflow is the figure and the one trusted inflow is `received` (even with nothing out). Token names: the sheet's judgments, a folded "Received" of the same amount, the reading's coins, the approval's token, the built-in table — a name is not a trust (untrusted inflow: no figure; untrusted outflow: never the headline). `FeedItem.priced`, `FeedDapp.failure`. |
| Titles / places (core) | Recorded intent capitalised; typed data reads by its recorded intent, a signed `Order` reads Swap; a pre-093 signature still reads by kind. Approvals of Permit2 take the owner of that Permit2 (`permit2_owner`); a contract the build does not know takes the reading's owner/name. |
| `dapp_activity` / `sign_request` / `clear_signing` (core) | `DappReading { address, name, owner, tokens }` = `ClearSigningView.record_reading` (ClearSign + Batch surfaces) → `SignApproveOpts.reading` → `DappSummary.{contract_name, owner, tokens}`; `with_approve_facts` also records the in-band token fee (`fee_token`, `fee_amount`). clear_signing.rs: one view field + `record_reading_of`, and `PERMIT2_ADDRESS` / `PANCAKE_PERMIT2` made `pub(crate)` — nothing else. |
| Shells (web+ext, desktop, iOS, Android) | Store the patch's `settlement` beside the status and hand it back; copy `record_reading` into the approve; draw a lone "+x" when nothing left (row and detail hero); fiat only when `priced`; a failed dApp detail says why under its chip with existing keys (`componentsTx.receipt.failedHint`, `componentsUi.signing.refused`, `send.txErrorGeneric` — the sheet's own ending words). |

## Tasks

- [x] T1 core: `proven_moves`, `SAFE_RECEIVED_TOPIC`, `TrackSettlement/TrackMove/TrackFailure`, patch on every confirm/fail path (relay receipt with logs, bundle tx receipt, chain event, late patch).
- [x] T2 core: feed draws proven lines; `priced`; `failure`; titles, places, contract names.
- [x] T3 core: `DappReading` through view → approve → summary; in-band fee leg.
- [x] T4 core tests on the pass's real requests + BSC receipts (`tests/fixtures/dapp097/`, `tests/app_dapp_activity_097.rs`); tracker/sign suites updated for reasons.
- [x] T5 regenerate wasm, pkg-web, TS mirrors; Swift bindings regenerated (unchanged).
- [x] T6 web + extension (same modules): tracker patch, feed mapping/validation, approve copy, row, detail; unit tests + e2e.
- [x] T7 desktop, T8 iOS, T9 Android: same wiring + tests (Android drift checks for the new wire types).
- [x] T10 screenshots (web phone/desktop en+zh; iOS simulator en+zh).

## Results

| Suite | Command | Result |
|---|---|---|
| core | `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,461 passed, 0 failed (new: `app_dapp_activity_097` 16) |
| core lint | `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings`; `cargo fmt --all --check` | clean |
| i18n | `i18n_residency` | ja+en resident 137,675 B (budget 141,800) — **0 bytes added**, no corpus change |
| web | `npx vitest run`; `pnpm check` | 175 files, 2,579 passed (5 skipped); 0 errors |
| web e2e | `playwright test e2e/dapp-activity-097.e2e.ts --project=chromium` | 3 passed |
| desktop | `cargo fmt --check && cargo clippy --all-targets && cargo test` | 912 passed, 49 ignored; no new clippy warning |
| Android | `:app:testDebugUnitTest -PvelaSkipRustBuild` | 966 tests, 0 failures |
| iOS | `xcodebuild test … -only-testing:VelaWalletTests` (own clone of iPhone 16 Pro); `DappActivity097ScreenshotTests` (UI) | 1,161 tests in 147 suites passed; UI screenshot test 1 passed |
| CI scripts | reachability / event payloads / dead controls | reachable; 0 mismatches (539 sites); 0 dead controls |
| artefacts | `build-web --check`, `gen-onboarding-types --check` | current |

What the pass's operations now read (core, real receipts): borrow **+0.3 USDC**; PancakeSwap/Uniswap USDC→BNB **−1.16 USDC +0.0015 BNB** (with the sheet's judgments; on a sheet without a simulation the USDC is named in the detail only — see open question 1); Aave withdraw **+0.003 BNB** (aToken line unnamed unless the sheet named it); Curve **−0.1 USDC +0.100012 USDT**; supply **−0.003 BNB +0.002999 aBnbWBNB**; repay **−0.300000036827585322 USDC**; 1inch create order **Create order on 1inch −0.003 BNB**, contract NativeOrderFactory, no fiat; refused withdraw **Failed** + "The network refused it — nothing was sent."

## Decisions taken (for the lead)

- The figures are the receipt's; the NAMES are what the wallet resolved at sign time (judgments, reading) or the scan admitted. Trust is unchanged from 083 F1: only a coin the wallet trusts leads a row, and an inflow of an untrusted token never carries a figure — a site's own contract can log `Transfer(you, …)` and answer "USDC".
- The coin the call sent is the record's `value` (no log states it); the Safe's `SafeReceived` is the only native inflow counted; EIP-7708 native lines are ignored so the value is never counted twice.
- An in-band fee paid in a token is in the same receipt; the summary keeps `fee_token/fee_amount` and the feed adds it back, so a USDC-fee swap reads −1.16, not −1.18.
- A confirmation found by the chain event alone carries `moved: None` (nothing claimed); a read receipt where nothing moved carries `moved: []`.
- Failure words reuse the sheet's endings (Reverted → failedHint, Refused → refused, NotSent → txErrorGeneric). A record failed by the sign machine's own pre-hand-off path (`SignRecordClose::Failed`, rare) keeps no reason.
- "Approve on Uniswap": an approval granted TO canonical Permit2 names Uniswap (its owner), PancakeSwap's Permit2 names PancakeSwap.

## Not done / open questions

1. **Web dApp sheet has no simulation** (already 093's open item): no judgments to record, so on web/extension a USDC outflow the receipt proves is named (from the reading) in the detail but never leads the row — "Swap on PancakeSwap +0.001499 BNB". Closing it needs the web sheet's simulation (or a trusted-set read at approve).
2. **Curve router name/place:** part A adds `0xa72c85…51cc` to the built-in table (N8 sheet part). Once it is there, Curve rows say "Curve" here through `protocol_of`; on this branch alone the test proves it through a recorded reading owner.
3. **Typed-data titles:** a signed `Order` (1inch LOP, CoW) reads Swap; any other typed data reads by its recorded intent (capitalised). Part A may change the 1inch reading's intent text.
4. Desktop screenshots not taken (no headless capture of the live feed); Android has no screenshot harness — both covered by their through-core unit tests.
5. Files of part A touched: `clear_signing.rs` (view field + `record_reading_of`, two constants `pub(crate)`). Part C: none.

## Screenshots (session scratchpad `shots097b/`)

- web: `web/activity-phone-{en,zh}.png`, `web/activity-desktop-en.png`, `web/{swap,borrow,failed,order}-detail-phone-en.png`, `web/failed-detail-phone-zh.png`, `web/{borrow,failed}-detail-desktop-en.png`
- iOS (simulator, `DappActivity097ScreenshotTests`): `ios/{en,zh}-097-activity.png`, `ios/{en,zh}-097-{swap,borrow,failed}-detail.png`
