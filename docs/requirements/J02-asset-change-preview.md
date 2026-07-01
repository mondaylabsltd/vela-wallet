# J02 · Asset-Change Preview (`BalanceChangePreview`)

| | |
|---|---|
| **Epic** | J — Simulation & Safety Guards |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | J01 |
| **Related** | J03, I01, I07 |

## 1. Summary

The simulation result (J01) is rendered as a **balance-change preview** — "what moved" — shown in the
signing sheet (I01) and in read-only history replay (L03): tokens/native in and out, with amounts using
on-chain decimals (I07). Confidence for *received* amounts is governed by the asymmetric trust model
(J03).

## 2. Background & context

A raw simulation is only useful if presented clearly. Showing net asset deltas ("you send X, you
receive Y") is the most intuitive way to answer "what will this do." Shared between the live sheet and
replay so the record matches what the user saw.

## 3. Users & stories

- As a **user**, I want to see what tokens will leave and enter my wallet, so that I understand the transaction's effect.
- As a **user reviewing history**, I want the same preview I saw when signing, so that the record is faithful (L03).

## 4. Functional requirements

- **FR-1** — Render net asset changes (native + tokens, in/out) from the simulation (J01).
- **FR-2** — Amounts use on-chain decimals + BigInt (I07) and atomic display (M03).
- **FR-3** — Apply the asymmetric trust model (J03): confident vs "unverified" received amounts.
- **FR-4** — Shared component used by the live signing sheet (I01) and read-only replay (L03).
- **FR-5** — When there's no simulation (`null`, J01), show "no preview available," not an empty/zero delta.

## 5. Non-functional requirements

- **NFR-1** — Never renders a fabricated change; absence of data is explicit.
- **NFR-2** — Localized (M05) and text-scale-aware (M02).

## 6. UX / flow notes

`BalanceChangePreview` sits beside the decoded intent (I01). Received amounts from unknown tokens degrade to "unverified" (J03).

## 7. Acceptance criteria

- [ ] **AC-1** — A swap shows the correct out/in tokens and amounts.
- [ ] **AC-2** — An unknown received token shows "unverified" rather than a confident amount (J03).
- [ ] **AC-3** — With no simulation, the preview shows "no preview available."

## 8. Out of scope / non-goals

- Simulation engines — **J01**; trust model — **J03**; decimals — **I07**.

## 9. Dependencies, risks & open questions

- **Risk:** spoofed received amounts — bounded by J03.
- **Open question:** None.

## 10. Source anchors

- `src/components/signing/BalanceChangePreview.tsx`, `src/services/sim-assets.ts`, `src/services/tx-simulation.ts`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 25.
