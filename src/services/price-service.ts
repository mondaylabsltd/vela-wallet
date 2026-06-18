/**
 * Price calculation module.
 *
 * Priority:
 *   1. DEX swap quotes on each chain (via multicall3, handled in wallet-api.ts)
 *   2. Chainlink price feeds on Ethereum mainnet (this module)
 *   3. Price unknown -> null
 *
 * Chainlink feeds are queried in a single multicall3 batch on Ethereum mainnet.
 * All feeds use 8 decimals (answer / 1e8 = USD price).
 */

import {
  MULTICALL3,
  encAggregate3,
  decAggregate3,
  encLatestRound,
  decChainlinkUsd,
  type Call3,
} from './abi';
import { poolRpcCall } from './rpc-pool';

// ---------------------------------------------------------------------------
// Chainlink price feed proxy addresses on Ethereum Mainnet
// These are immutable proxy contracts — the underlying aggregator changes,
// but the proxy address is stable.
// ---------------------------------------------------------------------------

const CHAINLINK_FEEDS: Record<string, string> = {
  ETH:   '0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419',
  BNB:   '0x14e613AC691a42F21B17a6Dc7232f070FF175d25',
  MATIC: '0x7bAC85A8a13A4BcD8abb3eB7d6b4d632c5a57676',
  AVAX:  '0xFF3EEb22B5E3dE6e705b44749C2559d704923FD7',
  DAI:   '0xAed0c38402a5d19df6E4c03F4E2DceD6e29c1ee9',
};

/**
 * Map wallet-internal native symbols to Chainlink feed keys.
 * e.g. nativeSymbol() returns "POL" for Polygon, but Chainlink uses "MATIC".
 */
const SYMBOL_ALIAS: Record<string, string> = {
  POL:  'MATIC',
  XDAI: 'DAI',
};

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

const PRICE_CACHE_TTL = 3 * 60 * 1000; // 3 min
let _priceCache: { prices: Record<string, number>; at: number } | null = null;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Fetch all native token USD prices from Chainlink feeds on Ethereum mainnet.
 * Returns a map of symbol -> USD price (e.g. { ETH: 2500.12, BNB: 312.5 }).
 * Results are cached for 3 minutes.
 */
export async function fetchChainlinkPrices(): Promise<Record<string, number>> {
  if (_priceCache && Date.now() - _priceCache.at < PRICE_CACHE_TTL) {
    return _priceCache.prices;
  }

  try {
    const symbols = Object.keys(CHAINLINK_FEEDS);
    const calls: Call3[] = symbols.map(sym => ({
      target: CHAINLINK_FEEDS[sym],
      allowFailure: true,
      callData: encLatestRound(),
    }));

    const data = encAggregate3(calls);
    const result = await ethCallOnEthereum(MULTICALL3, data);
    const decoded = decAggregate3(result);

    const prices: Record<string, number> = {};
    for (let i = 0; i < symbols.length; i++) {
      const r = decoded[i];
      if (r?.success && r.data.length >= 66) {
        const usd = decChainlinkUsd(r.data);
        if (Number.isFinite(usd) && usd > 0) prices[symbols[i]] = usd;
      } else {
        console.warn(`[Chainlink] ${symbols[i]} feed failed: success=${r?.success} dataLen=${r?.data?.length ?? 0}`);
      }
    }

    console.log(`[Chainlink] prices: ${Object.entries(prices).map(([k, v]) => `${k}=$${v.toFixed(2)}`).join(', ')}`);
    _priceCache = { prices, at: Date.now() };
    return prices;
  } catch (err) {
    console.warn(`[Chainlink] fetch failed: ${err instanceof Error ? err.message : String(err)}`);
    return _priceCache?.prices ?? {};
  }
}

/**
 * Resolve a native symbol to a Chainlink price.
 * Handles aliases (POL -> MATIC, xDAI -> DAI).
 */
export function resolveChainlinkPrice(
  nativeSym: string,
  prices: Record<string, number>,
): number | null {
  const upper = nativeSym.toUpperCase();
  // Stablecoin-denominated native gas (e.g. Tempo's "USD") is pegged to $1.
  if (upper === 'USD') return 1;
  return prices[upper] ?? prices[SYMBOL_ALIAS[upper] ?? ''] ?? null;
}

// ---------------------------------------------------------------------------
// RPC helper (uses global pool for Ethereum mainnet)
// ---------------------------------------------------------------------------

async function ethCallOnEthereum(to: string, data: string): Promise<string> {
  const response = await poolRpcCall('eth_call', [{ to, data }, 'latest'], 1);
  if (response.error) throw new Error(response.error.message);
  if (!response.result || response.result === '0x') throw new Error('Empty result');
  return response.result;
}
