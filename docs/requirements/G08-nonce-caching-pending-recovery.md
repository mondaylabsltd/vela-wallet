# G08 · Nonce Caching & Already-Pending Recovery

| | |
|---|---|
| **Epic** | G — Transaction Engine (ERC-4337) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G01 |
| **Related** | G09, D08, H01 |

## 1. Summary

To avoid concurrent-send collisions, the account nonce is **cached ~10s and optimistically
incremented**. If the bundler rejects a submission because a **prior op is already in-flight**, Vela
**extracts the existing op's hash and polls that receipt** instead of failing — turning a race into a
graceful recovery.

## 2. Background & context

4337 ops are async; two quick sends could reuse a nonce and collide. Optimistic increment prevents the
common case; the "already pending" recovery handles the case where an op is mid-flight, so the user sees
their transaction tracked rather than an error.

## 3. Users & stories

- As a **user sending twice quickly**, I want both to be handled without a nonce clash, so that neither is lost.
- As a **user**, if my op is already pending, I want it tracked, not reported as failed.

## 4. Functional requirements

- **FR-1** — Cache the nonce ~10s and optimistically increment on each build (G01) to prevent collisions.
- **FR-2** — On a bundler "already pending / replacement" rejection, **extract the in-flight op hash** and poll its receipt (G09).
- **FR-3** — Reconcile the recovered op into the activity feed (D08) rather than surfacing a hard error.
- **FR-4** — Invalidate the cached nonce appropriately after confirmation/failure.

## 5. Non-functional requirements

- **NFR-1** — Recovery is idempotent — a recovered op is tracked once, not duplicated.
- **NFR-2** — Nonce cache TTL bounded (~10s) to avoid staleness on the next distinct send.

## 6. UX / flow notes

Invisible in the happy path; on recovery the user simply sees their pending tx (D08), not an error toast.

## 7. Acceptance criteria

- [ ] **AC-1** — Two rapid sends don't collide on nonce.
- [ ] **AC-2** — An "already pending" rejection results in polling the existing op, not a failure.
- [ ] **AC-3** — The recovered op appears once in activity.

## 8. Out of scope / non-goals

- Receipt polling mechanics — **G09**; op construction — **G01**.

## 9. Dependencies, risks & open questions

- **Risk:** stale nonce after a long pause — TTL + on-chain re-read guards this.
- **Open question:** None.

## 10. Source anchors

- `src/services/safe-transaction.ts:585-597,1226-1233` — nonce cache + already-pending recovery.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 52.
