# 097 — findings of the second real-money dApp pass (2026-10-03)

**Build:** Chrome extension 0.9.6 dev package from `096-clear-signing-readable` (main `cca03b56` content). Golden Safe `0x88cC…6894`, BNB Chain.
**Evidence:** session scratchpad `dapp096/`:
- `shots/r2-*.png`
- `logs/r2-req-*.json` — the raw requests the wallet received
- `logs/r2-pagereqs-*.json` — what each page sent and got back

## Flows

| Flow | Result | Tx | Fee coin | Sheet | Activity row |
|---|---|---|---|---|---|
| PancakeSwap BNB→USDC 0.003 | landed | `0xaf5d0de6…a7c3` | BNB | Swap −0.003 BNB → +2.30985453836239 USDC (a minimum, unlabelled: N3) | "Swap on PancakeSwap −0.003 BNB +2.319944 USDC" |
| PancakeSwap USDC→BNB 1.16 (3-call batch) | landed | `0x3c8446de…ee29` | BNB, picked automatically (F2 fixed); a manual USDC pick is warned | Approve to PancakeSwap Permit2; Permit2 approve; Swap, receive (min) | "Swap on PancakeSwap", no figures (N5) |
| Uniswap BNB→USDC 0.003 | landed | `0x4404138a…d2a8` | BNB | Swap −0.003 BNB → +2.312… USDC (minimum unlabelled: N3) | "Swap on Uniswap −0.003 BNB +2.322695 USDC" |
| Uniswap USDC→BNB 1.16 | landed (approve, Permit2 sig, swap) | `0xfa9a1699…b318` | BNB | Approve (capped to 1.16), Permit2 PermitSingle: all fields match | "Approve on app.uniswap.org" (N7); "Swap on Uniswap", no figures (N5) |
| 1inch BNB→USDC 0.003 (native order) | order created, not filled (1inch quote far off market), refunded at expiry | `0xaf22b121…88b8` / refund `0x1c1f474e…c9bb` | BNB | "create order −0.003 BNB +2.418… **tokens**", **Beneficiary 78098611...991444** (N1) | "create order on 1inch.com −0.003 BNB" (N7) |
| 1inch USDC→BNB 1.16 | approve + order signature; filled | fill `0x8832c78b…05fa` | BNB | "1inch Order −1.16 USDC +0.00143… WBNB, To 0x00000000...00000000" (N6) | "Sign structured data on 1inch" (N7) |
| Aave supply 0.003 BNB | landed | `0xdaa84485…599c` | BNB | "Supply −0.003 BNB" (F4 fixed) | "Supply on Aave −0.003 BNB +0.002999 aBnbWBNB" |
| Aave borrow 0.3 USDC | landed | `0xcb06d01b…c5ee` | BNB | "Borrow **−0.3 USDC**" (N2) | "Borrow on Aave", no figures (N5) |
| Aave repay / withdraw | landed; debt 0 | `0x6d19214c…b5ff` / `0xcacf781d…bd7e` | BNB | "Repay −All", "Withdraw +All" | no figures (N5) |
| CoW USDC→BNB 1 | approve + pre-sign; solver filled | fill `0x5244b4d5…7ab8` | BNB | Order id + valid until + terms caution (as designed) | "Swap on CoW", no figures |
| Curve USDC→USDT 0.1 | landed | `0xb0c2f6fb…84d9` | BNB | Unnamed router `0xa72c85…51cc`; best-effort "Exchange" with raw arrays | "Contract interaction on www.curve.finance" (N7, N8) |
| Deliberate failure (Aave withdraw of unsupplied USDC) | refused after Submitted; nothing sent | — | — | "Submitted — Sent!", then the window closed silently (N4) | "Withdraw on Aave · Failed", no reason (N4) |

## Findings

- **N1 · S2 — 1inch native order: recipient as a decimal; unknown token treated as certain.**
  - **Recipient:** `Beneficiary 78098611...991444` is the Safe's address as a `uint256`. `format_address` (`clear_signing.rs:4664-4683`) shortens whatever text it gets.
  - **Unknown token:** `guess_token_decimals(None)` returns 18 as if known, and the symbol falls back to "tokens" (`clear_signing.rs:4640`). It was right only because USDC on BSC has 18 decimals; a 6-decimal token would read 10¹² off.
  - **Evidence:** `r2-076-1inch-sheet.png`, `r2-077-1inch-tech.png`, `logs/r2-req-r2-081-1inch-sheet.json`.
- **N2 · S3 — Borrow reads as an outflow.** `infer_field_roles` (`clear_signing.rs:4911-4931`) counts only withdraw, redeem, unstake and claim as incoming. The Aave descriptors label the amount "Borrow" (`clear_signing.rs:5777`, `5842`). Evidence: `r2-104-aave-borrow-sheet.png`.
- **N3 · S3 — Single-call swap shows the minimum as the received amount.** The "(min)" label is dropped in web `live.ts:490-503`; batch legs keep it. Evidence: `r2-020-pcs-bnb-sheet.png`, `r2-050-uni-bnb-sheet.png`.
- **N4 · S3 — A refusal after Submitted is never shown.**
  - **What happened:** the sheet said "Submitted — Sent! … UserOp Hash 0x4319f0f7…", then the window closed at about 12 s with no words. The page got −32603 "the network refused this transaction; nothing was sent", and Activity shows "Failed" with no reason.
  - **Cause:** `signing/dapp-receipt.ts` has a refused state (lines 94-95, 177-183), but the extension closes the window as soon as the request is answered (`src/lib/dapp/background.test.ts:1252`).
  - **Evidence:** `r2-160-fail-sheet.png`, `r2-161-fail-status-*.png`.
- **N5 · S3 — Activity has no figures for calls that send no native coin; their receipts vanish.**
  - **What's missing:** received tokens (0.3 USDC borrowed, 0.1 USDT from Curve) appear nowhere.
  - **Why:**
    - The web build never persists the sheet's balance changes (`approveOptsOf`, `live.ts:1264-1288`), so `dapp_item` (`activity_feed.rs:1540-1551`) has only the native value.
    - A matching "Received" record is merged into the dApp row and dropped as its own row (`activity_feed.rs:1325-1328`).
    - `ActivityRow.svelte:50` draws the "+" figure only when the row has a main amount.
  - **Evidence:** `r2-196-activity-all.png`, `r2-121-aave-borrow-detail.png`.
- **N6 · S3 — The 1inch `Order` signature hides terms that change what is signed.**
  - A zero receiver (= the maker) reads as `0x0000…0000`.
  - The expiry (in `makerTraits`) and the unwrap-to-native flag are not shown, so the sheet says WBNB while BNB arrives.
  - The minimum is not labelled.
- **N7 · S4 — Activity titles and places are inconsistent.**
  - **Titles:** "create order on 1inch.com" (lowercase, host) and "Sign structured data on 1inch" (no intent).
  - **Places:** "Approve on app.uniswap.org", because Permit2 is not tied to a protocol (`clear_signing.rs:380`, `activity_feed.rs:1914-1932`); the Curve rows use the host.
  - **Detail sheet:** "≈ $0.00" under real amounts (`r2-022-pcs-bnb-detail.png`, `r2-122-1inch-create-detail.png`).
- **N8 · S4 — Unnamed contracts and tokens.**
  - The Curve router on BSC (`0xa72c85…51cc`) has no name.
  - Batch legs show the USDC contract as a raw address under "Interacting with".
  - 1inch NativeOrderFactory is named on the sheet but raw in Activity.
  - Approve/permit Token rows show only the symbol.

## WYSIWYS (EIP-712)

- **Uniswap Permit2 `PermitSingle` — pass.** Token, amount (uint160 max → Unlimited), spender (Universal Router), expiration, sigDeadline, chain and verifying contract all match. The nonce is not shown, which is harmless.
- **1inch `Order` — partial (N6).** Amounts, maker and domain match. The zero receiver is misleading; expiry, flags and the salt's extension hash are not shown.
- **CoW pre-sign (on-chain) — pass as designed.**

## Status of 096 findings

- **Fixed:** F1, F2, F4, F6, F7, F11.
- **Partly fixed:**
  - F5 (Curve and the 1inch descriptor still raw);
  - F8 (N4);
  - F9 (N5, N7).
- **Not reproduced:** F3. Landed batches answer 200.
- **External:** F10 (PancakeSwap offers Vela only as "MetaMask").

## Not attempted

- **Sky collateral minting:** Sky's own widget requires a minimum borrow of 30,000 USDS, on Ethereum only.

## Balances after the pass (BNB Chain)

- **Holdings:** BNB 0.048339, WBNB 0.009, USDC 0.0626, USDT 0.1000; no Aave position or debt.
- **Leftover allowances:**
  - WBNB→Permit2 0.003;
  - USDC→Aave Pool 0.000999963;
  - aBnbWBNB→Gateway ≈0.0001;
  - Permit2 USDC→Uniswap router unlimited until 2026-11-01. This one is inert: the USDC→Permit2 allowance is 0.
