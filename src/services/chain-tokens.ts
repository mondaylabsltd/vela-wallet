/**
 * Chain token discovery from ethereum-data API.
 *
 * Fetches stablecoins, wrapped native token, and DEX info per chain.
 * Data source: https://ethereum-data.awesometools.dev/chains/eip155-{chainId}.json
 */

import { getEthereumDataURL } from './storage';
const CACHE_TTL = 30 * 60 * 1000; // 30 min

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface StableToken {
  symbol: string;
  type: string;   // "native" | "bridge"
  contract: string;
}

export interface DexInfo {
  dex: string;         // "Uniswap", "PancakeSwap", "Aerodrome", etc.
  protocol: string;    // "uniswap-v3", "solidly", "liquidity-book", "curve"
  contracts: Record<string, string>;
  url?: string;
}

export interface ChainTokenData {
  chainId: number;
  nativeCurrency: { name: string; symbol: string; decimals: number };
  stables: StableToken[];
  wrappedNativeToken: string | null;
  dex: DexInfo | null;
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

const cache = new Map<number, { data: ChainTokenData; at: number }>();

// ---------------------------------------------------------------------------
// Built-in DEX overrides — guaranteed correct, never depend on remote API.
// Each entry uses the most mainstream DEX on that chain.
// ---------------------------------------------------------------------------

const BUILTIN_DEX: Record<number, DexInfo> = {
  // Ethereum — Uniswap V3 (dominant DEX)
  1: {
    dex: 'Uniswap', protocol: 'uniswap-v3',
    contracts: { factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984', quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e' },
  },
  // BSC — PancakeSwap V3 (dominant DEX)
  56: {
    dex: 'PancakeSwap', protocol: 'uniswap-v3',
    contracts: { factory: '0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865', quoterV2: '0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997' },
  },
  // Polygon — Uniswap V3 (dominant DEX)
  137: {
    dex: 'Uniswap', protocol: 'uniswap-v3',
    contracts: { factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984', quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e' },
  },
  // Arbitrum — Uniswap V3 (dominant DEX)
  42161: {
    dex: 'Uniswap', protocol: 'uniswap-v3',
    contracts: { factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984', quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e' },
  },
  // Optimism — Uniswap V3 (dominant DEX)
  10: {
    dex: 'Uniswap', protocol: 'uniswap-v3',
    contracts: { factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984', quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e' },
  },
  // Base — Aerodrome (dominant DEX by TVL)
  8453: {
    dex: 'Aerodrome', protocol: 'solidly',
    contracts: { factory: '0x420DD381b31aEf6683db6B902084cB0FFECe40Da', router: '0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43' },
  },
  // Avalanche — Uniswap V3 (compatible quoter, Trader Joe uses unsupported liquidity-book)
  43114: {
    dex: 'Uniswap', protocol: 'uniswap-v3',
    contracts: { factory: '0x740b1c1de25031C31FF4fC9A62f554A55cdC1baD', quoterV2: '0xbe0F5544EC67e9B3b2D979aaA43f18Fd87E6257F' },
  },
  // Gnosis — SushiSwap V3 (best available V3 quoter)
  100: {
    dex: 'SushiSwap', protocol: 'uniswap-v3',
    contracts: { factory: '0xf78031CBCA409F2FB6876BDFDBc1b2df24cF9bEf', quoterV2: '0xb1E835Dc2785b52265711e17fCCb0fd018226a6e' },
  },
  // Unichain — Uniswap V3 (canonical deploy addresses)
  130: {
    dex: 'Uniswap', protocol: 'uniswap-v3',
    contracts: { factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984', quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e' },
  },
  // Monad — Uniswap V3 (canonical deploy addresses)
  143: {
    dex: 'Uniswap', protocol: 'uniswap-v3',
    contracts: { factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984', quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e' },
  },
  // World Chain — Uniswap V3 (canonical deploy addresses)
  480: {
    dex: 'Uniswap', protocol: 'uniswap-v3',
    contracts: { factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984', quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e' },
  },
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/** Fetch chain token data with caching. Returns null if chain is unknown. */
export async function fetchChainTokens(chainId: number): Promise<ChainTokenData | null> {
  const cached = cache.get(chainId);
  if (cached && Date.now() - cached.at < CACHE_TTL) return cached.data;

  try {
    const res = await fetch(`${getEthereumDataURL()}/chains/eip155-${chainId}.json`);
    if (!res.ok) return null;

    const raw = await res.json();
    const rawDec = raw.nativeCurrency?.decimals;
    const decimals = typeof rawDec === 'number' && rawDec >= 0 && rawDec <= 255
      ? rawDec : 18;

    const data: ChainTokenData = {
      chainId,
      nativeCurrency: {
        name: raw.nativeCurrency?.name ?? 'Ether',
        symbol: raw.nativeCurrency?.symbol ?? 'ETH',
        decimals,
      },
      stables: Array.isArray(raw.stables) ? raw.stables : [],
      wrappedNativeToken: raw.wrappedNativeToken ?? null,
      // Built-in DEX overrides take priority over API data
      dex: BUILTIN_DEX[chainId] ?? raw.dex ?? null,
    };

    cache.set(chainId, { data, at: Date.now() });
    return data;
  } catch {
    return null;
  }
}

/**
 * Pick the best quote token from the stables list for DEX price queries.
 * Prefers native USDC > any USDC > USDT > first stablecoin.
 */
export function pickQuoteToken(stables: StableToken[]): StableToken | null {
  return (
    stables.find(s => s.symbol === 'USDC' && s.type === 'native') ??
    stables.find(s => s.symbol === 'USDC') ??
    stables.find(s => s.symbol === 'USDT') ??
    stables[0] ??
    null
  );
}
