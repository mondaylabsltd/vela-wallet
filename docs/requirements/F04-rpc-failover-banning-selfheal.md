# F04 · RPC Failover, Banning & Self-Heal

| | |
|---|---|
| **Epic** | F — Networks & RPC Infrastructure |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F03 |
| **Related** | F05, F08, D02 |

## 1. Summary

The RPC pool applies **two-tier banning** and **self-heal**: temporary bans (rate-limit / 401 / 403,
~1h) vs permanent bans (0 successes + ≥6 failures, auto-expires 24h), persisted to AsyncStorage. If
**everything is banned**, the pool clears and rebuilds so the wallet never dead-ends. Automatic
failover with exponential cooldown routes around bad endpoints.

## 2. Background & context

Discovery (F03) finds many endpoints; some will fail or rate-limit. Banning bad ones and failing over
keeps reads flowing, while self-heal guarantees a fully-banned pool recovers instead of bricking the
app's data layer.

## 3. Users & stories

- As a **user**, I want the wallet to route around dead RPCs automatically, so that it stays usable.
- As a **user after an outage**, I want the pool to recover on its own, so that I don't have to reset anything.

## 4. Functional requirements

- **FR-1** — On failure, fail over to the next-best endpoint (F03) with exponential cooldown.
- **FR-2** — **Temporary ban** (~1h) for rate-limit / 401 / 403; **permanent ban** for 0 successes + ≥6 failures (auto-expires 24h).
- **FR-3** — Persist ban state to AsyncStorage (A06) across restarts.
- **FR-4** — **Self-heal**: if all endpoints are banned, clear and rebuild the pool.
- **FR-5** — Rate-limit outcomes drive calm UX (F08), not error banners.

## 5. Non-functional requirements

- **NFR-1** — No configuration a user must touch to recover.
- **NFR-2** — Banning never applies to `eth_getLogs` range-cap errors (those are returned, F05).

## 6. UX / flow notes

Invisible in the common case. Persistent trouble surfaces via `RpcTroubleBanner` (F08) only when genuinely degraded, not on transient rate limits.

## 7. Acceptance criteria

- [ ] **AC-1** — A 429-ing endpoint is temporarily banned and reads fail over.
- [ ] **AC-2** — With all endpoints banned, the pool self-heals and reads resume.
- [ ] **AC-3** — Ban state survives an app restart.

## 8. Out of scope / non-goals

- Discovery/scoring — **F03**; range-cap handling — **F05**; rate-limit UX — **F08**.

## 9. Dependencies, risks & open questions

- **Risk:** over-aggressive permanent bans could shrink the pool — the 24h auto-expiry + self-heal bound this.
- **Open question:** None.

## 10. Source anchors

- `src/services/rpc-pool.ts:63-488` — two-tier banning + self-heal.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 56.
