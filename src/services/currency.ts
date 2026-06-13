/**
 * Display-currency preference + USD→fiat rate.
 *
 * Only the total balance is shown in the chosen fiat (line items stay in token
 * units). Rate comes from the existing getvela exchange-rate endpoint
 * (`fetchExchangeRate`), cached per session. Preference persists locally.
 */
import AsyncStorage from '@react-native-async-storage/async-storage';
import { fetchExchangeRate } from '@/services/wallet-api';

export interface Currency { code: string; symbol: string; name: string }

// USD first, then the core target-market currencies (Vietnam / Nigeria / Argentina) + common ones.
export const CURRENCIES: Currency[] = [
  { code: 'USD', symbol: '$',   name: 'US Dollar' },
  { code: 'CNY', symbol: '¥',   name: 'Chinese Yuan' },
  { code: 'EUR', symbol: '€',   name: 'Euro' },
  { code: 'VND', symbol: '₫',   name: 'Vietnamese Dong' },
  { code: 'NGN', symbol: '₦',   name: 'Nigerian Naira' },
  { code: 'ARS', symbol: 'AR$', name: 'Argentine Peso' },
  { code: 'GBP', symbol: '£',   name: 'British Pound' },
  { code: 'INR', symbol: '₹',   name: 'Indian Rupee' },
  { code: 'BRL', symbol: 'R$',  name: 'Brazilian Real' },
];

const KEY = 'vela.displayCurrency';
let _code = 'USD';

export async function loadCurrency(): Promise<string> {
  try { const v = await AsyncStorage.getItem(KEY); if (v) _code = v; } catch { /* keep default */ }
  return _code;
}
export function getCurrencyCode(): string { return _code; }
export function getCurrency(): Currency {
  return CURRENCIES.find((c) => c.code === _code) ?? CURRENCIES[0];
}
export async function setCurrency(code: string): Promise<void> {
  _code = code;
  try { await AsyncStorage.setItem(KEY, code); } catch { /* best effort */ }
}

const rateCache = new Map<string, number>();
/** USD → `code` rate (1 for USD). Best-effort; falls back to 1 on failure. */
export async function getRate(code: string): Promise<number> {
  if (code === 'USD') return 1;
  const cached = rateCache.get(code);
  if (cached != null) return cached;
  try {
    const r = await fetchExchangeRate(code);
    if (r > 0) { rateCache.set(code, r); return r; }
  } catch { /* fall through */ }
  return 1;
}
