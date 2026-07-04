# E06 · Display-Currency Selection & Formatting Rules

| | |
|---|---|
| **Epic** | E — Pricing & Fiat |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | E05 |
| **Related** | M02, M03, E07 |

## 1. Summary

Users pick a **display currency** from a **data-driven list** (whatever the FX endpoint returns,
E05). Formatting follows explicit rules: **no cents when `|value| ≥ 100,000`** or for zero-decimal
currencies (JPY / KRW / IDR / VND / …) — e.g. `¥259,770`. Number grouping/decimals come from the
Intl-free locale formatter (M02); atomic display is `AmountText` (M03).

## 2. Background & context

Hermes' incomplete ICU makes `Intl` unreliable in React Native, so Vela uses explicit format presets
(M02) rather than `Intl.NumberFormat`. Currency conventions (which currencies show cents, grouping
style) must be encoded deliberately so amounts read natively in each locale.

## 3. Users & stories

- As a **user**, I want to choose my display currency and see it formatted naturally, so that money reads correctly.
- As a **JPY/VND user**, I don't want fake decimal places, so that amounts look right.

## 4. Functional requirements

- **FR-1** — Present a currency picker populated from the FX endpoint's supported list (E05) — data-driven, not hardcoded.
- **FR-2** — Persist the chosen display currency (A06); apply it app-wide.
- **FR-6** — Entry point is **Settings › Localization, first row** (N01 FR-1) with a live example
  subtitle (`USD · $1,234.56`) — the home hero carries only a passive `· CODE` unit label.
- **FR-7** — **First-launch seeding**: with no stored preference, derive the currency from the
  device region (`expo-localization getLocales()[i].currencyCode`; native NSLocale/java.util —
  no Hermes `Intl`). Commit the seed **only after a real rate resolves** (`resolveRate`, not the
  rate-1 fallback), else stay on USD and retry next launch. A stored key is never overwritten.
- **FR-3** — Drop decimals when `|value| ≥ 100,000` or for zero-decimal currencies (JPY/KRW/IDR/VND/etc.).
- **FR-4** — Use the Intl-free number formatter (M02) for grouping (comma_dot / dot_comma / space_comma / indian) with an `auto` device path.
- **FR-5** — Render amounts atomically via `AmountText` (M03).

## 5. Non-functional requirements

- **NFR-1** — Formatting is deterministic and locale-correct without `Intl`.
- **NFR-2** — Switching currency is restart-free and instant (uses cached rates, E05).

## 6. UX / flow notes

`CurrencySheet` for selection, opened from the Settings row (2026-07: the home-hero currency chip
was removed — a set-once preference doesn't earn hero real estate; the hero label shows
`Total balance · CODE` so `$` stays unambiguous). Compact suffixes use universal K/M/B/T (CJK
myriad avoided on purpose, M02).

## 7. Acceptance criteria

- [ ] **AC-1** — The currency list reflects the FX endpoint's supported currencies.
- [ ] **AC-2** — A JPY amount shows no decimals; a large USD amount ≥ 100k shows no cents.
- [ ] **AC-3** — Indian grouping renders lakh/crore correctly when selected.

## 8. Out of scope / non-goals

- FX rate fetching — **E05**; number-format presets — **M02**; atomic display — **M03**.

## 9. Dependencies, risks & open questions

- **Risk:** a currency the endpoint reports but lacks a decimals rule for — default sensibly and encode the exception.
- **Open question:** cross-device sync of currency choice is 🔜 (A06).

## 10. Source anchors

- `src/services/currency.ts:98-100` (decimal-drop rule), `src/services/currency-catalog.ts`, `src/components/ui/CurrencySheet.tsx`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 77, 80.
