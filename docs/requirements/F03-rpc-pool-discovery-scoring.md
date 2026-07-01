# F03 · RPC Pool Auto-Discovery & 6-Tier Scoring

| | |
|---|---|
| **Epic** | F — Networks & RPC Infrastructure |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F01 |
| **Related** | F04, F05, F07, F08, D02 |

## 1. Summary

Each chain auto-discovers **~15–25 RPC endpoints**, scored by a **6-tier source priority** blended with
measured-latency and reliability signals: `user > provider (Alchemy/dRPC/Ankr) > default > public >
builtin > fallback`. EMA latency (`0.7·avg + 0.3·new`) and a reliability bonus refine the ranking;
measured latency wins after warm-up. **No single hardcoded RPC** — the pool is the transport for all
reads.

## 2. Background & context

Public RPCs are individually unreliable; a resilient wallet needs many with smart selection. Ranking by
trust tier first, then real measured performance, gives both safety (prefer your own/provider nodes) and
speed (route to whatever's actually fast right now).

## 3. Users & stories

- As a **user**, I want reads to route to the best available RPC automatically, so that the wallet stays fast and up.
- As a **self-hoster**, I want my own RPC preferred, so that I control my data path.

## 4. Functional requirements

- **FR-1** — Discover ~15–25 endpoints per chain from multiple sources.
- **FR-2** — Rank by 6-tier priority: `user > provider > default > public > builtin > fallback`.
- **FR-3** — Blend in EMA latency (`0.7·avg + 0.3·new`) and a reliability bonus; measured latency dominates after warm-up.
- **FR-4** — Serve as the transport for all chain reads (balances D02, pricing E01, logs D05, history D06) and bundler routing (F07).
- **FR-5** — Interoperate with failover/banning (F04) and range-cap handling (F05).

## 5. Non-functional requirements

- **NFR-1** — Selection state persists (A06) so warm rankings survive restarts.
- **NFR-2** — Bounded concurrency; a read-flood can't starve signing (F06).

## 6. UX / flow notes

No direct UI beyond RPC provider settings (N02) where a user can add their own endpoint (top tier). Rate-limit states surface calmly (F08).

## 7. Acceptance criteria

- [ ] **AC-1** — A user-added RPC is preferred over public ones.
- [ ] **AC-2** — After warm-up, a faster measured endpoint outranks a nominally higher-tier slow one.
- [ ] **AC-3** — Reads succeed across ~15–25 discovered endpoints without a hardcoded single RPC.

## 8. Out of scope / non-goals

- Banning/self-heal — **F04**; range-cap — **F05**; bundler routing — **F07**.

## 9. Dependencies, risks & open questions

- **Risk:** provider-key exhaustion; scoring must demote and fail over (F04).
- **Open question:** None.

## 10. Source anchors

- `src/services/rpc-pool.ts:255-508` — discovery + scoring.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 29, 55.
