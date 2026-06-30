/**
 * On-chain asset query engine.
 *
 * Direct on-chain queries using Multicall3. Each network is queried with a
 * single eth_call containing batched balance + DEX price queries.
 *
 * Architecture:
 *   1. Discover tokens per chain (native + stablecoins + wrapped native + custom ERC-20s)
 *   2. Query balances via Multicall3 (one RPC call per network)
 *   3. Query prices via DEX swap quotes in the same Multicall3 batch
 *   4. Fall back to Chainlink feeds on Ethereum mainnet for missing prices
 *   5. Filter to non-zero balances, sort by USD value
 */

import type { APIToken, CustomToken } from '@/models/types';
import { tokenUsdValue, tokenChainId, isNativeToken } from '@/models/types';
import { getAllNetworksSync, networkId, chainName, nativeSymbol } from '@/models/network';
import { loadCustomTokens } from './storage';
import { fetchWithTimeout, NET_TIMEOUTS } from './net';
import { poolRpcCall, getFailedRpcChains } from './rpc-pool';
import { priceShouldNull } from './dev/fault-injection';
import { fetchChainTokens, pickQuoteToken, type ChainTokenData } from './chain-tokens';
import { fetchChainlinkPrices, resolveChainlinkPrice } from './price-service';
import {
  MULTICALL3,
  encAggregate3, decAggregate3,
  encBalanceOf, encDecimals, encGetEthBalance,
  encQuoteV3, encGetAmountsOut,
  encLatestRound, decChainlinkUsd,
  decU256, decU8, decAmountsOut,
  type Call3, type McResult,
} from './abi';

// ---------------------------------------------------------------------------
// Per-chain Chainlink native/USD feed addresses.
// Queried directly on each chain as part of the multicall — no extra RPC call.
// All feeds use 8 decimals (answer / 1e8 = USD price).
// ---------------------------------------------------------------------------

const NATIVE_CHAINLINK_FEEDS: Record<number, string> = {
  1:     '0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419', // ETH/USD on Ethereum
  56:    '0x0567F2323251f0Aab15c8dFb1967E4e8A7D42aeE', // BNB/USD on BSC
  // 137: Polygon — no working Chainlink feed (MATIC→POL migration), DEX covers it
  42161: '0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612', // ETH/USD on Arbitrum
  10:    '0x13e3Ee699D1909E989722E753853AE30b17e08c5', // ETH/USD on Optimism
  8453:  '0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70', // ETH/USD on Base
  43114: '0x0A77230d17318075983913bC2145DB16C7366156', // AVAX/USD on Avalanche
  100:   '0x678df3415fc31947dA4324eC63212874be5a82f8', // DAI/USD on Gnosis
};

// ---------------------------------------------------------------------------
// Cache (same interface as before)
// ---------------------------------------------------------------------------

const TOKEN_CACHE_TTL_MS = 5 * 60 * 1000; // 5 min

type TokenCacheEntry = {
  fetchedAt: number;
  tokens: APIToken[];
  inFlight?: Promise<APIToken[]>;
};

const tokenCache = new Map<string, TokenCacheEntry>();

export type FetchTokensOptions = {
  forceRefresh?: boolean;
  maxAgeMs?: number;
  /** Include tokens with zero balance (for managing watchlist). Default: false. */
  includeZeroBalance?: boolean;
  /** Called each time a chain finishes, with the accumulated tokens so far (sorted by USD value). */
  onProgress?: (tokens: APIToken[]) => void;
  /** Called after all chains finish, with the chain IDs whose RPC endpoints all failed. */
  onFailedChains?: (chainIds: number[]) => void;
};

export class APIError extends Error {
  constructor(message = 'Failed to fetch data from server.') {
    super(message);
    this.name = 'APIError';
  }
}

// ---------------------------------------------------------------------------
// Public API (same interface as before)
// ---------------------------------------------------------------------------

/** Fetch token balances across all supported networks. */
export async function fetchTokens(
  address: string,
  options: FetchTokensOptions = {},
): Promise<APIToken[]> {
  const cacheKey = address.trim().toLowerCase();
  const maxAgeMs = options.maxAgeMs ?? TOKEN_CACHE_TTL_MS;
  const cached = tokenCache.get(cacheKey);
  const now = Date.now();

  // includeZeroBalance bypasses cache (different result set)
  if (!options.forceRefresh && !options.includeZeroBalance && cached) {
    if (cached.inFlight) return cloneTokens(await cached.inFlight);
    if (now - cached.fetchedAt < maxAgeMs) return cloneTokens(cached.tokens);
  }

  const request = fetchAllChainTokens(address, options.onProgress, options.onFailedChains, options.includeZeroBalance);

  // Don't pollute the main cache with includeZeroBalance results
  if (!options.includeZeroBalance) {
    tokenCache.set(cacheKey, {
      fetchedAt: cached?.fetchedAt ?? 0,
      tokens: cached?.tokens ?? [],
      inFlight: request,
    });
  }

  try {
    const tokens = await request;
    if (!options.includeZeroBalance) {
      tokenCache.set(cacheKey, { fetchedAt: Date.now(), tokens });
    }
    return cloneTokens(tokens);
  } catch (error) {
    if (cached?.tokens.length) {
      tokenCache.set(cacheKey, cached);
    } else {
      tokenCache.delete(cacheKey);
    }
    throw error;
  }
}

export function clearTokenCache(address?: string): void {
  if (address) tokenCache.delete(address.trim().toLowerCase());
  else tokenCache.clear();
}

/**
 * Synchronously read the ERC-20 token addresses the user is known to hold on a
 * chain, from the in-memory token cache (lowercased). Empty when the cache is
 * cold — never triggers a fetch. Used by transaction simulation to trust a
 * *received* token the user already holds (a real token, not a spoofed one).
 */
export function getCachedHeldTokens(address: string | undefined, chainId: number): string[] {
  if (!address) return [];
  const entry = tokenCache.get(address.trim().toLowerCase());
  if (!entry?.tokens?.length) return [];
  const out: string[] = [];
  for (const t of entry.tokens) {
    if (tokenChainId(t) === chainId && !isNativeToken(t) && t.tokenAddress) {
      out.push(t.tokenAddress.toLowerCase());
    }
  }
  return out;
}

/** Fetch USD to target currency exchange rate (unchanged). */
export async function fetchExchangeRate(currency = 'CNY'): Promise<number> {
  const url = `https://getvela.app/api/exchange-rate?currency=${encodeURIComponent(currency)}`;
  const response = await fetchWithTimeout(url, {}, { timeoutMs: NET_TIMEOUTS.fiatRates });
  if (!response.ok) throw new APIError(`/exchange-rate failed: HTTP ${response.status}`);
  const data: { currency: string; rate: number } = await response.json();
  return data.rate;
}

// ---------------------------------------------------------------------------
// Core: orchestrate all chains
// ---------------------------------------------------------------------------

async function fetchAllChainTokens(
  address: string,
  onProgress?: (tokens: APIToken[]) => void,
  onFailedChains?: (chainIds: number[]) => void,
  includeZeroBalance?: boolean,
): Promise<APIToken[]> {
  // Phase 1: load prerequisites in parallel
  const [customTokens, clPrices] = await Promise.all([
    loadCustomTokens(),
    fetchChainlinkPrices(),
  ]);

  // Phase 2: query each chain in parallel, streaming results as each chain finishes
  const networks = getAllNetworksSync();
  const accumulated: APIToken[] = [];

  const sortAndFilter = () =>
    accumulated
      .filter(t => includeZeroBalance || parseFloat(t.balance) > 0)
      .sort((a, b) => tokenUsdValue(b) - tokenUsdValue(a));

  // Cap each chain so one dead/slow RPC can't hold the whole fetch (a chain
  // with no healthy endpoint can otherwise burn ~60s on sequential failover).
  // A capped-out chain contributes nothing this round and is retried next time.
  const PER_CHAIN_TIMEOUT_MS = 18_000;
  await Promise.allSettled(
    networks.map(net => {
      const chainTokensP = queryChainAssets(
        address,
        net.chainId,
        customTokens.filter(ct => ct.chainId === net.chainId),
        clPrices,
      ).catch(() => [] as APIToken[]);
      const bounded = Promise.race([
        chainTokensP,
        new Promise<APIToken[]>(resolve => setTimeout(() => resolve([]), PER_CHAIN_TIMEOUT_MS)),
      ]);
      return bounded.then(chainTokens => {
        if (chainTokens.length > 0) {
          accumulated.push(...chainTokens);
          onProgress?.(sortAndFilter());
        }
      });
    }),
  );

  // Report chains where all RPC endpoints failed
  const failed = networks
    .map(n => n.chainId)
    .filter(id => getFailedRpcChains().has(id));
  if (failed.length > 0) onFailedChains?.(failed);

  return sortAndFilter();
}

// ---------------------------------------------------------------------------
// Per-chain: build multicall, execute, decode
// ---------------------------------------------------------------------------

/** Category for internal token tracking. */
type TokenCategory = 'native' | 'stable' | 'wrapped' | 'custom';

interface TokenSlot {
  symbol: string;
  name: string;
  contract: string | null; // null = native token
  category: TokenCategory;
  knownDecimals: number | null;
}

async function queryChainAssets(
  address: string,
  chainId: number,
  customTokens: CustomToken[],
  chainlinkPrices: Record<string, number>,
): Promise<APIToken[]> {
  // 1. Discover tokens on this chain
  const chainData = await fetchChainTokens(chainId);
  const nativeCurrency = chainData?.nativeCurrency
    ?? { name: nativeSymbol(chainId), symbol: nativeSymbol(chainId), decimals: 18 };
  const stables = chainData?.stables ?? [];
  const wrappedNative = chainData?.wrappedNativeToken ?? null;
  const dex = chainData?.dex ?? null;

  // 2. Build token slots and multicall batch
  const slots: TokenSlot[] = [];
  const calls: Call3[] = [];
  const balIdx: number[] = [];       // calls index for each slot's balance
  const decIdx: (number | null)[] = []; // calls index for each slot's decimals

  // --- Native token ---
  addSlot('native', nativeCurrency.symbol, nativeCurrency.name, null, nativeCurrency.decimals);
  balIdx.push(calls.length);
  calls.push(mc(MULTICALL3, encGetEthBalance(address)));
  decIdx.push(null);

  // --- Stablecoins ---
  for (const s of stables) {
    addSlot('stable', s.symbol, s.symbol, s.contract, null);
    balIdx.push(calls.length);
    calls.push(mc(s.contract, encBalanceOf(address)));
    decIdx.push(calls.length);
    calls.push(mc(s.contract, encDecimals()));
  }

  // --- Wrapped native token ---
  if (wrappedNative) {
    addSlot('wrapped', 'W' + nativeCurrency.symbol, 'Wrapped ' + nativeCurrency.name, wrappedNative, null);
    balIdx.push(calls.length);
    calls.push(mc(wrappedNative, encBalanceOf(address)));
    decIdx.push(calls.length);
    calls.push(mc(wrappedNative, encDecimals()));
  }

  // --- Custom ERC-20s ---
  for (const ct of customTokens) {
    addSlot('custom', ct.symbol, ct.name, ct.contractAddress, ct.decimals);
    balIdx.push(calls.length);
    calls.push(mc(ct.contractAddress, encBalanceOf(address)));
    decIdx.push(null); // decimals already known
  }

  // --- DEX price queries ---
  const quoteToken = pickQuoteToken(stables);
  // Track which call index was used for the quote token's decimals
  let quoteTokenDecCallIdx: number | null = null;
  if (quoteToken) {
    const qi = stables.indexOf(quoteToken);
    if (qi >= 0) quoteTokenDecCallIdx = decIdx[1 + qi]; // +1 because native is slot 0
  }

  // Pick a secondary quote token (e.g. USDT if primary is USDC) for fallback
  const secondaryQuoteToken = stables.find(s => s.contract !== quoteToken?.contract) ?? null;

  // Native price: wrappedNative -> quoteToken (USDC/USDT)
  const nativePriceCallIdxs: number[] = [];
  if (quoteToken && wrappedNative && dex) {
    const amountIn = 10n ** BigInt(nativeCurrency.decimals);
    addDexPriceCalls(dex, wrappedNative, quoteToken.contract, amountIn, nativePriceCallIdxs);
    // Also try secondary stablecoin (e.g. USDT on BSC where it's more liquid)
    if (secondaryQuoteToken) {
      addDexPriceCalls(dex, wrappedNative, secondaryQuoteToken.contract, amountIn, nativePriceCallIdxs);
    }
  }

  // Custom ERC-20 prices:
  //   Path A: token → ANY stablecoin (USDC, USDC.e, USDT, etc.)
  //   Path B: token → wrappedNative (WETH/WMATIC) — then multiply by nativePriceUsd
  const customDirectIdxs = new Map<number, number[]>();
  const customViaNativeIdxs = new Map<number, number[]>();
  if (dex) {
    for (let i = 0; i < slots.length; i++) {
      if (slots[i].category !== 'custom' || !slots[i].contract) continue;
      const dec = slots[i].knownDecimals ?? 18;
      const amountIn = 10n ** BigInt(dec);
      // Path A: token → stablecoin (try ALL stablecoins — e.g. USDC.e has pools native USDC doesn't)
      const directIdxs: number[] = [];
      for (const stable of stables) {
        addDexPriceCalls(dex, slots[i].contract!, stable.contract, amountIn, directIdxs);
      }
      if (directIdxs.length > 0) customDirectIdxs.set(i, directIdxs);
      // Path B: token → wrappedNative (most tokens have WETH/WMATIC pools)
      if (wrappedNative) {
        const nativeIdxs: number[] = [];
        addDexPriceCalls(dex, slots[i].contract!, wrappedNative, amountIn, nativeIdxs);
        if (nativeIdxs.length > 0) customViaNativeIdxs.set(i, nativeIdxs);
      }
    }
  }

  // --- Per-chain Chainlink native/USD feed (queried in the same multicall) ---
  let onChainChainlinkIdx: number | null = null;
  const nativeFeed = NATIVE_CHAINLINK_FEEDS[chainId];
  if (nativeFeed) {
    onChainChainlinkIdx = calls.length;
    calls.push(mc(nativeFeed, encLatestRound()));
  }

  // 3. Execute multicall
  if (calls.length === 0) return [];

  let results: McResult[];
  try {
    const encoded = encAggregate3(calls);
    const raw = await ethCall(chainId, MULTICALL3, encoded);
    results = decAggregate3(raw);
  } catch {
    return []; // chain unsupported or RPC failed
  }

  // 4. Decode quote token decimals (for price conversion)
  let quoteDecimals = 6; // sensible default for USDC
  if (quoteTokenDecCallIdx != null) {
    const r = results[quoteTokenDecCallIdx];
    if (r?.success) quoteDecimals = decU8(r.data);
  }

  // 5. Resolve native price: DEX → on-chain Chainlink → Ethereum Chainlink
  let nativePriceUsd: number | null = null;
  let nativePriceSource = 'none';

  // Try DEX (use correct decoder based on protocol)
  const dexDecoder = dex?.protocol === 'solidly' ? decAmountsOut : decU256;
  const dexPrice = extractPrice(results, nativePriceCallIdxs, quoteDecimals, dexDecoder);

  // Try on-chain Chainlink feed (queried in same multicall, zero extra cost)
  let onChainClPrice: number | null = null;
  if (onChainChainlinkIdx != null) {
    const clr = results[onChainChainlinkIdx];
    if (clr?.success && clr.data.length >= 66) {
      const usd = decChainlinkUsd(clr.data);
      if (Number.isFinite(usd) && usd > 0) onChainClPrice = usd;
    }
  }

  // Try Ethereum mainnet Chainlink
  const ethClPrice = resolveChainlinkPrice(nativeCurrency.symbol, chainlinkPrices);

  // Pick best price: DEX preferred, but sanity-check against Chainlink.
  // If DEX price deviates >50% from Chainlink, DEX likely has low liquidity → prefer Chainlink.
  const clBestPrice = onChainClPrice ?? ethClPrice;
  if (dexPrice != null && clBestPrice != null) {
    const ratio = dexPrice / clBestPrice;
    if (ratio > 0.5 && ratio < 2.0) {
      nativePriceUsd = dexPrice;
      nativePriceSource = 'DEX';
    } else {
      nativePriceUsd = clBestPrice;
      nativePriceSource = 'Chainlink(sanity)';
    }
  } else if (dexPrice != null) {
    nativePriceUsd = dexPrice;
    nativePriceSource = 'DEX';
  } else if (onChainClPrice != null) {
    nativePriceUsd = onChainClPrice;
    nativePriceSource = 'Chainlink(local)';
  } else if (ethClPrice != null) {
    nativePriceUsd = ethClPrice;
    nativePriceSource = 'Chainlink(ETH)';
  }

  // Log summary: which source was used + status of all sources
  const dexOk = nativePriceCallIdxs.filter(i => results[i]?.success).length;
  const dexTotal = nativePriceCallIdxs.length;
  console.log(
    `[Price] chain=${chainId} ${nativeCurrency.symbol} → $${nativePriceUsd?.toFixed(2) ?? '?'} via ${nativePriceSource}` +
    ` | DEX: ${dexPrice != null ? `$${dexPrice.toFixed(2)}` : `FAIL(${dexOk}/${dexTotal})`}` +
    ` | CL-local: ${onChainClPrice != null ? `$${onChainClPrice.toFixed(2)}` : 'n/a'}` +
    ` | CL-ETH: ${ethClPrice != null ? `$${ethClPrice.toFixed(2)}` : 'n/a'}`,
  );

  // 6. Build APIToken array
  const netId = networkId(chainId);
  const netName = chainName(chainId);
  const tokens: APIToken[] = [];

  for (let i = 0; i < slots.length; i++) {
    const slot = slots[i];

    // Balance
    const balR = results[balIdx[i]];
    const rawBal = balR?.success ? decU256(balR.data) : 0n;

    // Decimals
    let dec = slot.knownDecimals;
    if (dec == null && decIdx[i] != null) {
      const decR = results[decIdx[i]!];
      dec = decR?.success ? decU8(decR.data) : 18;
    }
    dec = dec ?? 18;

    // Price
    let priceUsd: number | null = null;
    switch (slot.category) {
      case 'native':
      case 'wrapped':
        priceUsd = nativePriceUsd;
        break;
      case 'stable':
        priceUsd = 1.0;
        break;
      case 'custom': {
        // Path A: direct token → stablecoin
        const directIdxs = customDirectIdxs.get(i);
        priceUsd = extractPrice(results, directIdxs ?? [], quoteDecimals, dexDecoder);
        let erc20Source = priceUsd != null ? 'direct' : '';
        // Path B: token → wrappedNative, then multiply by native USD price
        if (priceUsd == null && nativePriceUsd != null) {
          const viaNativeIdxs = customViaNativeIdxs.get(i);
          const priceInNative = extractPrice(results, viaNativeIdxs ?? [], nativeCurrency.decimals, dexDecoder);
          if (priceInNative != null) {
            priceUsd = priceInNative * nativePriceUsd;
            erc20Source = 'viaNative';
          }
        }
        const dOk = (directIdxs ?? []).filter(j => results[j]?.success).length;
        const dTot = directIdxs?.length ?? 0;
        const nOk = (customViaNativeIdxs.get(i) ?? []).filter(j => results[j]?.success).length;
        const nTot = customViaNativeIdxs.get(i)?.length ?? 0;
        console.log(`[Price] chain=${chainId} ERC20 ${slot.symbol} → $${priceUsd?.toFixed(4) ?? '?'} via ${erc20Source || 'FAIL'} | direct(${dOk}/${dTot}) viaNative(${nOk}/${nTot})`);
        break;
      }
    }

    tokens.push({
      network: netId,
      chainName: netName,
      symbol: slot.symbol,
      balance: formatRawBalance(rawBal, dec),
      decimals: dec,
      logo: null,
      name: slot.name,
      tokenAddress: slot.contract,
      priceUsd,
      spam: false,
    });
  }

  // Dev fault injection: drop prices so held tokens still render but the total
  // undercounts — reproduces the "balance silently dropped" scenario.
  if (priceShouldNull(chainId)) {
    for (const tk of tokens) tk.priceUsd = null;
  }

  return tokens;

  // --- Local helpers ---

  function addSlot(
    category: TokenCategory,
    symbol: string,
    name: string,
    contract: string | null,
    knownDecimals: number | null,
  ) {
    slots.push({ symbol, name, contract, category, knownDecimals });
  }

  function addDexPriceCalls(
    dexInfo: NonNullable<ChainTokenData['dex']>,
    tokenIn: string,
    tokenOut: string,
    amountIn: bigint,
    outIdxs: number[],
  ) {
    const { protocol, contracts } = dexInfo;

    if (protocol === 'uniswap-v3' && contracts.quoterV2) {
      // Try common fee tiers: 500 (0.05%), 3000 (0.3%), 2500 (0.25% — PancakeSwap V3), 10000 (1% — exotic pairs)
      for (const fee of [500, 3000, 2500, 10000]) {
        outIdxs.push(calls.length);
        calls.push(mc(contracts.quoterV2, encQuoteV3(tokenIn, tokenOut, amountIn, fee)));
      }
    } else if (protocol === 'solidly' && contracts.router) {
      // Aerodrome/Velodrome V2: getAmountsOut with Route struct, try both volatile and stable
      for (const stable of [false, true]) {
        outIdxs.push(calls.length);
        calls.push(mc(contracts.router, encGetAmountsOut(amountIn, tokenIn, tokenOut, stable)));
      }
    }
    // liquidity-book & curve: rely on Chainlink fallback
  }
}

// ---------------------------------------------------------------------------
// Multicall helper
// ---------------------------------------------------------------------------

function mc(target: string, callData: string): Call3 {
  return { target, allowFailure: true, callData };
}

// ---------------------------------------------------------------------------
// Price extraction
// ---------------------------------------------------------------------------

/** Extract a USD price from DEX quote results. Takes the first successful quote. */
function extractPrice(
  results: McResult[],
  callIdxs: number[],
  quoteDecimals: number,
  decoder: (hex: string) => bigint = decU256,
): number | null {
  for (const idx of callIdxs) {
    const r = results[idx];
    if (r?.success && r.data.length >= 66) { // 0x + 64 hex chars minimum for uint256
      const amountOut = decoder(r.data);
      if (amountOut > 0n) {
        return Number(amountOut) / 10 ** quoteDecimals;
      }
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Balance formatting
// ---------------------------------------------------------------------------

/** Convert a raw bigint balance to a human-readable decimal string. */
function formatRawBalance(raw: bigint, decimals: number): string {
  if (raw === 0n) return '0';
  const s = raw.toString();
  if (decimals === 0) return s;

  let intPart: string;
  let decPart: string;

  if (s.length <= decimals) {
    intPart = '0';
    decPart = s.padStart(decimals, '0');
  } else {
    intPart = s.slice(0, s.length - decimals);
    decPart = s.slice(s.length - decimals);
  }

  // Trim trailing zeros
  decPart = decPart.replace(/0+$/, '');
  return decPart ? `${intPart}.${decPart}` : intPart;
}

// ---------------------------------------------------------------------------
// RPC helper (uses the global RPC pool for load balancing + failover)
// ---------------------------------------------------------------------------

async function ethCall(chainId: number, to: string, data: string): Promise<string> {
  const response = await poolRpcCall('eth_call', [{ to, data }, 'latest'], chainId);
  if (response.error) throw new Error(response.error.message);
  if (!response.result || response.result === '0x') throw new Error('Empty result');
  return response.result;
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

function cloneTokens(tokens: APIToken[]): APIToken[] {
  return tokens.map(t => ({ ...t }));
}
