# F08 · Rate-Limit UX & Graceful Degradation

| | |
|---|---|
| **Epic** | F — Networks & RPC Infrastructure |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F04 |
| **Related** | D01, D02, N03 |

## 1. Summary

RPC 429 / rate-limit is treated as **transient, not an outage**: the wallet shows **cached balances**
and does **not** swap in a scary "RPC down" banner. A dedicated banner (`RpcTroubleBanner`) appears only
for genuine, persistent trouble. `getRateLimitedChains()` couples the pool state to the home screen and
the fault-injection harness (`vela.rateLimitRpc`, N03).

## 2. Background & context

Public RPCs rate-limit routinely; surfacing that as failure erodes trust and looks broken. The correct
UX is calm degradation: keep showing the last good data, keep retrying quietly (F04), and only alarm when
the chain is actually unreachable.

## 3. Users & stories

- As a **user**, I want a brief rate-limit to be invisible, so that the wallet doesn't look broken over nothing.
- As a **user in a real outage**, I want an honest indicator, so that I know data may be stale.

## 4. Functional requirements

- **FR-1** — On 429/rate-limit, retain and display **cached** balances (A06); no error banner.
- **FR-2** — Expose `getRateLimitedChains()` so the home screen (D01) can reflect degraded chains subtly.
- **FR-3** — Show `RpcTroubleBanner` only for persistent, non-transient failure (distinct from rate-limit).
- **FR-4** — The fault-injection harness can simulate rate-limits (`vela.rateLimitRpc`, N03) to validate this UX.

## 5. Non-functional requirements

- **NFR-1** — Never render cached-as-fresh without the ability to distinguish (A06); avoid implying live data during degradation.
- **NFR-2** — Degradation is per-chain, not global.

## 6. UX / flow notes

Rate-limited chains keep their last balance; pull-to-refresh (M04) retries. A true outage escalates to the trouble banner. No modal alarms.

## 7. Acceptance criteria

- [ ] **AC-1** — Simulating rate-limit (N03) shows cached balances with no error banner.
- [ ] **AC-2** — `getRateLimitedChains()` reports the affected chains.
- [ ] **AC-3** — A simulated full outage shows the trouble banner; a rate-limit does not.

## 8. Out of scope / non-goals

- Failover/banning internals — **F04**; fault harness — **N03**.

## 9. Dependencies, risks & open questions

- **Risk:** distinguishing "rate-limited" from "down" must be reliable to avoid false calm.
- **Open question:** None.

## 10. Source anchors

- `src/services/rpc-pool.ts` (`getRateLimitedChains`), `src/components/ui/RpcTroubleBanner.tsx`.
- memory `project_rate_limit_ux`; `docs/CONTENT-SOURCE-100-CLUES.md` — clues 29, 56.
