# E02 · DEX Price Adapters (Uniswap / Pancake / Aerodrome / Sushi)

| | |
|---|---|
| **Epic** | E — Pricing & Fiat |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | E01 |
| **Related** | E03, F01, D04 |

## 1. Summary

DEX pricing supports **two protocol families** with hardcoded per-chain contracts: Uniswap-V3-style
`QuoterV2.quoteExactInputSingle` (fee tiers 500/3000/2500/10000) on most chains; **PancakeSwap V3** on
BSC; **Aerodrome** solidly router (`getAmountsOut`, volatile+stable) on Base; **SushiSwap V3** on
Gnosis. Built-in DEX overrides take precedence over any remote API.

## 2. Background & context

There's no universal DEX; each chain has a dominant venue. Encoding the right quoter per chain (and
preferring built-in overrides) yields reliable on-chain quotes without trusting a remote aggregator.

## 3. Users & stories

- As a **user on BSC/Base/Gnosis**, I want prices from the chain's real DEX, so that values are accurate there.
- As a **maintainer**, I want per-chain venues declared in one place, so that adding a chain is a config change.

## 4. Functional requirements

- **FR-1** — Uniswap-V3-style quoter (fee tiers 500/3000/2500/10000) as the default family.
- **FR-2** — PancakeSwap V3 on BSC; Aerodrome (volatile+stable) on Base; SushiSwap V3 on Gnosis.
- **FR-3** — Built-in per-chain DEX overrides beat the remote API.
- **FR-4** — Return a comparable USD-denominated quote for use by E01/E03.

## 5. Non-functional requirements

- **NFR-1** — Quotes execute inside the per-chain Multicall3 batch (D02) where possible.
- **NFR-2** — A venue that returns nothing degrades to the next price source (E01), not an error.

## 6. UX / flow notes

No direct UI. Chain→venue mapping lives with the chain/token config.

## 7. Acceptance criteria

- [ ] **AC-1** — Base tokens price via Aerodrome; BSC via Pancake V3; Gnosis via Sushi V3.
- [ ] **AC-2** — A built-in override supersedes any remote API value.
- [ ] **AC-3** — Fee-tier selection picks a liquid pool where present.

## 8. Out of scope / non-goals

- Price selection/guard logic — **E03**; feed registry — **E04**.

## 9. Dependencies, risks & open questions

- **Risk:** venue/contract drift per chain; keep the per-chain map current.
- **Open question:** adding new venues as chains are added (F01/F02).

## 10. Source anchors

- `src/services/chain-tokens.ts:48-104`, `src/services/wallet-api.ts:514-526`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 61.
