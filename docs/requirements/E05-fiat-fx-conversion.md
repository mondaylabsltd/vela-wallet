# E05 · Fiat FX Conversion (Configurable Endpoint + Optional Chainlink ENS)

| | |
|---|---|
| **Epic** | E — Pricing & Fiat |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A06 |
| **Related** | E06, E07, N02 |

## 1. Summary

USD amounts are converted to a user's chosen fiat currency via a **configurable, no-API-key FX
endpoint** (default **Frankfurter v2**, `?base=USD`, FOSS, Docker-self-hostable, ~160 currencies incl.
VND). Contract: `rate(code)` = units per 1 USD; `displayed = usdAmount × rate`. An **optional
decentralized mode** resolves Chainlink fiat feeds via ENS (`<ccy>-usd.data.eth`). The cache is keyed
by endpoint URL so swapping providers refetches instantly.

## 2. Background & context

Users think in their local currency. Vela keeps this trustless-ish and self-hostable: the FX endpoint
is swappable (N02), needs no API key, and defaults to an open-source provider. For maximal
decentralization, Chainlink ENS-addressed feeds are an alternative source.

## 3. Users & stories

- As a **non-USD user**, I want balances in my currency, so that values are meaningful to me.
- As a **self-hoster**, I want to point FX at my own endpoint, so that I depend on no one.

## 4. Functional requirements

- **FR-1** — Default FX = Frankfurter v2 (`?base=USD`); `rate(code)` returns units per 1 USD; `displayed = usd × rate`.
- **FR-2** — The FX endpoint is configurable (N02); a candidate is validated before use (health/shape).
- **FR-3** — Cache is **keyed by endpoint URL** so swapping providers triggers an immediate refetch (A06).
- **FR-4** — Optional Chainlink ENS mode: resolve `<ccy>-usd.data.eth` via the mainnet ENS registry, read `latestRoundData()`, with per-feed `decimals()` (e.g. PHP uses 18, not 8); ~16 codes.
- **FR-5** — The selectable-currency list is **data-driven** — whatever the endpoint returns "just appears" (E06).

## 5. Non-functional requirements

- **NFR-1** — No API key; works with a self-hosted Frankfurter (Docker).
- **NFR-2** — A failed FX fetch keeps the last cached rates rather than dropping to raw USD silently.

## 6. UX / flow notes

Currency selection UI (E06) lists whatever the endpoint supports. Switching endpoints in Settings → Advanced (N02) refetches immediately.

## 7. Acceptance criteria

- [ ] **AC-1** — With the default endpoint, USD converts to the selected currency correctly.
- [ ] **AC-2** — Swapping the FX endpoint refetches rates immediately (endpoint-keyed cache).
- [ ] **AC-3** — Chainlink ENS mode reads a fiat feed with correct per-feed decimals.

## 8. Out of scope / non-goals

- Currency selection/formatting UI — **E06**; fiat-amount entry — **E07**.

## 9. Dependencies, risks & open questions

- **Risk:** a broken/hostile endpoint — validated on entry (N02) and cached-last-known on failure.
- **Open question:** default set of Chainlink-ENS currencies vs Frankfurter coverage.

## 10. Source anchors

- `src/services/fiat-fx.ts`, `src/services/fiat-rates.ts`, `src/services/currency.ts`.
- `docs/fiat-price.md`; `docs/CONTENT-SOURCE-100-CLUES.md` — clues 75, 76.
