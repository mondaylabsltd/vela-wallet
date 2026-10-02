# 096 part B — what you see is what you sign (results)

Branch `096-clear-signing-readable` (from `origin/main` 07a7287c0). Findings
F4, F5, F6, F7 and F9 of the 2026-10-02 real-dApp pass. Every rule is
decided in the core (`rust/crates/vela-core/src/app/clear_signing.rs`,
`dapp_activity.rs`, `activity_feed.rs`); the four shells only draw the new
facts.

## What changed

### Core (rules, once)

| Finding | Rule | Where |
|---|---|---|
| F4 | `ClearSigningView.native_value` — the coin a lone contract call sends (decoded **or** blind), exact (`value/10^18`, locale marks), unless a descriptor row already states `@.value`. Batch calls keep their own `ClearBatchCall.amount` (089). | `native_value_of`, `conclude_calldata` |
| F4 | Best-effort parameters: the ABI name in words (`referralCode` → "Referral code"); an unnamed integer is "Number", never "Value"; a parameter named as a time (`deadline`, `expiry`, `validTo`…) holding a plausible unix time is its date; `type(uint256).max` is "Max". | `build_best_effort_fields`, `pretty_type`, `format_generic_value` |
| F6 | A worded amount ("Unlimited", "All") is followed by a **Token** row (symbol, else short address + full address). Applies to every approve/permit descriptor: ERC-20 approve, ERC-2612 permit, Permit2 `PermitSingle`, new Permit2 `PermitBatch` (one cap row per token, ≤ 5), new Permit2 calldata `approve`. PancakeSwap's Permit2 typed messages read like Uniswap's. Spender rows name known contracts. | `format_token_amount`, `token_row`, `permit2_batch_descriptor`, `local_typed_descriptor` |
| F5 | Token amounts are exact at the token's decimals (the 4-place cut read 0.00005 WBNB as "0" and "You pay 2.3417" over 2.341714); `type(uint256).max` with no threshold reads "All" (`ValueAll`). Native-sentinel amounts say the chain's coin (Seaport's "1.5 tokens" → "1.5 ETH"). | `format_token_value`, `format_native_amount` |
| F5 | Known-contract table is **chain-scoped** (`known_contract(chain_id, addr)`, `place` flag replaces the name/owner string test). Address rows and batch targets (`ClearBatchCall.to_name`) are named by it. | `KNOWN_CONTRACTS` |
| F5 | Built-in readings (zero round trips): Universal Router swaps read from the router's **commands** (Uniswap 1.x/2.x, PancakeSwap Infinity: V2/V3 exact-in/out, Pancake stable swaps, WRAP/UNWRAP, SWEEP/TRANSFER/PAY_PORTION/PERMIT2_TRANSFER_FROM recipients; V4/INFI swaps read as a swap with unread amounts → "incomplete"; any other command → ordinary ladder); Permit2 `approve` (Uniswap + PancakeSwap deployments); Aave pool on BNB; Aave `WrappedTokenGatewayV3` (depositETH/withdrawETH/repayETH/borrowETH); Spark PSM3 (swapExactIn/Out); CoW `setPreSignature` → "Swap · Order 0x… · Valid until …" with `ClearSignResult.terms_off_chain` (`signed=false` reads "Revoke"). | `chain_scoped_descriptor`, `read_router_call`, `build_cow_presign_result` |
| F9 | Those readings are `BuiltIn`, so `record_intent` is the intent ("Swap", "Supply", "Withdraw"); the batch headline already sets approvals aside. `dapp_activity::summarize`, `protocol_of`, `contract_name_of` take the record's chain. | `dapp_activity.rs`, `activity_feed.rs::place_of`, `sign_request.rs` |

New wire fields (all `serde(default)`, additive): `ClearSigningView.native_value: ClearNativeValue{value_wei, amount}`,
`ClearBatchCall.to_name`, `ClearSignResult.terms_off_chain`; new `ClearTerm`s
`labelOrder`, `valueAll`.

### Shells (draw only)

| Shell | Change |
|---|---|
| web + extension (`app-web/vela-wallet/src/lib/signing/live.ts`) | F7: loading says `componentsUi.signing.loading` (was the cap prompt `signingApprove.choosePrompt`); the slide is also gated on `!clear.resolving && surface != loading`. F4: `native_value` as the hero amount (or "Amount −x BNB" row beside a decoded amount), and on the blind card. F5: `terms_off_chain` caution (single + batch); batch "Interacting with" = `to_name`. Terms list gains `labelOrder`, `valueAll`. |
| desktop (`signing/live.rs`, `signing/mod.rs`) | Same four: gate (`confirm_enabled`), coin row first in the rows / on the blind card, `warn_order_terms`, `to_name`. Loading already said "Loading…". |
| iOS (`SigningLive.swift`, `ClearWire.swift`) | Same four; ClearWire mirrors the three fields (optional). |
| Android (`SigningLive.kt`, `ClearWire.kt`) | Same four; ClearWire mirrors; drift test now registers ClearNativeValue, ClearBatchCall, ClearBatchView. |
| all four | A row value the core names ("PancakeSwap Permit2") reads in the text face; monospace only for `0x…` values. |

No shell lacks the surface (all four draw the dApp signing sheet; the
extension is the web build).

## Contracts added — each checked against the protocol's own source

| Name (owner) | Address | Chains | Source |
|---|---|---|---|
| Uniswap Universal Router 2.1.2 (Uniswap) | `0xDc264714F68d84CF29BC605589405E78bDBE7C9f` | 56, 137 | `Uniswap/universal-router` `deploy-addresses/bsc.json` (`UniversalRouterV2_1_2`); docs v4 deployments, BNB |
| Permit2 (Uniswap, no place) | `0x000000000022D473030F116dDEE9F6B43aC78BA3` | all (already listed) | developers.uniswap.org/contracts/permit2/overview |
| PancakeSwap Permit2 (no place) | `0x31c2F6fcFf4F8759b3Bd5Bf0e1084A055615c768` | 1, 56, 8453, 42161 | developer.pancakeswap.finance/contracts/permit2/addresses; confirmed compiled into `0xd9C5…` |
| PancakeSwap Universal Router (Infinity) | `0xd9C500DfF816a1Da21A48A732d3498Bf09dc9AEB` | 56, 8453 (was any-chain) | developer.pancakeswap.finance/contracts/universal-router/addresses; `infinity-universal-router` `deploy-addresses/bsc-mainnet.json` |
| Aave V3 Pool (Aave) | `0x6807dc923806fE8Fd134338EABCA509979a7e0cB` | 56 | `aave-dao/aave-address-book` `src/AaveV3BNB.sol` (`POOL`) |
| Aave Wrapped Token Gateway (Aave) | `0x0c2C95b24529664fE55D4437D7A31175CFE6c4f7` | 56 | same file (`WETH_GATEWAY`); ABI from `aave-v3-origin` `WrappedTokenGatewayV3.sol` |
| CoW Protocol (settlement) | `0x9008D19f58AAbD9eD0D60971565AA8510560ab41` | 1, 10, 56, 100, 137, 8453, 42161, 43114 | `cowprotocol/contracts` `networks.json`; docs.cow.fi core contracts |
| CoW Vault Relayer (CoW) | `0xC92E8bdf79f0507f65a392b0ab4667716BFE0110` | same | same |
| Spark PSM (Spark) | `0x1601843c5E9bC251A3272907010AFa41Fa18347E` | 8453 | `sparkdotfi/spark-address-registry` `src/Base.sol` (`PSM3`); docs.spark.fi Spark PSM |

Removed: the "CoW Protocol" row at `0x9008…ab42` — in no CoW source, no code
on any listed chain; a one-digit typo of the settlement address.

Router command tables: Uniswap `Commands.sol`/`Dispatcher.sol` @ 2.1.2 (2.1.x
swap inputs carry a 6th `uint256[] minHopPriceX36`; the reader tries 6 then 5
fields); PancakeSwap `infinity-universal-router` @ b303b14a (mask `0x3f`,
0x10 INFI_SWAP, 0x22/0x23 stable swaps). Not read: Uniswap 2.1.x
`executeSigned` (falls to the ordinary ladder).

## Before / after — the real requests (`rust/crates/vela-core/tests/fixtures/dapp096/`)

"Before" is origin/main's core with the same scripted shell (no descriptor
service, chain answers decimals/symbol, 4-byte DB unnamed); `(detail)` rows are
dropped by desktop/iOS/Android and shown at the bottom on web. Times are
UTC+8, ISO.

**PancakeSwap USDC→BNB batch** — before: `1·Approve 2.3417 USDC, Spender
0x31c2f6...15c768 · 2·Approve (best effort) [Address, Address, Value
2,341,714,000,000,000,000, Value 1,793,538,960] · 3·Execute (best effort)
[Data 0x000c, Data […], Value 1,790,948,152]`, record none (Activity "Batch on
PancakeSwap"). After:
```
1 · Approve   Amount 2.341714 USDC · Spender PancakeSwap Permit2 · Interacting with 0x8AC7…580d
2 · Approve   Amount 2.341714 USDC · Spender PancakeSwap Universal Router · Expires 2026-11-01, 21:16 · Interacting with PancakeSwap Permit2
3 · Swap      You pay 2.341714 USDC · You receive (min) 0.002983249209629786 BNB · Recipient 0x88cca0...266894 · Deadline 2026-10-02, 21:35 · Interacting with PancakeSwap Universal Router
record "Swap" → Activity "Swap on PancakeSwap" / 在 PancakeSwap 兑换
```
**Uniswap Permit2 PermitSingle** — before: `Approve · Amount Unlimited · Spender
0xdc2647...be7c9f · Expires · Valid until` (no token). After: `Approve · Amount
Unlimited · Token WBNB · Spender Uniswap Universal Router · Expires
2026-11-01, 21:20 · Valid until 2026-10-02, 21:50`.

**Aave depositETH (sends 0.003 BNB)** — before: `Deposit eth (best effort) ·
Address · Address · Value 0` (the referral code; 0.003 BNB nowhere). After:
`Supply · Supply 0.003 BNB · On behalf of 0x88cca0...266894`, record
"Supply" → "Supply on Aave" / 在 Aave 存入. (A call the wallet does not know
still states "Amount −0.003 BNB" through `native_value`.)

**Aave supply** — before: `Supply (best effort) · Value 3,000,000,000,000,000 ·
Value 0`. After: `Supply · Supply 0.003 WBNB · On behalf of 0x88cca0...266894`.

**Aave withdraw-all** — before: `Withdraw (best effort) · Value
115,792,…,639,935` (78 digits). After: `Withdraw · Withdraw All · Token WBNB ·
Recipient 0x88cca0...266894` (zh 全部 · 代币 WBNB), record "Withdraw".

**Aave unlimited approve** — before: `Approve · Unlimited · Spender
0x6807dc...a7e0cb`. After: `Approve · Unlimited · Token WBNB · Spender Aave V3
Pool`, Activity "Approve on Aave".

**CoW approve + pre-sign** — before: `2 · Set pre signature (best effort) ·
Data 0x09e95d9c…bfb9fe · Flag true`. After: `2 · Swap · Order
0x09e95d9c...6abfb9fe · Valid until 2026-10-02, 22:04` + caution "This order's
amounts aren't shown here — check them on the site first." · spender "CoW
Vault Relayer" · target "CoW Protocol"; record "Swap" → "Swap on CoW".

**Sky USDC→USDS (Base)** — before: `1·Approve 0.0349 USDC, Spender
0x160184...18347e · 2·Swap exact in (best effort) [Value 34,929, Value
34,929,000,000,000,000, …]`. After: `1·Approve 0.034929 USDC, Spender Spark
PSM · 2·Swap · You pay 0.034929 USDC · You receive (min) 0.034929 USDS ·
Recipient 0x88cca0...266894`; record "Swap" → "Swap on Spark".

## F7

Every shell's slide now also waits for `ClearSigningView.resolving` (it armed
under "Loading…" on all four — the fee alone decided). Web's loading line is
the neutral `componentsUi.signing.loading` (加载中...), no longer "Set a
finite amount to continue." Tested on every shell.

## Tests

| Suite | Result |
|---|---|
| core `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2392 passed, 0 failed (new `tests/app_clear_signing_dapp096.rs`: 14 + 1 ignored printer; 2 updated in `app_clear_signing.rs`; 1 new + 1 updated in `dapp_activity.rs`) |
| core clippy `-D warnings`, `cargo fmt --check` | clean |
| web `npx vitest run` / `pnpm check` | 2437 passed, 5 skipped / 0 errors, 0 warnings (5 new in `live.test.ts`) |
| desktop fmt / clippy --all-targets / `cargo test` | clean / no new warnings / 898 passed, 49 ignored (3 new) |
| Android `testDebugUnitTest` | 956 passed, 0 failed (3 new in `SigningLiveTest`, drift registrations) |
| iOS `SigningLiveTests` + drift suites / full `VelaWalletTests` | 40 passed / 1150 passed in 147 suites (3 new; two run the real core) |
| `check-native-reachability`, `check-event-payloads`, `check-dead-controls` | pass |
| i18n gen / lint / verify / dump:vectors | pass; 75,680 comparisons, 0 divergences |

## i18n

`componentsUi.signing.{labelOrder, valueAll, warnOrderTerms}` × 15 locales
(zh-HK in written Cantonese). ja+en resident 135,801 → 136,014 bytes (+213;
JSON route +221); `SC005_BUDGET` 141,800 (owner, 2026-10-02); gen-i18n path
count 1798 → 1801.

## Screenshots

`/private/tmp/claude-501/-Volumes-data-production-vela-wallet/d4a496f4-ab96-4481-916a-65326d48067e/scratchpad/096b/`:
web `web-{pcs,uni,aave}-{zh,en}.png`, iOS simulator `ios-x096-{pcs,uni,aave}-{zh,en}.png`.
Both were drawn by the real sheet builders (`buildSigningModel`,
`SigningLive.model`) over the real cores reading the fixture requests with
a scripted chain, through a temporary gallery state that is **not committed**
(the scripts are kept beside the PNGs: `x096-gallery.scratch.test.ts`,
`gallery-patch.diff`, `X096Gallery.swift`). Desktop has no signing-sheet
gallery and Android no JVM screenshot harness — covered by their unit tests.

## Not done / open

- Sky's convert is named **Spark** ("Swap on Spark", "Spark PSM"): the
  official registry says PSM3 is Spark's. Say if the owner wants "Sky".
- Exact amounts can be long (`0.002983249209629786 BNB`); chosen over a
  rounded figure for WYSIWYS. A shorter form needs an owner rule.
- Web draws "−无限额" as the hero and the token on its own row below; iOS and
  Android permit sheets still add the guard's own spender row by short
  address (pre-existing).
- Not read by the built-ins: Uniswap 2.1.x `executeSigned`, v4/Infinity swap
  amounts (shown as "Swap" + incomplete), 1inch v6 `unoswap*` (untested in the
  pass).
- Expected merge points: `scripts/gen-i18n.mjs` count, `SC005_BUDGET`
  comment, the corpus lines next to `labelOnBehalfOf`/`valueUnlimited`,
  `dapp_activity::summarize` (new `chain_id` argument), and the guard-warning
  lines of web `live.ts` that 094 also edits.
