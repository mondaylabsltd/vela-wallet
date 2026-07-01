# G09 · Receipt Polling (Unconfirmed vs Bundler-Unreachable)

| | |
|---|---|
| **Epic** | G — Transaction Engine (ERC-4337) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G01 |
| **Related** | G08, D08, L01 |

## 1. Summary

After submission, Vela polls for the UserOp receipt with **adaptive 1s→3s backoff** and a **120s
default timeout**. It carefully distinguishes **"unconfirmed"** from **"bundler unreachable,"** treats
`success === false` as a terminal **"dropped,"** and **never implies failure** for an op that may still
land — the messaging depends on whether the bundler was ever reachable.

## 2. Background & context

The worst wallet UX is telling a user their tx failed when it actually succeeded (or vice versa).
Honest status requires separating "we can't reach the bundler to check" from "the op is confirmed
failed." This feeds the activity feed's reconciliation (D08) and pending-op recovery (L01).

## 3. Users & stories

- As a **user**, I want accurate status, so that I never think a successful tx failed (or a failed one succeeded).
- As a **user during a bundler hiccup**, I want "still checking," not a false "failed."

## 4. Functional requirements

- **FR-1** — Poll the receipt with adaptive **1s→3s** backoff, **120s** default timeout.
- **FR-2** — Treat `success === false` as terminal **dropped**.
- **FR-3** — Differentiate messaging: **bundler reachable but unconfirmed** vs **bundler unreachable** — never assert failure for a possibly-landing op.
- **FR-4** — On timeout, leave the op **pending** (recoverable via L01/D08), not failed.
- **FR-5** — Emit the final status to reconciliation (D08).

## 5. Non-functional requirements

- **NFR-1** — Status transitions are monotonic where possible (pending → confirmed/dropped), never confirmed → pending.
- **NFR-2** — Polling shares the resilient RPC/bundler path (F07/F04).

## 6. UX / flow notes

`TransactionReceipt` / `TxStatusBadge` show pending/confirmed/dropped/checking. A pending op past 24h is re-polled on mount (L01).

## 7. Acceptance criteria

- [ ] **AC-1** — A confirmed op shows confirmed; a `success===false` op shows dropped.
- [ ] **AC-2** — With the bundler unreachable, the op stays "checking/pending," not "failed."
- [ ] **AC-3** — A 120s timeout leaves the op recoverable, not marked failed.

## 8. Out of scope / non-goals

- Nonce/pending recovery — **G08**; feed rendering — **D08**; dApp op persistence — **L01**.

## 9. Dependencies, risks & open questions

- **Risk:** long-pending ops need re-poll on relaunch (L01) so they aren't stuck.
- **Open question:** configurable timeout for slow chains.

## 10. Source anchors

- `src/services/safe-transaction.ts:1459-1532` — receipt polling + reachable/unreachable distinction.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 53.
