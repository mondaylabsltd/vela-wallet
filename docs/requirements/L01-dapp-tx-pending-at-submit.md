# L01 · dApp Tx Persisted Pending-At-Submit

| | |
|---|---|
| **Epic** | L — Connection Activity & History |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | K07 |
| **Related** | L02, G09, D08 |

## 1. Summary

The moment the bundler accepts a dApp operation, a **`pending` record is written** (then patched
in-place to confirmed/failed as the receipt arrives, G09) — so nothing is lost if the signing sheet
closes or the app reloads. A mount-time effect **re-polls any still-pending op newer than 24h**, so a
closed/reloaded app recovers and reconciles rather than orphaning a transaction.

## 2. Background & context

dApp ops are async and the sheet may close before confirmation. Persisting at submit-time (not at
receipt) guarantees the op is tracked no matter what happens to the UI, and the 24h re-poll closes the
loop after an app restart.

## 3. Users & stories

- As a **user**, I want a dApp transaction I submitted to be remembered even if I close the sheet, so that I never lose track of it.
- As a **user**, I want a pending op to resolve after I reopen the app, so that it doesn't hang forever.

## 4. Functional requirements

- **FR-1** — Write a `pending` record **at submit time** (bundler-accept), before the receipt wait (G09).
- **FR-2** — Patch the record **in place** to confirmed/failed as the receipt resolves.
- **FR-3** — On mount, **re-poll** any still-pending op **newer than 24h**.
- **FR-4** — Surface these ops in the activity feed (D08) and the Connections panel.
- **FR-5** — Persistence survives sheet-close and app reload (A06).

## 5. Non-functional requirements

- **NFR-1** — Reconciliation is idempotent (no duplicate records; a confirmed op never reverts to pending).
- **NFR-2** — Bounded re-poll window (24h) to avoid unbounded background work.

## 6. UX / flow notes

Pending dApp ops appear immediately in activity (D08) with a pending badge, then update. Their full record is replayable (L02/L03).

## 7. Acceptance criteria

- [ ] **AC-1** — Submitting a dApp op writes a pending record before the receipt.
- [ ] **AC-2** — Closing the sheet / reloading does not lose the op; it reconciles.
- [ ] **AC-3** — A pending op < 24h old re-polls on mount and resolves.

## 8. Out of scope / non-goals

- The replay record contents — **L02**; replay UI — **L03**; receipt polling — **G09**.

## 9. Dependencies, risks & open questions

- **Risk:** ops older than 24h left pending — acceptable; user can re-check via explorer.
- **Open question:** None.

## 10. Source anchors

- `src/models/dapp-connection.tsx:497-543,715-734` — pending-at-submit + 24h re-poll.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 86.
