/**
 * Shared in-band fee-token options loader — the SINGLE source for "which assets can pay gas
 * on this chain": the native coin plus every whitelisted stablecoin the Safe actually HOLDS.
 *
 * Used by BOTH the Send confirm slide and the dApp GasFeeCard so the two can't drift (they
 * previously duplicated this, and SendScreen's copy filtered against the wallet's token list —
 * a snapshot that wasn't loaded yet at confirm-open, so held stables silently vanished). The
 * bundler's address-only quote now supplies balances, prices, and metadata in one response;
 * this loader must not issue separate chain/token/balance reads.
 *
 * Tempo is excluded (its fee is always pathUSD — no choice). Returns null → no selector
 * (native only).
 */

import { useEffect, useState } from 'react';
import { nativeLogoURLs, tokenLogoURLsByAddress } from '@/models/types';
import { fetchInBandGasQuotes } from '@/services/bundler-service';
import { isTempoChain } from '@/services/tempo';

export interface FeeTokenOption {
  asset: 'native' | 'erc20';
  symbol: string;
  /** null = the native coin; else a whitelisted stablecoin contract. */
  contract: string | null;
  /** Raw balance in base units, returned alongside the quote. */
  balance: bigint;
  /** Decimals returned by the bundler quote. Needed to render balance and cost. */
  decimals: number;
  /** Bundler recipient for this fee asset. */
  recipient: string;
  /** Quote-supplied USD data, used instead of a separate price query. */
  usdBalance: string;
  usdPrice: string;
  /** Logo URL candidates (checksummed → lowercase) resolved by (chain, address) so
   *  the row shows a real token icon, not a letter fallback. Works without a wallet
   *  token list, so the dApp signing sheet gets logos too. */
  logoUrls: string[];
}

/**
 * The pure loader (no React) — returns the fee-asset options for a chain, or null when there
 * is no choice (not in-band or Tempo). It deliberately makes exactly one bundler RPC; the
 * result is also shared with fee estimation via the quote cache. Exported for tests.
 */
export async function loadInBandFeeTokenOptions(
  chainId: number,
  safeAddress: string,
): Promise<FeeTokenOption[] | null> {
  if (isTempoChain(chainId)) return null;
  const quotes = await fetchInBandGasQuotes(chainId, safeAddress);
  if (!quotes) return null;

  // Keep the native row for context even if it is empty. Zero-balance stables cannot pay the
  // fee, so omit them from the picker just as before. No other data request is needed.
  return quotes
    .filter((quote) => quote.asset === 'native' || quote.balance > 0n)
    .map((quote): FeeTokenOption => ({
      symbol: quote.symbol,
      asset: quote.asset,
      contract: quote.feeToken,
      balance: quote.balance,
      decimals: quote.decimals,
      recipient: quote.recipient,
      usdBalance: quote.usdBalance,
      usdPrice: quote.usdPrice,
      logoUrls: quote.asset === 'native'
        ? nativeLogoURLs(chainId, quote.symbol)
        : tokenLogoURLsByAddress(chainId, quote.feeToken!),
    }));
}

export function useInBandFeeTokenOptions(
  chainId: number | null,
  safeAddress: string | null,
  active: boolean,
): FeeTokenOption[] | null {
  const [options, setOptions] = useState<FeeTokenOption[] | null>(null);

  useEffect(() => {
    setOptions(null);
    if (!active || chainId === null || !safeAddress) return;
    let cancelled = false;
    loadInBandFeeTokenOptions(chainId, safeAddress)
      .then((opts) => { if (!cancelled) setOptions(opts); })
      .catch(() => { /* no selector — native only */ });
    return () => { cancelled = true; };
  }, [chainId, safeAddress, active]);

  return options;
}
