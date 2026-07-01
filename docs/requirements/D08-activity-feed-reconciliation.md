# D08 · Activity Feed & Transaction Reconciliation

| | |
|---|---|
| **Epic** | D — Balances, Portfolio & Activity |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G09 |
| **Related** | D05, L01, D01, D07 |

## 1. Summary

The activity feed shows sends, receives (D05), and dApp operations (L01) in one timeline, reconciling
**optimistic/pending** entries with confirmed on-chain results. Pending sends are written immediately
and patched to confirmed/failed as receipts arrive (G09), so nothing is lost on reload.

## 2. Background & context

Users need a trustworthy record of "what happened." Because sends are async 4337 UserOps and deposits
come from log polling (D05), the feed must merge multiple sources and never show a stale pending as
lost or a confirmed as pending.

## 3. Users & stories

- As a **user**, I want a single timeline of my activity, so that I can see what I sent and received.
- As a **user**, I want a just-submitted tx to appear immediately as pending, so that I'm reassured it went through.

## 4. Functional requirements

- **FR-1** — Aggregate outgoing sends (H01), incoming transfers (D05), and dApp ops (L01) into one chronological feed per account.
- **FR-2** — Write a **pending** entry at submit time; reconcile to confirmed/failed via receipt polling (G09).
- **FR-3** — Re-poll still-pending entries newer than 24h on mount (L01) so a closed/reloaded app recovers state.
- **FR-4** — Distinguish "dropped" (terminal) from "still pending" vs "bundler unreachable" (G09) in the row status.
- **FR-5** — Support token-scoped filtering for token detail (D07).

## 5. Non-functional requirements

- **NFR-1** — Reconciliation is idempotent; a confirmed entry never reverts to pending.
- **NFR-2** — Feed renders from cache first (A06), then updates.

## 6. UX / flow notes

`ActivityRow` + `TxStatusBadge` show direction, counterparty (resolved name via H02 where available), amount (M03), and status. Tapping opens `TransactionDetailSheet`.

## 7. Acceptance criteria

- [ ] **AC-1** — A new send appears as pending immediately and becomes confirmed on receipt.
- [ ] **AC-2** — Reloading mid-pending recovers and re-polls the entry.
- [ ] **AC-3** — A dropped op shows a terminal "dropped" state, not an infinite spinner.

## 8. Out of scope / non-goals

- Receipt polling internals — **G09**; deposit detection — **D05**; dApp op persistence — **L01**.

## 9. Dependencies, risks & open questions

- **Risk:** activity completeness for native/internal deposits — forward FR in D05.
- **Open question:** retention window for historical activity.

## 10. Source anchors

- `src/services/activity.ts`, `src/services/tx-reconciler.ts`, `src/components/ui/ActivityRow.tsx`, `src/components/ui/TxStatusBadge.tsx`.
- `src/models/dapp-connection.tsx:497-543,715-734` — pending persistence & re-poll.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 53, 86.
