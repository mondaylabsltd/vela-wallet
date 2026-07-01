/**
 * fiat-convert — the shared fiat⇄token amount math, used by BOTH the single-send
 * fiat-input toggle ([SendScreen]) and the payroll batch importer ([BatchImportSheet]).
 *
 * Rates are expressed exactly as the rest of the app already models them, so callers
 * never have to reconcile two conventions:
 *   - `priceUsd`      — a token's unit price in USD (USDT ⇒ 1, ETH ⇒ ~3000; null ⇒ unpriced).
 *   - `usdToFiatRate` — the USD→fiat multiplier from `getRate(code)` in currency.ts
 *                       (1 USD ≈ 7.1 CNY). Chainlink/Frankfurter under the hood.
 * A token's price in the display fiat is therefore just `priceUsd × usdToFiatRate`.
 *
 * Pure and dependency-free on purpose, so the money-shaping logic is exhaustively
 * unit-testable away from the price/rate/RPC stack.
 */

/** Strip trailing zeros (and a bare trailing dot) without mangling an integer. */
function stripTrailingZeros(s: string): string {
  if (!s.includes('.')) return s; // integers are left alone (guards decimals=0 tokens)
  return s.replace(/0+$/, '').replace(/\.$/, '');
}

/**
 * A token's unit price expressed in a fiat currency, or `0` when it can't be known.
 * `0` is the "can't convert" sentinel every consumer checks before dividing.
 */
export function tokenPriceInFiat(priceUsd: number | null | undefined, usdToFiatRate: number): number {
  if (!priceUsd || priceUsd <= 0) return 0;
  const rate = usdToFiatRate > 0 ? usdToFiatRate : 1;
  return priceUsd * rate;
}

/**
 * Convert a fiat amount into a human token-amount string, truncated (via `toFixed`)
 * to `decimals` so we never emit more precision than the token can carry — the same
 * precision guard the on-chain `toBaseUnits` will apply. `priceInFiat` is the token's
 * unit price in the SAME fiat currency as `fiat` (see {@link tokenPriceInFiat}).
 * Returns '0' for a non-positive fiat OR an unknown (≤0) price.
 */
export function fiatToTokenAmount(fiat: number, priceInFiat: number, decimals: number = 18): string {
  if (!(priceInFiat > 0)) return '0';
  if (!(fiat > 0)) return '0';
  return stripTrailingZeros((fiat / priceInFiat).toFixed(Math.max(0, Math.trunc(decimals))));
}

/**
 * The single-send helper (identical signature + semantics to the former inline
 * SendScreen version): in fiat-input mode the typed `amount` is in the user's
 * display currency, so divide by the token's price in that currency. In token
 * mode — or for an unpriced token — the raw typed amount is returned unchanged.
 */
export function resolveTokenAmount(
  amount: string,
  inFiat: boolean,
  priceUsd: number | null | undefined,
  decimals: number = 18,
  rate: number = 1,
): string {
  if (!inFiat || !priceUsd || priceUsd <= 0) return amount;
  const fiat = parseFloat(amount || '0');
  if (fiat <= 0) return '0';
  return fiatToTokenAmount(fiat, tokenPriceInFiat(priceUsd, rate), decimals);
}
