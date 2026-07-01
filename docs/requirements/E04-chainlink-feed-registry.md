# E04 · Per-Chain Chainlink Feed Registry

| | |
|---|---|
| **Epic** | E — Pricing & Fiat |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F01 |
| **Related** | E01, E03 |

## 1. Summary

A hardcoded registry maps each chain to its **Chainlink native/USD feed** (8-decimal answer): ETH / BNB
/ AVAX / DAI feeds per chain. **Polygon is deliberately omitted** (the MATIC→POL migration broke its
feed; DEX pricing covers it, E02/E03). This registry is the reliability anchor for native pricing
(E03).

## 2. Background & context

Chainlink feeds are the trusted price oracle, but they're per-chain and their decimals/availability
vary. A hardcoded, reviewed registry avoids guessing feed addresses at runtime and documents the known
gaps (Polygon).

## 3. Users & stories

- As a **maintainer**, I want feed addresses in one reviewed registry, so that pricing is auditable and correct.
- As a **user**, I want reliable native pricing even when DEX liquidity is thin, so that my balance is trustworthy.

## 4. Functional requirements

- **FR-1** — Map supported chains to their Chainlink native/USD feed address (8-decimal answer).
- **FR-2** — Read `latestRoundData()` (or equivalent) for the native price used by E03.
- **FR-3** — **Omit Polygon** (no reliable feed); E03 relies on DEX there.
- **FR-4** — Feeds are read on-chain over the pool (F03), no third-party proxy.

## 5. Non-functional requirements

- **NFR-1** — Registry changes are reviewed (auto-High if they affect pricing, O02).
- **NFR-2** — Answer scaling (8-decimal) handled explicitly.

## 6. UX / flow notes

No direct UI; consumed by E03. Documented gaps (Polygon) are intentional, not bugs.

## 7. Acceptance criteria

- [ ] **AC-1** — Each registered chain returns a native price from its feed.
- [ ] **AC-2** — Polygon has no feed entry and prices via DEX (E03).
- [ ] **AC-3** — 8-decimal answers scale correctly to USD.

## 8. Out of scope / non-goals

- Fallback/guard logic — **E03**; fiat-currency feeds — **E05/E06**.

## 9. Dependencies, risks & open questions

- **Risk:** a feed deprecation (as with MATIC→POL) needs a registry update + DEX fallback check.
- **Open question:** feeds for newly added chains (Tempo/Monad/World Chain/Unichain) where available.

## 10. Source anchors

- `src/services/wallet-api.ts:40-49` — per-chain Chainlink feed registry.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 63.
