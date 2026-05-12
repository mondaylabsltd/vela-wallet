/**
 * Bundler funding service.
 *
 * When using the built-in bundler (bundler.getvela.app), each Safe wallet
 * has a dedicated bundler EOA per chain. This EOA must be funded with
 * native tokens to relay transactions.
 *
 * This module queries the bundler REST API for account info and checks
 * whether the EOA has sufficient balance before transaction submission.
 */

import { isUsingBuiltinBundler, getBuiltinBundlerUrl, poolRpcCall, getChainRpcUrl } from './rpc-pool';
import { nativeSymbol } from '@/models/network';

/** Timeout for bundler REST API calls. */
const FETCH_TIMEOUT_MS = 10_000;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface BundlerAccountInfo {
  /** Chain this info applies to. */
  chainId: number;
  /** The dedicated bundler EOA address to fund. */
  depositAddress: string;
  /** Current on-chain balance (wei). */
  onchainBalance: bigint;
  /** Balance available for spending (on-chain minus reserved). */
  spendableBalance: bigint;
  /** EOA status from bundler. */
  status: string;
  /** Native token symbol for this chain. */
  nativeSym: string;
}

export interface FundingNeeded {
  /** Deposit address for the bundler EOA. */
  depositAddress: string;
  /** The Safe wallet address (needed to re-query bundler API). */
  safeAddress: string;
  /** Chain ID. */
  chainId: number;
  /** Native token symbol. */
  nativeSym: string;
  /** Recommended minimum deposit (wei). */
  recommendedWei: bigint;
  /** Current spendable balance (wei). */
  currentBalance: bigint;
  /** Human-readable recommended amount. */
  recommendedFormatted: string;
  /** Human-readable current balance. */
  currentFormatted: string;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Default recommended initial funding: 10M gas × estimated gas price. */
const RECOMMENDED_GAS_UNITS = 10_000_000n;
/** Minimum balance threshold — below this, prompt funding. */
const MIN_BALANCE_WEI = BigInt('500000000000000'); // 0.0005 ETH

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

const infoCache = new Map<string, { info: BundlerAccountInfo; at: number }>();
const INFO_CACHE_TTL = 30_000; // 30s

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Check if the bundler needs funding before sending a transaction.
 *
 * Returns null if funding is sufficient or if using a user-configured bundler.
 * Returns FundingNeeded with deposit info if the built-in bundler EOA is underfunded.
 */
export async function checkBundlerFunding(
  chainId: number,
  safeAddress: string,
  estimatedGasCostWei?: bigint,
): Promise<FundingNeeded | null> {
  // Skip check if user has their own bundler (not vela bundler)
  const builtin = await isUsingBuiltinBundler(chainId);
  console.log(`[BundlerFunding] chain=${chainId} isVelaBundler=${builtin}`);
  if (!builtin) return null;

  const info = await fetchBundlerAccountInfo(chainId, safeAddress);
  console.log(`[BundlerFunding] account info:`, info ? `deposit=${info.depositAddress} balance=${info.spendableBalance} status=${info.status}` : 'unreachable');
  if (!info) return null; // Can't reach bundler — let the transaction attempt proceed

  // Check if balance is sufficient.
  // The bundler requires: spendableBalance >= expectedCost × balanceReserveMultiplier (2x).
  // expectedCost ≈ totalGas × outerGasPrice (which includes bundler tip).
  // Use 4x the wallet's gas estimate to match: 2x for bundler's outerGasPrice overhead, 2x for reserve.
  const threshold = estimatedGasCostWei
    ? estimatedGasCostWei * 4n
    : MIN_BALANCE_WEI;

  console.log(`[BundlerFunding] threshold=${threshold} spendable=${info.spendableBalance} sufficient=${info.spendableBalance >= threshold} (gasCost=${estimatedGasCostWei ?? 'default'})`);

  if (info.spendableBalance >= threshold) return null;

  // Recommend enough for ~10M gas worth of transactions
  const recommendedWei = await estimateRecommendedFunding(chainId);

  return {
    depositAddress: info.depositAddress,
    safeAddress,
    chainId,
    nativeSym: info.nativeSym,
    recommendedWei,
    currentBalance: info.spendableBalance,
    recommendedFormatted: formatWei(recommendedWei),
    currentFormatted: formatWei(info.spendableBalance),
  };
}

/**
 * Fetch bundler account info from the REST API.
 * Results are cached for 30 seconds.
 */
export async function fetchBundlerAccountInfo(
  chainId: number,
  safeAddress: string,
): Promise<BundlerAccountInfo | null> {
  const key = `${chainId}:${safeAddress.toLowerCase()}`;
  const cached = infoCache.get(key);
  if (cached && Date.now() - cached.at < INFO_CACHE_TTL) return cached.info;

  try {
    const baseUrl = getBuiltinBundlerUrl();
    const url = `${baseUrl}/v1/account/${chainId}/${safeAddress.toLowerCase()}`;

    // Pass chain RPC URL so the bundler can reach non-registry chains (e.g. 31337)
    const chainRpc = await getChainRpcUrl(chainId);
    const headers: Record<string, string> = { 'Accept': 'application/json' };
    if (chainRpc) headers['X-Rpc-Url'] = chainRpc;

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
    let res: Response;
    try {
      res = await fetch(url, { headers, signal: controller.signal });
    } finally {
      clearTimeout(timer);
    }
    if (!res.ok) return null;

    const data = await res.json();

    const info: BundlerAccountInfo = {
      chainId,
      depositAddress: data.activeDepositAddress ?? '',
      onchainBalance: parseBigIntHex(data.onchainBalance),
      spendableBalance: parseBigIntHex(data.spendableBalance),
      status: data.status ?? 'UNKNOWN',
      nativeSym: nativeSymbol(chainId),
    };

    infoCache.set(key, { info, at: Date.now() });
    return info;
  } catch (err) {
    console.warn(`[BundlerFunding] Failed to fetch account info for chain=${chainId}:`, err);
    return null;
  }
}

/** Clear cached account info (e.g. after funding). */
export function clearBundlerCache(chainId: number, safeAddress?: string): void {
  if (safeAddress) {
    infoCache.delete(`${chainId}:${safeAddress.toLowerCase()}`);
  } else {
    // Clear all entries for this chain
    for (const key of infoCache.keys()) {
      if (key.startsWith(`${chainId}:`)) infoCache.delete(key);
    }
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

export async function estimateRecommendedFunding(chainId: number): Promise<bigint> {
  // Try to get current gas price for a more accurate recommendation
  try {
    const res = await poolRpcCall('eth_gasPrice', [], chainId);
    const gasPrice = parseBigIntHex(res.result);
    if (gasPrice > 0n) {
      return RECOMMENDED_GAS_UNITS * gasPrice;
    }
  } catch { /* use fallback */ }

  // Fallback: assume 20 gwei
  return RECOMMENDED_GAS_UNITS * 20_000_000_000n;
}

function parseBigIntHex(value: any): bigint {
  if (!value) return 0n;
  if (typeof value === 'string') {
    return BigInt(value.startsWith('0x') ? value : `0x${value}`);
  }
  if (typeof value === 'number') return BigInt(value);
  return 0n;
}

export function formatWei(wei: bigint): string {
  const eth = Number(wei) / 1e18;
  if (eth === 0) return '0';
  if (eth < 0.000001) return '< 0.000001';
  if (eth < 0.001) return eth.toFixed(6);
  if (eth < 1) return eth.toFixed(4);
  return eth.toFixed(3);
}
