# D02 · Portfolio Aggregation (Multicall3 Per Chain)

| | |
|---|---|
| **Epic** | D — Balances, Portfolio & Activity |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F03 |
| **Related** | D03, D04, E01, F08, D01 |

## 1. Summary

For each chain, Vela fetches the **entire portfolio in a single `Multicall3 aggregate3` call** —
balances, `decimals`, per-token DEX quotes, and the chain's Chainlink native/USD feed — bounded by an
18s `Promise.race`. Token-metadata reads batch 40 tokens per call. This keeps multi-chain balance
loading fast and resilient over the RPC pool (F03).

## 2. Background & context

Naively, a portfolio is N×M RPC calls (tokens × fields × chains). Packing everything into one
Multicall3 per chain collapses that to one round-trip per chain, which matters over flaky public RPCs
and rate limits (F08). A per-chain timeout ensures one slow chain can't stall the whole portfolio.

## 3. Users & stories

- As a **user**, I want my multi-chain balances to load quickly, so that the wallet feels instant.
- As a **user on a slow chain**, I want other chains to still load, so that one bad RPC doesn't block everything.

## 4. Functional requirements

- **FR-1** — Per chain, pack balances + `decimals` + per-token DEX quotes + Chainlink native feed into one `aggregate3` call.
- **FR-2** — Bound each chain's fetch with an 18s `Promise.race`; a timeout yields partial/cached data (A06/F08), never a crash.
- **FR-3** — Token metadata batches 40 tokens per call (D04).
- **FR-4** — Merge results per chain so a failing chain never zeros a healthy chain's balance.
- **FR-5** — Prices come from the on-chain pricing engine (E01); missing price renders `null`, not `0` (D01).

## 5. Non-functional requirements

- **NFR-1** — Executes over the RPC pool with failover (F03/F04).
- **NFR-2** — Results cached (A06) for instant next-launch paint, then reconciled to fresh.

## 6. UX / flow notes

No direct UI; feeds home (D01), token list (D03), token detail (D07). Merge-by-chain guarantees a transient chain failure doesn't blank the portfolio.

## 7. Acceptance criteria

- [ ] **AC-1** — A full portfolio for a chain resolves in one Multicall3 call.
- [ ] **AC-2** — A chain that times out at 18s yields cached/partial data; other chains are unaffected.
- [ ] **AC-3** — A failing chain never zeros a healthy chain's balance (merge-by-chain).

## 8. Out of scope / non-goals

- Pricing math — **E01/E03**; metadata resolution — **D04**; RPC scoring — **F03**.

## 9. Dependencies, risks & open questions

- **Risk:** Multicall3 must exist on the chain (validated for custom networks, F02).
- **Open question:** None.

## 10. Source anchors

- `src/services/wallet-api.ts:251-518` — aggregate3 portfolio call; `:40-49` Chainlink feeds.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 60, 62, 63; memory `project_wallet_bundler_gasprice_parity` (merge-by-chain).
