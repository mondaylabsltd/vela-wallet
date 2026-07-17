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
import { fundingShouldForce } from './dev/fault-injection';
import { formatWeiToEth } from './format-eth';
import { getActiveBundlerBaseUrl, getChainRpcUrl, isUsingBuiltinBundler, poolBundlerCall, poolRpcCall } from './rpc-pool';
import { loadServiceEndpoints } from './storage';
import { isTempoChain, TEMPO_DEFAULT_FEE_TOKEN } from './tempo';
import { fetchWithTimeout, isTimeoutError, NET_TIMEOUTS } from './net';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface BundlerAccountInfo {
  /** Chain this info applies to. */
  chainId: number;
  /** The dedicated bundler EOA address to fund. */
  depositAddress: string;
  /** Where the in-band gas reimbursement should be sent. Normally equals
   *  depositAddress; the bundler's TREASURY when its vault mode is enabled
   *  (vela-bundler docs/pool-queue-architecture.md Stage 2). Absent on old
   *  bundlers — callers fall back to depositAddress. Funding/deposit flows
   *  must keep using depositAddress; only the reimbursement leg follows this. */
  settlementRecipient?: string;
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
  /**
   * How the funding sheet should open. 'topup' = ask the user to deposit
   * (sponsorship was denied — carry `denialReason`); 'confirming' = money is
   * already on its way (sponsorship granted or possibly-landed) — the sheet
   * just waits for the balance to reflect it. Set by attemptSilentSponsorship.
   */
  presentation?: 'topup' | 'confirming';
  /** Server denial reason from the silent sponsorship attempt, if any. */
  denialReason?: string;
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

  // Dev-only: `vela.forceFunding()` makes the gas account read as underfunded so
  // the in-sheet funding UX (docs/KNOWN-BUGS.md BUG-1) can be exercised without
  // draining a real gas account. `fundingShouldForce` is always false in prod.
  if (!fundingShouldForce(chainId) && info.spendableBalance >= threshold) return null;

  // Balance insufficient — return info for the caller to resolve, normally by
  // calling attemptSilentSponsorship() and only surfacing the funding sheet if
  // sponsorship is denied.
  const recommendedWei = recommendedFundingWei(threshold, info.spendableBalance);

  return {
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
 * Exposed for the funding sheet's retry affordance.
 */
export async function requestGasSponsorship(
  chainId: number,
  safeAddress: string,
  requiredWei: bigint,
): Promise<{ sponsored: boolean; reason?: string }> {
  return requestSponsorship(chainId, safeAddress, requiredWei);
}

// ---------------------------------------------------------------------------
// Silent sponsorship
// ---------------------------------------------------------------------------

export type SilentSponsorship =
  /** Gas account is usable — proceed without any funding UI. `sponsored` is
   *  true when the treasury actually granted funds in THIS attempt (drives the
   *  "covered by Vela" note on the confirm screen). */
  | { outcome: 'funded'; sponsored: boolean }
  /** Funds are (probably) on their way but the balance read hasn't caught up —
   *  open the sheet in its waiting state, never as a denial. */
  | { outcome: 'confirming' }
  /** Sponsorship refused — open the sheet in top-up mode with the reason. */
  | { outcome: 'denied'; denialReason?: string };

/** Client-side politeness throttle: identical DENIED attempts within this
 *  window return the cached denial instead of re-hitting the endpoint (the
 *  user re-tapping Continue after a cancel shouldn't spam the treasury gate).
 *  Success/confirming outcomes are never cached — balances move. */
const SILENT_DENY_TTL = 25_000;
const silentDenials = new Map<string, { reason?: string; at: number }>();

/**
 * Try to make the gas account usable with no UI: request treasury sponsorship
 * and re-verify the balance. This is the ONLY automatic treasury touchpoint —
 * it runs when the user is demonstrably about to transact (Continue/approve),
 * never speculatively, so the wallet adds no treasury exposure beyond the
 * server's own eligibility gates.
 */
export async function attemptSilentSponsorship(
  funding: FundingNeeded,
  opts?: { force?: boolean },
): Promise<SilentSponsorship> {
  const { chainId, safeAddress, thresholdWei } = funding;

  // Dev seams: `vela.forceFunding()` exists to exercise the funding sheet, and
  // parallel-space fixture Safes are never registered with the real bundler —
  // in both cases skip the doomed/counterproductive network round-trip and go
  // straight to the sheet (founder decision 2026-07-06 for the test env).
  if (fundingShouldForce(chainId) || (globalThis as { __VELA_PARALLEL__?: boolean }).__VELA_PARALLEL__) {
    return { outcome: 'denied' };
  }

  const key = `${chainId}:${safeAddress.toLowerCase()}`;
  const recent = silentDenials.get(key);
  if (!opts?.force && recent && Date.now() - recent.at < SILENT_DENY_TTL) {
    return { outcome: 'denied', denialReason: recent.reason };
  }

  const verify = async (): Promise<boolean> => {
    clearBundlerCache(chainId, safeAddress);
    const info = await fetchBundlerAccountInfo(chainId, safeAddress);
    return !!info && info.spendableBalance >= thresholdWei;
  };

  const result = await requestGasSponsorship(chainId, safeAddress, thresholdWei);

  if (result.sponsored || result.reason === 'already_funded') {
    // The bundler waits (≤15s) for the transfer receipt before answering, so
    // the money is normally on-chain already; a lagging balance read must
    // surface as "confirming", NEVER as a denial (the old flow showed a
    // successful sponsorship as "Free activation unavailable" + a payment QR).
    silentDenials.delete(key);
    if (await verify()) return { outcome: 'funded', sponsored: result.sponsored };
    return { outcome: 'confirming' };
  }

  if (result.reason === 'pending_unknown' || result.reason === 'already_in_progress') {
    // pending_unknown: timeout mid-transfer — the treasury tx may have landed;
    // a second request would risk a double-spend. already_in_progress: another
    // grant for this account is literally in flight. Either way money may be
    // arriving: the sheet's poll reconciles.
    silentDenials.delete(key);
    return { outcome: 'confirming' };
  }

  silentDenials.set(key, { reason: result.reason, at: Date.now() });
  return { outcome: 'denied', denialReason: result.reason };
}

// ---------------------------------------------------------------------------
// Sponsorship probe (dry run)
// ---------------------------------------------------------------------------

export type SponsorProbe =
  /** Server says the gates pass — defer the actual grant to the moment of
   *  maximum commitment (the confirm slide), so the treasury only ever funds
   *  transactions that are about to execute and recoup. */
  | { outcome: 'eligible' }
  /** An old server without dryRun support performed the real grant — fine,
   *  that is simply the pre-dryRun behavior. */
  | { outcome: 'granted' }
  /** Grant outcome unknown (timeout mid-transfer on an old server). */
  | { outcome: 'confirming' }
  | { outcome: 'denied'; reason?: string };

/**
 * Ask the bundler whether sponsorship WOULD succeed, without moving money
 * (body `dryRun: true`). Lets the Send flow route denials to the funding
 * sheet at Continue (where an external deposit is still a graceful ask)
 * while delaying the actual treasury transfer to the confirm slide.
 *
 * Backward compatible: a server that predates dryRun ignores the flag and
 * grants for real — mapped to 'granted', which callers treat as sponsored.
 */
export async function probeGasSponsorship(funding: FundingNeeded): Promise<SponsorProbe> {
  const { chainId, safeAddress, thresholdWei } = funding;
  if (fundingShouldForce(chainId) || (globalThis as { __VELA_PARALLEL__?: boolean }).__VELA_PARALLEL__) {
    return { outcome: 'denied' };
  }
  try {
    await ensureEndpoints();
    const baseUrl = await getActiveBundlerBaseUrl(chainId);
    const url = `${baseUrl}/v1/sponsor/${chainId}/${safeAddress.toLowerCase()}`;
    const res = await fetchWithTimeout(
      url,
      {
        method: 'POST',
        headers: { 'Accept': 'application/json', 'Content-Type': 'application/json' },
        body: JSON.stringify({ requiredWei: '0x' + thresholdWei.toString(16), dryRun: true }),
      },
      // Sponsor-tier timeout, NOT the short REST one: an old server without
      // dryRun support performs the REAL grant here (waits ≤15s for the
      // receipt) — timing it out would map to 'eligible' and fire a duplicate
      // attempt at confirm time.
      { timeoutMs: NET_TIMEOUTS.bundlerSponsor },
    );
    if (!res.ok) {
      let reason = res.status === 503 || res.status === 429 ? 'service_unavailable' : 'request_failed';
      try {
        const body = await res.json();
        if (body && typeof body.reason === 'string' && body.reason) reason = body.reason;
      } catch { /* keep status-derived reason */ }
      return { outcome: 'denied', reason };
    }
    const body = await res.json();
    if (body.sponsored) return { outcome: 'granted' };
    if (body.dryRun) {
      // Defensive: a server that reports "needs no grant" reasons as
      // ineligible must not bounce a payable user to the funding sheet.
      if (body.eligible || body.reason === 'already_funded' || body.reason === 'amount_too_small') {
        return { outcome: 'eligible' };
      }
      return { outcome: 'denied', reason: body.reason };
    }
    // Old server, real attempt: mirror attemptSilentSponsorship's mapping.
    if (body.reason === 'pending_unknown' || body.reason === 'already_in_progress') return { outcome: 'confirming' };
    if (body.reason === 'already_funded') return { outcome: 'eligible' };
    return { outcome: 'denied', reason: body.reason };
  } catch {
    // The probe is advisory — on any transport failure defer the decision to
    // the real grant at confirm time (whose failure path shows the sheet).
    return { outcome: 'eligible' };
  }
}

/**
 * Request auto-sponsorship of the gas account from the bundler's treasury.
 * The bundler checks eligibility (relayer nonce ≤ 6, Safe balance ≥ 2× the
 * sponsor amount on native chains, WebAuthn index registration, treasury
 * balance) and transfers from the treasury to the gas account if all pass.
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
      // Stable idempotency key: sponsorship is a non-idempotent treasury transfer,
      // so a timeout-then-retry must not double-spend. A backend that honours this
      // header collapses the same (chain, safe, amount) request into one transfer.
      'Idempotency-Key': `sponsor:${chainId}:${safeAddress.toLowerCase()}:0x${requiredWei.toString(16)}`,
    };
    if (chainRpc) headers['X-Rpc-Url'] = chainRpc;

    const res = await fetchWithTimeout(
      url,
      {
        method: 'POST',
        headers,
        body: JSON.stringify({ requiredWei: '0x' + requiredWei.toString(16) }),
      },
      { timeoutMs: NET_TIMEOUTS.bundlerSponsor },
    );
    if (!res.ok) {
      // Prefer the server's structured reason — a 503 carries
      // 'passkey_index_unavailable', which the server documents as "retry
      // later" infrastructure trouble, NOT a business rejection. Collapsing it
      // to a generic failure used to dead-end users who were fully eligible.
      let reason = res.status === 503 || res.status === 429 ? 'service_unavailable' : 'request_failed';
      try {
        const body = await res.json();
        if (body && typeof body.reason === 'string' && body.reason) reason = body.reason;
      } catch { /* keep the status-derived reason */ }
      return { sponsored: false, reason };
    }
    return await res.json();
  } catch (err) {
    // A timeout is NOT a definitive failure: the treasury transfer may have gone
    // through and we simply didn't see the response. Report it as pending/unknown
    // so the caller reconciles by polling the gas-account balance (the funding
    // modal's self-fund step already polls) rather than treating it as denied —
    // which could otherwise tempt the user into a second, duplicate sponsorship.
    if (isTimeoutError(err)) return { sponsored: false, reason: 'pending_unknown' };
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

    const res = await fetchWithTimeout(url, { headers }, { timeoutMs: NET_TIMEOUTS.bundlerRest });
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

    // Accept settlementRecipient only if it is a well-formed address — a corrupted
    // field must degrade to the depositAddress fallback, never poison the fee leg.
    const settlementRecipient =
      typeof data.settlementRecipient === 'string' && /^0x[0-9a-fA-F]{40}$/.test(data.settlementRecipient)
        ? data.settlementRecipient
        : undefined;

    const info: BundlerAccountInfo = {
      chainId,
      depositAddress: data.activeDepositAddress ?? '',
      settlementRecipient,
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

// Splitter info is constant per bundler (a pure function of the operator secret), so cache
// per base URL indefinitely.
const splitterInfoCache = new Map<string, { address: string; treasury: string }>();

/**
 * Fetch the VelaGasSettlementSplitter address + its treasury from the SAME bundler the pool
 * submits to (getActiveBundlerBaseUrl) — a per-network override may use a different operator
 * secret and thus a different splitter. The wallet uses `treasury` to compute the splitter's
 * CREATE2 address LOCALLY (it never trusts bundler-supplied deploy calldata). Returns null on
 * any failure so callers fail safe (skip the in-batch deploy rather than deploy something wrong).
 */
export async function fetchSplitterInfo(
  chainId: number,
): Promise<{ address: string; treasury: string } | null> {
  await ensureEndpoints();
  try {
    const baseUrl = await getActiveBundlerBaseUrl(chainId);
    const cached = splitterInfoCache.get(baseUrl);
    if (cached) return cached;

    const url = `${baseUrl}/v1/splitter`;
    const res = await fetchWithTimeout(url, { headers: { Accept: 'application/json' } }, { timeoutMs: NET_TIMEOUTS.bundlerRest });
    if (!res.ok) return null;

    const data = await res.json();
    const address = String(data.address ?? '');
    const treasury = String(data.treasury ?? '');
    const isAddr = (a: string) => /^0x[0-9a-fA-F]{40}$/.test(a);
    if (!isAddr(address) || !isAddr(treasury)) return null;

    const info = { address, treasury };
    splitterInfoCache.set(baseUrl, info);
    return info;
  } catch (err) {
    console.warn(`[Splitter] Failed to fetch splitter info for chain=${chainId}:`, err);
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

/** @deprecated alias of {@link formatWeiToEth}; kept for existing callers and tests. */
export const formatWei = formatWeiToEth;

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
 * The threshold (18-dec wei-scale) implied by a parsed underfunded error.
 * Tempo reports pathUSD amounts in 6-dec units while account info scales
 * balances to 18 decimals — comparing across units would make any dust
 * balance read as "funded" and loop the submit. Returns null when the
 * message carried no required amount.
 */
export function underfundedRequiredWei(underfunded: BundlerUnderfunded): bigint | null {
  if (underfunded.requiredWei == null) return null;
  return underfunded.asset === 'pathUSD'
    ? underfunded.requiredWei * 10n ** 12n
    : underfunded.requiredWei;
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
// ---------------------------------------------------------------------------
// In-band gas settlement (generic chains) — vela-bundler docs/inband-gas-settlement.md
// ---------------------------------------------------------------------------

/** A sizing quote from `vela_getInBandGasQuote`: transfer exactly `requiredAmount`
 *  of the chosen asset to `recipient` inside the UserOp batch. The bundler
 *  re-verifies at submit; the amount already includes its 3× markup and, for
 *  stablecoins, the $0.01-equivalent floor. */
export interface InBandGasQuote {
  recipient: string;
  asset: 'native' | 'erc20';
  feeToken: string | null;
  requiredAmount: bigint;
  decimals?: number;
  markupX: number;
}

/**
 * Ask the bundler how much to transfer in-band for `nativeCostWei` of estimated
 * outer gas. `feeToken` null/undefined → native; else a whitelisted stablecoin.
 * Returns null when the quote fails for reasons the caller should fall back on
 * (chain not in-band, token not whitelisted / unpriceable, transient error).
 */
/** Short-lived cache for in-band quotes (the stablecoin path costs a DEX quoterV2 eth_call on
 *  the bundler, ~hundreds of ms) so flipping the fee-token chip back and forth doesn't re-hit
 *  it each time. Keyed by (chain, safe, feeToken, nativeCost bucketed to 1%) — a quote is only
 *  reused for the same tx sizing. TTL is short: the bundler re-verifies at submit anyway. */
const QUOTE_CACHE_TTL = 8_000;
const inBandQuoteCache = new Map<string, { at: number; quote: InBandGasQuote | null }>();

function quoteCacheKey(chainId: number, safe: string, nativeCostWei: bigint, feeToken?: string | null): string {
  // Bucket the cost to ~1% so tiny basis jitter still hits the cache.
  const bucket = nativeCostWei > 0n ? nativeCostWei / (nativeCostWei / 100n + 1n) : 0n;
  return `${chainId}:${safe.toLowerCase()}:${(feeToken ?? 'native').toLowerCase()}:${bucket}`;
}

export function _resetInBandQuoteCache(): void {
  inBandQuoteCache.clear();
}

export async function fetchInBandGasQuote(
  chainId: number,
  safeAddress: string,
  nativeCostWei: bigint,
  feeToken?: string | null,
): Promise<InBandGasQuote | null> {
  const key = quoteCacheKey(chainId, safeAddress, nativeCostWei, feeToken);
  const cached = inBandQuoteCache.get(key);
  if (cached && Date.now() - cached.at < QUOTE_CACHE_TTL) return cached.quote;
  const quote = (await fetchInBandGasQuoteDetailed(chainId, safeAddress, nativeCostWei, feeToken)).quote;
  // Cache only real quotes — a transient null must not stick.
  if (quote) inBandQuoteCache.set(key, { at: Date.now(), quote });
  return quote;
}

/** Like fetchInBandGasQuote but distinguishes a DEFINITIVE "chain is not in-band"
 *  (bundler says not enabled, or an old bundler without the method) from transient
 *  failures — only the former may negative-cache the chain's capability. */
async function fetchInBandGasQuoteDetailed(
  chainId: number,
  safeAddress: string,
  nativeCostWei: bigint,
  feeToken?: string | null,
): Promise<{ quote: InBandGasQuote | null; notEnabled: boolean }> {
  try {
    const resp = await poolBundlerCall(
      'vela_getInBandGasQuote',
      [{
        safeAddress,
        nativeCost: '0x' + nativeCostWei.toString(16),
        ...(feeToken ? { feeToken } : {}),
      }],
      chainId,
    );
    if (resp.error || !resp.result) {
      const notEnabled =
        /not enabled/i.test(resp.error?.message ?? '') ||
        resp.error?.code === -32601; // method not found — pre-in-band bundler
      console.log(`[InBand] quote unavailable on chain=${chainId}: ${resp.error?.message ?? 'empty result'}`);
      return { quote: null, notEnabled };
    }
    const r = resp.result;
    if (!r.recipient || !/^0x[0-9a-fA-F]{40}$/.test(r.recipient)) return { quote: null, notEnabled: false };
    const requiredAmount = parseBigIntHex(r.requiredAmount);
    if (requiredAmount <= 0n) return { quote: null, notEnabled: false };
    return {
      quote: {
        recipient: r.recipient,
        asset: r.asset === 'erc20' ? 'erc20' : 'native',
        feeToken: r.feeToken ?? null,
        requiredAmount,
        decimals: typeof r.decimals === 'number' ? r.decimals : undefined,
        markupX: typeof r.markupX === 'number' ? r.markupX : 3,
      },
      notEnabled: false,
    };
  } catch (err) {
    console.warn(`[InBand] quote failed for chain=${chainId}:`, err);
    return { quote: null, notEnabled: false };
  }
}

/** Per-chain in-band capability, learned from a probe quote. Tempo is always
 *  in-band; other chains flip when the bundler enables them. */
const inBandSupport = new Map<number, { at: number; supported: boolean }>();
const INBAND_SUPPORT_TTL = 5 * 60_000;

export async function isInBandChain(chainId: number, safeAddress: string): Promise<boolean> {
  if (isTempoChain(chainId)) return true;
  const cached = inBandSupport.get(chainId);
  if (cached && Date.now() - cached.at < INBAND_SUPPORT_TTL) return cached.supported;
  // 1-wei probe. Only a DEFINITIVE outcome may be cached: a transient RPC failure
  // negative-cached for 5 minutes would route sends down the legacy path (maxFee>0,
  // prefund gate) against a bundler that expects in-band — every one rejected.
  const { quote, notEnabled } = await fetchInBandGasQuoteDetailed(chainId, safeAddress, 1n);
  if (quote !== null) {
    inBandSupport.set(chainId, { at: Date.now(), supported: true });
    return true;
  }
  if (notEnabled) {
    inBandSupport.set(chainId, { at: Date.now(), supported: false });
    return false;
  }
  // Transient failure: fall back to the last known answer (even expired), never cache.
  return cached?.supported ?? false;
}

/** Test hook: reset the in-band capability cache. */
export function _resetInBandSupportCache(): void {
  inBandSupport.clear();
}

// ---------------------------------------------------------------------------
// Treasury bootstrap (user-funded relayer float on depleted/dev networks)
// ---------------------------------------------------------------------------

/** Per-chain treasury status from GET /v1/treasury/:chainId. `bootstrapNeeded`
 *  means the relayer can't operate until someone funds the treasury directly —
 *  a NON-REFUNDABLE operator-float contribution, not gas credit. */
export interface TreasuryStatus {
  chainId: number;
  address: string;
  asset: 'native' | 'pathUSD';
  balance: bigint;
  floor: bigint;
  bootstrapNeeded: boolean;
}

export async function fetchTreasuryStatus(chainId: number): Promise<TreasuryStatus | null> {
  try {
    const baseUrl = await getActiveBundlerBaseUrl(chainId);
    const res = await fetchWithTimeout(
      `${baseUrl}/v1/treasury/${chainId}`,
      { headers: { Accept: 'application/json' } },
      { timeoutMs: NET_TIMEOUTS.bundlerRest },
    );
    if (!res.ok) return null;
    const data = await res.json();
    if (!data.address || !/^0x[0-9a-fA-F]{40}$/.test(data.address)) return null;
    return {
      chainId,
      address: data.address,
      asset: data.asset === 'pathUSD' ? 'pathUSD' : 'native',
      balance: parseBigIntHex(data.balance),
      floor: parseBigIntHex(data.floor),
      bootstrapNeeded: data.bootstrapNeeded === true,
    };
  } catch (err) {
    console.warn(`[Treasury] status fetch failed for chain=${chainId}:`, err);
    return null;
  }
}

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
