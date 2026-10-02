# Results: 093 — every dApp interaction in Activity

**Status**: implemented on `093-dapp-activity` (from `origin/main` @ `ec033f231`), not pushed. Core,
web + extension, desktop, iOS and Android all draw the core's rows; screenshots on web (phone + desktop
width), desktop (headless gpui), iOS (simulator). Android has no screenshot harness.

## Success criteria

| SC | Verdict | Evidence |
|---|---|---|
| SC-001 core tests | **Pass** — the 3 pinned "signatures excluded" tests rewritten (`feed_and_transactions_are_account_scoped`, `a_personal_sign_is_a_row_with_no_figure_status_or_hash`, `signatures_are_rows_and_connections_stay_out`); new: summary from a request > 4 KB and > 8 KB, legacy rows, protocol vs host, permit limited / unlimited, SIWE matched / mismatched, batch headline (machine + feed), swap receipt folding, transfer subtitles, recipient fact, lenient stored summary | `tests/app_activity_feed.rs`, `tests/app_dapp_activity.rs`, `tests/app_clear_signing.rs`, `src/app/dapp_activity.rs` |
| SC-002 suites | **Pass** — see below | — |
| SC-003 residency | **Pass** — `history.dappRowTitle` adds 89 B of ja + en JSON (runtime route 138,720 → 138,809); engine-resident ja + en 134,785; budget 140,800 | `i18n_residency` |
| SC-004 screenshots | **Pass** (iOS sim, web phone + desktop, desktop headless); Android: no harness | below |

## Suites

| Suite | Command | Result |
|---|---|---|
| Core | `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,341 passed, 0 failed |
| Core | `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings`; `cargo fmt --all --check` | clean |
| Web | `pnpm build:extension && npx vitest run && pnpm check` | 171 files, 2,414 passed, 5 skipped; check 0 errors |
| Web e2e | `e2e/dapp-activity.e2e.ts` (+ activity-delete, reopen-pending, tx-detail-account-switch, signing-scenarios, extension-signing on the first pass) | 5/5 (18/18) |
| Desktop | `cargo fmt --check && cargo clippy --all-targets && cargo test` | 885 passed, 0 failed, 49 ignored; clippy 54/56 pre-existing warnings, none in changed code |
| Android | `:app:testDebugUnitTest -PvelaSkipRustBuild` | 926 tests, 0 failures |
| iOS | `xcodebuild test … -only-testing:VelaWalletTests` (cloned iPhone 16 Pro, iOS 18.2) | 1,116 tests in 142 suites passed; 16 targeted suites 181 passed; `DappActivityScreenshotTests` 1 passed; `check-ios-core-fresh` ok |
| CI scripts | `check-native-reachability`, `check-event-payloads`, `check-dead-controls` | reachable; 0 mismatches (532 sites); 0 dead controls |
| Artefacts | `build-web --check`, `gen-core-types --check`, `gen-onboarding-types --check` | current |

## Screenshots (session scratchpad `093-shots/`)

- web: `activity-phone-{en,zh}.png`, `activity-desktop-en.png`, `permit-detail-{phone,desktop}-en.png`, `swap-detail-phone-en.png`, `contact-detail-phone-en.png`
- desktop (headless): `activity-{en,zh}.png`, `contact-{en,zh}.png`, `detail-{permit,permit-technical,swap-technical,siwe-technical}-{en,zh}.png`
- iOS: `{en,zh}-{home-activity,history,permit-detail,permit-technical,permit-technical-scrolled,swap-detail,contact-page}.png`

## Decisions taken while building (for the lead)

- The tx hash sits in Technical details (and behind the explorer link) so the facts stay ≤ 6 on every
  action (an approval with an expiry has six without it).
- A batch is titled by its one shared verb once Approve / Approve NFT / Approve all NFTs / Authorize
  spending calls are set aside (`[approve, swap]` → Swap); its granted allowance is a detail fact.
- Permit2, WETH and the Coinbase smart wallet never name the place (infrastructure, not where it
  happened).
- The detail names who got the money (`recipient`, `componentsTx.detail.to`) for a plain send or a token
  transfer, else the contract under `tokenDetail.labelContract` (the 083 F3 noun) — raised by the desktop
  shell after the first pass.
- `FeedTxRecord.summary` / `.balance_changes` are read leniently (raised by the iOS shell): an unreadable
  stored value drops for that record instead of failing the feed.
- Unlimited allowances are never masked under balance privacy (a risk, not a balance) on web, desktop
  and iOS.
- Lead round 3: Technical details show the core's `request_display` (typed data pretty-printed, a message
  as its text or hex, call data pretty-printed) on every shell; desktop's own pretty-printer is gone. A
  contact's page draws the feed's `contact_rows` (`ContactFilterChanged`) with Activity's row builder on
  every shell — iOS built rows from raw records (a dApp transfer to a contact read 已发送), web built its
  own titles, desktop/Android filtered the feed themselves.

## Not done / open

- Web dApp-sheet simulation (SHOULD): not done. The core's simulation verdict (`sim_outcome`) has no wasm
  export and the web sheet has no live "Balance changes" block; doing it in TypeScript would put a rule
  in the shell. Web rows get figures from the call's own value and from folded receipts only.
- Android never masks live Activity figures under balance privacy (pre-existing); the allowance follows.
- Follow-up (accepted by the lead): the history cap of 200 records (web / Android / iOS) is now shared by
  sends, receives and signatures.
- Accepted as is: a batch's unlimited approval shows only in the detail's spending-cap fact; iOS rows
  wrap a long title to two lines; `ts-rs` prints "failed to parse serde attribute deserialize_with" when
  built with `bindings` (gen-core-types).
- Web and Android draw a contact's rows only while `contact_rows` belong to the open contact (a guard for
  the dispatch gap between opening one contact and the core switching over).
- Merge with 090–092: `scripts/gen-i18n.mjs` path pin (1787 here, 1788 on main → 1789 after merge), the
  i18n catalogs, `assets/wasm`, `rust/pkg-web` and the TS mirrors conflict — regenerate after merging.
