/**
 * Gas account service.
 *
 * When using the built-in bundler (bundler.getvela.app), each Safe wallet
 * has a dedicated gas account (EOA) per chain. The bundler auto-sponsors
 * new users from its treasury; if auto-sponsorship is unavailable the user
 * is prompted to fund the gas account manually.
 *
 * This module queries the bundler REST API for account info, requests
 * auto-sponsorship, and checks balance before transaction submission.
 */

import { isUsingBuiltinBundler, getBuiltinBundlerUrl, poolRpcCall, getChainRpcUrl } from './rpc-pool';
import { nativeSymbol } from '@/models/network';
import { loadServiceEndpoints } from './storage';

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
  /** Why funding is needed. */
  reason: 'deposit_needed' | 'wallet_balance_too_low';
  /** Whether this network supports sponsored activation. */
  sponsorshipAvailable: boolean;
  /** Deposit address for the bundler EOA. */
  depositAddress: string;
  /** The Safe wallet address (needed to re-query bundler API). */
  safeAddress: string;
  /** Chain ID. */
  chainId: number;
  /** Native token symbol. */
  nativeSym: string;
  /** Minimum balance required to proceed (wei). */
  thresholdWei: bigint;
  /** Recommended deposit amount (wei) — threshold minus current + 20% buffer. */
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

/** Recommended funding: 2M gas — enough for ~2-3 simple transactions. */
const RECOMMENDED_GAS_UNITS = 2_000_000n;
/** Minimum balance threshold — below this, prompt funding. */
const MIN_BALANCE_WEI = BigInt('100000000000000'); // 0.0001 ETH

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

const infoCache = new Map<string, { info: BundlerAccountInfo; at: number }>();
const INFO_CACHE_TTL = 30_000; // 30s

/** Ensure user-configured endpoints are loaded so getBuiltinBundlerUrl() returns the right URL. */
let _endpointsReady = false;
async function ensureEndpoints(): Promise<void> {
  if (_endpointsReady) return;
  await loadServiceEndpoints();
  _endpointsReady = true;
}

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
  await ensureEndpoints();
  const builtin = await isUsingBuiltinBundler(chainId);
  console.log(`[BundlerFunding] chain=${chainId} isVelaBundler=${builtin}`);
  if (!builtin) return null;

  const info = await fetchBundlerAccountInfo(chainId, safeAddress);
  console.log(`[BundlerFunding] account info:`, info ? `deposit=${info.depositAddress} balance=${info.spendableBalance} status=${info.status}` : 'unreachable');
  if (!info) return null; // Can't reach bundler — let the transaction attempt proceed

  // Check if balance covers the worst-case gas cost (gasLimit × gasPrice).
  // No additional reserve multiplier needed — the gas limits are already
  // 3-5× actual usage, and the bundler's baseFeeMultiplier (1.25×) provides
  // headroom for gas price fluctuations.
  const threshold = estimatedGasCostWei ?? MIN_BALANCE_WEI;

  console.log(`[BundlerFunding] threshold=${threshold} spendable=${info.spendableBalance} sufficient=${info.spendableBalance >= threshold} (gasCost=${estimatedGasCostWei ?? 'default'})`);

  if (info.spendableBalance >= threshold) return null;

  // Balance insufficient — return info for the funding modal.
  // Sponsorship is NOT attempted here; the modal lets the user explicitly
  // request it so they understand what's happening.
  const deficit = threshold - info.spendableBalance;
  const base = deficit > 0n ? deficit : threshold;
  const recommendedWei = (base * 12n) / 10n;

  // Custom networks don't support sponsored activation — only built-in networks do.
  const { DEFAULT_NETWORKS } = await import('@/models/network');
  const isBuiltinNetwork = DEFAULT_NETWORKS.some((n: { chainId: number }) => n.chainId === chainId);

  return {
    reason: 'deposit_needed',
    sponsorshipAvailable: isBuiltinNetwork,
    depositAddress: info.depositAddress,
    safeAddress,
    chainId,
    nativeSym: info.nativeSym,
    thresholdWei: threshold,
    recommendedWei,
    currentBalance: info.spendableBalance,
    recommendedFormatted: formatWei(recommendedWei),
    currentFormatted: formatWei(info.spendableBalance),
  };
}

/**
 * Request gas sponsorship from the bundler's treasury.
 * Exposed for the funding modal to call on user action.
 */
export async function requestGasSponsorship(
  chainId: number,
  safeAddress: string,
  requiredWei: bigint,
): Promise<{ sponsored: boolean; reason?: string }> {
  return requestSponsorship(chainId, safeAddress, requiredWei);
}

/**
 * Request auto-sponsorship of the gas account from the bundler's treasury.
 * The bundler checks eligibility (nonce ≤ 3, WebAuthn registration, treasury balance)
 * and transfers ETH from the treasury to the gas account if all conditions are met.
 */
async function requestSponsorship(
  chainId: number,
  safeAddress: string,
  requiredWei: bigint,
): Promise<{ sponsored: boolean; reason?: string }> {
  try {
    await ensureEndpoints();
    const baseUrl = getBuiltinBundlerUrl();
    const url = `${baseUrl}/v1/sponsor/${chainId}/${safeAddress.toLowerCase()}`;

    const chainRpc = await getChainRpcUrl(chainId);
    const headers: Record<string, string> = {
      'Accept': 'application/json',
      'Content-Type': 'application/json',
    };
    if (chainRpc) headers['X-Rpc-Url'] = chainRpc;

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 20_000);
    let res: Response;
    try {
      res = await fetch(url, {
        method: 'POST',
        headers,
        body: JSON.stringify({ requiredWei: '0x' + requiredWei.toString(16) }),
        signal: controller.signal,
      });
    } finally {
      clearTimeout(timer);
    }
    if (!res.ok) return { sponsored: false, reason: 'request_failed' };
    return await res.json();
  } catch {
    return { sponsored: false, reason: 'network_error' };
  }
}

/**
 * Fetch bundler account info from the REST API.
 * Results are cached for 30 seconds.
 */
export async function fetchBundlerAccountInfo(
  chainId: number,
  safeAddress: string,
): Promise<BundlerAccountInfo | null> {
  await ensureEndpoints();
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
