# D07 · Token Detail Screen

| | |
|---|---|
| **Epic** | D — Balances, Portfolio & Activity |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | D02 |
| **Related** | D06, D08, E01, H01, H09 |

## 1. Summary

Tapping a token opens a detail screen showing its balance (atomic display, M03), fiat value (E01/E06),
a 7-day history chart (D06), token-scoped activity (D08), and quick actions (send H01 / receive H09).
It consolidates everything about one asset in one place.

## 2. Background & context

The home is activity-first (D01); per-asset depth belongs on a dedicated screen. Because pricing and
history are on-chain (E01/D06), the detail view is a composition of existing engines, not new data
sources.

## 3. Users & stories

- As a **user**, I want to see one token's balance, value, trend, and history, so that I understand that holding.
- As a **user**, I want to send/receive that token from its detail screen, so that actions are in context.

## 4. Functional requirements

- **FR-1** — Show token identity (logo/symbol, D04), balance (M03), and fiat value (E01/E06).
- **FR-2** — Render the 7-day history chart (D06) with graceful empty/partial states.
- **FR-3** — Show token-scoped activity (D08): sends/receives of this token.
- **FR-4** — Provide Send (H01, pre-selected token) and Receive (H09) actions.
- **FR-5** — Reflect rate-limit/cached states (F08) without scary banners.

## 5. Non-functional requirements

- **NFR-1** — Composes cached-then-fresh data (A06); no blank flash.
- **NFR-2** — Adapts to text scale (M02) and platform (A04).

## 6. UX / flow notes

Header = balance hero; body = chart + activity; footer/actions = send/receive. Uses design system primitives (M01–M04).

## 7. Acceptance criteria

- [ ] **AC-1** — Detail shows correct balance, fiat value, and 7-day chart for the token.
- [ ] **AC-2** — Send from detail pre-selects that token.
- [ ] **AC-3** — Missing price/history degrade honestly (null/partial), not fabricated.

## 8. Out of scope / non-goals

- Pricing math — **E01**; history — **D06**; send flow — **H01**.

## 9. Dependencies, risks & open questions

- **Risk:** None notable (pure composition).
- **Open question:** None.

## 10. Source anchors

- `src/screens/wallet/TokenDetailScreen.tsx`, `src/components/ui/BarChart.tsx`, `src/components/ui/AmountText.tsx`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 65, 79.
