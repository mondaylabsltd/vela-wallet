# M03 · `AmountText` Atomic-Number Display

| | |
|---|---|
| **Epic** | M — Design System & UI Primitives |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | M02 |
| **Related** | M01, E06, D01, D07 |

## 1. Summary

`AmountText` renders money as **one never-wrapping unit** (Apple Wallet / Cash App style) via a 3-step
cascade: **fit-to-width shrink** → **compact-notation floor** (`$1,234,567.89 → $1.23M` once shrinking
would go illegible) → **two-tier typography** (large integer head, subordinated decimal tail). Width is
**estimated** (`width / (len·0.6em)`), not left to flaky `adjustsFontSizeToFit`.

## 2. Background & context

Balances vary wildly in magnitude; naive text wraps or truncates unattractively, and native
auto-shrink is unreliable across platforms. A deterministic estimate-then-shrink-then-compact cascade
gives a premium, always-legible money display consistent with the "big-tech UX" bar (M04).

## 3. Users & stories

- As a **user**, I want my balance to always display as one clean figure, so that it looks polished at any magnitude.
- As a **user with a huge or tiny balance**, I want it legible, not truncated, so that I can read it.

## 4. Functional requirements

- **FR-1** — Render an amount as a single, **non-wrapping** unit.
- **FR-2** — Cascade: (1) fit-to-width shrink; (2) compact-notation floor to K/M/B/T (M02) once shrinking would be illegible; (3) two-tier typography (large integer, subordinated decimals).
- **FR-3** — Estimate width via `width / (len·0.6em)` rather than relying on `adjustsFontSizeToFit`.
- **FR-4** — Respect the display currency + formatting rules (E06) and text scale (M02).

## 5. Non-functional requirements

- **NFR-1** — Deterministic across iOS/Android/web (A04); no platform-specific auto-shrink dependence.
- **NFR-2** — Never wraps or clips the amount.

## 6. UX / flow notes

Used for the balance hero (D01), token detail (D07), and signing amounts (I07). Two-tier typography subordinates cents for a clean headline.

## 7. Acceptance criteria

- [ ] **AC-1** — A very large balance compacts to `$1.23M`-style rather than overflowing.
- [ ] **AC-2** — A normal balance shows full precision with two-tier typography, never wrapping.
- [ ] **AC-3** — Rendering is consistent across platforms at all text scales.

## 8. Out of scope / non-goals

- Currency rules — **E06**; number presets — **M02**; on-chain decimals — **I07**.

## 9. Dependencies, risks & open questions

- **Risk:** the 0.6em width estimate can vary by font — tuned for Plus Jakarta Sans (A05).
- **Open question:** None.

## 10. Source anchors

- `src/components/ui/AmountText.tsx`, `docs/dynamic-amount-display.md`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 79.
