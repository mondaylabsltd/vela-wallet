/**
 * Fiat FX rates fetched directly on the client (no server proxy) from the
 * user-configurable endpoint (`ServiceEndpoints.fiatRatesURL`, default
 * Frankfurter v2 — FOSS, self-hostable, ~160 currencies incl. VND).
 *
 * The endpoint must return USD-based rates in one of two shapes (`normalizeRates`
 * accepts both, so providers are swappable):
 *
 *   Frankfurter v2 (array):  [{ base:'USD', quote:'EUR', rate:0.92 }, …]
 *   open.er-api / v1 (object): { rates: { EUR: 0.92, … } }
 *
 * Either way `rate` is X per 1 USD — exactly the USD→fiat multiplier the app
 * needs. The provider decides which currencies exist, so the displayed list is
 * whatever it returns (plus Chainlink + USD).
 *
 * Cached in-memory (6h) + persisted for offline/first-paint; the cache is keyed
 * by endpoint URL so changing the endpoint refetches.
 */
import AsyncStorage from '@react-native-async-storage/async-storage';
import { getFiatRatesURL } from '@/services/storage';

/** Normalize a provider response (array or `{rates}` object) to `{ USD:1, X:rate }`, or null. */
export function normalizeRates(data: unknown): Record<string, number> | null {
  const out: Record<string, number> = { USD: 1 };
  if (Array.isArray(data)) {
    // Frankfurter v2: [{ base, quote, rate }] (base must be USD)
    for (const row of data as Array<{ quote?: string; rate?: unknown }>) {
      const code = String(row?.quote ?? '').toUpperCase();
      const n = Number(row?.rate);
      if (code && isFinite(n) && n > 0) out[code] = n;
    }
  } else if (data && typeof (data as any).rates === 'object') {
    // open.er-api / Frankfurter v1: { rates: { CODE: rate } }
    for (const [k, v] of Object.entries((data as any).rates as Record<string, unknown>)) {
      const n = Number(v);
      if (isFinite(n) && n > 0) out[k.toUpperCase()] = n;
    }
  }
  return Object.keys(out).length > 1 ? out : null;
}

const CACHE_KEY = 'vela.fxRates.v1';
// ECB / Frankfurter rates update once per business day (~16:00 CET), so a long
// cache is plenty — we refetch a few times a day, not on every screen open.
const TTL = 6 * 60 * 60 * 1000; // 6h

interface FxCache { url: string; rates: Record<string, number>; at: number }

let _cache: FxCache | null = null;
let _inflight: Promise<Record<string, number>> | null = null;

/**
 * USD-based rate map for every currency the configured endpoint returns
 * (`{ USD: 1, EUR: 0.92, … }`). Cached 1h; falls back to the persisted map.
 */
export async function fetchFxRates(): Promise<Record<string, number>> {
  const url = getFiatRatesURL();
  if (_cache && _cache.url === url && Date.now() - _cache.at < TTL) return _cache.rates;
  if (_inflight) return _inflight;

  _inflight = (async () => {
    try {
      const res = await fetch(url);
      if (res.ok) {
        const rates = normalizeRates(await res.json());
        if (rates) {
          _cache = { url, rates, at: Date.now() };
          AsyncStorage.setItem(CACHE_KEY, JSON.stringify(_cache)).catch(() => {});
          console.log(`[FX] ${Object.keys(rates).length} rates from ${url}`);
          return rates;
        }
      }
      console.warn(`[FX] bad response from ${url} (HTTP ${res.status})`);
    } catch (err) {
      console.warn(`[FX] fetch failed: ${err instanceof Error ? err.message : String(err)}`);
    }
    return loadPersisted();
  })();

  try { return await _inflight; } finally { _inflight = null; }
}

/** USD → `code` multiplier from the configured endpoint, or null if unavailable. */
export async function getFxRate(code: string): Promise<number | null> {
  const c = code.toUpperCase();
  if (c === 'USD') return 1;
  const rates = await fetchFxRates();
  const r = rates[c];
  return r && r > 0 ? r : null;
}

/** Currency codes the configured endpoint can price (drives the offered list). */
export async function getSupportedFxCodes(): Promise<string[]> {
  return Object.keys(await fetchFxRates());
}

async function loadPersisted(): Promise<Record<string, number>> {
  if (_cache) return _cache.rates;
  try {
    const raw = await AsyncStorage.getItem(CACHE_KEY);
    if (raw) {
      const c = JSON.parse(raw) as FxCache;
      if (c?.rates) { _cache = c; return c.rates; }
    }
  } catch { /* ignore */ }
  return {};
}
