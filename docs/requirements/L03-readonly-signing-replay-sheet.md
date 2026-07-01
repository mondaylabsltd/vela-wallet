# L03 · Read-Only Signing Replay Sheet

| | |
|---|---|
| **Epic** | L — Connection Activity & History |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | L02, I01 |
| **Related** | J02, K05 |

## 1. Summary

The Connections panel **replays a past dApp operation through the same `<SigningSheet>`** used at sign
time, but in **read-only** mode: the original intent, decoded fields, and "what moved" (the sign-time
simulation, L02) — with **no re-simulation** and no ability to sign. One render path (I01) for live and
historical signing keeps the record faithful.

## 2. Background & context

Reusing the exact live signing component for replay guarantees that history looks like what the user
saw, and avoids maintaining a second renderer that could drift. Read-only mode disables confirmation and
uses stored data only.

## 3. Users & stories

- As a **user**, I want to reopen a past signature and see exactly what I approved, so that I can verify my history.
- As a **user**, I want replay to be clearly read-only, so that I can't accidentally re-sign.

## 4. Functional requirements

- **FR-1** — Render a stored record (L02) via the same `<SigningSheet>` as live signing (I01), in **read-only** mode.
- **FR-2** — Show intent, decoded fields, and the **sign-time** balance-change preview (J02) — **no re-simulation**.
- **FR-3** — Disable all signing/confirm affordances in replay.
- **FR-4** — Reachable from the Connections panel / event detail sheet.

## 5. Non-functional requirements

- **NFR-1** — Single render path (prod live + replay + harness) — behavior can't diverge (I01).
- **NFR-2** — Replay works offline (uses stored record, not the network).

## 6. UX / flow notes

`SigningReplaySheet` / `ConnectionEventDetailSheet`. Visually distinct "read-only / history" affordance; no biometric, no confirm.

## 7. Acceptance criteria

- [ ] **AC-1** — Replaying a record shows the same intent/fields/preview as at sign time.
- [ ] **AC-2** — Replay never triggers a new simulation or allows signing.
- [ ] **AC-3** — Replay renders with the network unavailable (offline).

## 8. Out of scope / non-goals

- Record capture — **L02**; live signing — **I01/K05**.

## 9. Dependencies, risks & open questions

- **Risk:** stored record schema drift vs the live sheet — keep the single render path in lockstep.
- **Open question:** None.

## 10. Source anchors

- `src/components/ui/SigningReplaySheet.tsx`, `src/components/ui/ConnectionEventDetailSheet.tsx`.
- memory `project_connection_activity_replay`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 87.
