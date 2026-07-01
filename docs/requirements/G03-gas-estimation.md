# G03 · Gas Estimation (Inflation, Floors, Refuse-Doomed Ops)

| | |
|---|---|
| **Epic** | G — Transaction Engine (ERC-4337) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G01 |
| **Related** | G04, G05, H01 |

## 1. Summary

Gas limits are estimated then **inflated 1.5× with hard floors** (300k deployed / 2M undeployed
verification, 100k call, +10k preVerificationGas), plus **static L2 bumps** (+600k Arbitrum, +150k
OP-Stack). If estimation **fails and calldata > 1024 bytes**, the send is **refused** (retryable) rather
than submitting an op likely to silently fail on-chain.

## 2. Background & context

Under-estimated 4337 ops fail during EntryPoint validation/execution, often burning gas for nothing.
Inflation + floors give headroom; refusing a doomed complex op (large calldata that won't estimate) is
safer than a confident submission that reverts. L2s have extra overhead that a naive estimate misses.

## 3. Users & stories

- As a **user**, I want my transaction to have enough gas to actually land, so that it doesn't silently fail.
- As a **user**, I'd rather be told "try again" than pay for an op that can't succeed.

## 4. Functional requirements

- **FR-1** — Multiply estimated limits by **1.5×** with floors: verification 300k (deployed) / 2M (undeployed, G01), call 100k, preVerificationGas +10k.
- **FR-2** — Add static L2 bumps: **+600k Arbitrum**, **+150k OP-Stack**.
- **FR-3** — If estimation fails **and** calldata > 1024 bytes, **refuse** the send (retryable error), don't submit.
- **FR-4** — Feed the final limits into UserOp fields (G01) and pricing (G04/G05).

## 5. Non-functional requirements

- **NFR-1** — Estimation runs over the resilient pool (F03/F07).
- **NFR-2** — Floors chosen to cover deploy+execute worst cases without gross over-charging.

## 6. UX / flow notes

The user sees a final fee (G05); estimation is invisible unless it refuses, which surfaces a clear retryable message.

## 7. Acceptance criteria

- [ ] **AC-1** — An undeployed-account op uses the 2M verification floor.
- [ ] **AC-2** — An Arbitrum op includes the +600k bump.
- [ ] **AC-3** — A failed estimate with >1024-byte calldata refuses instead of submitting.

## 8. Out of scope / non-goals

- Gas price/tiers — **G04**; fee display/cap — **G05**.

## 9. Dependencies, risks & open questions

- **Risk:** floors/bumps drift as chains evolve — revisit per chain.
- **Open question:** None.

## 10. Source anchors

- `src/services/safe-transaction.ts:421-546` — inflation, floors, L2 bumps, refuse-doomed.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 50.
