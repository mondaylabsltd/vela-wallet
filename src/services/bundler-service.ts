/**
 * Gas account service.
 *
 * When using the built-in bundler (vela-bundler.getvela.app), each Safe wallet
 * has a dedicated gas account (EOA) per chain. The bundler auto-sponsors
 * new users from its treasury; if auto-sponsorship is unavailable the user
 * is prompted to fund the gas account manually.
 *
 * This module queries the bundler REST API for account info, requests
 * auto-sponsorship, and checks balance before transaction submission.
 */

import { nativeSymbol } from '@/models/network';
import { getActiveBundlerBaseUrl, getChainRpcUrl, isUsingBuiltinBundler, poolRpcCall } from './rpc-pool';
import { loadServiceEndpoints } from './storage';
import { isTempoChain, TEMPO_DEFAULT_FEE_TOKEN } from './tempo';

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
/**
 * Volatility buffer on the recommended deposit amount (150% = +50%). Absorbs
 * gas-price spikes between the user funding the gas account and the bundle
 * executing. Mirrors the bundler's server-side SPONSOR_VOLATILITY_BUFFER_BPS so
 * the self-funding path and the treasury-sponsored path leave the same headroom.
 */
const FUNDING_BUFFER_BPS = 15_000n;

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
 * Recommended deposit to lift the gas account to `thresholdWei`, plus the
 * volatility buffer (FUNDING_BUFFER_BPS). When the balance is already at/above
 * threshold the deficit is zero, so we buffer the full threshold instead — a
 * meaningful top-up rather than ~0. Single source of truth for the deposit math
 * shared by every funding entry point (Send, dApp request, funding modal).
 */
export function recommendedFundingWei(thresholdWei: bigint, currentBalance: bigint): bigint {
  const deficit = thresholdWei > currentBalance ? thresholdWei - currentBalance : 0n;
  const base = deficit > 0n ? deficit : thresholdWei;
  return (base * FUNDING_BUFFER_BPS) / 10_000n;
}

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
  const recommendedWei = recommendedFundingWei(threshold, info.spendableBalance);

  // Sponsorship is always available — the bundler decides whether to sponsor
  // based on treasury balance, nonce limits, and Safe wallet balance.
  return {
    reason: 'deposit_needed',
    // Sponsorship works on every chain: Tempo sponsors a pathUSD float to the gas
    // account via a 0x76; other chains transfer native from the treasury.
    sponsorshipAvailable: true,
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
    // Sponsor the gas account on the SAME bundler that will sign the bundle (see
    // getActiveBundlerBaseUrl) — a per-network bundler override must not sponsor one
    // bundler's EOA while submitting to another.
    const baseUrl = await getActiveBundlerBaseUrl(chainId);
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
    // Resolve the deposit address from the SAME bundler the pool submits to — NOT
    // always the built-in one. On Tempo the reimbursement is paid to this bundler's
    // per-Safe EOA; reading it from a different bundler makes the submitting bundler
    // reject the op (reimbursed=0). See getActiveBundlerBaseUrl.
    const baseUrl = await getActiveBundlerBaseUrl(chainId);
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

    let onchainBalance = parseBigIntHex(data.onchainBalance);
    let spendableBalance = parseBigIntHex(data.spendableBalance);
    let nativeSym = nativeSymbol(chainId);

    // Tempo has no native coin: eth_getBalance is a sentinel. Report the gas account's
    // pathUSD balance instead, scaled to 18 decimals so the wei-based funding UI renders
    // the correct USD value.
    if (isTempoChain(chainId) && data.activeDepositAddress) {
      try {
        const callData =
          '0x70a08231000000000000000000000000' +
          String(data.activeDepositAddress).slice(2).toLowerCase();
        const balRes = await poolRpcCall('eth_call', [{ to: TEMPO_DEFAULT_FEE_TOKEN, data: callData }, 'latest'], chainId);
        const path6 = parseBigIntHex(balRes.result); // pathUSD, 6 decimals
        onchainBalance = path6 * 10n ** 12n; // -> 18-dec USD representation
        spendableBalance = onchainBalance;
        nativeSym = 'pathUSD';
      } catch { /* keep native fallback */ }
    }

    const info: BundlerAccountInfo = {
      chainId,
      depositAddress: data.activeDepositAddress ?? '',
      onchainBalance,
      spendableBalance,
      status: data.status ?? 'UNKNOWN',
      nativeSym,
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

// ---------------------------------------------------------------------------
// Underfunded-error detection
// ---------------------------------------------------------------------------

export interface BundlerUnderfunded {
  /** Gas account's current spendable balance (wei), if the message reported it. */
  spendableWei?: bigint;
  /** Balance the bundler needs to proceed (wei), if the message reported it. */
  requiredWei?: bigint;
  /** Address to deposit gas funds to, parsed straight from the message. */
  depositAddress?: string;
  /** What the gas account is denominated in: 'pathUSD' on Tempo, else native. */
  asset: 'native' | 'pathUSD';
}

/**
 * Detect the bundler "gas account underfunded" error and pull out its parts.
 *
 * The bundler has worded this two ways over time — the legacy
 * "...dedicated bundler EOA" and the current
 * "Insufficient native balance on dedicated bundler gas account.
 *  Spendable: X, required: Y. Deposit to: 0x..." (pathUSD on Tempo).
 *
 * Match on the stable signal rather than one exact phrase, so a future wording
 * tweak doesn't silently drop us back to dumping a raw error at the user. The
 * deposit address + required amount are parsed out so callers can open the
 * funding modal even when a follow-up account lookup fails.
 */
export function parseBundlerUnderfunded(msg: string | undefined | null): BundlerUnderfunded | null {
  if (!msg) return null;
  const isUnderfunded =
    /dedicated bundler (gas account|EOA)/i.test(msg) ||
    (/Deposit to:\s*0x/i.test(msg) && /required:/i.test(msg));
  if (!isUnderfunded) return null;

  const big = (re: RegExp): bigint | undefined => {
    const m = msg.match(re);
    return m ? BigInt(m[1]) : undefined;
  };
  const dep = msg.match(/Deposit to:\s*(0x[0-9a-fA-F]{40})/i);
  return {
    spendableWei: big(/Spendable:\s*(\d+)/i),
    requiredWei: big(/required:\s*(\d+)/i),
    depositAddress: dep ? dep[1] : undefined,
    asset: /pathUSD/i.test(msg) ? 'pathUSD' : 'native',
  };
}
