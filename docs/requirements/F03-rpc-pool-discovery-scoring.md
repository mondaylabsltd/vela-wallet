# F03 · RPC Pool Auto-Discovery & 6-Tier Scoring

| | |
|---|---|
| **Epic** | F — Networks & RPC Infrastructure |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F01 |
| **Related** | F04, F05, F07, F08, D02 |

## 1. Summary

Each chain auto-discovers **~15–25 RPC endpoints**, scored by a **5-tier source priority** blended with
measured-latency and reliability signals: `user > default > public > builtin > fallback`. EMA latency
(`0.7·avg + 0.3·new`) and a reliability bonus refine the ranking;
measured latency wins after warm-up. **No single hardcoded RPC** — the pool is the transport for all
reads.

## 2. Background & context

Public RPCs are individually unreliable; a resilient wallet needs many with smart selection. Ranking by
trust tier first, then real measured performance, gives both safety (prefer your own nodes) and
speed (route to whatever's actually fast right now).

**Amended 2026-10-29 (owner decision): the RPC provider tier is retired.** Settings › RPC Providers
kept one Alchemy/dRPC/Ankr key each and slotted the URLs they built between a network's own RPC and
Vela's — a second, invisible priority level beside a per-network RPC that already does the job. A
network's own RPC (Settings › Networks) is now the one way to bring your own endpoint, keyed or not.
A wallet that saved keys keeps exactly the endpoint each network was using: on its first load the core
(`network_admin::retire_provider_keys`) writes, for every network the provider tier was serving (no RPC
of its own, a slug for the chain; several keys → the old Alchemy › dRPC › Ankr order picks), that URL
as the network's own RPC, and deletes `vela.rpcProviders` only once the write went through. A failed
write keeps the keys and retries on the next load; a half-finished one finishes without writing twice.

## 3. Users & stories

- As a **user**, I want reads to route to the best available RPC automatically, so that the wallet stays fast and up.
- As a **self-hoster**, I want my own RPC preferred, so that I control my data path.

## 4. Functional requirements

- **FR-1** — Discover ~15–25 endpoints per chain from multiple sources.
- **FR-2** — Rank by 5-tier priority: `user > default > public > builtin > fallback` (the `provider`
  tier is retired, §2).
- **FR-3** — Blend in EMA latency (`0.7·avg + 0.3·new`) and a reliability bonus; measured latency dominates after warm-up.
- **FR-4** — Serve as the transport for all chain reads (balances D02, pricing E01, logs D05, history D06) and bundler routing (F07).
- **FR-5** — Interoperate with failover/banning (F04) and range-cap handling (F05).

## 5. Non-functional requirements

- **NFR-1** — Selection state persists (A06) so warm rankings survive restarts.
- **NFR-2** — Bounded concurrency; a read-flood can't starve signing (F06).

## 6. UX / flow notes

No direct UI beyond Settings › Networks, where a user sets a network's own RPC (top tier — a keyed
provider URL included). Rate-limit states surface calmly (F08).

## 7. Acceptance criteria

- [ ] **AC-1** — A user-added RPC is preferred over public ones.
- [ ] **AC-2** — After warm-up, a faster measured endpoint outranks a nominally higher-tier slow one.
- [ ] **AC-3** — Reads succeed across ~15–25 discovered endpoints without a hardcoded single RPC.

## 8. Out of scope / non-goals

- Banning/self-heal — **F04**; range-cap — **F05**; bundler routing — **F07**.

## 9. Dependencies, risks & open questions

- **Risk:** a keyed RPC's quota exhaustion; scoring must demote and fail over (F04).
- **Open question:** None.

## 10. Source anchors

- `src/services/rpc-pool.ts:255-508` — discovery + scoring.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 29, 55.
