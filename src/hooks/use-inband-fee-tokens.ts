/**
 * Shared in-band fee-token options loader — the SINGLE source for "which assets can pay gas
 * on this chain": the native coin plus every whitelisted stablecoin the Safe actually HOLDS.
 *
 * Used by BOTH the Send confirm slide and the dApp GasFeeCard so the two can't drift (they
 * previously duplicated this, and SendScreen's copy filtered against the wallet's token list —
 * a snapshot that wasn't loaded yet at confirm-open, so held stables silently vanished). This
 * reads balances ON-CHAIN (readErc20Balance), which is timing-robust and works in the dApp
 * context too (no wallet token list there).
 *
 * Gates: in-band chain (bundler capability probe) AND a uniswap-v3 QuoterV2 + wrapped native in
 * the chain's ethereum-data (the bundler prices stables only via that DEX shape). Tempo is
 * excluded (its fee is always pathUSD — no choice). Returns null → no selector (native only).
 */

import { useEffect, useState } from 'react';
import { nativeSymbol } from '@/models/network';
import { isInBandChain } from '@/services/bundler-service';
import { fetchChainTokens } from '@/services/chain-tokens';
import { readErc20Balance } from '@/services/token-reads';
import { isTempoChain } from '@/services/tempo';

export interface FeeTokenOption {
  symbol: string;
  /** null = the native coin; else a whitelisted stablecoin contract. */
  contract: string | null;
}

/**
 * The pure loader (no React) — returns the fee-asset options for a chain, or null when there
 * is no choice (not in-band, no DEX, Tempo, or no held stables). Reads balances ON-CHAIN so it
 * never misses a stable that hadn't yet loaded into the wallet's token list. Exported for tests.
 */
export async function loadInBandFeeTokenOptions(
  chainId: number,
  safeAddress: string,
): Promise<FeeTokenOption[] | null> {
  if (isTempoChain(chainId)) return null;
  if (!(await isInBandChain(chainId, safeAddress))) return null;
  const data = await fetchChainTokens(chainId);
  // The bundler prices stables ONLY via a uniswap-v3 QuoterV2 + wrapped native — gate on the
  // exact capability so no chip can dead-end at quote time.
  if (!data?.dex?.contracts?.quoterV2 || !data.wrappedNativeToken || data.stables.length === 0) {
    return null;
  }
  // Offer only stables the Safe HOLDS — a zero-balance chip can only produce a doomed op.
  // A read failure excludes that token (fail closed; native always works).
  const balances = await Promise.all(
    data.stables.map((s) => readErc20Balance(chainId, s.contract, safeAddress)),
  );
  const held = data.stables.filter((_, i) => (balances[i] ?? 0n) > 0n);
  return [
    { symbol: nativeSymbol(chainId), contract: null },
    ...held.map((s) => ({ symbol: s.symbol, contract: s.contract })),
  ];
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
