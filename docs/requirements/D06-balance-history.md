# D06 · 7-Day Balance History (Block-Time Estimation + Archive RPC)

| | |
|---|---|
| **Epic** | D — Balances, Portfolio & Activity |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F03 |
| **Related** | D07, E01, F04 |

## 1. Summary

Vela renders a **7-day balance history** derived entirely on-chain: it estimates average block time by
sampling `latest` vs `latest−1000`, converts past midnights into block numbers, auto-discovers an
**archive-capable RPC** by test-querying ~1-day-old balances, and validates each point within ±1h of
its target. No third-party history API.

## 2. Background & context

Historical balance normally requires an indexer. Vela stays indexer-free (A03) by reconstructing
history from archive `eth_call`s at estimated historical blocks. Auto-discovering an archive node is
necessary because most pool RPCs (F03) are pruned.

## 3. Users & stories

- As a **user**, I want a short balance history chart, so that I can see how my holdings moved this week.
- As a **user**, I want it to work without a data provider, so that it aligns with Vela's no-API stance.

## 4. Functional requirements

- **FR-1** — Estimate avg block time from `latest` vs `latest−1000`; map the last 7 midnights to block numbers.
- **FR-2** — Auto-discover an archive-capable RPC by test-querying ~1-day-old balances; use it for historical reads.
- **FR-3** — Validate each history point falls within ±1h of its target time; discard/adjust otherwise.
- **FR-4** — Degrade gracefully when no archive RPC is found (fewer points / hidden chart), never a crash.

## 5. Non-functional requirements

- **NFR-1** — Bounded work; runs over the pool (F03) with failover (F04).
- **NFR-2** — On-chain only — no external history API.

## 6. UX / flow notes

Rendered on token detail (D07) via `BarChart`. Missing data shows an honest empty/partial state, not a fabricated line.

## 7. Acceptance criteria

- [ ] **AC-1** — For a chain with an archive RPC, 7 daily points render within ±1h of each target.
- [ ] **AC-2** — With no archive RPC available, the chart degrades without error.
- [ ] **AC-3** — Block-time estimation adapts per chain (fast vs slow chains).

## 8. Out of scope / non-goals

- Current-balance aggregation — **D02**; pricing — **E01**.

## 9. Dependencies, risks & open questions

- **Risk:** archive discovery may fail on some chains; must degrade cleanly.
- **Open question:** whether to extend beyond 7 days (currently bounded).

## 10. Source anchors

- `src/services/balance-history.ts` — block-time estimation + archive discovery + validation.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 65.
