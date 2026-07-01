# E03 · Native Price 3-Tier Fallback + Chainlink Sanity Guard

| | |
|---|---|
| **Epic** | E — Pricing & Fiat |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | E01, E04 |
| **Related** | E02, D02 |

## 1. Summary

The **native coin's** USD price uses a 3-tier fallback with a **Chainlink sanity guard**: a DEX price
is accepted only if it's within **0.5×–2.0× of Chainlink** (a >50% deviation is treated as
low-liquidity and discarded); otherwise fall back DEX → on-chain Chainlink → Ethereum-mainnet
Chainlink. Stablecoins are hard-pegged to $1.

## 2. Background & context

Native-coin pricing is high-stakes (it prices gas and the headline balance). A thin DEX pool could
misprice it wildly, so Chainlink acts as a sanity bound rather than a blind fallback — combining DEX
freshness with feed reliability.

## 3. Users & stories

- As a **user**, I want my native balance and gas costs priced accurately, so that I trust the headline number.
- As a **user on a low-liquidity chain**, I don't want a manipulated DEX quote to distort my balance.

## 4. Functional requirements

- **FR-1** — Prefer the DEX native price **only if** within 0.5×–2.0× of Chainlink; else discard as low-liquidity.
- **FR-2** — Fallback order: DEX (guarded) → on-chain Chainlink (E04) → Ethereum-mainnet Chainlink.
- **FR-3** — Hard-peg recognized stablecoins to $1.
- **FR-4** — Return `null` if all tiers fail (E01 contract: "—", never $0).

## 5. Non-functional requirements

- **NFR-1** — Deterministic guard bounds; no silent acceptance of an out-of-range DEX quote.
- **NFR-2** — Runs within the portfolio batch (D02).

## 6. UX / flow notes

No direct UI; underpins the balance hero (D01/M03) and gas cost display (G05).

## 7. Acceptance criteria

- [ ] **AC-1** — A DEX quote 3× off Chainlink is rejected; Chainlink is used.
- [ ] **AC-2** — With no on-chain feed, mainnet Chainlink is used.
- [ ] **AC-3** — A stablecoin prices at exactly $1.

## 8. Out of scope / non-goals

- Feed registry — **E04**; token (non-native) pricing — **E01/E02**.

## 9. Dependencies, risks & open questions

- **Risk:** Polygon's native feed is intentionally omitted (MATIC→POL migration broke it) — DEX covers it (E04).
- **Open question:** None.

## 10. Source anchors

- `src/services/wallet-api.ts:385-447` — 3-tier fallback + guard.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 62, 63.
