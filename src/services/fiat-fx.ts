/**
 * Fiat FX rates fetched directly on the client (no server proxy) from the
 * user-configurable endpoint (`ServiceEndpoints.fiatRatesURL`, default
 * open.er-api.com — ~160 currencies incl. VND). The endpoint must return
 * USD-based rates as:
 *
 *   { rates: { EUR: 0.92, JPY: 155.3, … } }      // rates[X] = X per 1 USD
 *
 * which is exactly the USD→fiat multiplier the app needs. The provider decides
 * which currencies exist — so the displayed list is whatever it returns (plus
 * Chainlink + USD). Swap the endpoint (e.g. to open.er-api.com) for broader
 * coverage (VND, etc.) without any code change.
 *
 * Cached in-memory (1h) + persisted for offline/first-paint; the cache is keyed
 * by endpoint URL so changing the endpoint refetches.
 */
import AsyncStorage from '@react-native-async-storage/async-storage';
import { getFiatRatesURL } from '@/services/storage';

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
        const data = await res.json();
        if (data?.rates && typeof data.rates === 'object') {
          const rates: Record<string, number> = { USD: 1 };
          for (const [k, v] of Object.entries(data.rates)) {
            const n = Number(v);
            if (isFinite(n) && n > 0) rates[k.toUpperCase()] = n;
          }
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
