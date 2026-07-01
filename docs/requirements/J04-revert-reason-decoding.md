# J04 · Revert-Reason Decoding

| | |
|---|---|
| **Epic** | J — Simulation & Safety Guards |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | J01 |
| **Related** | J02, I01, G03 |

## 1. Summary

When a simulation (J01) or pre-check reverts, Vela decodes the reason from **`Error(string)`** and
**`Panic(uint256)`** with bounds-checked length parsing, so the user sees *why* a transaction is
expected to fail. A bare "execution reverted" is **suppressed** (the UI already says "expected to
fail"), avoiding redundant noise.

## 2. Background & context

A transaction that will revert should be surfaced clearly, ideally with the contract's own reason. But a
contentless "execution reverted" adds nothing over "expected to fail," so it's suppressed. Bounds
checking prevents malformed revert data from causing errors.

## 3. Users & stories

- As a **user**, I want to know why a transaction will fail, so that I can fix the input or avoid signing.
- As a **user**, I don't want redundant/empty error noise, so that the message is useful.

## 4. Functional requirements

- **FR-1** — Decode `Error(string)` and `Panic(uint256)` revert reasons with bounds-checked length parsing.
- **FR-2** — Suppress a bare "execution reverted" (no added information over "expected to fail").
- **FR-3** — Surface the decoded reason in the signing sheet (I01) alongside the "expected to fail" state.
- **FR-4** — Malformed revert data must not crash decoding (defensive parsing).

## 5. Non-functional requirements

- **NFR-1** — Robust to arbitrary/hostile revert payloads.
- **NFR-2** — Complements gas estimation's refuse-doomed behavior (G03).

## 6. UX / flow notes

A predicted revert shows "expected to fail" + the decoded reason when meaningful. Panic codes are mapped to human descriptions where possible.

## 7. Acceptance criteria

- [ ] **AC-1** — An `Error(string)` revert surfaces its message.
- [ ] **AC-2** — A `Panic(uint256)` revert surfaces a decoded panic description.
- [ ] **AC-3** — A bare "execution reverted" is not shown redundantly.

## 8. Out of scope / non-goals

- Simulation engines — **J01**; gas refuse-doomed — **G03**.

## 9. Dependencies, risks & open questions

- **Risk:** non-standard revert encodings — fall back to a generic "expected to fail."
- **Open question:** None.

## 10. Source anchors

- `src/services/sim-assets.ts:181` — `Error(string)` / `Panic(uint256)` decoding + suppression.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 73.
