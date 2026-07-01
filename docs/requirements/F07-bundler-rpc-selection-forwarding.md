# F07 · Bundler RPC Selection & `X-Rpc-Url` Forwarding

| | |
|---|---|
| **Epic** | F — Networks & RPC Infrastructure |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F03, G01 |
| **Related** | G04, G06, G10 |

## 1. Summary

When calling the Vela bundler, Vela **picks the fastest RPC for the chain and forwards it** via an
`X-Rpc-Url` header, so the bundler reaches the chain through a known-good endpoint. `poolBundlerCall`
races all endpoints with a 3s `eth_chainId` ping, caches the winner per chain (~1h), and passes it
along — **critical for Tempo's in-band reimbursement** to reach the right bundler EOA (G10).

## 2. Background & context

The bundler needs a working RPC for the target chain; rather than trusting its default, Vela shares the
endpoint it just measured as fastest (F03). This keeps bundler submission on the same resilient path as
reads and ensures chain-specific flows (Tempo, G10) hit the correct node.

## 3. Users & stories

- As a **user**, I want tx submission to use a fast, working RPC, so that sends are reliable.
- As a **Tempo user**, I want the bundler to reach the right node for stablecoin-gas reimbursement, so that my tx settles.

## 4. Functional requirements

- **FR-1** — `poolBundlerCall` races endpoints with a 3s `eth_chainId` ping and selects the winner.
- **FR-2** — Cache the winning RPC per chain (~1h); reuse for subsequent bundler calls.
- **FR-3** — Forward the chosen RPC to the bundler via the `X-Rpc-Url` header.
- **FR-4** — Ensure Tempo's in-band `pathUSD` reimbursement (G10) targets the correct bundler EOA via this forwarded RPC.
- **FR-5** — Fall back/re-race if the cached winner degrades (F04).

## 5. Non-functional requirements

- **NFR-1** — Bundler routing shares the pool's resilience (F03/F04).
- **NFR-2** — The 3s ping bound keeps selection snappy.

## 6. UX / flow notes

No direct UI; underpins send (H01) and dApp tx (K07) submission reliability.

## 7. Acceptance criteria

- [ ] **AC-1** — A bundler call forwards a reachable `X-Rpc-Url` for the chain.
- [ ] **AC-2** — The per-chain winner is cached and reused within the TTL.
- [ ] **AC-3** — Tempo submission reaches the correct EOA via the forwarded RPC.

## 8. Out of scope / non-goals

- Gas pricing — **G04**; gas account — **G06**; Tempo gas mechanics — **G10**.

## 9. Dependencies, risks & open questions

- **Risk:** cross-repo coupling with the bundler's header handling — keep in sync (G07).
- **Open question:** None.

## 10. Source anchors

- `src/services/rpc-pool.ts:587-630,747-870` — `poolBundlerCall` + `X-Rpc-Url` forwarding.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 57.
