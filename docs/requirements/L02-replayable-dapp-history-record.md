# L02 · Replayable dApp History Record (Bounded 24KB)

| | |
|---|---|
| **Epic** | L — Connection Activity & History |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | L01 |
| **Related** | L03, I01, J02 |

## 1. Summary

Every approved dApp operation is captured as a **replayable history record**: the **original request**
(bounded to **24KB**, progressively clipping calldata) plus the **sign-time simulation** (J02). This
lets the Connections panel replay exactly what the user saw when they signed — no re-simulation, no
guessing — via the read-only signing sheet (L03).

## 2. Background & context

Users should be able to audit what they signed. Storing the original request + the simulation captured
at sign time makes history faithful and independent of current chain state. The 24KB bound (with
progressive calldata clipping) keeps storage sane for large requests.

## 3. Users & stories

- As a **user**, I want to review a past dApp signature exactly as I saw it, so that I can audit my activity.
- As a **user**, I don't want history to re-simulate and show different results, so that the record is trustworthy.

## 4. Functional requirements

- **FR-1** — On approval, capture the **original request** and the **sign-time simulation** (J02) as a history record.
- **FR-2** — Bound the record to **24KB**, progressively clipping calldata when needed (largest/last data trimmed first).
- **FR-3** — Store enough to render the full signing sheet read-only (intent, fields, "what moved") without re-simulation (L03).
- **FR-4** — Associate records with the connection/account (K03/C05).

## 5. Non-functional requirements

- **NFR-1** — Deterministic replay: the record renders identically regardless of current chain state.
- **NFR-2** — Storage-bounded (24KB/record); no unbounded growth.

## 6. UX / flow notes

Records power the Connections panel history and the event detail sheet. Replay opens the read-only sheet (L03).

## 7. Acceptance criteria

- [ ] **AC-1** — Approving a dApp op stores a record with request + sign-time simulation.
- [ ] **AC-2** — A large request is clipped to ≤24KB but still renders its intent/fields.
- [ ] **AC-3** — Replaying a record shows the sign-time simulation, not a fresh one.

## 8. Out of scope / non-goals

- Pending persistence — **L01**; replay UI — **L03**; live simulation — **J01/J02**.

## 9. Dependencies, risks & open questions

- **Risk:** clipping could drop detail — clip calldata last and mark it clipped.
- **Open question:** retention count/window for history records.

## 10. Source anchors

- `src/services/dapp-history.ts` — record capture + 24KB bound + calldata clipping.
- memory `project_connection_activity_replay`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 87.
