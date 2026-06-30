/**
 * Chain info fetcher from ethereum-data API.
 *
 * Data source: https://ethereum-data.awesometools.dev/
 * - Chain info: /chains/eip155-{chainId}.json
 * - Chain logo: /chainlogos/eip155-{chainId}.png
 * - Search index: /index/fuse-chains.json (Fuse.js compatible)
 */

import { getEthereumDataURL } from './storage';
import { fetchWithTimeout, NET_TIMEOUTS } from './net';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ChainInfo {
  chainId: number;
  name: string;
  shortName: string;
  nativeCurrency: {
    name: string;
    symbol: string;
    decimals: number;
  };
  rpcUrl: string;
  /** All available HTTPS RPC URLs */
  rpcUrls: string[];
  explorerUrl: string;
  logoURL: string;
  isTestnet: boolean;
}

export interface ChainSearchResult {
  chainId: number;
  name: string;
  shortName: string;
  nativeCurrencySymbol: string;
  hasLogo: boolean;
}

// ---------------------------------------------------------------------------
// Fetch single chain info by ID
// ---------------------------------------------------------------------------

export async function fetchChainInfo(chainId: number): Promise<ChainInfo | null> {
  try {
    const res = await fetchWithTimeout(
      `${getEthereumDataURL()}/chains/eip155-${chainId}.json`,
      {},
      { timeoutMs: NET_TIMEOUTS.ethereumData },
    );
    if (!res.ok) return null;

    const data = await res.json();
    return parseChainData(data, chainId);
  } catch {
    return null;
  }
}

function parseChainData(data: any, chainId: number): ChainInfo {
  return {
    chainId: data.chainId ?? chainId,
    name: data.name ?? `Chain ${chainId}`,
    shortName: data.shortName ?? '',
    nativeCurrency: {
      name: data.nativeCurrency?.name ?? 'Ether',
      symbol: data.nativeCurrency?.symbol ?? 'ETH',
      decimals: data.nativeCurrency?.decimals ?? 18,
    },
    rpcUrl: extractRpcUrl(data),
    rpcUrls: extractAllRpcUrls(data),
    explorerUrl: extractExplorerUrl(data),
    logoURL: `${getEthereumDataURL()}/chainlogos/eip155-${chainId}.png`,
    isTestnet: data.testnet === true,
  };
}

// ---------------------------------------------------------------------------
// Search chains (by name, symbol, or chainId)
// ---------------------------------------------------------------------------

let _searchCache: ChainSearchResult[] | null = null;
let _searchCacheTime = 0;
const SEARCH_CACHE_TTL = 30 * 60 * 1000; // 30 min

async function loadSearchIndex(): Promise<ChainSearchResult[]> {
  const now = Date.now();
  if (_searchCache && now - _searchCacheTime < SEARCH_CACHE_TTL) {
    return _searchCache;
  }

  try {
    const res = await fetchWithTimeout(
      `${getEthereumDataURL()}/index/fuse-chains.json`,
      {},
      { timeoutMs: NET_TIMEOUTS.ethereumData },
    );
    if (!res.ok) return _searchCache ?? [];

    const json = await res.json();
    _searchCache = json.data as ChainSearchResult[];
    _searchCacheTime = now;
    return _searchCache;
  } catch {
    return _searchCache ?? [];
  }
}

/**
 * Search chains by query string.
 * Matches against: name, shortName, nativeCurrencySymbol, chainId.
 * Returns top 10 results.
 */
export async function searchChains(query: string): Promise<ChainSearchResult[]> {
  const chains = await loadSearchIndex();
  if (!query.trim()) return [];

  const q = query.toLowerCase().trim();

  // Exact chainId match first
  const chainIdNum = parseInt(q, 10);
  const exactMatch = !isNaN(chainIdNum) ? chains.find(c => c.chainId === chainIdNum) : null;

  // Fuzzy search by name, symbol, shortName
  const matches = chains.filter(c =>
    c.name.toLowerCase().includes(q) ||
    c.nativeCurrencySymbol.toLowerCase().includes(q) ||
    c.shortName.toLowerCase().includes(q) ||
    String(c.chainId).includes(q)
  );

  // Dedupe and prioritize exact chainId match
  const results: ChainSearchResult[] = [];
  if (exactMatch) results.push(exactMatch);
  for (const m of matches) {
    if (!results.find(r => r.chainId === m.chainId)) {
      results.push(m);
    }
    if (results.length >= 10) break;
  }

  return results;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function extractAllRpcUrls(data: any): string[] {
  if (!Array.isArray(data.rpc)) return [];
  return data.rpc.filter((u: unknown) =>
    typeof u === 'string' && u.startsWith('https://') && !u.includes('${') && !u.includes('API_KEY')
  ) as string[];
}

function extractRpcUrl(data: any): string {
  if (Array.isArray(data.rpc) && data.rpc.length > 0) {
    // Filter to HTTPS only (exclude wss://, http://)
    const httpsList = data.rpc.filter((u: unknown) =>
      typeof u === 'string' && u.startsWith('https://'),
    ) as string[];
    // Prefer URLs without API key placeholders
    const clean = httpsList.find(u => !u.includes('${') && !u.includes('API_KEY'));
    return clean ?? httpsList[0] ?? '';
  }
  return '';
}

function extractExplorerUrl(data: any): string {
  if (Array.isArray(data.explorers) && data.explorers.length > 0) {
    return data.explorers[0].url ?? '';
  }
  return '';
}
