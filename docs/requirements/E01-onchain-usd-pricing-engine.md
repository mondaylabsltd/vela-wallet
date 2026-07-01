# E01 · On-Chain USD Pricing Engine (DEX → Chainlink → null)

| | |
|---|---|
| **Epic** | E — Pricing & Fiat |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F03 |
| **Related** | E02, E03, E04, D02 |

## 1. Summary

All crypto USD prices are derived **on-chain**: DEX swap quotes first, then Chainlink feeds, then
`null`. There is **no CoinGecko / CMC / Moralis** or any third-party price API. A missing price is
`null` (rendered as "—"), never `0` — an unpriced token must never look worthless-but-confident.

## 2. Background & context

Third-party price APIs are a centralization and privacy leak (A03) and a single point of failure.
Deriving prices from the same chains the wallet already talks to keeps pricing trustless and consistent
with "don't trust — verify." Packed into the Multicall3 portfolio call (D02), it's also cheap.

## 3. Users & stories

- As a **user**, I want fiat values without a price API tracking me, so that pricing matches Vela's privacy stance.
- As a **user**, I want an unknown token to show "—" not "$0", so that I'm not misled.

## 4. Functional requirements

- **FR-1** — Resolve a token's USD price via: (1) DEX quote (E02), (2) Chainlink feed (E04), (3) `null`.
- **FR-2** — Never fall back to a third-party price API.
- **FR-3** — Return `null` for unpriceable tokens; consumers render "—", never `0`.
- **FR-4** — Stablecoins are hard-pegged to $1 (E03).
- **FR-5** — Prices are fetched inside the per-chain Multicall3 batch (D02) where possible.

## 5. Non-functional requirements

- **NFR-1** — Over the RPC pool with failover (F03/F04); rate-limit tolerant (F08).
- **NFR-2** — Cached (A06) then reconciled; a transient failure keeps the last known price rather than dropping to `0`.

## 6. UX / flow notes

No direct UI; feeds fiat values on home (D01), token list (D03), token detail (D07), and send (H01). Display formatting/currency handled by E06.

## 7. Acceptance criteria

- [ ] **AC-1** — A token with DEX liquidity prices from the DEX quote.
- [ ] **AC-2** — A token with no DEX but a Chainlink feed prices from Chainlink.
- [ ] **AC-3** — A token with neither renders "—", not "$0".

## 8. Out of scope / non-goals

- DEX adapter specifics — **E02**; native price guard — **E03**; fiat conversion — **E05**.

## 9. Dependencies, risks & open questions

- **Risk:** low-liquidity DEX quotes can be manipulated — the native guard (E03) bounds this for native; token quotes rely on curated DEX overrides (E02).
- **Open question:** None.

## 10. Source anchors

- `src/services/price-service.ts:1-11`, `src/services/wallet-api.ts:364-406`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 28.
