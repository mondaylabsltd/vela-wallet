# F06 · Read-Only dApp RPC Gate (Concurrency / Queue)

| | |
|---|---|
| **Epic** | F — Networks & RPC Infrastructure |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F03 |
| **Related** | K04, B02, F04 |

## 1. Summary

A **defense-in-depth gate** bounds read-only RPC requests coming from a connected dApp: at most **6
concurrent** reads, up to **512 queued**, identical concurrent keys **collapse to one execution** (never
cached across time), and excess is rejected with a retryable `-32005`. **Signing requests bypass the
gate** so a read-flood can never starve a user's confirmation.

## 2. Background & context

A hostile or buggy dApp can spray read requests. Without a gate, that could exhaust the RPC pool (F03)
or delay the user's own signing. The gate isolates dApp reads and explicitly prioritizes user signing.

## 3. Users & stories

- As a **user**, I want my signing to always go through, even if a dApp is hammering reads, so that I'm never blocked.
- As a **user**, I want a runaway dApp's reads throttled, so that the wallet stays responsive.

## 4. Functional requirements

- **FR-1** — Cap dApp read concurrency at 6; queue up to 512; reject excess with retryable `-32005`.
- **FR-2** — Collapse identical concurrent request keys to a single execution (dedupe in-flight, not a time cache).
- **FR-3** — **Signing requests bypass the gate** entirely (B02/K05).
- **FR-4** — Gated reads still route through the resilient pool (F03/F04).

## 5. Non-functional requirements

- **NFR-1** — Deduping never returns a stale cached value across time — only collapses truly concurrent identical reads.
- **NFR-2** — Backpressure is explicit (`-32005` retryable), not a silent drop.

## 6. UX / flow notes

No direct UI; protects the signing sheet (I01) responsiveness under dApp read load.

## 7. Acceptance criteria

- [ ] **AC-1** — 7 concurrent identical reads execute as fewer underlying calls (collapse) within the 6-slot cap.
- [ ] **AC-2** — Excess beyond 512 queued returns retryable `-32005`.
- [ ] **AC-3** — A signing request completes even under a saturated read gate.

## 8. Out of scope / non-goals

- Capability advertisement — **K04**; pool scoring — **F03**.

## 9. Dependencies, risks & open questions

- **Risk:** queue limits too low for legit heavy dApps — 512 chosen as a balance; revisit if needed.
- **Open question:** None.

## 10. Source anchors

- `src/services/readonly-rpc-gate.ts` — concurrency/queue/dedupe + signing bypass.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 59.
