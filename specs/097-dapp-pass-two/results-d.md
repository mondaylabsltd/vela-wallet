# 097 part D — one token registry in the core, chain-scoped (plan, tasks, results)

**Branch:** `097-token-registry`, on top of `097-dapp-activity-amounts` (parts A and C, then B merged). **Spec:** [spec.md](spec.md); evidence in [findings.md](findings.md).

## The problem

The core named tokens from two private, address-only copies of one "well-known token" table:

- `clear_signing.rs` `KNOWN_TOKENS` (19 entries);
- `token_trust.rs` `KNOWN_TOKENS` (20 entries; its doc already called unifying them follow-up work).

Both were Ethereum-centric and had no chain id. Found in the real-money pass on BNB Chain:

- **Sheet (097 A):** the sheet shows a token's short address beside any symbol the registry did not give. BNB Chain's USDC (`0x8ac76a…cd580d`, 18 decimals there) is the wallet's own registry stablecoin on that chain, yet it read "USDC (0x8ac76a...cd580d)".
- **Activity (097 B):** only a coin the wallet trusts may lead a row. The web's USDC→BNB swap therefore read only "+0.0015 BNB".
- **Wrong key:** an address alone cannot name a token.
  - `0x4200…0006` is WETH on the OP-stack chains but Plume's wrapped coin.
  - `0x779d…3736` is "USD₮0" on X Layer but "USDT0" on Mantle.
  - USDC has 6 decimals on Ethereum and 18 on BNB Chain.
  - An Ethereum address on another chain may be anybody's contract.

## Plan

| Layer | Change |
|---|---|
| `app/token_registry.rs` (new) | `registry_token(chain_id, address) -> Option<RegistryToken { symbol, decimals }>`. Case-insensitive; binary search over `token_registry/table.rs`. |
| `app/token_registry/table.rs` (generated) | `(chain id, address, symbol, decimals)`: 108 tokens on 22 networks, sorted strictly by (chain, address). It is the union of two sources (detail under "Outside facts"):<ul><li>each built-in network's chain-data `stables[]` (the service's symbol) and `wrappedNativeToken` (its own `symbol()`);</li><li>the old well-known entries, pinned to their chains: the Ethereum list → 1, Polygon USDC/USDC.e → 137, Arbitrum USDC/USDT → 42161.</li></ul>Every row's decimals are the contract's own `decimals()`. |
| `scripts/gen-token-registry.mjs` (new) | Reads the chain ids and RPCs from `network_admin.rs BUILTIN_CHAINS`, fetches `ethereum-data.getvela.app/chains/eip155-<id>.json`, and reads `decimals()` / `symbol()` over the chain's RPCs. Leaves out a chain-data row whose decimals cannot be read. Stops when a curated entry's decimals disagree with the chain. Writes the table (`#[rustfmt::skip]`, one token per line). `--check` regenerates in memory and diffs; it needs the network. Nothing at runtime or in tests reads the network. |
| `clear_signing.rs` | Table and `known_token_symbol/decimals` removed. `token_symbol`, `token_decimals` and the unknown-token probe filter (`unknown_token_addrs`) look up the request's chain. New `Model.chain_id` (set in `start_tx`/`start_typed`) lets `record_reading_of` name tokens on the request's chain. It used to match the caches by address suffix, across chains. |
| `token_trust.rs` | `pub KNOWN_TOKENS` and `pub fn known_token` removed. Nothing outside the core used them (grep of all shells, uniffi, wasm). The registry of the delta's chain:<ul><li>joins the trusted set in `judge_session` ("a registry token is trusted", now per chain);</li><li>supplies metadata in `judge_session` and `scan_meta_of`;</li><li>skips the metadata read in `sim_requested`;</li><li>blocks re-admission in auto-add.</li></ul>`judge_delta` now trusts only the caller's verdict. Its one caller folds the registry in. |
| `activity_feed.rs` | `proven_meta` `built_in` = `registry_token(t.chain_id, token)`. |
| `dapp_activity.rs` | `summarize` passes its `chain_id` to `call_summary` / `typed_summary` / `with_grant`. An allowance's token takes the registry's name on that chain first, then the sheet's resolved metadata. The sheet names it the same way. |
| Shells | No UI change. Each shell's signing wiring test, iOS `SimulationSheetTests` and the web Activity e2e were updated: they encoded the old form for a registry token, or used one as "a token nobody vouches for". All are listed under "Updated expectations". |

## Tasks

- [x] T1 generator + generated table; cross-checked against the lead's sweep (`scratchpad/sweep/registry.json`): 96 chain-data rows identical (Plume `0x4200…06` left out), plus 12 curated-only Ethereum tokens.
- [x] T2 `token_registry.rs` + unit tests.
- [x] T3 rewire `clear_signing`, `token_trust`, `activity_feed`, `dapp_activity`; remove both tables.
- [x] T4 core tests (new + updated); derived fixture `oneinch-native-order-unlisted.json`.
- [x] T5 regenerate wasm, pkg-web, TS mirrors (doc-only); Swift bindings (unchanged).
- [x] T6 shell wiring tests: web, desktop, iOS, Android.
- [x] T7 suites, CI scripts, results.

## Outside facts

- **Chain-data source:** `https://ethereum-data.getvela.app/chains/eip155-<id>.json`, the same document every shell's asset list reads (web `services/chain-tokens.ts` and its twins). Fetched 2026-10-03 for the 24 networks of `BUILTIN_CHAINS`.
  - Arc (5042) and 4663 list no stables or wrapped coin.
  - Plume's `wrappedNativeToken` (`0x4200…0006`) answers no `decimals()` on any of its RPCs, so it is left out.
- **Symbols:** the service's for stables, as the asset list shows them. Where the contract's own `symbol()` differs, the service's wins:
  - Polygon `0xc213…8e8f`: "USDT" (the contract says "USDT0");
  - Arbitrum `0xfd08…cbb9`: "USDT" (the contract says "USD₮0").
- **Curated decimals:** all 20 were confirmed on-chain by the generator.

## Behaviour before → after

**Sheet** (the pass's requests, `tests/fixtures/dapp097/`, real core):

| Request | Before | After |
|---|---|---|
| Curve approve (BSC) | `Token: USDC (0x8ac76a...cd580d)` | `Token: USDC` |
| PancakeSwap USDC batch, leg 1 | `Interacting with: USDC (0x8ac76a...cd580d)` | `Interacting with: USDC` |
| Uniswap PermitSingle, Aave approve and withdraw-all, Permit2 batch (WBNB) | `Token: WBNB (0xbb4cdb...bc095c)` | `Token: WBNB` |
| 1inch native order, node down | `Receive amount — 0x8ac7...`, "Incomplete" | `Receive amount 2.418146082462759045 USDC` (the registry's 18 decimals); complete |
| Ethereum's USDC address on BNB Chain (`0xa0b8…eb48`) | "USDC", 6 decimals, as on Ethereum | The contract's own answer with its address: `USDC (0xa0b869...06eb48)`. With the chain silent, `— 0xa0b8...` and "Incomplete". |

**Activity** (the pass's receipts):

- **Web-style USDC→BNB swap** (reading only, no simulation):
  - *before:* the row led with nothing, only `+0.001496… BNB`;
  - *after:* `−1.16 USDC`, received `+0.001496447240777689 BNB`.
  - The same holds with no reading at all, on PancakeSwap and Uniswap alike.
- **Curve with a reading:** the USDT inflow had no figure; it now reads `+0.100012293602944751 USDT`.
- **Allowance on BNB Chain's USDC from a batch leg:** the symbol and decimals were `None`; they are now "USDC" / 18.

**Trust:**
- **Ethereum's list elsewhere:** no longer trusted, nor answered without a read, on other chains. On BNB Chain, an inflow at Ethereum's USDC address answering "USDC" is unverified.
- **BNB Chain's registry coins:** USDC, USDT and WBNB are trusted, with no metadata read, even before the shell's registry snapshot arrives.

## Tests

### New (core)

- **`token_registry` unit tests (4):**
  - the table is sorted, unique (strictly increasing (chain, address)) and well formed;
  - USDC has its chain's decimals (BSC 18, Base 6, Ethereum 6);
  - the same address resolves per chain (`0x4200…06` is WETH on seven OP-stack chains and absent on Plume; USD₮0 vs USDT0; Ethereum and BSC USDC each absent on the other);
  - the well-known tokens live on their own chains.
- **`app_clear_signing_dapp097`:**
  - `a_registry_token_reads_with_the_chain_silent`;
  - `a_registry_name_holds_on_its_own_chain_only` (the Token row with the address, unknown when silent; registry-alone on Ethereum).
- **`app_dapp_activity_097`:**
  - `the_registry_names_the_coins_of_a_record_that_named_none` (both USDC→BNB swaps lead with −1.16 USDC);
  - `a_registry_coin_is_named_on_its_own_chain_only` (the same receipt filed under Ethereum: no name, no lead);
  - `an_allowance_names_its_token_on_its_own_chain`.
- **`app_token_trust`:**
  - `the_registry_vouches_only_on_its_own_chain` (machine: Ethereum USDC on BSC is read and untrusted; BSC USDC is trusted without a read);
  - `the_registry_is_per_chain` (replaces `known_token_table_matches_the_ts_canon`).

**Fails on the old code.** The new and updated tests were run against the pre-097 D core: the five `src/app` files restored from `099d98e94` and the new module left undeclared.

| Test file | Failing tests on the old core |
|---|---|
| `app_clear_signing_dapp096` | the 4 WBNB rows |
| `app_clear_signing_dapp097` | 5: `a_registry_name_holds_on_its_own_chain_only`, `a_registry_token_reads_with_the_chain_silent`, `curve_approve_…`, `a_batch_call_on_a_token_names_the_token`, `an_amount_whose_token_never_answered_…` (the old table called Ethereum's USDC address USDC/6 on BNB Chain) |
| `app_dapp_activity_097` | 3: `the_registry_names_the_coins_of_a_record_that_named_none`, `the_sheets_reading_…`, `an_allowance_names_its_token_on_its_own_chain` |
| `app_token_trust` | 2: `the_registry_vouches_only_on_its_own_chain` and `judge_delta_edges_fail_toward_unverified`. This file imports the new module, so it was run as a temporary copy without the import and without `the_registry_is_per_chain`. |

`a_registry_coin_is_named_on_its_own_chain_only` passes on the old core as well: the old table did not know BNB Chain's USDC either. It is a guard against an address-only fix, such as adding `0x8ac7…` to a table with no chain.

### Updated expectations (097 A/B/C) — only where the token is a registry token on that chain

| Test | Was | Now | Why |
|---|---|---|---|
| `app_clear_signing_dapp096`: `uniswap_permit_names_its_token_and_its_spender`, `aave_withdraw_all_reads_all_and_names_the_token`, `aave_unlimited_approve_names_token_and_pool`, `a_permit2_batch_names_every_token` | `Token: WBNB (0xbb4cdb...bc095c)` | `Token: WBNB` | WBNB is BNB Chain's wrapped native in the registry |
| `app_clear_signing_dapp097`: `curve_approve_names_the_router_and_the_registrys_usdc` (renamed) | `Token: USDC (0x8ac76a...cd580d)`, row address set | `Token: USDC`, no row address | BSC USDC is the registry's |
| `app_clear_signing_dapp097`: `a_batch_call_on_a_token_names_the_token` (renamed) | `Interacting with: USDC (0x8ac76a...cd580d)` | `Interacting with: USDC` | same |
| `app_clear_signing_dapp097`: `an_amount_whose_token_never_answered_is_unknown_and_incomplete` | read the pass's order with BSC USDC | reads `oneinch-native-order-unlisted.json` (takerAsset = Ethereum's USDC address), `— 0xa0b8...` | the rule is unchanged; its token had become a registry token |
| `app_dapp_activity_097`: `the_sheets_reading_names_the_coins_without_a_simulation` | USDC never led (`value == None`); Curve's USDT inflow had no figure | USDC leads with 1.16; USDT `+0.100012…` | both are BSC registry stables. "A name is not a trust" is kept on the aToken (aBnbWBNB, not in the registry): named by the reading, no figure. |
| `app_token_trust`: `judge_delta_edges_fail_toward_unverified` (first case) | `judge_delta` trusted Ethereum USDC by address alone | with `trusted=false` it is unverified | trust is now the caller's per-chain verdict (covered by the new machine test) |
| web `live.test.ts` (N1), desktop `signing::live` `the_pass_requests_say_nothing_unknown_as_certain` | chain-down native order with BSC USDC | the unlisted fixture, `— 0xa0b8...` | as above |
| iOS `DappSigningTests` `anUnscaledAmountIsIncompleteAndATokenTargetIsNamed`, Android `SigningLiveTest` (097 N1, N8) | chain-down borrow of BSC USDC; target `USDC (0x8ac76a...cd580d)` | borrow asset = Ethereum's USDC address; target `USDC` | as above |
| web `live.test.ts` (N8), desktop target assertion | `USDC (0x8ac76a...cd580d)` | `USDC` | BSC USDC is the registry's |
| iOS `SimulationSheetTests` (5 tests share the constant) | Gnosis USDC (`0xddaf…7a83`, chain 100) stood in for "a token nobody vouches for" | `0x5a5a…5a5a` | Gnosis USDC is the registry's stablecoin on Gnosis, so the core now trusts it with no metadata read. The asymmetry tests need a stranger. |
| web e2e `dapp-activity-097` | the swap detail had one `−1.16 USDC` | it has two (hero + line); a row assertion `−1.16` was added | the row now leads with the registry's USDC |

## Results

| Suite | Command | Result |
|---|---|---|
| core | `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,490 passed, 0 failed, 2 ignored |
| core lint | `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings`; `cargo fmt --all --check` | clean; clean |
| i18n | `i18n_residency` | ja+en resident 137,675 B (budget 141,800): **0 bytes added**, no corpus change |
| web | `pnpm build:extension`; `npx vitest run`; `pnpm check` | 175 files, 2,591 passed, 5 skipped; 0 errors, 0 warnings |
| web e2e | `playwright test e2e/dapp-activity-097.e2e.ts --project=chromium` | 3 passed. One assertion updated: the swap detail says −1.16 USDC twice (hero + line); a row assertion added. |
| desktop | `cargo fmt --check`; `clippy --all-targets`; `cargo test` | clean; no new warnings (the 3 in `signing/live.rs` predate this); 914 passed, 49 ignored |
| Android | `:app:testDebugUnitTest -PvelaSkipRustBuild` | 968 tests, 0 failures |
| iOS | `build-ios-xcframework.sh`; `xcodebuild test … -only-testing:VelaWalletTests/SigningLiveTests`, then `VelaWalletTests` (own clone of iPhone 16 Pro) | `SigningLiveTests` 25 passed; first full run 1,164 tests with 7 issues, all in `SimulationSheetTests` (its stand-in token became a registry token; see above); after that fix `SimulationSheetTests` 12 passed and the full `VelaWalletTests` 1,164 tests in 147 suites passed |
| CI scripts | reachability / event payloads / dead controls | reachable; 0 mismatches (539 sites); 0 dead controls |
| artefacts | `build-web --check`, `gen-onboarding-types --check` | current. Wasm 4,437,774 → 4,442,118 B (+4,344, the table). |

## Shell copies of the table (reported, not refactored)

- **Web, `app-web/vela-wallet/src/lib/services/tokens.ts` `KNOWN_TOKENS`:** the same 20 addresses, address-only. `token-metadata.ts resolveTokenMetadata` consults it before any RPC, and that function answers the core's metadata questions in three places:
  - `token-trust-executor.ts` (`MulticallErc20Meta`);
  - `guard-executor.ts` (the approval guard's token);
  - `send-executor.ts`.

  So on web, an address on that list on another chain is answered with Ethereum's symbol and decimals, without reading the chain:
  - **Trust is unaffected:** inflow trust is the registry or held tokens.
  - **Outflows and the guard are affected:** an outflow, or the guard's metadata, can read "USDC"/6 for another chain's contract at that address.
  - **Suggested follow-up:** delete the table and let `resolveTokenMetadata` read what the core asks. The core already skips its registry tokens; iOS `TokenMetadata.swift` has no table, by design.
  - **Also:** `token-autoadd.ts` reads `knownTokenSymbol`, but its only caller (`reconcilePendingTransactions`) has no live caller.
- **Trusted signer, `app-web/trusted-signer/src/lib/registry.js` `TOKENS`:** 7 Ethereum addresses, address-only, read by `resolve.js` on the static signer page. Its own comment says the shipped app's data arrives from the wallet. It is separate from the core.
- **iOS, Android, desktop:** no copy. iOS `TokenMetadata.swift` documents why it has none. The Android and desktop matches are test fixtures only.

## Screenshots

Session scratchpad `097d/`. There is no UI change, so these show the core's new answers drawn by the real web sheet and Activity.

**Sheet** (temporary gallery state, not committed; it reuses 097 A's patch with `x097d-gallery.scratch.test.ts`, `gallery-patch.diff` and `shoot.mjs` beside the PNGs):
- `web-x097d-curve-approve-{en,zh}.png`: `Token USDC`.
- `web-x097d-batch-{en,zh}.png`: leg 1 `Interacting with USDC`.
- `web-x097d-native-order-down-{en,zh}.png`: node down, `+2.418146082462759045 USDC` and no "Incomplete".
- `web-x097d-native-order-unlisted-down-{en,zh}.png`: an unlisted token with the node down, `— 0xa0b8...` plus "Incomplete" and the unverified-amount caution.

**Activity** (the 097 B e2e, rerun): `web-activity/activity-phone-en.png` shows `Swap on PancakeSwap −1.16 USDC +0.001499 BNB`. 097 B's `shots097b/web/activity-phone-en.png` showed `+0.001499 BNB` only. The folder also holds `activity-phone-zh.png`, `activity-desktop-en.png` and the detail sheets.

**Other shells:** desktop, iOS and Android draw the same core strings; their wiring tests above cover them. 097 A's own "before" web shot of the Curve approve (`097a/web-x097-curve-approve-en.png`) caught the splash screen, so it is no comparison.

## Open questions

1. **A dollar figure by symbol alone.** `clear_signing` values a verified amount at $1 when its symbol is in `STABLE_SYMBOLS`, whoever named it. A non-registry contract that answers "USDC" therefore gets "≈ $1.16" beside `USDC (0xa0b869...06eb48)`. Pricing at $1 only when the symbol is the registry's on that chain would close this. Not changed here: it is out of this part's scope.
2. **Refresh cadence.** The table changes only when someone runs the generator. `--check` needs the network, so it is not wired into CI.
3. **Symbol precedence.** The table uses the chain-data service's symbol, so the sheet matches the asset list ("USDT" on Polygon and Arbitrum). The contract itself says "USDT0" / "USD₮0". Say if the contract's word should win.
4. **Web table.** Should the web `services/tokens.ts` table be removed (see above)? That is a shell change, so it was not made here.
5. **Monospace token rows (web).** Web draws any row that carries a `token_address` in monospace (`live.ts fieldRow`, unchanged since 026). A registry token row now reads "USDC" alone, in monospace. 096 B moved contract names to the text face; a registry symbol may want the same. Not changed: no shell UI change in this part.
