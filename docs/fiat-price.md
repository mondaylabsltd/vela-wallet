# Fiat Price / Exchange-Rate System

A small, portable system for **converting USD-denominated values into a user-chosen
fiat currency and displaying them** — with a swappable rate source, a data-driven
currency list, and locale-aware formatting.

It was built for a crypto wallet (balances are priced in USD on-chain, shown in the
user's currency), but the design is generic: anything that has USD values and wants
to show them in EUR / JPY / VND / … can reuse it.

- **No API key required.** Default source is FOSS + self-hostable.
- **Provider-agnostic.** The rate endpoint is configurable and the parser accepts
  multiple response shapes.
- **Framework-light.** The only platform dependency is a key/value store for caching
  (React Native `AsyncStorage` here; swap for `localStorage`/Redis/etc.).

---

## 1. The one idea

Everything hangs off a single value:

```
rate(code) = how many units of `code` equal 1 USD     // USD → fiat multiplier
displayed  = usdAmount * rate(code)
```

So `getRate('JPY') === 155.3` means `$10` shows as `¥1,553`. USD's rate is `1`.

That's the entire contract. A rate provider just has to tell you "X per 1 USD".

---

## 2. Architecture

```
            ┌─────────────────────────────────────────────┐
 usdAmount  │  getRate(code)  → number  (USD→fiat)         │
   ──────►  │                                              │
            │   1. Chainlink fiat/USD feed  (optional,     │
            │      decentralized, on-chain)  ── if present │
            │   2. HTTP FX endpoint (configurable)         │
            │   3. fallback → 1                             │
            └─────────────────────────────────────────────┘
                              │
                              ▼
        formatFiat(usdAmount * rate, code, symbol) → "¥1,553"
```

Two tiers, tried in order (see `getRate` in [`currency.ts`](../src/services/currency.ts)):

1. **Chainlink fiat/USD feeds** — on-chain, decentralized, for the ~16 currencies that
   have a feed. *Optional* — drop this tier if you don't need on-chain rates.
2. **HTTP FX endpoint** — a configurable URL returning USD-based rates for everything
   else (and as the fallback for tier 1). This is the part most projects will use.
3. **Fallback** — return `1` so the UI always renders (shows the USD-equivalent number).

The **list of selectable currencies is driven by what the endpoint returns** — not a
hardcoded list. Point the endpoint at a broader provider and more currencies appear,
no code change.

---

## 3. The rate-provider contract (HTTP endpoint)

The endpoint must return **USD-based** rates in one of two shapes. Both are accepted by
`normalizeRates` (see [`fiat-fx.ts`](../src/services/fiat-fx.ts)), so providers are swappable:

**A. Array shape** (Frankfurter v2):
```json
[
  { "base": "USD", "quote": "EUR", "rate": 0.8646 },
  { "base": "USD", "quote": "VND", "rate": 26208 }
]
```

**B. Object shape** (open.er-api, Frankfurter v1, exchangerate.host, …):
```json
{ "rates": { "EUR": 0.8646, "VND": 26208 } }
```

`normalizeRates` turns either into `{ USD: 1, EUR: 0.8646, VND: 26208, … }`.

### Provider options

| Provider | URL | Currencies | Shape | Notes |
|---|---|---|---|---|
| **Frankfurter v2** (default) | `https://api.frankfurter.dev/v2/rates?base=USD` | ~160 (84 central banks, incl. VND) | array | FOSS, self-hostable, no key. **`?base=USD` is mandatory** — without it the base is EUR (wrong). |
| Frankfurter v1 | `https://api.frankfurter.dev/v1/latest?base=USD` | 29 (ECB only, **no VND**) | object | |
| open.er-api | `https://open.er-api.com/v6/latest/USD` | ~160 (incl. VND) | object | Free tier, not self-hostable. |
| self-hosted | your URL | your choice | either | Run Frankfurter via Docker, or proxy any source — just return one of the two shapes, USD-based. |

> ⚠️ **Always pin the base to USD.** If a provider defaults to another base, every
> conversion is silently wrong. Frankfurter needs `?base=USD`.

---

## 4. Public API

From [`currency.ts`](../src/services/currency.ts):

```ts
getRate(code: string): Promise<number>           // USD → code multiplier (1 for USD)
formatFiat(value, code, symbol): string          // already-converted value → display string
shouldShowDecimals(value, code): boolean         // false for yen/won/big sums
currencyMeta(code): { code, symbol, name }       // ISO metadata (catalog + fallback)

getSupportedCurrenciesSync(): Currency[]         // instant first paint (cached/static)
loadSupportedCurrencies(): Promise<Currency[]>   // full list driven by the endpoint

loadCurrency() / getCurrencyCode() / setCurrency(code)   // persisted user preference
```

From [`fiat-fx.ts`](../src/services/fiat-fx.ts) (the HTTP source):

```ts
fetchFxRates(): Promise<Record<string, number>>  // { USD:1, EUR:0.86, … }, cached + persisted
getFxRate(code): Promise<number | null>
getSupportedFxCodes(): Promise<string[]>         // drives the currency list
normalizeRates(data): Record<string, number> | null   // array OR {rates} → map
```

From [`fiat-rates.ts`](../src/services/fiat-rates.ts) (optional Chainlink tier):

```ts
getChainlinkRate(code): Promise<number | null>
isChainlinkFiat(code): boolean
FIAT_FEED_CODES: readonly string[]               // EUR, GBP, JPY, … ARS
```

### Typical usage in a component

```ts
const code = getCurrencyCode();              // e.g. "JPY"
const { symbol } = currencyMeta(code);       // "¥"
const rate = await getRate(code);            // 155.3
const label = formatFiat(usdAmount * rate, code, symbol);   // "¥1,553"
```

A `useDisplayCurrency()` hook ([`use-display-currency.ts`](../src/hooks/use-display-currency.ts))
bundles this as `{ code, symbol, rate, fmt }` where `fmt(usd) = formatFiat(usd*rate, …)`.

---

## 5. Display rules (`formatFiat`)

- **Decimals are dropped** when the amount is large (≥ 100,000 — cents are visual noise)
  or for **zero-decimal currencies** (JPY, KRW, VND, IDR, ISK, HUF, CLP, …) which have no
  commonly-used minor unit. See `ZERO_DECIMAL_CODES` / `shouldShowDecimals`.
- Grouping/decimal separators come from the app's number-format locale (see
  [`locale-format.ts`](../src/services/locale-format.ts)); replace `formatNumber` with
  `value.toLocaleString()` if you don't need that.

```
formatFiat(1428.2,  'ARS', 'AR$') → "AR$1,428.20"
formatFiat(259770,  'JPY', '¥')   → "¥259,770"       // no decimals
formatFiat(2460539, 'USD', '$')   → "$2,460,539"     // large → no cents
```

---

## 6. Currency list (data-driven)

- `currencyMeta(code)` resolves ISO `code → { name, symbol }` from a static catalog
  ([`currency-catalog.ts`](../src/services/currency-catalog.ts)), falling back to the bare
  code for anything unknown — so **any** code the endpoint returns still renders.
- `getSupportedCurrenciesSync()` returns a static base (~30 majors + Chainlink) for
  instant first paint; `loadSupportedCurrencies()` then expands it to **exactly what the
  endpoint returns** (USD + Chainlink + provider codes), ordered majors-first.
- This is why VND "just appears" when you switch to a provider that carries it.

---

## 7. Caching

- **HTTP rates**: cached in-memory **6h** (ECB/Frankfurter update ~once/business day) and
  persisted to the KV store for offline / first-paint. The cache is **keyed by endpoint
  URL**, so changing the endpoint refetches.
- **Chainlink**: feed *addresses* (ENS-resolved) cached 30 days (proxies are immutable);
  the computed rate map cached ~5 min + persisted.
- Never block the UI: on any failure, serve the last good cached value, else `1`.

---

## 8. Optional: Chainlink tier (on-chain rates)

Skippable. If you want decentralized rates for major currencies
([`fiat-rates.ts`](../src/services/fiat-rates.ts)):

- Feeds are addressed by ENS: `<ccy>-usd.data.eth` (e.g. `gbp-usd.data.eth`), resolved on
  Ethereum mainnet, then read with `latestRoundData()`.
- The feed answer is `<CCY>/USD` (USD per 1 unit), so the USD→fiat multiplier is
  `1 / answer`.
- **Feed decimals vary** (most 8, but e.g. PHP is 18) — read `decimals()` per feed; don't
  assume 8.
- Reads are batched via Multicall3 (ENS resolve + `latestRoundData`/`decimals` in a few
  `eth_call`s).

---

## 9. Porting checklist

1. Copy `fiat-fx.ts` (the HTTP source + `normalizeRates`) and `currency-catalog.ts`.
2. Replace `AsyncStorage` with your KV store (2 calls: get/set a JSON blob).
3. Decide the default endpoint (Frankfurter v2 recommended) and make it configurable.
4. Take `getRate` + `formatFiat` + `shouldShowDecimals` from `currency.ts`. Drop the
   Chainlink branch if you don't need on-chain rates — `getRate` becomes just
   "endpoint → 1".
5. (Optional) Take `currencyMeta` + `loadSupportedCurrencies` for a data-driven picker.

### Minimal standalone version (no Chainlink, no RN)

```ts
const ENDPOINT = 'https://api.frankfurter.dev/v2/rates?base=USD';
let cache: { at: number; rates: Record<string, number> } | null = null;
const TTL = 6 * 60 * 60 * 1000;

function normalizeRates(data: any): Record<string, number> | null {
  const out: Record<string, number> = { USD: 1 };
  if (Array.isArray(data)) {
    for (const r of data) { const n = Number(r?.rate); if (r?.quote && n > 0) out[String(r.quote).toUpperCase()] = n; }
  } else if (data?.rates) {
    for (const [k, v] of Object.entries(data.rates)) { const n = Number(v); if (n > 0) out[k.toUpperCase()] = n; }
  }
  return Object.keys(out).length > 1 ? out : null;
}

export async function getRate(code: string): Promise<number> {
  if (code === 'USD') return 1;
  if (!cache || Date.now() - cache.at > TTL) {
    try {
      const rates = normalizeRates(await (await fetch(ENDPOINT)).json());
      if (rates) cache = { at: Date.now(), rates };
    } catch { /* keep stale */ }
  }
  return cache?.rates[code.toUpperCase()] ?? 1;
}

const ZERO_DEC = new Set(['JPY', 'KRW', 'VND', 'IDR', 'ISK', 'HUF', 'CLP']);
export function formatFiat(value: number, code: string, symbol: string): string {
  const digits = ZERO_DEC.has(code.toUpperCase()) || Math.abs(value) >= 1e5 ? 0 : 2;
  return symbol + value.toLocaleString('en-US', { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

// usage:  formatFiat(usd * await getRate('JPY'), 'JPY', '¥')
```

---

## 10. Gotchas

- **`?base=USD`** — pin the base, or conversions are silently wrong.
- **Zero-decimal currencies** — `¥1,553`, not `¥1,553.00`.
- **Provider coverage differs** — the *list* of currencies follows the endpoint; ECB-only
  sources lack VND and many others. The catalog/formatting handle any code regardless.
- **Cache by endpoint URL** — so swapping providers takes effect immediately.
- **Rounding for tiny prices** — a `$0.0001` token at 2 decimals shows `$0.00`. Use more
  precision (or compact notation) where token *prices* matter, not just balances.
