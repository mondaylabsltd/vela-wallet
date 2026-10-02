# 097 part A — the sheet never states the false or the unknown as certain (results)

Branch `097-signing-readable` (from `origin/main` cca03b56). Findings N1, N2,
N3, N6 and the sheet part of N8. Every rule is decided once in
`rust/crates/vela-core/src/app/clear_signing.rs`; the shells draw it.

## What changed

### Core (rules, once)

| Finding | Rule | Where |
|---|---|---|
| N1 address | An `addressName` field, or a `tokenPath`, whose value is a `uint256` is the address it holds when it fits in 160 bits (1inch's `Address` type). A wider number is shown whole, never cut to an address. | `uint_as_address`, `token_ref`, `format_address`, `format_token_amount`, `collect_token_addrs` |
| N1 amount | An amount with no token the reading can name — not an address, not the native sentinel — is unverified: the em dash, never "N tokens" at a guessed 18 decimals (`guess_token_decimals(None)` returned `(18, true)`). Any unverified amount marks the reading `partial` ("Incomplete"). | `format_token_amount`, `token_decimals`, `incomplete` |
| N1 role | "Beneficiary" reads as a recipient (a party row: short + full address). | `infer_field_roles` |
| N2 | A borrow (intent or label containing "borrow", not "repay") is `ReceiveAmount`. Repay stays `SendAmount`. | `infer_field_roles`, `borrows` |
| N3 | New wire field `ClearSignField.bound: Option<ClearAmountBound>` (`min`/`max`), graded with the roles from the label's whole words (`min`, `minimum`, `max`, `maximum`). | `bound_of` |
| N6 | Built-in typed reading of 1inch LOP v4 `Order` verified by the Aggregation Router V6 (`0x1111…2a65`), ahead of the service's descriptor. Pay `makingAmount`; receive `takingAmount` — "You receive (min)" when `NO_PARTIAL_FILLS` (bit 255), else pay is "You pay (max)"; `UNWRAP_WETH` (bit 247) makes the received coin the chain's own; a zero `receiver` is the maker; the expiry is `makerTraits >> 80 & (2^40−1)`. Intent "Swap", so the record keeps "Swap". | `read_oneinch_order`, `oneinch_order_descriptor`, `local_typed_descriptor`, `eip712_context` |
| N8 | Curve's router on BNB Chain is a known contract ("Curve Router", owner Curve), chain 56 only. A token named by the registry (`KNOWN_TOKENS`) stands alone; one named only by its own `symbol()` carries its short address — on Token rows (approve/permit) and as a batch call's `to_name`. | `KNOWN_CONTRACTS`, `token_symbol`, `token_name`, `batch_call_view` |

The native-order descriptor reaches the core from Vela's descriptor service
(`/erc7730/calldata/eip155-56/0xe12e…ff01.json`, `Fetched`); the rules above
are general, so it now reads right without a 1inch-specific path. No new
corpus keys.

### Outside facts, checked at the source

- **1inch `makerTraits`:** `1inch/limit-order-protocol` `contracts/libraries/MakerTraitsLib.sol`. The layout used (bits 255 and 247, the expiration at offset 80 with a 40-bit mask) is identical at tag `4.0.0` (c8be9c67) and master `50bd1300`. The pass's order: `makerTraits = 0x8a80…6abfdee2…` → expiry 1790959330 = **2026-10-02 16:42:10 UTC**; NO_PARTIAL_FILLS, UNWRAP_WETH, HAS_EXTENSION and POST_INTERACTION set.
- **Curve router, BNB Chain:** `0xA72C85C258A81761433B4e8da60505Fe3Dd551CC`, listed in two places:
  - `curvefi/curve-router-ng` README, "BSC" (master 2d49362b);
  - `curvefi/curve-js` `src/constants/network_constants.ts`, `ALIASES_BSC.router` (master fdf70ea6).

  The same address is `crypto_calc` on other chains, hence the chain-56 scope.

### Shells (draw only)

| Shell | Change |
|---|---|
| web + extension (`src/lib/signing/live.ts`) | **N3:** the hero line's caption is the field's label when the core sets `bound`. **N1:** an unverified hero amount is drawn in caution with no sign. `partial` now says `partialWarning`; it said the best-effort sentence. An unverified amount says `unverifiedWarning`, as the other three shells do. New `SigningMessages.warnPartial`. |
| desktop (`signing/live.rs`) | Nothing new to draw. Rows carry their labels, so "(min)" is visible. `partial`, `amount_unknown`, `to_name` and the party rows were already drawn. Two wiring tests run the real core. |
| iOS (`SigningLive.swift`) | Nothing new to draw, for the same reason. Two wiring tests run the real core. |
| Android (`SigningLive.kt`) | Nothing new to draw, for the same reason. Two wiring tests run the real core. |

**N2 and N3 surfaces:** both are visible only on web, the one shell whose hero draws an amount without its label and with a direction sign. Desktop, iOS and Android draw every field as a labelled row with no sign. The core fact is shared, and no shell lacks the sheet.

## Before → after (the pass's own requests, `tests/fixtures/dapp097/`)

**Before** is the pass sheet (main cca03b56). **After** is the real core with the pass's shell scripted: the service serves the native-order descriptor, and the chain names WBNB, USDC and USDT. Times are UTC+8 unless marked.

- **1inch native order:**
  - *Before:* `create order · −0.003 BNB · +2.418… tokens · Beneficiary 78098611...991444`.
  - *After:* `Amount to Send 0.003 BNB · Receive amount 2.418146082462759045 USDC · Beneficiary 0x88cca0...266894` (full address beneath on web).
  - *Chain down:* `Receive amount — 0x8ac7...` in caution, plus "Incomplete" and "Token amount couldn't be verified".
- **1inch `Order`:**
  - *Before:* `1inch Order · −1.16 USDC · +0.00143… WBNB · From 0x88cca0ee…89266894 · To 0x00000000…00000000` (fetched).
  - *After:* `Swap · You pay 1.16 USDC · You receive (min) 0.001430509396956033 BNB · Recipient 0x88cca0...266894 · Valid until 2026-10-03, 00:42` (16:42 UTC). Built in, so there is no "descriptor service" caution; record "Swap".
- **Aave borrow:** *before* `Borrow −0.3 USDC`; *after* `Borrow +0.3 USDC` (web hero, success tone).
- **PancakeSwap and Uniswap BNB→USDC:** the hero `+2.30985453836239 USDC` now carries the caption "You receive (min)" (你最少收到).
- **Curve approve:** *before* `Unlimited · Token USDC · Spender 0xa72c85...d551cc`; *after* `Unlimited · Token USDC (0x8ac76a...cd580d) · Spender Curve Router`.
- **Curve swap:** best effort as before; technical details now say "Curve Router" (owner Curve).
- **PancakeSwap USDC batch:** leg 1 "Interacting with" changes from `0x8AC76a51…580d` to `USDC (0x8ac76a...cd580d)`.

## Tests

Every finding's core test fails on the old core: 10 of 12 behaviourally (run against `HEAD~` with the bound assertions stripped). The other two are a "registry token stays alone" guard and the bound test, which cannot compile there.

| Suite | Result |
|---|---|
| core `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2457 passed, 0 failed, 2 ignored. New `tests/app_clear_signing_dapp097.rs`: 12 + 1 ignored printer. 4 updated in `app_clear_signing_dapp096.rs` (Token rows now carry the address). |
| core clippy `-D warnings` / `cargo fmt --check` | clean / clean |
| web `npx vitest run` / `pnpm check` (after `pnpm build:extension`) | 2577 passed, 5 skipped / 0 errors, 0 warnings. 5 new in `live.test.ts`, on the real wasm core. |
| desktop fmt / clippy --all-targets / `cargo test` | clean / no new warnings (the 3 in `signing/live.rs` predate this) / 912 passed, 49 ignored. 2 new real-core tests in `signing::live`. |
| Android `testDebugUnitTest` | 965 passed, 0 failed. 2 new in `SigningLiveTest`. |
| iOS `SigningLiveTests` + `CoreWireDriftTests` / full `VelaWalletTests` | 33 passed / 1160 passed in 147 suites. 2 new in `SigningLiveTests`. |
| `check-native-reachability`, `check-event-payloads`, `check-dead-controls` | pass |
| i18n gen / lint / verify / dump:vectors | pass; 76,070 comparisons, 0 divergences |
| `build-web --check`, `gen-onboarding-types --check` | current |

## i18n

No keys added or changed. ja+en resident is 137,675 bytes (JSON route 141,793), unchanged; `SC005_BUDGET` stays at 141,800. Every word used was already in the corpus:
- `labelYouReceiveMin`, `labelYouPayMax` and `labelValidUntil` (ClearTerm);
- `partialWarning` and `unverifiedWarning`.

## Screenshots

`/private/tmp/claude-501/-Volumes-data-production-vela-wallet/d4a496f4-ab96-4481-916a-65326d48067e/scratchpad/097a/`:

- **Web:** `web-x097-{order,native-order,native-order-down,borrow,swap}-{zh,en}.png`, `web-x097-batch-zh.png`, `web-x097-curve-approve-zh.png`.
- **iOS simulator:** `ios-x097-{order,native-order,native-order-down,borrow,swap,batch}-{zh,en}.png`.

The real sheet builders drew them (`buildSigningModel`, `SigningLive.model`), over the real cores reading the fixture requests with a scripted chain. They went through a temporary gallery state that is not committed; its sources sit beside the PNGs:
- `x097-gallery.scratch.test.ts` and `gallery-patch.diff` (web);
- `X097Gallery.swift` and `ios-gallery-patch.diff` (iOS).

Desktop has no signing-sheet gallery and Android no JVM screenshot harness; their unit tests cover them.

## Not done / open

- **"The account itself" (N6 receiver):** the row shows the maker's own address, short and full, as PancakeSwap's recipient row does. The corpus's only "you" word is `selfName` ("{{name}} (you)"), which needs the account's name and the knowledge that the row *is* the account. `clear_signing` is never told the signing account; passing it in would touch 3 events × 4 shells. Say if the owner wants "Parallel Multi (you)" on every recipient that is the account.
- **1inch native order (N1):** still read through the service's descriptor (`Fetched`, with its caution). Its takingAmount is not labelled a minimum and its expiry is not shown, because the descriptor says neither. A built-in reading like the `Order` one would add both; say if wanted.
- **Partial-fill `Order`s:** they read "You pay (max) / You receive". The received figure scales with the fill, so it is not called a minimum.
- **Sheet intent text:** the descriptor's "create order" stays lowercase on the sheet. N7 (capitalisation) was scoped to Activity, part B.
- **Files touched that B/C also touch:** none of `activity_feed.rs`, `dapp_activity.rs`, the Activity rows, `approveOptsOf`, the extension request lifecycle or the receipt states. Two side effects reach Activity, both via `clear_signing.rs`:
  - `known_contract` now names Curve's router, so `place_of` reads "Curve" on BNB Chain;
  - `record_intent` is "Swap" for a 1inch `Order` signature.

  Expected merge points with B: `KNOWN_CONTRACTS` (B may add NativeOrderFactory) and the 096 test file.
