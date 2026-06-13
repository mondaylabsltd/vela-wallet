/**
 * Display-currency preference + USD→fiat rate.
 *
 * Rates resolve through a source abstraction (see `getRate`):
 *   1. Chainlink fiat/USD feeds on Ethereum mainnet (decentralized, on-chain) for
 *      the currencies with a feed — see `FIAT_FEED_CODES` in `fiat-rates.ts`.
 *   2. The configurable fiat-rate endpoint (default Frankfurter / ECB), fetched
 *      directly on the client — see `fiat-fx.ts`.
 *
 * The OFFERED list is data-driven: USD + Chainlink feeds + every code the
 * configured endpoint returns. So "everything the endpoint can price is
 * searchable" — point it at a broader provider (e.g. open.er-api.com) to add
 * currencies like VND with no code change. Names/symbols come from the catalog.
 */
import AsyncStorage from '@react-native-async-storage/async-storage';
import { currencyMeta } from '@/services/currency-catalog';
import { getFxRate, getSupportedFxCodes } from '@/services/fiat-fx';
import { getChainlinkRate, isChainlinkFiat, FIAT_FEED_CODES } from '@/services/fiat-rates';
import { formatNumber } from '@/services/locale-format';

export interface Currency { code: string; symbol: string; name: string }

export { currencyMeta };

// A safe, widely-supported base set (ECB/Frankfurter codes) — used, together
// with the Chainlink feeds, only for the offline/first-paint list before the
// live endpoint responds. The live list is normally larger (the default
// endpoint covers ~160 currencies incl. VND).
export const FRANKFURTER_CODES = [
  'AUD', 'BGN', 'BRL', 'CAD', 'CHF', 'CNY', 'CZK', 'DKK', 'EUR', 'GBP', 'HKD',
  'HUF', 'IDR', 'ILS', 'INR', 'ISK', 'JPY', 'KRW', 'MXN', 'MYR', 'NOK', 'NZD',
  'PHP', 'PLN', 'RON', 'SEK', 'SGD', 'THB', 'TRY', 'ZAR',
] as const;

// Guaranteed-priceable base (Chainlink ∪ Frankfurter ∪ USD).
const BASE_CODES = Array.from(new Set<string>(['USD', ...FIAT_FEED_CODES, ...FRANKFURTER_CODES]));

// Majors first in the picker; everything else follows alphabetically.
const PREFERRED_ORDER = [
  'USD', 'EUR', 'GBP', 'CNY', 'JPY', 'KRW', 'HKD', 'INR', 'BRL', 'MXN', 'ARS',
  'AUD', 'CAD', 'CHF', 'SGD', 'NZD', 'TRY', 'PHP', 'IDR', 'ZAR', 'THB', 'MYR',
  'PLN', 'SEK', 'NOK', 'DKK', 'CZK', 'HUF', 'RON', 'BGN', 'ILS', 'ISK',
];
const PREFERRED_SET = new Set(PREFERRED_ORDER);

function orderCodes(codes: Iterable<string>): string[] {
  const set = new Set<string>();
  for (const c of codes) set.add(c.toUpperCase());
  const head = PREFERRED_ORDER.filter((c) => set.has(c));
  const tail = [...set].filter((c) => !PREFERRED_SET.has(c)).sort();
  return [...head, ...tail];
}

function toCurrencies(codes: Iterable<string>): Currency[] {
  return orderCodes(codes).map((c) => currencyMeta(c));
}

/** Whether a code is in the guaranteed-priceable base (Chainlink ∪ Frankfurter ∪ USD). */
export function isSupportedCurrency(code: string): boolean {
  return BASE_CODES.includes(code.toUpperCase());
}

// Static base list — instant first paint and offline fallback for the picker.
export const CURRENCIES: Currency[] = toCurrencies(BASE_CODES);

// Live supported codes once the endpoint has responded (cached across opens).
let _liveCodes: string[] | null = null;

/** Cached/static list for synchronous first paint. */
export function getSupportedCurrenciesSync(): Currency[] {
  return toCurrencies(_liveCodes ?? BASE_CODES);
}

/**
 * Full offered list: USD + Chainlink feeds + everything the configured endpoint
 * returns. Falls back to the static base if the endpoint is unreachable.
 */
export async function loadSupportedCurrencies(): Promise<Currency[]> {
  try {
    const fx = await getSupportedFxCodes();
    if (fx.length > 1) {
      _liveCodes = Array.from(new Set<string>(['USD', ...FIAT_FEED_CODES, ...fx]));
    }
  } catch { /* keep previous / base */ }
  return getSupportedCurrenciesSync();
}

// Currencies conventionally shown without minor units (no decimals), regardless
// of magnitude — yen, won, rupiah, króna, forint have no commonly-used sub-unit.
export const ZERO_DECIMAL_CODES = new Set([
  'JPY', 'KRW', 'IDR', 'ISK', 'HUF', 'VND', 'CLP', 'PYG', 'RWF', 'UGX', 'XOF', 'XAF', 'XPF', 'KMF', 'DJF', 'GNF', 'VUV',
]);

// Above this, the cents are visual noise on a large balance, so we drop them.
const DECIMAL_DROP_THRESHOLD = 100_000;

/** Whether to render minor units for `value` in `code` (no for yen/won/big sums). */
export function shouldShowDecimals(value: number, code: string): boolean {
  return !ZERO_DECIMAL_CODES.has(code.toUpperCase()) && Math.abs(value) < DECIMAL_DROP_THRESHOLD;
}

/**
 * Format an already-converted fiat `value` for display, e.g.
 *   (1428.2, "ARS", "AR$") → "AR$1,428.20"
 *   (259770, "JPY", "¥")   → "¥259,770"   (no decimals)
 */
export function formatFiat(value: number, code: string, symbol: string): string {
  const digits = shouldShowDecimals(value, code) ? 2 : 0;
  return symbol + formatNumber(value, { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

const KEY = 'vela.displayCurrency';
let _code = 'USD';

export async function loadCurrency(): Promise<string> {
  try { const v = await AsyncStorage.getItem(KEY); if (v) _code = v; } catch { /* keep default */ }
  return _code;
}
export function getCurrencyCode(): string { return _code; }
export function getCurrency(): Currency { return currencyMeta(_code); }
export async function setCurrency(code: string): Promise<void> {
  _code = code;
  try { await AsyncStorage.setItem(KEY, code); } catch { /* best effort */ }
}

/**
 * USD → `code` rate (1 for USD), resolved through the source abstraction:
 *   1. Chainlink fiat/USD feed (decentralized, on-chain) when available.
 *   2. The configurable fiat-rate endpoint (Frankfurter/ECB by default).
 * Falls back to 1 so the balance always renders.
 */
export async function getRate(code: string): Promise<number> {
  if (code === 'USD') return 1;

  // 1. Chainlink fiat/USD feed (ENS-addressed on Ethereum mainnet).
  if (isChainlinkFiat(code)) {
    try {
      const r = await getChainlinkRate(code);
      if (r != null && r > 0) return r;
    } catch { /* fall through to the configured endpoint */ }
  }

  // 2. Configurable fiat-rate endpoint (cached + persisted in fiat-fx).
  try {
    const r = await getFxRate(code);
    if (r != null && r > 0) return r;
  } catch { /* fall through */ }

  return 1;
}
