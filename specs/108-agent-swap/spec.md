# 108 — Swaps the agent can prepare, without reading a page

**Status:** draft (2026-10-27), for the owner's review.

## The problem

The owner (2026-10-27): "比如你对 Agent 说：'打开 Uniswap，帮我把 100 USDC 换成 ETH。' 按照当前能力，它可以打开 Uniswap，但不能读取页面、选择交易对、填写金额或者点击 Swap。我希望能达到这个能力，不读取网页内容，是一个明确的能力限制。"

In English: the agent can open Uniswap but cannot read the page, choose the pair, enter the amount or press Swap. The owner wants the swap itself, and not reading web content is an explicit limit.

So the swap is the wallet's own capability. It is not driven through a dApp's page. The agent never reads, clicks or types in a site (spec 107 already says so, and it stays so). It asks the wallet for a swap, the way it asks for a transfer. The wallet builds it, shows it on its own review, and the person confirms it there with their passkey.

What the core already has:
- `clear_signing` reads Uniswap's Universal Router calls and PancakeSwap's for the signing review. It knows the routers by address on each chain.
- `safe.rs` batches calls into one operation (MultiSend).
- The transfer pattern: an agent drafts, its submit only asks the person, and the passkey is never raised over a review nobody chose to confirm.

## Proposed decisions (for the owner)

| | Proposal |
|---|---|
| S1 | **No page, ever.** A swap is `prepare_swap`, drafted into the wallet's own swap review. As with transfers, only the person confirms, and their passkey signs. |
| S2 | **The route comes from the chain itself:** Uniswap's on-chain Quoter, executed through Uniswap's Universal Router (the contract the review already reads). There is no API key, and no third party sees the request. An aggregator (0x, 1inch, LI.FI) can be added later behind the same review if prices need it. |
| S3 | **Only known tokens.** The token sold must be one the wallet holds. The token bought must be the chain's own coin, or a token in the wallet's list or the verified registry. An agent can never buy an arbitrary contract it names, which is the fake-"ETH" trap. |
| S4 | **Protections are the wallet's, not the agent's:** slippage defaults to 0.5%, and an agent may ask for at most 3%. Price impact over 3% is warned on the review; over 10% is refused. A quote is fetched again on confirm if it is older than 60 seconds. The token approval is for the exact amount, never unlimited, and goes in the same operation as the swap. |
| S5 | **The person can swap too.** A capability the agent has, the person has (spec 104). The asset page gains Swap, which opens the same form and the same review. |
| S6 | **Networks:** those this wallet serves where Uniswap's Quoter and Universal Router are deployed (Ethereum, Base, Arbitrum, Optimism, Polygon, BNB Chain, …), in the current space. |

## Design

- **Core (`vela_core::app::swap`):** a machine like `send`.
  - The form (sell, buy, amount, slippage).
  - The quote: the Quoter's `quoteExactInputSingle` across the fee tiers, plus a two-hop route through the chain's wrapped coin.
  - The review: sell X, receive at least Y, the rate, the price impact, the route, the network fee.
  - The batch: an exact approve to Permit2 or the router, then the router call.
  - It hands the batch to the existing signing and relay path.
  - The review is clear-signed by the same reader that reads a dApp's Uniswap call.
- **Intents:**
  - `prepare_swap { chain_id, sell, buy, amount, slippage_bps? }` (Transact): answers `accepted` with an operation, as `prepare_transfer` does.
  - `submit_swap { operation }`: an agent's submit only asks.
  - `cancel_swap { operation }`.
  - `vela_wait` follows a swap as it follows a transfer.
- **Not in the agent's reach:** confirming it, raising the passkey, or changing slippage past S4 without the person's own edit on the form.

## Phases

| Phase | What |
|---|---|
| 1 | Core: the swap machine (quote, review, batch) with tests against recorded Quoter answers; the intents |
| 2 | One shell end to end (Android first, the S22 test wallet), on a testnet with a Uniswap deployment, then a small mainnet swap on the test wallet |
| 3 | The other three shells |
| 4 | Gates and device runs |

## Open

- Whether to add an aggregator for better prices (S2), and which.
- Uniswap v4 pools (the Universal Router 2.x reads them; the Quoter for v4 differs).
