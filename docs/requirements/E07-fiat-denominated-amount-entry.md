# E07 · Fiat-Denominated Amount Entry

| | |
|---|---|
| **Epic** | E — Pricing & Fiat |
| **Status** | 🚧 In progress (branch `feat/contacts-groups-payroll-batch`) |
| **Owner** | Shelchin |
| **Depends on** | E05, E06 |
| **Related** | E01, H01, H08 |

## 1. Summary

Let users **enter an amount in fiat** ("send $50 of ETH") and have Vela convert it to the correct token
amount at the current price. The conversion is a small, pure module (`fiat-convert.ts`) that turns a
fiat figure + token price + decimals into a token base-unit string, used by Send (H01) and the payroll
batch importer (H08, where table amounts are fiat by default).

## 2. Background & context

People think in money, not tokens. Fiat-denominated entry removes the mental math of "how much ETH is
$50." It composes on-chain pricing (E01) and FX (E05) with careful decimal handling so no rounding
drift creeps into what's actually sent.

## 3. Users & stories

- As a **user**, I want to type "$50" and send that much of a token, so that I don't compute token amounts by hand.
- As a **payer**, I want payroll rows in fiat to convert correctly, so that everyone gets the intended value (H08).

## 4. Functional requirements

- **FR-1** — `tokenPriceInFiat(priceUsd, usdToFiatRate)` computes the token's price in the display currency (E05/E06).
- **FR-2** — `fiatToTokenAmount(fiat, priceInFiat, decimals)` returns the token amount as a base-unit-safe string.
- **FR-3** — `resolveTokenAmount(...)` supports entering either side (fiat or token) and reflecting the other.
- **FR-4** — When price is `null` (E01), fiat entry is disabled/guarded (can't convert without a price).
- **FR-5** — Used by Send (H01) and the payroll importer (H08) uniformly.

## 5. Non-functional requirements

- **NFR-1** — BigInt/base-unit math; no float drift, correct at the token's real decimals (D04).
- **NFR-2** — Pure and synchronous; unit-testable in isolation.

## 6. UX / flow notes

Amount field offers a fiat/token toggle; the non-active unit shows as a live secondary value. If price is unavailable, the toggle disables with an explanation.

## 7. Acceptance criteria

- [ ] **AC-1** — Entering a fiat amount yields the correct token base units for a 6- and 18-decimal token.
- [ ] **AC-2** — With a `null` price, fiat entry is disabled gracefully.
- [ ] **AC-3** — Payroll fiat rows (H08) convert identically to single-send fiat entry.

## 8. Out of scope / non-goals

- Price derivation — **E01**; FX rates — **E05**; batch send — **H07/H08**.

## 9. Dependencies, risks & open questions

- **Risk:** stale price at submit time — re-check price at confirm; small deviations acceptable, large ones warn.
- **Open question:** slippage/price-move tolerance policy for fiat entry.

## 10. Source anchors

- `src/services/fiat-convert.ts:26` (`tokenPriceInFiat`), `:39` (`fiatToTokenAmount`), `:51` (`resolveTokenAmount`).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 28, 75.
